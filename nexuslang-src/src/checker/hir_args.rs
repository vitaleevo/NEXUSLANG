use std::ptr;

use crate::ast::{Expr, Span};
use crate::auth_ops::CheckedAuthOperationArgs;
use crate::hir::{HirExprId, HirProgram};
use crate::model_ops::CheckedModelOperationArgs;

use super::Scope;

#[derive(Debug, Clone, Copy)]
pub(super) struct HirOperationArgs<'a> {
    pub(super) raw: &'a [Expr],
    ids: &'a [HirExprId],
}

impl<'a> HirOperationArgs<'a> {
    pub(super) fn from_static_call(source: &'a Expr, ids: &'a [HirExprId]) -> Option<Self> {
        let Expr::StaticCall { args: raw, .. } = source else {
            return None;
        };
        if raw.len() != ids.len() {
            return None;
        }
        Some(Self { raw, ids })
    }

    pub(super) fn id_for(self, expr: &Expr) -> Option<HirExprId> {
        self.raw
            .iter()
            .zip(self.ids.iter())
            .find_map(|(candidate, id)| ptr::eq(candidate, expr).then_some(*id))
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CheckedHirOperationArg<'a> {
    source: &'a Expr,
    hir_id: Option<HirExprId>,
}

impl<'a> CheckedHirOperationArg<'a> {
    pub(super) fn new(source: &'a Expr, args: Option<HirOperationArgs<'a>>) -> Self {
        Self {
            source,
            hir_id: args.and_then(|args| args.id_for(source)),
        }
    }

    pub(super) fn source(self) -> &'a Expr {
        self.source
    }

    pub(super) fn hir_id(self) -> Option<HirExprId> {
        self.hir_id
    }

    pub(super) fn span(self) -> Span {
        self.source.span()
    }

