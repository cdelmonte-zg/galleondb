use std::any::Any;

use anyhow::{Context, Ok, bail};

use crate::{
    parser::{
        Expr, FunctionArgument, FunctionCall, Query, Select,
        SelectionItem::{self, SelectionExpr, Wildcard},
        Statement, TableExpr,
    },
    semantic::{
        BoundTableExpr, BoundColumn, BoundExpr, BoundExprKind, BoundFunctionArgument,
        BoundFunctionCall, BoundQuery, BoundSelect, BoundStatement, BoundTable,
        catalog::{Catalog, TableMetadata, TableSchema},
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

        let bound_projections = self.resolve_selection_items(&select_stmt.projection)?;

        Ok(BoundSelect {
            bound_projections,
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
            Expr::FunctionCall(function_call) => Ok(self.resolve_function_call(&function_call)?),
            _ => bail!("not implemented"),
        }
    }

    fn resolve_column(&self, column_name: &str) -> anyhow::Result<BoundExpr> {
        let bound_columns: Vec<BoundExpr> = self.scope.relations.iter().flat_map(| relation| {
            relation.metadata.schema.columns.iter().enumerate().filter_map(|(column_index, column)| {
                if column_name.to_ascii_lowercase() == column.name.to_ascii_lowercase() {
                    Some(BoundExpr {
                        sql_type: super::TypeInfo {
                            logical_type: column.logical_type(),
                            nullable: column.nullable,
                        },
                        expr: BoundExprKind::Column(BoundColumn { 
                            relation_index: relation.relation_index, 
                            column_index: column_index}),
                    }) 
                } else {
                    None
                }
            }            
        )}
        ).collect();

        match bound_columns.len() {
            0 => bail!("No columns found"),
            1 => Ok(bound_columns.into_iter().next().unwrap()),
            _ => bail!("Ambiguous column name"),
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

        let definition = functions::resolve(&function_call.name, &argument_types)?;

        let logical_type = match definition.return_type() {
            ReturnType::Fixed(t) => t,
            ReturnType::SameAsArgument(index) => match argument_types[index] {
                ArgumentType::Type(t) => t,
                ArgumentType::Wildcard => bail!("Ẅildcard canoit determine return type"),
            },
        };

        let bound_expression = BoundExpr {
            sql_type: super::TypeInfo {
                logical_type: logical_type,
                nullable: false,
            },
            expr: BoundExprKind::FunctionCall(BoundFunctionCall {
                function: definition.id(),
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
            root_page: table_metadata.root_page as usize,
        })
    }
}
