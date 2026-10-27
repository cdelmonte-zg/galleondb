use anyhow::{Context, bail};

use super::Row;
use crate::database::{Cell, Database};
use crate::logical_plan::LogicalPlan;
use crate::parser::{BinaryOperation, Literal};
use crate::semantic::catalog::{TableId, TableMetadata};
use crate::semantic::functions::FunctionKind;
use crate::semantic::{
    BoundColumn, BoundExpr, BoundExprKind, BoundFunctionArgument, BoundFunctionCall,
};
use crate::semantic::{catalog::Catalog, functions::FunctionId};
use crate::value::Value;

pub(crate) struct Executor<'a> {
    pub(crate) catalog: &'a Catalog,
    pub(crate) database: &'a mut Database,
}

impl<'a> Executor<'a> {
    pub(crate) fn execute_plan(&self, logical_plan: &LogicalPlan) -> anyhow::Result<Vec<Row>> {
        match logical_plan {
            LogicalPlan::Scan {
                table_id,
                relation_index: _,
            } => self.execute_scan(*table_id),

            LogicalPlan::Filter { input, predicate } => {
                if let LogicalPlan::Scan { table_id, .. } = input.as_ref()
                    && let Some(rows) = self.try_index_lookup(*table_id, predicate)? {
                        return Ok(rows);
                    }

                let input_rows = self.execute_plan(input)?;
                let mut output = Vec::with_capacity(input_rows.len());

                for row in input_rows {
                    let value = self.evaluate_expression(predicate, &row)?;

                    if matches!(value, Value::Boolean(true)) {
                        output.push(row)
                    }
                }
                Ok(output)
            }

            LogicalPlan::Aggregate { input, expressions } => {
                self.execute_aggregate(input, expressions)
            }

            LogicalPlan::Projection { input, expressions } => {
                let input_rows = self.execute_plan(input)?;
                let mut output_rows = Vec::with_capacity(input_rows.len());

                for row in input_rows {
                    let mut values = Vec::with_capacity(expressions.len());

                    for expression in expressions {
                        values.push(self.evaluate_expression(expression, &row)?);
                    }

                    output_rows.push(Row { values });
                }

                Ok(output_rows)
            }
        }
    }

    fn try_index_lookup(
        &self,
        table_id: TableId,
        predicate: &BoundExpr,
    ) -> anyhow::Result<Option<Vec<Row>>> {
        let BoundExprKind::Binary { left, right, op } = &predicate.expr else {
            return Ok(None);
        };

        if !matches!(op, BinaryOperation::Equal) {
            return Ok(None);
        }

        let (column, literal) = match (&left.expr, &right.expr) {
            (BoundExprKind::Column(column), BoundExprKind::Literal(literal)) => (column, literal),
            (BoundExprKind::Literal(literal), BoundExprKind::Column(column)) => (column, literal),
            _ => return Ok(None),
        };

        let search_value = self.evaluate_literal(literal)?;

        let Some(index) = self.catalog.find_index(
            table_id,
            crate::semantic::catalog::ColumnIndex(column.column_index),
        ) else {
            return Ok(None);
        };

        let rows = self.execute_index_lookup(table_id, index.root_page, &search_value)?;

        Ok(Some(rows))
    }

    fn execute_index_lookup(
        &self,
        table_id: TableId,
        index_root_page: u64,
        search_value: &Value,
    ) -> anyhow::Result<Vec<Row>> {
        let table_metadata = self.catalog.get_table(table_id)?;

        let row_ids =
            self.database
                .lookup_index(index_root_page, self.catalog.page_size(), search_value)?;

        let mut rows = Vec::with_capacity(row_ids.len());

        for row_id in row_ids {
            let cell = self.database.read_table_row(
                table_metadata.root_page as u64,
                self.catalog.page_size(),
                row_id,
            )?;

            rows.push(self.cell_to_row(table_metadata, &cell)?);
        }

        Ok(rows)
    }

