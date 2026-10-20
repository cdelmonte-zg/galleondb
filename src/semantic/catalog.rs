use std::collections::HashMap;
use anyhow::{Context, bail};

use crate::database::Database;
use crate::semantic::schema_parser::ColumnSchema;
use crate::value::Value;


#[derive(Debug)]
pub(crate) struct Catalog {
    pub(crate) database: DatabaseMetadata,
    pub(crate) tables: Vec<TableMetadata>,
    pub(crate) table_names: HashMap<String, TableId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TableId(pub(crate) usize);

#[derive(Debug)]
pub(crate) struct DatabaseMetadata {
    pub(crate) schema_cell_count: usize,
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
    pub fn get_table(&self, table_id: TableId) -> anyhow::Result<&TableMetadata> {
        self.tables
            .get(table_id.0)
            .with_context(|| format!("Invalid table id: {}", table_id.0))
    }

    pub fn get_table_id(&self, name: &str) -> anyhow::Result<TableId> {
        self.table_names
            .get(name)
            .copied()
            .with_context(|| format!("table not found: {name}"))
    }

    pub fn page_size(&self) -> u32 {
        self.database.page_size
    }

    pub fn schema_cell_count(&self) -> usize {
        self.database.schema_cell_count
    }

    pub fn load(database: &Database) -> anyhow::Result<Catalog> {
        let (page_size, schema_cell_count, cells) = database.read_schema_cells()?;

        let mut tables = Vec::new();
        let mut table_names = HashMap::new();

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

                    if table_names.contains_key(name) {
                        bail!("Duplicate table in catalog: {name}")
                    }

                    let table_id = TableId(tables.len());

                    tables.push(TableMetadata {
                        root_page: *rootpage,
                        schema: TableSchema { columns },
                        statistics: TableStatistics {
                            num_rows: Some(
                                database.leaf_page_cell_count(rootpage, page_size)? as u64
                            ),
                        },
                    });

                    table_names.entry(name.clone()).or_insert(table_id);
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
            table_names,
        })
    }
}
