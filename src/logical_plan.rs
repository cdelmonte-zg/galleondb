use anyhow::bail;

use crate::semantic::{
    BoundExpr, BoundExprKind, BoundFunctionArgument, BoundQuery, BoundSelect, BoundStatement,
    BoundTableExpr, functions::FunctionKind,
};

use super::semantic::catalog::TableId;

#[derive(Debug)]
pub(crate) enum LogicalPlan {
    Scan {
        table_id: TableId,
        
        #[allow(dead_code)]
        relation_index: usize,
    },
    Filter {
        input: Box<LogicalPlan>,
        predicate: BoundExpr,
    },
    Aggregate {
        input: Box<LogicalPlan>,
        expressions: Vec<BoundExpr>,
    },
    Projection {
        input: Box<LogicalPlan>,
        expressions: Vec<BoundExpr>,
    },
}

pub(crate) struct LogicalPlanner;

impl LogicalPlanner {
    pub(crate) fn plan(bound_statement: BoundStatement) -> anyhow::Result<LogicalPlan> {
        match bound_statement {
            BoundStatement::BoundQuery(query) => Self::plan_query(query),
        }
    }

    fn plan_query(query: BoundQuery) -> anyhow::Result<LogicalPlan> {
        Self::plan_select(query.bound_select)
    }

    pub(crate) fn plan_select(select: BoundSelect) -> anyhow::Result<LogicalPlan> {
        let mut plan = match select.bound_from {
            Some(BoundTableExpr::Table(table)) => LogicalPlan::Scan {
                table_id: table.table_id,
                relation_index: table.relation_index,
            },

            None => bail!("SELECT without FROM not implemented"),
        };

        if let Some(predicate) = select.bound_filter {
            plan = LogicalPlan::Filter {
                input: Box::new(plan),
                predicate,
            }
        };

        let has_aggregate = select
            .bound_projections
            .iter()
            .any(Self::contains_aggregate);

        if has_aggregate {
            if !select.bound_projections.iter().all(Self::is_aggregate) {
                bail!("MIxed aggregate and non aggregate expressions are not supported")
            }
            
            plan = LogicalPlan::Aggregate {
                input: Box::new(plan),
                expressions: select.bound_projections,
            }
        } else {
            plan = LogicalPlan::Projection {
                input: Box::new(plan),
                expressions: select.bound_projections,
            };
        }

        Ok(plan)
    }

    fn is_aggregate(expr: &BoundExpr) -> bool {
        match &expr.expr {
            BoundExprKind::FunctionCall(function_call) => {
                matches!(function_call.function_kind, FunctionKind::Aggregate)
            }

            _ => false
        }
        
    }

    fn contains_aggregate(expr: &BoundExpr) -> bool {
        match &expr.expr {
            BoundExprKind::FunctionCall(function_call) => {
                matches!(function_call.function_kind, FunctionKind::Aggregate)
                    || function_call.args.iter().any(|arg| match arg {
                        BoundFunctionArgument::Expr(expr) => Self::contains_aggregate(expr),
                        BoundFunctionArgument::Wildcard => false,
                    })
            }

            BoundExprKind::Binary { left, right , ..} => {
                Self::contains_aggregate(left) || Self::contains_aggregate(right)
            }

            _ => false,
        }
    }
}
