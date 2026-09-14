use anyhow::{Ok, bail};

use crate::{
    parser::{
        Expr, Query, Select,
        SelectionItem::{self, SelectionExpr, Wildcard},
        Statement, TableExpr,
    },
    semantic::{
        Bound_TableExpr, BoundColumn, BoundExpr, BoundExprKind, BoundQuery, BoundSelect,
        BoundStatement, BoundTable,
        catalog::{Catalog, Scope},
    },
};

#[derive(Debug)]
pub(crate) struct Resolver<'a> {
    pub catalog: &'a mut Catalog,
    pub scope: Option<Scope>,
}

impl<'a> Resolver<'a> {
    pub(crate) fn new(catalog: &'a mut Catalog) -> Self {
        let scope = None;

        Self { catalog, scope }
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
        let bound_projections = self.resolve_selection_items(&select_stmt.projection)?;

        let bound_from = if let Some(from_stmt) = &select_stmt.from {
            Some(self.resolve_from(from_stmt)?)
        } else {
            None
        };

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

    fn resolve_expression(&mut self, expression: &Expr) -> anyhow::Result<BoundExpr> {
        match expression {
            Expr::Column(col) => Ok(BoundExpr {
                sql_type: super::TypeInfo {
                    logical_type: super::LogicalType::Text,
                    nullable: true,
                },
                expr: BoundExprKind::Column(BoundColumn {
                    relation_index: 0,
                    column_index: 2,
                }),
            }),
            _ => bail!("not implemented"),
        }
    }

    fn resolve_projection_wildcard(&mut self) -> anyhow::Result<BoundExpr> {
        bail!("Not implemented")
    }

    fn resolve_from(&mut self, from: &TableExpr) -> anyhow::Result<Bound_TableExpr> {
        match from {
            TableExpr::Table(table) => Ok(Bound_TableExpr::Table(self.resolve_table(table)?)),
            _ => bail!("To be implemented"),
        }
    }

    fn resolve_table(&mut self, table_name: &str) -> anyhow::Result<BoundTable> {
        let rootpage = self.catalog.get_table(table_name)?.root_page;
        let schema_index = rootpage as usize;
        let relation_index = 777;

        Ok(BoundTable {
            schema_index,
            relation_index,
        })
    }
}
