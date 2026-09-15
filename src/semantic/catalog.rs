use std::{collections::HashMap};

use anyhow::Context;

use crate::{database::{Database, Value}, semantic::schema_parser::ColumnSchema};

#[derive(Debug)]
pub(crate) struct Catalog {
    pub(crate) database: DatabaseMetadata,
    pub(crate) tables: HashMap<String, TableMetadata>,
}

#[derive(Debug)]
pub(crate) struct DatabaseMetadata {
    pub(crate) schema_cell_count: u16,
    pub(crate) page_size: u32,
}

#[derive(Debug)]
pub(crate) struct TableMetadata {
    pub(crate) root_page: i64,
    pub(crate) schema: TableSchema,
    pub(crate) statistics: TableStatistics,
}

#[derive(Debug)]
pub(crate) struct TableSchema {
    pub(crate) columns: Vec<ColumnSchema>,
}

#[derive(Debug, Default)]
pub(crate) struct TableStatistics {
    pub(crate) num_rows: Option<u64>,
}

impl Catalog {
    pub fn get_table(&self, table_name: &str) -> anyhow::Result<&TableMetadata> {
        let table_metadata = self
            .tables
            .get(table_name)
            .with_context(|| format!("Table metadata not found for name {table_name}"))?;
        Ok(table_metadata)
    }

    pub fn page_size(&self) -> u32 {
        self.database.page_size
    }

    pub fn schema_cell_count(&self) -> u16 {
        self.database.schema_cell_count
    }

    pub fn load(database: &mut Database) -> anyhow::Result<Catalog> {
        let (page_size, schema_cell_count) = database.get_db_info()?;

        let (_, _, cells) = database.get_cells()?;

        let mut tables = HashMap::new();

        for cell in &cells {
            if let [
                Value::Text(object_type),
                Value::Text(name),
                Value::Text(_tbl_name),
                Value::Integer(rootpage),
                Value::Text(sql),
            ] = cell.record.body.as_slice()
            {
                if object_type == "table" && !name.starts_with("sqlite_") {
                    let columns = super::schema_parser::parse_table_schema(sql)?;

                    tables.entry(name.clone()).or_insert(TableMetadata { 
                        root_page: *rootpage, 
                        schema: TableSchema { columns }, 
                        statistics: TableStatistics {
                            num_rows: Some(database.leaf_page_cell_count(rootpage, page_size)? as u64)
                        }
                    });
                }
            }
        }

        let database = DatabaseMetadata {
            schema_cell_count,
            page_size,
        };

        Ok(Catalog {
            database,
            tables,
        })
    }
}
