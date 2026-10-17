use anyhow::bail;

use crate::database::Database;
use crate::execution::ExecutionResult;
use crate::execution::executor::Executor;
use crate::logical_plan::LogicalPlanner;
use crate::semantic::catalog::Catalog;
use crate::{logical_plan, parser, semantic, shell};
use std::fs::File;

pub struct Engine {
    pub(crate) database: Database,
    pub(crate) catalog: Catalog,
}

impl Engine {
    pub fn open(file: File) -> anyhow::Result<Self> {
        let database = Database { file };
        let catalog = Catalog::load(&database)?;

        // dbg!("{:?}", &catalog);

        Ok(Self { database, catalog })
    }

    pub fn execute(&mut self, command: &str) -> anyhow::Result<ExecutionResult> {
        match command {
            ".dbinfo" | ".tables" => {
                Ok(ExecutionResult::Shell(shell::run(&command, &self.catalog)?))
            }
            _ => self.execute_sql(command),
        }
    }

    fn execute_sql(&mut self, sql: &str) -> anyhow::Result<ExecutionResult> {
        if let Some(statement) = parser::parse(sql)? {
            let bound_statement = semantic::analyze(&statement, &self.catalog)?;
            // println!("Helo {:?}", &bound_statement);

            let logical_plan = LogicalPlanner::plan(bound_statement)?;

            let mut executor = Executor {
                catalog: &self.catalog,
                database: &mut self.database,
            };

            let rows = executor.execute_plan(&logical_plan)?;

            return Ok(ExecutionResult::Query(rows));
        }

        bail!("No sql statement")
    }
}
