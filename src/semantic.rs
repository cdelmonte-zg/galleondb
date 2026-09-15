use crate::semantic::{catalog::Catalog, resolver::Resolver};

use super::parser::{BinaryOperation, Literal, Statement, UnaryOperation};
use functions::FunctionId;

pub mod catalog;
mod functions;
mod resolver;
mod schema_parser;
mod types;

#[derive(Debug)]
pub enum BoundStatement {
    BoundQuery(BoundQuery),
}

#[derive(Debug)]
pub struct BoundQuery {
    bound_select: BoundSelect,
    // bound_orderby: Vec<BoundOrderBy>,
    // bound_limit: Option<BoundLimit>
}

#[derive(Debug)]
pub struct BoundSelect {
    bound_projections: Vec<BoundExpr>,
    bound_from: Option<Bound_TableExpr>,
}

#[derive(Debug)]
pub enum Bound_TableExpr {
    Table(BoundTable),
}

#[derive(Debug)]
pub struct BoundExpr {
    sql_type: TypeInfo,
    expr: BoundExprKind,
}

#[derive(Debug)]
pub enum BoundExprKind {
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
pub struct BoundTable {
    relation_index: usize,
    root_page: usize,
}

#[derive(Debug)]
pub struct BoundColumn {
    relation_index: usize,
    column_index: usize,
}

#[derive(Debug)]
pub struct BoundFunctionCall {
    function: FunctionId,
    args: Vec<BoundFunctionArgument>,
}

#[derive(Debug)]
pub enum BoundFunctionArgument {
    Expr(BoundExpr),
    Wildcard,
}

#[derive(Debug)]
pub struct TypeInfo {
    logical_type: LogicalType,
    nullable: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LogicalType {
    Integer,
    Real,
    Text,
    Boolean,
    Blob,
    Numeric,
    Null,
}

pub fn analyze(statement: &Statement, catalog: &Catalog) -> anyhow::Result<BoundStatement> {
    let mut resolver = Resolver::new(catalog);
    resolver.resolve(statement)
}