    pub(super) fn string_literal(self) -> Option<&'a str> {
        match self.source {
            Expr::StringLit { value, .. } => Some(value.as_str()),
            _ => None,
        }
    }

    pub(super) fn ident_name(self) -> Option<&'a str> {
        match self.source {
            Expr::Ident { name, .. } => Some(name.as_str()),
            _ => None,
        }
    }

    pub(super) fn integer_literal(self) -> Option<(i64, Span)> {
        match self.source {
            Expr::Integer { value, span } => Some((*value, *span)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CheckedHirModelLookupArgs<'a> {
    pub(super) field: CheckedHirOperationArg<'a>,
    pub(super) value: CheckedHirOperationArg<'a>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CheckedHirModelOrderingArgs<'a> {
    pub(super) field: CheckedHirOperationArg<'a>,
    pub(super) direction: CheckedHirOperationArg<'a>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CheckedHirModelPaginationArgs<'a> {
    pub(super) limit: CheckedHirOperationArg<'a>,
    pub(super) offset: CheckedHirOperationArg<'a>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CheckedHirModelAdvancedFilterArgs<'a> {
    pub(super) field: CheckedHirOperationArg<'a>,
    pub(super) operator: CheckedHirOperationArg<'a>,
    pub(super) value: CheckedHirOperationArg<'a>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CheckedHirModelRangeFilterArgs<'a> {
    pub(super) field: CheckedHirOperationArg<'a>,
    pub(super) min: CheckedHirOperationArg<'a>,
    pub(super) max: CheckedHirOperationArg<'a>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CheckedHirAuthOperationArgs<'a> {
    auth_config: Option<CheckedHirOperationArg<'a>>,
}

impl<'a> CheckedHirAuthOperationArgs<'a> {
    fn from_checked(
        checked: CheckedAuthOperationArgs<'a>,
        args: Option<HirOperationArgs<'a>>,
    ) -> Self {
        Self {
            auth_config: checked
                .auth_config_expr()
                .map(|expr| CheckedHirOperationArg::new(expr, args)),
        }
    }

    pub(super) fn auth_config(self) -> Option<CheckedHirOperationArg<'a>> {
        self.auth_config
    }
}

#[derive(Debug, Clone)]
pub(super) struct CheckedHirModelOperationArgs<'a> {
    lookup: Option<CheckedHirModelLookupArgs<'a>>,
    advanced_filter: Option<CheckedHirModelAdvancedFilterArgs<'a>>,
    range_filter: Option<CheckedHirModelRangeFilterArgs<'a>>,
    composite_filters: Option<Vec<CheckedHirModelLookupArgs<'a>>>,
    ordering: Option<CheckedHirModelOrderingArgs<'a>>,
    pagination: Option<CheckedHirModelPaginationArgs<'a>>,
}

impl<'a> CheckedHirModelOperationArgs<'a> {
    fn from_checked(
        checked: CheckedModelOperationArgs<'a>,
        args: Option<HirOperationArgs<'a>>,
    ) -> Self {
        let checked_arg = |source| CheckedHirOperationArg::new(source, args);
        let lookup = checked
            .lookup()
            .map(|(field, value)| CheckedHirModelLookupArgs {
                field: checked_arg(field),
                value: checked_arg(value),
            });
        let advanced_filter = checked.advanced_filter().map(|(field, operator, value)| {
            CheckedHirModelAdvancedFilterArgs {
                field: checked_arg(field),
                operator: checked_arg(operator),
                value: checked_arg(value),
            }
        });
        let range_filter =
            checked
                .range_filter()
                .map(|(field, min, max)| CheckedHirModelRangeFilterArgs {
                    field: checked_arg(field),
                    min: checked_arg(min),
                    max: checked_arg(max),
                });
        let composite_filters = match checked.composite_filter_args() {
            Some(filters) => {
                let mut pairs = Vec::new();
                for pair in filters.chunks(2) {
                    let [field, value] = pair else {
                        return Self {
                            lookup,
                            advanced_filter,
                            range_filter,
                            composite_filters: None,
                            ordering: None,
                            pagination: None,
                        };
                    };
                    pairs.push(CheckedHirModelLookupArgs {
                        field: checked_arg(field),
                        value: checked_arg(value),
                    });
                }
                Some(pairs)
            }
            None => None,
        };
        let ordering = checked
            .ordering
            .map(|ordering| CheckedHirModelOrderingArgs {
                field: checked_arg(ordering.field),
                direction: checked_arg(ordering.direction),
            });
        let pagination = checked
            .pagination
            .map(|pagination| CheckedHirModelPaginationArgs {
                limit: checked_arg(pagination.limit),
                offset: checked_arg(pagination.offset),
            });

        Self {
            lookup,
            advanced_filter,
            range_filter,
            composite_filters,
            ordering,
            pagination,
        }
    }

    pub(super) fn lookup(&self) -> Option<CheckedHirModelLookupArgs<'a>> {
        self.lookup
    }

    pub(super) fn advanced_filter(&self) -> Option<CheckedHirModelAdvancedFilterArgs<'a>> {
        self.advanced_filter
    }

    pub(super) fn range_filter(&self) -> Option<CheckedHirModelRangeFilterArgs<'a>> {
        self.range_filter
    }

    pub(super) fn composite_filters(&self) -> Option<&[CheckedHirModelLookupArgs<'a>]> {
        self.composite_filters.as_deref()
    }

    pub(super) fn ordering(&self) -> Option<CheckedHirModelOrderingArgs<'a>> {
        self.ordering
    }

    pub(super) fn pagination(&self) -> Option<CheckedHirModelPaginationArgs<'a>> {
        self.pagination
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct HirOperationContext<'hir, 'program, 'args, 'scope> {
    hir: Option<&'hir HirProgram<'program>>,
    args: Option<HirOperationArgs<'args>>,
    scope: &'scope Scope,
}

pub(super) type ModelOperationContext<'hir, 'program, 'args, 'scope> =
    HirOperationContext<'hir, 'program, 'args, 'scope>;

impl<'hir, 'program, 'args, 'scope> HirOperationContext<'hir, 'program, 'args, 'scope> {
    pub(super) fn ast(scope: &'scope Scope) -> Self {
        Self {
            hir: None,
            args: None,
            scope,
        }
    }

    pub(super) fn with_hir(
        hir: &'hir HirProgram<'program>,
        args: HirOperationArgs<'args>,
        scope: &'scope Scope,
    ) -> Self {
        Self {
            hir: Some(hir),
            args: Some(args),
            scope,
        }
    }

    pub(super) fn hir_expr(self, expr: &Expr) -> Option<(&'hir HirProgram<'program>, HirExprId)> {
        let hir = self.hir?;
        let expr_id = self.args?.id_for(expr)?;
        Some((hir, expr_id))
    }

    pub(super) fn hir_arg(
        self,
        expr_id: HirExprId,
    ) -> Option<(&'hir HirProgram<'program>, HirExprId)> {
        Some((self.hir?, expr_id))
    }

    pub(super) fn checked_hir_model_args(
        self,
        checked: CheckedModelOperationArgs<'args>,
    ) -> CheckedHirModelOperationArgs<'args> {
        CheckedHirModelOperationArgs::from_checked(checked, self.args)
    }

    pub(super) fn checked_hir_auth_args(
        self,
        checked: CheckedAuthOperationArgs<'args>,
    ) -> CheckedHirAuthOperationArgs<'args> {
        CheckedHirAuthOperationArgs::from_checked(checked, self.args)
    }

    pub(super) fn scope(self) -> &'scope Scope {
        self.scope
    }
}
