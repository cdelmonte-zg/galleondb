use anyhow::{Context, bail};

use super::Row;
use crate::database::Database;
use crate::logical_plan::LogicalPlan;
use crate::parser::{BinaryOperation, Literal};
use crate::semantic::catalog::TableId;
use crate::semantic::functions::FunctionKind;
use crate::semantic::{
    BoundColumn, BoundExpr, BoundExprKind, BoundFunctionArgument, BoundFunctionCall, BoundTableExpr,
};
use crate::semantic::{
    BoundQuery, BoundSelect, BoundStatement, catalog::Catalog, functions::FunctionId,
};
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

    fn execute_scan(&self, table_id: TableId) -> anyhow::Result<Vec<Row>> {
        let table_metadata = self.catalog.get_table(table_id)?;
        let cells = self
            .database
            .read_table(table_metadata.root_page as u64, self.catalog.page_size())?;

        let mut rows = Vec::with_capacity(cells.len());

        for cell in cells {
            let mut values = Vec::with_capacity(cell.record.body.len());

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

            rows.push(Row { values })
        }

        Ok(rows)
    }

    fn execute_aggregate(
        &self,
        input: &LogicalPlan,
        expressions: &Vec<BoundExpr>,
    ) -> anyhow::Result<Vec<Row>> {
        let input_rows = &self.execute_plan(input)?;

        let mut values = Vec::with_capacity(expressions.len());

        for expression in expressions {
            values.push(self.evaluate_aggregates(expression, &input_rows)?);
        }

        Ok(vec![Row { values }])
    }

    fn evaluate_aggregates(
        &self,
        expression: &BoundExpr,
        input_rows: &Vec<Row>,
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
        input_rows: &Vec<Row>,
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

        match op {
            BinaryOperation::Equal => {
                if left == Value::Null || right == Value::Null {
                    Ok(Value::Null)
                } else {
                    Ok(Value::Boolean(left == right))
                }
            }

            BinaryOperation::NotEqual => Ok(Value::Boolean(left != right)),

            _ => bail!("Not implemented"),
        }
    }

    pub(crate) fn execute(&mut self, bound_statement: &BoundStatement) -> anyhow::Result<Vec<Row>> {
        match bound_statement {
            BoundStatement::BoundQuery(query) => self.execute_query(query),
            _ => bail!("Statement unknown"),
        }
    }

    fn execute_query(&mut self, query: &BoundQuery) -> anyhow::Result<Vec<Row>> {
        self.execute_select(&query.bound_select)
    }

    fn execute_select(&mut self, select: &BoundSelect) -> anyhow::Result<Vec<Row>> {
        let table_id = match &select.bound_from {
            Some(BoundTableExpr::Table(table)) => table.table_id,
            _ => bail!("Table name is missing."),
        };

        let columns: Vec<Vec<Value>> = select
            .bound_projections
            .iter()
            .map(|projection: &BoundExpr| match &projection.expr {
                BoundExprKind::FunctionCall(function_call) => {
                    self.execute_function(function_call, table_id)
                }

                BoundExprKind::Column(column) => self.execute_column(table_id, column),

                _ => bail!("Not Implemented"),
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let row_count = columns.first().map(Vec::len).unwrap_or(0);

        if columns.iter().any(|column| column.len() != row_count) {
            bail!("Projections return different rows")
        }

        let mut rows = Vec::with_capacity(row_count);

        for row_index in 0..row_count {
            let mut values = Vec::with_capacity(columns.len());

            for column in &columns {
                values.push(column[row_index].clone());
            }

            rows.push(Row { values });
        }

        Ok(rows)
    }

    fn execute_column(
        &self,
        table_id: TableId,
        column: &BoundColumn,
    ) -> anyhow::Result<Vec<Value>> {
        let table_metadata = self.catalog.get_table(table_id)?;
        let cells = self
            .database
            .read_table(table_metadata.root_page as u64, self.catalog.page_size())?;

        let mut values = Vec::new();

        for cell in cells {
            let value = cell
                .record
                .body
                .get(column.column_index)
                .context("column index out of range")?
                .clone();

            values.push(value)
        }

        Ok(values)
    }

    fn execute_function(
        &mut self,
        function_call: &BoundFunctionCall,
        table_id: TableId,
    ) -> anyhow::Result<Vec<Value>> {
        match function_call.function {
            FunctionId::Count => match function_call.args.as_slice() {
                [arg] => match arg {
                    BoundFunctionArgument::Wildcard => self.exec_count_wildcard(table_id),
                    BoundFunctionArgument::Expr(expr) => self.exec_count_expr(table_id, expr),
                },
                _ => bail!("Too much arguments for count"),
            },
            _ => bail!("Not yet impelmented"),
        }
    }

    fn exec_count_wildcard(&mut self, table_id: TableId) -> anyhow::Result<Vec<Value>> {
        let table_metadata = self.catalog.get_table(table_id)?;
        let Some(num_rows) = table_metadata.statistics.num_rows else {
            bail!("Error in statistics. Non number of rows present.")
        };

        let mut values = Vec::new();
        values.push(Value::Integer(num_rows as i64));

        Ok(values)
    }

    fn exec_count_expr(
        &mut self,
        table_id: TableId,
        column: &BoundExpr,
    ) -> anyhow::Result<Vec<Value>> {
        todo!()
    }
}
