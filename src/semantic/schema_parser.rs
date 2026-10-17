use anyhow::{Context, bail};

use crate::{
    parser::{self, ColumnConstraint, ColumnDefinition, DeclaredType, Statement},
    semantic::LogicalType,
};

#[derive(Debug)]
pub(crate) struct ColumnSchema {
    pub(crate) name: String,
    pub(crate) affinity: TypeAffinity,
    pub(crate) nullable: bool,
    pub(crate) is_rowid_alias: bool,
}

impl ColumnSchema {
    pub(crate) fn logical_type(&self) -> LogicalType {
        match self.affinity {
            TypeAffinity::Text => LogicalType::Text,
            TypeAffinity::Integer => LogicalType::Integer,
            TypeAffinity::Blob => LogicalType::Blob,
            TypeAffinity::Numeric => LogicalType::Numeric,
            TypeAffinity::Real => LogicalType::Real,
        }
    }
}

#[derive(Debug)]
pub enum TypeAffinity {
    Integer,
    Real,
    Text,
    Numeric,
    Blob,
}

pub fn parse_table_schema(sql: &str) -> anyhow::Result<Vec<ColumnSchema>> {
    let statement = parser::parse(sql)?.context("Expected CREATE TABLE statement")?;

    let Statement::CreateTable(create_table) = statement else {
        bail!("Expected CREATE TABLE statement");
    };

    create_table
        .columns
        .into_iter()
        .map(column_definition_to_schema)
        .collect()
}

fn column_definition_to_schema(column: ColumnDefinition) -> anyhow::Result<ColumnSchema> {
    let affinity = type_affinity_from_declared_type(&column.declared_type);

    let nullable = !column.constraints.iter().any(|constraint| {
        matches!(
            constraint,
            ColumnConstraint::NotNull | ColumnConstraint::PrimaryKey
        )
    });

    let is_rowid_alias = column.constraints.iter().any(|constraint| {
        matches!(
            constraint,
            ColumnConstraint::PrimaryKey
        )
    });

    Ok(ColumnSchema {
        name: column.name,
        affinity,
        nullable,
        is_rowid_alias,
    })
}

fn type_affinity_from_declared_type(declared: &DeclaredType) -> TypeAffinity {
    let name = declared.name.as_str();

    if name.contains("INT") {
        TypeAffinity::Integer
    } else if name.contains("CHAR") || name.contains("CLOB") || name.contains("TEXT") {
        TypeAffinity::Text
    } else if name.contains("BLOB") {
        TypeAffinity::Blob
    } else if name.contains("REAL") || name.contains("FLOA") || name.contains("DOUB") {
        TypeAffinity::Real
    } else {
        TypeAffinity::Numeric
    }
}
