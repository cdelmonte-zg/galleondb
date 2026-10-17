use anyhow::{Ok, bail};

use crate::{
    parser::{
        BinaryOp, BinaryOperation, Expr, FunctionArgument, FunctionCall, Literal, Query, Select,
        SelectionItem::{self, SelectionExpr, Wildcard},
        Statement, TableExpr,
    },
    semantic::{
        BoundColumn, BoundExpr, BoundExprKind, BoundFunctionArgument, BoundFunctionCall,
        BoundQuery, BoundSelect, BoundStatement, BoundTable, BoundTableExpr, LogicalType, TypeInfo,
        catalog::{Catalog, TableMetadata},
        functions::{self, ArgumentType, ReturnType},
    },
};

#[derive(Debug)]
pub(crate) struct Resolver<'a> {
    pub catalog: &'a Catalog,
    pub scope: Scope<'a>,
}

#[derive(Debug)]
pub(crate) struct Scope<'a> {
    pub(crate) relations: Vec<ScopeRelation<'a>>,
}

#[derive(Debug)]
pub(crate) struct ScopeRelation<'a> {
    relation_index: usize,
    table_name: String,
    metadata: &'a TableMetadata,
}

impl<'a> Resolver<'a> {
    pub(crate) fn new(catalog: &'a Catalog) -> Self {
        Self {
            catalog,
            scope: Scope {
                relations: Vec::new(),
            },
        }
    }

    pub(super) fn resolve(&mut self, statement: &Statement) -> anyhow::Result<BoundStatement> {
        match statement {
            Statement::Query(query) => Ok(BoundStatement::BoundQuery(self.resolve_query(query)?)),
            _ => bail!("Statement {:?} unkown.", statement),
        }
    }

    fn resolve_query(&mut self, query: &Query) -> anyhow::Result<BoundQuery> {
        let bound_select = self.resolve_select(&query.select)?;

        Ok(BoundQuery { bound_select })
    }

    fn resolve_select(&mut self, select_stmt: &Select) -> anyhow::Result<BoundSelect> {
        let bound_from = if let Some(from_stmt) = &select_stmt.from {
            Some(self.resolve_from(from_stmt)?)
        } else {
            None
        };

        let bound_filter = if let Some(filter_stmt) = &select_stmt.filter {
            let filter = self.resolve_expression(filter_stmt)?;

            if filter.sql_type.logical_type != LogicalType::Boolean {
                bail!("WHERE clause must evaluate to boolean");
            }

            Some(filter)
        } else {
            None
        };

        let bound_projections = self.resolve_selection_items(&select_stmt.projection)?;

        Ok(BoundSelect {
            bound_projections,
            bound_filter,
            bound_from,
        })
    }

    fn resolve_selection_items(
        &mut self,
        selection_items: &[SelectionItem],
    ) -> anyhow::Result<Vec<BoundExpr>> {
        selection_items
            .iter()
            .map(|item| match item {
                SelectionExpr(expr) => self.resolve_expression(expr),
                Wildcard => self.resolve_projection_wildcard(),
                _ => bail!("Select param unknown: {:?}", item),
            })
            .collect()
    }

    fn resolve_expression(&self, expression: &Expr) -> anyhow::Result<BoundExpr> {
        match expression {
            Expr::Column(col) => self.resolve_column(col),
            Expr::Literal(literal) => self.resolve_literal(literal),
            Expr::BinaryOp(binary_op) => self.resolve_binary_op(binary_op),
            Expr::FunctionCall(function_call) => self.resolve_function_call(function_call),
            _ => bail!("not implemented"),
        }
    }

    fn resolve_column(&self, column_name: &str) -> anyhow::Result<BoundExpr> {
        let bound_columns: Vec<BoundExpr> = self
            .scope
            .relations
            .iter()
            .flat_map(|relation| {
                relation
                    .metadata
                    .schema
                    .columns
                    .iter()
                    .enumerate()
                    .filter_map(|(column_index, column)| {
                        if column_name.eq_ignore_ascii_case(&column.name) {
                            Some(BoundExpr {
                                sql_type: TypeInfo {
                                    logical_type: column.logical_type(),
                                    nullable: column.nullable,
                                },
                                expr: BoundExprKind::Column(BoundColumn {
                                    relation_index: relation.relation_index,
                                    column_index: column_index,
                                }),
                            })
                        } else {
                            None
                        }
                    })
            })
            .collect();

        match bound_columns.len() {
            0 => bail!("No columns found"),
            1 => Ok(bound_columns.into_iter().next().unwrap()),
            _ => bail!("Ambiguous column name"),
        }
    }

