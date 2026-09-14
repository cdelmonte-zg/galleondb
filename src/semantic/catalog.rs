use std::collections::HashMap;

use anyhow::Context;

use super::LogicalType;
use crate::database::{Database, Value};

#[derive(Debug)]
pub(crate) struct Catalog {
    pub(crate) tables: HashMap<String, TableSchema>,
    cells_nr: u16,
    page_size: u32,
}

#[derive(Debug)]
pub(crate) struct TableSchema {
    pub(crate) root_page: i64,
    pub(crate) columns: Vec<ColumnSchema>,
}

#[derive(Debug)]
pub(crate) struct ColumnSchema {
    pub(crate) name: String,
    pub(crate) logical_type: LogicalType,
}

#[derive(Debug)]
pub(crate) struct Scope {
    pub(crate) table_schema: TableSchema,
}

impl Catalog {
    pub fn get_table(&mut self, table_name: &str) -> anyhow::Result<&TableSchema> {
        let table_schema = self
            .tables
            .get(table_name)
            .context("Table schema not found for name {table_name}")?;
        Ok(table_schema)
    }

    pub fn page_size(&self) -> u32 {
        self.page_size
    }

    pub fn cells_nr(&self) -> u16 {
        self.cells_nr
    }

    pub fn load(database: &mut Database) -> anyhow::Result<Catalog> {
        let (page_size, cells_nr) = match database.get_db_info() {
            Ok((page_size, cells_nr)) => (page_size, cells_nr),
            Err(e) => return Err(e),
        };

        let (_, _, cells) = database.get_cells()?;

        let mut tables = HashMap::new();

        for cell in &cells {
            if let [
                Value::Text(object_type),
                Value::Text(name),
                Value::Text(_tbl_name),
                Value::Integer(rootpage),
                Value::Text(_sql),
            ] = cell.record.body.as_slice()
            {
                if object_type == "table" && !name.starts_with("sqlite_") {
                    tables.entry(name.clone()).or_insert(TableSchema {
                        root_page: *rootpage,
                        columns: Vec::new(),
                    });
                }
            }
        }

        Ok(Catalog {
            tables,
            cells_nr,
            page_size,
        })
    }
}
