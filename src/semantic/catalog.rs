use anyhow::{Context, bail};
use std::collections::HashMap;

use crate::database::Database;
use crate::semantic::schema_parser::ColumnSchema;
use crate::value::Value;

#[derive(Debug)]
pub(crate) struct Catalog {
    pub(crate) database: DatabaseMetadata,
    pub(crate) tables: Vec<TableMetadata>,
    pub(crate) table_names: HashMap<String, TableId>,
    pub(crate) indexes: Vec<IndexMetadata>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TableId(pub(crate) usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ColumnIndex(pub(crate) usize);


#[derive(Debug)]
pub(crate) struct DatabaseMetadata {
    
    #[allow(dead_code)]
    pub(crate) schema_cell_count: usize,
    
    pub(crate) page_size: u32,
}

#[derive(Debug)]
pub(crate) struct TableMetadata {
    pub(crate) root_page: i64,
    pub(crate) schema: TableSchema,
    
    #[allow(dead_code)]
    pub(crate) statistics: TableStatistics,
}

#[derive(Debug)]
pub(crate) struct TableSchema {
    pub(crate) columns: Vec<ColumnSchema>,
}

#[derive(Debug, Default)]
pub(crate) struct TableStatistics {

    #[allow(dead_code)]
    pub(crate) num_rows: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct IndexMetadata {
    #[allow(dead_code)]
    pub(crate) name: String,
    pub(crate) table_id: TableId,
    pub(crate) root_page: u64,
    pub(crate) keys: Vec<IndexKeyMetadata>,
}

#[derive(Debug)]
pub(crate) struct IndexKeyMetadata {
    pub(crate) column_index: ColumnIndex,
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

    #[allow(dead_code)]
    pub fn schema_cell_count(&self) -> usize {
        self.database.schema_cell_count
    }

    pub fn load(database: &Database) -> anyhow::Result<Catalog> {
        let (page_size, schema_cell_count, cells) = database.read_schema_cells()?;

        let mut tables = Vec::new();
        let mut table_names = HashMap::new();
        let mut indexes = Vec::new();

        for cell in &cells {
            if let [
                Value::Text(object_type),
                Value::Text(name),
                Value::Text(tbl_name),
                Value::Integer(rootpage),
                Value::Text(sql),
            ] = cell.record.body.as_slice()
                && !name.starts_with("sqlite_") {
                    match object_type.as_str() {
                        "table" => {
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
                                        database.leaf_page_cell_count(rootpage, page_size)? as u64,
                                    ),
                                },
                            });

                            table_names.entry(name.clone()).or_insert(table_id);
                        }

                        "index" => {
                            let index = super::schema_parser::parse_index(sql)?;

                            let table_id = *table_names
                                .get(tbl_name)
                                .with_context(|| format!("Table not found for index: {tbl_name}"))?;

                            let name = index.name;

                            let keys = index.keys.iter().map(|key| {    
                                let table = &tables[table_id.0];
                                    
                                let column_index = table.schema.columns
                                    .iter()
                                    .position(|column| column.name == key.column)
                                    .with_context(|| format!(
                                        "Column {} not found in table {}",
                                        key.column, tbl_name
                                    ))?;

                                Ok(IndexKeyMetadata {
                                    column_index: ColumnIndex(column_index) 
                                })
                            }).collect::<anyhow::Result<Vec<_>>>()?;

                            let index_metadata = IndexMetadata {
                                name,
                                table_id,
                                root_page: u64::try_from(*rootpage)?,
                                keys,
                            };

                            indexes.push(index_metadata);
                        }
                        _ => {},
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
            indexes,
        })
    }

    pub(crate) fn find_index(&self, table_id: TableId, column_index: ColumnIndex) -> Option<&IndexMetadata> {
        self.indexes
            .iter()
            .find(|index| 
                index.table_id == table_id 
                    && index.keys
                        .first()
                        .is_some_and(|key| 
                            key.column_index == column_index))
    }

}