    fn resolve_literal(&self, literal: &Literal) -> anyhow::Result<BoundExpr> {
        match literal {
            Literal::String(text) => Ok(BoundExpr {
                sql_type: TypeInfo {
                    logical_type: LogicalType::Text,
                    nullable: false,
                },
                expr: BoundExprKind::Literal(Literal::String(text.clone())),
            }),

            Literal::Integer(integer) => Ok(BoundExpr {
                sql_type: TypeInfo {
                    logical_type: LogicalType::Integer,
                    nullable: false,
                },
                expr: BoundExprKind::Literal(Literal::Integer(*integer)),
            }),

            Literal::Float(float) => Ok(BoundExpr {
                sql_type: TypeInfo {
                    logical_type: LogicalType::Real,
                    nullable: false,
                },
                expr: BoundExprKind::Literal(Literal::Float(*float)),
            }),

            Literal::Null => Ok(BoundExpr {
                sql_type: TypeInfo {
                    logical_type: LogicalType::Null,
                    nullable: true,
                },
                expr: BoundExprKind::Literal(Literal::Null),
            }),

            Literal::Boolean(boolean) => Ok(BoundExpr {
                sql_type: TypeInfo {
                    logical_type: LogicalType::Boolean,
                    nullable: false,
                },
                expr: BoundExprKind::Literal(Literal::Boolean(*boolean)),
            }),

            _ => bail!("Cannot recognize literal"),
        }
    }

    fn resolve_binary_op(&self, binary_op: &BinaryOp) -> anyhow::Result<BoundExpr> {
        let left = self.resolve_expression(&binary_op.left)?;
        let right = self.resolve_expression(&binary_op.right)?;

        let nullable = left.sql_type.nullable || right.sql_type.nullable;
        let logical_type = self.resolve_binary_type(&left, &binary_op.op, &right)?;

        let sql_type = TypeInfo {
            logical_type,
            nullable,
        };

        Ok(BoundExpr {
            sql_type,
            expr: BoundExprKind::Binary {
                left: Box::new(left),
                op: binary_op.op.clone(),
                right: Box::new(right),
            },
        })
    }

    fn resolve_binary_type(
        &self,
        left: &BoundExpr,
        op: &BinaryOperation,
        right: &BoundExpr,
    ) -> anyhow::Result<LogicalType> {
        match op {
            super::BinaryOperation::And | super::BinaryOperation::Or => {
                if left.sql_type.logical_type != LogicalType::Boolean
                    || right.sql_type.logical_type != LogicalType::Boolean
                {
                    bail!("And / Or require boolean operands")
                }

                Ok(LogicalType::Boolean)
            }

            super::BinaryOperation::Equal
            | super::BinaryOperation::NotEqual
            | super::BinaryOperation::GreaterThan
            | super::BinaryOperation::GreaterEqualThan
            | super::BinaryOperation::LessThan
            | super::BinaryOperation::LessEqualThan => Ok(LogicalType::Boolean),

            super::BinaryOperation::Divide
            | super::BinaryOperation::Multiply
            | super::BinaryOperation::Add
            | super::BinaryOperation::Subtract => Ok(LogicalType::Numeric),
        }
    }

    fn resolve_function_call(&self, function_call: &FunctionCall) -> anyhow::Result<BoundExpr> {
        let mut bound_args = vec![];
        let mut argument_types = vec![];

        for arg in &function_call.args {
            match arg {
                FunctionArgument::Wildcard => {
                    bound_args.push(BoundFunctionArgument::Wildcard);
                    argument_types.push(ArgumentType::Wildcard);
                }
                FunctionArgument::Expr(expr) => {
                    let bound_expr: BoundExpr = self.resolve_expression(expr)?;
                    argument_types.push(ArgumentType::Type(bound_expr.sql_type.logical_type));
                    bound_args.push(BoundFunctionArgument::Expr(bound_expr));
                }
            }
        }

        let function_definition = functions::resolve(&function_call.name, &argument_types)?;

        let logical_type = match function_definition.return_type() {
            ReturnType::Fixed(t) => t,
            ReturnType::SameAsArgument(index) => match argument_types[index] {
                ArgumentType::Type(t) => t,
                ArgumentType::Wildcard => bail!("Ẅildcard canoit determine return type"),
            },
        };

        let bound_expression = BoundExpr {
            sql_type: TypeInfo {
                logical_type,
                nullable: false,
            },
            expr: BoundExprKind::FunctionCall(BoundFunctionCall {
                function: function_definition.id(),
                function_kind: function_definition.kind(),
                args: bound_args,
            }),
        };

        Ok(bound_expression)
    }

    fn resolve_projection_wildcard(&self) -> anyhow::Result<BoundExpr> {
        let _ = "ḧello";

        bail!("Not implemented")
    }

    fn resolve_from(&mut self, from: &TableExpr) -> anyhow::Result<BoundTableExpr> {
        match from {
            TableExpr::Table(table) => Ok(BoundTableExpr::Table(self.resolve_table(table)?)),
            _ => bail!("To be implemented"),
        }
    }

    fn resolve_table(&mut self, table_name: &str) -> anyhow::Result<BoundTable> {
        let table_id = self.catalog.get_table_id(table_name)?;
        let table_metadata = self.catalog.get_table(table_id)?;
        let relation_index = self.scope.relations.len();

        self.scope.relations.push(ScopeRelation {
            relation_index,
            table_name: table_name.to_string(),
            metadata: table_metadata,
        });

        Ok(BoundTable {
            table_id,
            relation_index,
        })
    }
}
