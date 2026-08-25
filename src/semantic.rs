use crate::parser::{BinaryOperation, Expr, Literal, Statement, UnaryOperation};

mod resolver;
mod types;
mod functions;

#[derive(Debug)]
pub struct BoundSelect {
    bound_projections: Vec<BoundExpr>,
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

    FunctionCall(FunctionCall),
}

#[derive(Debug)]
pub struct BoundColumn {
    relation_index: usize,
    column_index: usize,
}

#[derive(Debug)]
pub struct FunctionCall {
    function: BoundFunction,
    args: Vec<BoundArguments>
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
    Boolean
}


pub fn analyze(statement: Statement) {
    resolver::resolve();   
}
