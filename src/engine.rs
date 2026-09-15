use crate::database::{self, Database};
use crate::semantic::catalog::{self, Catalog};
use crate::{parser, semantic, shell};
use std::fs::File;

pub struct Engine {
    pub database: Database,
    pub catalog: Catalog,
}

impl Engine {
    pub fn open(file: File) -> anyhow::Result<Self> {
        let mut database = Database { file };
        let catalog = Catalog::load(&mut database)?;
        dbg!("{:?}", &catalog);

        Ok(Self { database, catalog })
    }

    pub fn execute(&mut self, command: &str) -> anyhow::Result<()> {
        let _ = match command {
            ".dbinfo" | ".tables" => shell::run(&command, &self.catalog),
            _ => self.execute_sql(command),
        };

        Ok(())
    }

    fn execute_sql(&mut self, sql: &str) -> anyhow::Result<()> {
        if let Some(statement) = parser::parse(sql)? {
            let bound_statement = semantic::analyze(&statement, &self.catalog)?;
            println!("Helo {:?}", &bound_statement);
        }

        Ok(())
    }
}
