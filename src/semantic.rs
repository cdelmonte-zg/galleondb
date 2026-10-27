use crate::semantic::{
    catalog::{Catalog, TableId}, functions::FunctionKind, resolver::Resolver,
};

use super::parser::{BinaryOperation, Literal, Statement, UnaryOperation};
use functions::FunctionId;

pub mod catalog;
pub(crate) mod functions;
mod resolver;
mod schema_parser;
mod types;

#[derive(Debug)]
pub(crate) enum BoundStatement {
    BoundQuery(BoundQuery),
}

#[derive(Debug)]
pub(crate) struct BoundQuery {
    pub(crate) bound_select: BoundSelect,
    // bound_orderby: Vec<BoundOrderBy>,
    // bound_limit: Option<BoundLimit>
}

#[derive(Debug)]
pub(crate) struct BoundSelect {
    pub(crate) bound_projections: Vec<BoundExpr>,
    pub(crate) bound_filter: Option<BoundExpr>,
    pub(crate) bound_from: Option<BoundTableExpr>,
}

#[derive(Debug)]
pub(crate) enum BoundTableExpr {
    Table(BoundTable),
}

#[derive(Debug)]
pub(crate) struct BoundExpr {
    pub(crate) sql_type: TypeInfo,
    pub(crate) expr: BoundExprKind,
}

#[derive(Debug)]
pub(crate) enum BoundExprKind {
    Literal(Literal),

    Column(BoundColumn),

    Unary {
        op: UnaryOperation,
        operand: Box<BoundExpr>,
    },

    Binary {
        left: Box<BoundExpr>,
        op: BinaryOperation,
        right: Box<BoundExpr>,
    },

    FunctionCall(BoundFunctionCall),
}

#[derive(Debug)]
pub(crate) struct BoundTable {
    pub(crate) table_id: TableId,
    pub(crate) relation_index: usize,
}

#[derive(Debug)]
pub(crate) struct BoundColumn {
    #[allow(dead_code)]
    pub(crate) relation_index: usize,

    pub(crate) column_index: usize,
}

#[derive(Debug)]
pub(crate) struct BoundFunctionCall {
    pub(crate) function: FunctionId,
    pub(crate) function_kind: FunctionKind,
    pub(crate) args: Vec<BoundFunctionArgument>,
}

#[derive(Debug)]
pub(crate) enum BoundFunctionArgument {
    Expr(BoundExpr),
    Wildcard,
}

#[derive(Debug)]
pub(crate) struct TypeInfo {
    logical_type: LogicalType,
    nullable: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum LogicalType {
    Integer,
    Real,
    Text,
    Boolean,
    Blob,
    Numeric,
    Null,
}

pub(crate) fn analyze(statement: &Statement, catalog: &Catalog) -> anyhow::Result<BoundStatement> {
    let mut resolver = Resolver::new(catalog);
    resolver.resolve(statement)
}
