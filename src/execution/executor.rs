use anyhow::{Context, bail};

use crate::database::Database;
use crate::semantic::catalog::TableId;
use crate::semantic::{BoundColumn, BoundExpr, BoundExprKind, BoundFunctionArgument, BoundFunctionCall, BoundTableExpr};
use crate::semantic::{BoundQuery, BoundSelect, BoundStatement, catalog::Catalog, functions::FunctionId};
use super::Row;


pub(crate) struct Executor<'a> {
    pub(crate) catalog: &'a Catalog,
    pub(crate) database: &'a mut Database
}

impl <'a>Executor<'a> {
    pub(crate) fn execute(&mut self, bound_statement: &BoundStatement) -> anyhow::Result<Vec<Row>> {
        match bound_statement {
            BoundStatement::BoundQuery(query) => self.execute_query(query),
            _ => bail!("Statement unknown")
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

        let projection = match select.bound_projections.as_slice() {
            [projection] => projection,
            _ => bail!("Multiple projections not implemented")
        };
        
        match &projection.expr {
            BoundExprKind::FunctionCall(function_call) => self.execute_function(function_call, table_id),
            BoundExprKind::Column(column) => self.execute_column(table_id, column),
            _ => bail!("Not Implemented"),
        }
    }

    fn execute_column(&self, table_id: TableId, column: &BoundColumn) -> anyhow::Result<Vec<Row>>{
        let table_metadata = self.catalog.get_table(table_id)?;
        let cells = self.database.read_table(table_metadata.root_page as u64, self.catalog.page_size())?;

        let mut rows = Vec::new();

        for cell in cells {
            let value = cell
                .record
                .body
                .get(column.column_index)
                .context("column index out of range")?
                .clone();

            rows.push(Row {
                values: vec![value]
            })
        }

        Ok(rows)
    }

    fn execute_function(&mut self, function_call: &BoundFunctionCall, table_id: TableId) -> anyhow::Result<Vec<Row>> {   
        match function_call.function {
            FunctionId::Count => {
                
                match function_call.args.as_slice() {
                    [arg] => match arg {
                        BoundFunctionArgument::Wildcard => self.exec_count_wildcard(table_id),
                        BoundFunctionArgument::Expr(expr) => self.exec_count_expr(table_id, expr)
                    }
                    _ => bail!("Too much arguments for count")
                }
            }
            _ => bail!("Not yet impelmented")
        }    
    }

    fn exec_count_wildcard(&mut self, table_id: TableId) -> anyhow::Result<Vec<Row>>{
        let table_metadata = self.catalog.get_table(table_id)?;
        let Some(num_rows) = table_metadata.statistics.num_rows else {
            bail!("Error in statistics. Non number of rows present.")
        };

        let mut values = Vec::new();
        values.push(crate::database::Value::Integer(num_rows as i64));

        Ok(vec![Row { values }])
    }


    fn exec_count_expr(&mut self, table_id: TableId, column: &BoundExpr) -> anyhow::Result<Vec<Row>>{
        todo!()
    }

}