    fn execute_scan(&self, table_id: TableId) -> anyhow::Result<Vec<Row>> {
        let table_metadata = self.catalog.get_table(table_id)?;
        let cells = self
            .database
            .read_table(table_metadata.root_page as u64, self.catalog.page_size())?;

        let mut rows = Vec::with_capacity(cells.len());

        for cell in cells {
            rows.push(self.cell_to_row(table_metadata, &cell)?);
        }

        Ok(rows)
    }

    fn execute_aggregate(
        &self,
        input: &LogicalPlan,
        expressions: &[BoundExpr],
    ) -> anyhow::Result<Vec<Row>> {
        let input_rows = self.execute_plan(input)?;

        let mut values = Vec::with_capacity(expressions.len());

        for expression in expressions {
            values.push(self.evaluate_aggregates(expression, &input_rows)?);
        }

        Ok(vec![Row { values }])
    }

    fn evaluate_aggregates(
        &self,
        expression: &BoundExpr,
        input_rows: &[Row],
    ) -> anyhow::Result<Value> {
        match &expression.expr {
            BoundExprKind::FunctionCall(function_call)
                if matches!(function_call.function_kind, FunctionKind::Aggregate) =>
            {
                self.evaluate_aggregate_function(function_call, input_rows)
            }

            _ => bail!("Expected aggregate expression"),
        }
    }

    fn evaluate_aggregate_function(
        &self,
        function_call: &BoundFunctionCall,
        input_rows: &[Row],
    ) -> anyhow::Result<Value> {
        match function_call.function {
            FunctionId::Count => match function_call.args.as_slice() {
                [BoundFunctionArgument::Wildcard] => {
                    Ok(Value::Integer(i64::try_from(input_rows.len())?))
                }

                _ => bail!("Count variant not implemented"),
            },

            _ => bail!("Aggregate function not implemented"),
        }
    }

    fn evaluate_expression(&self, expression: &BoundExpr, row: &Row) -> anyhow::Result<Value> {
        match &expression.expr {
            BoundExprKind::Column(column) => self.evaluate_column(row, column),

            BoundExprKind::Literal(literal) => self.evaluate_literal(literal),

            BoundExprKind::Binary { left, op, right } => self.evaluate_binary(row, left, op, right),

            _ => bail!("Expression not implemented"),
        }
    }

    fn evaluate_column(&self, row: &Row, column: &BoundColumn) -> anyhow::Result<Value> {
        row.values
            .get(column.column_index)
            .cloned()
            .context("column index out of range")
    }

    fn evaluate_literal(&self, literal: &Literal) -> anyhow::Result<Value> {
        match literal {
            Literal::Boolean(boolean) => Ok(Value::Boolean(*boolean)),
            Literal::Null => Ok(Value::Null),
            Literal::String(text) => Ok(Value::Text(text.clone())),
            Literal::Float(real) => Ok(Value::Float(*real)),
            Literal::Integer(value) => Ok(Value::Integer(i64::try_from(*value)?)),
        }
    }

    fn evaluate_binary(
        &self,
        row: &Row,
        left: &BoundExpr,
        op: &BinaryOperation,
        right: &BoundExpr,
    ) -> anyhow::Result<Value> {
        let left = self.evaluate_expression(left, row)?;
        let right = self.evaluate_expression(right, row)?;

        if left == Value::Null || right == Value::Null {
            return Ok(Value::Null);
        }

        match op {
            BinaryOperation::Equal => Ok(Value::Boolean(left == right)),

            BinaryOperation::NotEqual => Ok(Value::Boolean(left != right)),

            _ => bail!("Not implemented"),
        }
    }

    fn cell_to_row(&self, table_metadata: &TableMetadata, cell: &Cell) -> anyhow::Result<Row> {
        let mut values = Vec::with_capacity(table_metadata.schema.columns.len());

        for (index, column) in table_metadata.schema.columns.iter().enumerate() {
            if column.is_rowid_alias {
                values.push(Value::Integer(cell.row_id))
            } else {
                values.push(
                    cell.record
                        .body
                        .get(index)
                        .context("Column index out of range")?
                        .clone(),
                )
            }
        }

        Ok(Row { values })
    }
}
