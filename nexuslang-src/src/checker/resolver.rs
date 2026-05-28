use std::collections::HashMap;

use crate::ast::Span;
use crate::hir::{HirDeclId, HirProgram, HirScopeId, HirSymbolId, HirSymbolKind};

type TopLevelKey<'a> = (HirSymbolKind, &'a str);
type BindingKey<'a> = (HirDeclId, HirSymbolKind, &'a str, usize, usize);
type ScopedBindingKey<'a> = (HirScopeId, HirSymbolKind, &'a str, usize, usize);
type ScopedNameKey<'a> = (HirScopeId, &'a str);

#[derive(Debug, Clone, Copy)]
struct ScopedBinding {
    symbol: HirSymbolId,
    kind: HirSymbolKind,
    span: Span,
}

#[derive(Debug, Default)]
pub(super) struct ResolvedProgram<'a> {
    top_level: HashMap<TopLevelKey<'a>, Vec<HirSymbolId>>,
    bindings: HashMap<BindingKey<'a>, HirSymbolId>,
    scoped_bindings: HashMap<ScopedBindingKey<'a>, HirSymbolId>,
    scoped_bindings_by_name: HashMap<ScopedNameKey<'a>, Vec<ScopedBinding>>,
    bindings_by_decl: HashMap<HirDeclId, Vec<HirSymbolId>>,
    symbols_by_scope: HashMap<HirScopeId, Vec<HirSymbolId>>,
    scope_parents: HashMap<HirScopeId, Option<HirScopeId>>,
}

impl<'a> ResolvedProgram<'a> {
    pub(super) fn top_level_symbol(&self, kind: HirSymbolKind, name: &str) -> Option<HirSymbolId> {
        self.top_level
            .get(&(kind, name))
            .and_then(|symbols| symbols.first().copied())
    }

    pub(super) fn binding_symbol(
        &self,
        decl: HirDeclId,
        kind: HirSymbolKind,
        name: &str,
        span: Span,
    ) -> Option<HirSymbolId> {
        self.bindings
            .get(&(decl, kind, name, span.line, span.column))
            .copied()
    }

    pub(super) fn binding_symbol_in_scope(
        &self,
        scope: HirScopeId,
        kind: HirSymbolKind,
        name: &str,
        span: Span,
    ) -> Option<HirSymbolId> {
        self.scoped_bindings
            .get(&(scope, kind, name, span.line, span.column))
            .copied()
    }

    pub(super) fn visible_binding_symbol(
        &self,
        scope: HirScopeId,
        kinds: &[HirSymbolKind],
        name: &str,
        use_span: Span,
    ) -> Option<HirSymbolId> {
        let mut current = Some(scope);
        while let Some(scope) = current {
            if let Some(bindings) = self.scoped_bindings_by_name.get(&(scope, name)) {
                for binding in bindings.iter().rev() {
                    if kinds.contains(&binding.kind) && binding_is_visible(binding.span, use_span) {
                        return Some(binding.symbol);
                    }
                }
            }
            current = self.scope_parent(scope);
        }
        None
    }

    pub(super) fn scope_parent(&self, scope: HirScopeId) -> Option<HirScopeId> {
        self.scope_parents.get(&scope).copied().flatten()
    }

    #[cfg(test)]
    fn bindings_for_decl(&self, decl: HirDeclId) -> &[HirSymbolId] {
        self.bindings_by_decl
            .get(&decl)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    #[cfg(test)]
    fn symbols_for_scope(&self, scope: HirScopeId) -> &[HirSymbolId] {
        self.symbols_by_scope
            .get(&scope)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    #[cfg(test)]
    fn parent_scope(&self, scope: HirScopeId) -> Option<HirScopeId> {
        self.scope_parent(scope)
    }
}

pub(super) fn resolve_program<'a>(hir: &'a HirProgram<'a>) -> ResolvedProgram<'a> {
    let mut resolved = ResolvedProgram::default();

    for scope in &hir.scopes {
        resolved.scope_parents.insert(scope.id, scope.parent);
    }

    for symbol in &hir.symbols {
        resolved
            .symbols_by_scope
            .entry(symbol.scope)
            .or_default()
            .push(symbol.id);

        if is_top_level_symbol(symbol.kind) {
            resolved
                .top_level
                .entry((symbol.kind, symbol.name))
                .or_default()
                .push(symbol.id);
            continue;
        }

        let Some(decl) = symbol.decl else {
            continue;
        };
        resolved
            .scoped_bindings
            .entry((
                symbol.scope,
                symbol.kind,
                symbol.name,
                symbol.span.line,
                symbol.span.column,
            ))
            .or_insert(symbol.id);
        resolved
            .scoped_bindings_by_name
            .entry((symbol.scope, symbol.name))
            .or_default()
            .push(ScopedBinding {
                symbol: symbol.id,
                kind: symbol.kind,
                span: symbol.span,
            });
        resolved
            .bindings
            .entry((
                decl,
                symbol.kind,
                symbol.name,
                symbol.span.line,
                symbol.span.column,
            ))
            .or_insert(symbol.id);
        resolved
            .bindings_by_decl
            .entry(decl)
            .or_default()
            .push(symbol.id);
    }

    resolved
}

fn binding_is_visible(binding_span: Span, use_span: Span) -> bool {
    if !binding_span.is_known() || !use_span.is_known() {
        return true;
    }
    binding_span.line < use_span.line
        || (binding_span.line == use_span.line && binding_span.column <= use_span.column)
}

fn is_top_level_symbol(kind: HirSymbolKind) -> bool {
    matches!(
        kind,
        HirSymbolKind::Function
            | HirSymbolKind::Model
            | HirSymbolKind::Workflow
            | HirSymbolKind::Auth
            | HirSymbolKind::Route
            | HirSymbolKind::ImportedSymbol
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Type;
    use crate::checker::Checker;
    use crate::hir::{
        lower_program, HirBinaryOp, HirDeclKind, HirExprKind, HirReferenceKind, HirScopeKind,
    };

    #[test]
    fn resolver_indexes_top_level_and_lexical_hir_symbols() {
        let source = r#"
model Customer {
    name: string
}

model User {
    email: string unique
}

auth UserAuth {
    model: User
    identity: email
}

fn summarize(customers: [Customer]) -> string {
    for customer in customers {
        let current: string = customer.name
        const label: string = current
    }
    return "ok"
}

route GET /customers/:name ?(limit: int = 10) {
    return Customer::find("name", name)
}
"#;

        let program = crate::parse_checked_source(source).unwrap();
        let hir = lower_program(&program);
        let resolved = resolve_program(&hir);

        assert!(resolved
            .top_level_symbol(HirSymbolKind::Model, "Customer")
            .is_some());
        assert!(resolved
            .top_level_symbol(HirSymbolKind::Auth, "UserAuth")
            .is_some());
        assert!(resolved
            .top_level_symbol(HirSymbolKind::Function, "summarize")
            .is_some());
        assert!(resolved
            .top_level_symbol(HirSymbolKind::Route, "/customers/:name")
            .is_some());

        for symbol in hir.symbols.iter().filter(|symbol| {
            matches!(
                symbol.kind,
                HirSymbolKind::Parameter
                    | HirSymbolKind::RouteParameter
                    | HirSymbolKind::QueryParameter
                    | HirSymbolKind::LetBinding
                    | HirSymbolKind::ConstBinding
                    | HirSymbolKind::ForBinding
            )
        }) {
            let decl = symbol.decl.expect("lexical HIR symbol should have a decl");
            assert_eq!(
                resolved.binding_symbol(decl, symbol.kind, symbol.name, symbol.span),
                Some(symbol.id)
            );
            assert_eq!(
                resolved.binding_symbol_in_scope(
                    symbol.scope,
                    symbol.kind,
                    symbol.name,
                    symbol.span
                ),
                Some(symbol.id)
            );
            assert!(resolved
                .symbols_for_scope(symbol.scope)
                .contains(&symbol.id));
        }

        let root_scope = hir
            .scopes
            .iter()
            .find(|scope| scope.kind == HirScopeKind::TopLevel)
            .unwrap();
        let customer_model = resolved
            .top_level_symbol(HirSymbolKind::Model, "Customer")
            .unwrap();
        assert!(resolved
            .symbols_for_scope(root_scope.id)
            .contains(&customer_model));

        let route = hir
            .decls
            .iter()
            .find(|decl| decl.kind == HirDeclKind::Route)
            .unwrap();
        let route_scope = route.scope.unwrap();
        assert_eq!(resolved.parent_scope(route_scope), Some(root_scope.id));
        assert!(resolved
            .symbols_for_scope(route_scope)
            .iter()
            .any(|symbol| {
                let symbol = hir.symbol(*symbol).unwrap();
                symbol.kind == HirSymbolKind::RouteParameter && symbol.name == "name"
            }));
        assert!(resolved
            .symbols_for_scope(route_scope)
            .iter()
            .any(|symbol| {
                let symbol = hir.symbol(*symbol).unwrap();
                symbol.kind == HirSymbolKind::QueryParameter && symbol.name == "limit"
            }));
        assert_eq!(resolved.bindings_for_decl(route.id).len(), 2);

        let loop_binding = hir
            .symbols
            .iter()
            .find(|symbol| symbol.kind == HirSymbolKind::ForBinding && symbol.name == "customer")
            .unwrap();
        let customers_param = hir
            .symbols
            .iter()
            .find(|symbol| symbol.kind == HirSymbolKind::Parameter && symbol.name == "customers")
            .unwrap();
        assert_eq!(
            resolved.visible_binding_symbol(
                loop_binding.scope,
                &[HirSymbolKind::Parameter],
                "customers",
                loop_binding.span
            ),
            Some(customers_param.id)
        );
    }

    #[test]
    fn checker_records_hir_definition_use_links_and_expr_types() {
        let source = r#"
model Customer {
    name: string
}

route GET /customers/:name {
    return Customer::find("name", name)
}
"#;

        let program = crate::parse_source(source).unwrap();
        let hir = lower_program(&program);
        let mut checker = Checker::new();
        checker.check_diagnostic(&program).unwrap();
        let metadata = checker.checked_hir_metadata();

        let route_name_symbol = hir
            .symbols
            .iter()
            .find(|symbol| symbol.kind == HirSymbolKind::RouteParameter && symbol.name == "name")
            .unwrap()
            .id;
        let route_name_expr = hir
            .exprs
            .iter()
            .find(|expr| matches!(&expr.kind, HirExprKind::Ident { name } if *name == "name"))
            .unwrap();
        assert_eq!(
            metadata.expr_symbol(route_name_expr.id),
            Some(route_name_symbol)
        );
        let name_ty = metadata.expr_type(route_name_expr.id).unwrap();
        assert_eq!(metadata.ty(name_ty), Some(&Type::String));

        let customer_symbol = hir
            .symbols
            .iter()
            .find(|symbol| symbol.kind == HirSymbolKind::Model && symbol.name == "Customer")
            .unwrap()
            .id;
        let static_call = hir
            .exprs
            .iter()
            .find(|expr| {
                matches!(
                    &expr.kind,
                    HirExprKind::StaticCall {
                        ty: "Customer",
                        method: "find",
                        ..
                    }
                )
            })
            .unwrap();
        assert_eq!(metadata.expr_symbol(static_call.id), Some(customer_symbol));
        let static_call_ty = metadata.expr_type(static_call.id).unwrap();
        assert_eq!(
            metadata.ty(static_call_ty),
            Some(&Type::Model("Customer".to_string()))
        );
    }

    #[test]
    fn checker_consumes_hir_metadata_for_route_return_exprs() {
        let source = r#"
model Customer {
    name: string
}

route GET /customers/:name {
    return Customer::find("name", name)
}
"#;

        let program = crate::parse_source(source).unwrap();
        let mut checker = Checker::new();
        checker.check_diagnostic(&program).unwrap();

        assert!(
            checker.hir_metadata_cache_hits() > 0,
            "checker should consume previously recorded HIR expression metadata"
        );
    }

    #[test]
    fn checker_uses_hir_scope_for_nested_local_binding_symbols() {
        let source = r#"
model Customer {
    name: string
}

fn summarize(customers: [Customer]) -> string {
    for customer in customers {
        let current: string = customer.name
        if true {
            const label: string = current
        }
    }
    return "ok"
}
"#;

        let program = crate::parse_source(source).unwrap();
        let hir = lower_program(&program);
        let mut checker = Checker::new();
        checker.check_diagnostic(&program).unwrap();
        let metadata = checker.checked_hir_metadata();

        assert!(
            checker.scoped_hir_binding_hits() >= 4,
            "checker should resolve function params, loop bindings, and nested locals through HirScopeId"
        );

        let current_symbol = hir
            .symbols
            .iter()
            .find(|symbol| symbol.kind == HirSymbolKind::LetBinding && symbol.name == "current")
            .unwrap()
            .id;
        let current_ident = hir
            .exprs
            .iter()
            .find(|expr| matches!(&expr.kind, HirExprKind::Ident { name } if *name == "current"))
            .unwrap();
        assert_eq!(metadata.expr_symbol(current_ident.id), Some(current_symbol));

        let customer_symbol = hir
            .symbols
            .iter()
            .find(|symbol| symbol.kind == HirSymbolKind::ForBinding && symbol.name == "customer")
            .unwrap()
            .id;
        let customer_ident = hir
            .exprs
            .iter()
            .find(|expr| matches!(&expr.kind, HirExprKind::Ident { name } if *name == "customer"))
            .unwrap();
        assert_eq!(
            metadata.expr_symbol(customer_ident.id),
            Some(customer_symbol)
        );
    }

    #[test]
    fn checker_records_and_consumes_typed_hir_bindings() {
        let source = r#"
fn sum(items: [int]) -> int {
    let total: int = 0
    total = 1
    for item in items {
        let current: int = item
        total = current
    }
    return total
}

route GET /items/:name ?(limit: int = 10) {
    return name + str(limit)
}
"#;

        let program = crate::parse_source(source).unwrap();
        let hir = lower_program(&program);
        let mut checker = Checker::new();
        checker.check_diagnostic(&program).unwrap();
        let metadata = checker.checked_hir_metadata();

        assert!(
            checker.typed_hir_binding_hits() > 0,
            "checker should consume typed HirSymbolId metadata for identifiers or assignments"
        );

        let symbol_ty = |kind, name| {
            let symbol = hir
                .symbols
                .iter()
                .find(|symbol| symbol.kind == kind && symbol.name == name)
                .unwrap()
                .id;
            metadata
                .ty(metadata.symbol_type(symbol).unwrap())
                .cloned()
                .unwrap()
        };

        assert_eq!(
            symbol_ty(HirSymbolKind::Parameter, "items"),
            Type::Array(Box::new(Type::Int))
        );
        assert_eq!(symbol_ty(HirSymbolKind::LetBinding, "total"), Type::Int);
        assert_eq!(symbol_ty(HirSymbolKind::ForBinding, "item"), Type::Int);
        assert_eq!(symbol_ty(HirSymbolKind::LetBinding, "current"), Type::Int);
        assert_eq!(
            symbol_ty(HirSymbolKind::RouteParameter, "name"),
            Type::String
        );
        assert_eq!(symbol_ty(HirSymbolKind::QueryParameter, "limit"), Type::Int);

        let total_symbol = hir
            .symbols
            .iter()
            .find(|symbol| symbol.kind == HirSymbolKind::LetBinding && symbol.name == "total")
            .unwrap()
            .id;
        let total_ident = hir
            .exprs
            .iter()
            .find(|expr| matches!(&expr.kind, HirExprKind::Ident { name } if *name == "total"))
            .unwrap();
        assert_eq!(metadata.expr_symbol(total_ident.id), Some(total_symbol));
        let total_ty = metadata.expr_type(total_ident.id).unwrap();
        assert_eq!(metadata.ty(total_ty), Some(&Type::Int));
    }

    #[test]
    fn checker_consumes_typed_hir_context_for_complex_route_exprs() {
        let source = r#"
model Customer {
    name: string
}

route GET /customers/:name {
    return Customer::find("name", name).name
}

route GET /labels/:name ?(limit: int = 10) {
    return name + str(limit)
}

route GET /literal {
    return Customer { name: "Ana" }
}
"#;

        let program = crate::parse_source(source).unwrap();
        let hir = lower_program(&program);
        let mut checker = Checker::new();
        checker.check_diagnostic(&program).unwrap();
        let metadata = checker.checked_hir_metadata();

        assert!(
            checker.typed_hir_expr_context_hits() >= 3,
            "checker should consume typed HIR context for route expression validation"
        );

        let customer_name_symbol = hir
            .symbols
            .iter()
            .find(|symbol| symbol.kind == HirSymbolKind::ModelField && symbol.name == "name")
            .unwrap()
            .id;

        let field_access = hir
            .exprs
            .iter()
            .find(|expr| {
                matches!(&expr.kind, HirExprKind::FieldAccess { field, .. } if *field == "name")
            })
            .unwrap();
        assert_eq!(
            metadata.expr_symbol(field_access.id),
            Some(customer_name_symbol)
        );
        let field_ty = metadata.expr_type(field_access.id).unwrap();
        assert_eq!(metadata.ty(field_ty), Some(&Type::String));

        let binop = hir
            .exprs
            .iter()
            .find(|expr| {
                matches!(&expr.kind, HirExprKind::Binary { op, .. } if *op == HirBinaryOp::Add)
            })
            .unwrap();
        let binop_ty = metadata.expr_type(binop.id).unwrap();
        assert_eq!(metadata.ty(binop_ty), Some(&Type::String));

        let str_call = hir
            .exprs
            .iter()
            .find(|expr| matches!(&expr.kind, HirExprKind::Call { name, .. } if *name == "str"))
            .unwrap();
        let str_call_ty = metadata.expr_type(str_call.id).unwrap();
        assert_eq!(metadata.ty(str_call_ty), Some(&Type::String));

        let object = hir
            .exprs
            .iter()
            .find(|expr| {
                matches!(&expr.kind, HirExprKind::Object { model, .. } if *model == "Customer")
            })
            .unwrap();
        let object_ty = metadata.expr_type(object.id).unwrap();
        assert_eq!(
            metadata.ty(object_ty),
            Some(&Type::Model("Customer".to_string()))
        );
    }

    #[test]
    fn checker_uses_typed_hir_expression_checker_for_function_exprs() {
        let source = r#"
model Customer {
    name: string
    score: int
}

fn summarize(customer: Customer, bonus: int) -> string {
    let current: string = customer.name
    let total: int = customer.score + bonus
    return current + str(total)
}
"#;

        let program = crate::parse_source(source).unwrap();
        let hir = lower_program(&program);
        let mut checker = Checker::new();
        checker.check_diagnostic(&program).unwrap();
        let metadata = checker.checked_hir_metadata();

        assert!(
            checker.typed_hir_expression_checker_hits() > 0,
            "checker should infer function expressions through HirExprId/HirExprKind"
        );

        let score_symbol = hir
            .symbols
            .iter()
            .find(|symbol| symbol.kind == HirSymbolKind::ModelField && symbol.name == "score")
            .unwrap()
            .id;
        let score_access = hir
            .exprs
            .iter()
            .find(|expr| {
                matches!(&expr.kind, HirExprKind::FieldAccess { field, .. } if *field == "score")
            })
            .unwrap();
        assert_eq!(metadata.expr_symbol(score_access.id), Some(score_symbol));
        let score_ty = metadata.expr_type(score_access.id).unwrap();
        assert_eq!(metadata.ty(score_ty), Some(&Type::Int));

        let int_binop = hir
            .exprs
            .iter()
            .find(|expr| {
                matches!(&expr.kind, HirExprKind::Binary { op, .. } if *op == HirBinaryOp::Add)
                    && metadata.expr_type(expr.id).and_then(|ty| metadata.ty(ty))
                        == Some(&Type::Int)
            })
            .unwrap();
        let int_binop_ty = metadata.expr_type(int_binop.id).unwrap();
        assert_eq!(metadata.ty(int_binop_ty), Some(&Type::Int));

        let str_call = hir
            .exprs
            .iter()
            .find(|expr| matches!(&expr.kind, HirExprKind::Call { name, .. } if *name == "str"))
            .unwrap();
        let str_call_ty = metadata.expr_type(str_call.id).unwrap();
        assert_eq!(metadata.ty(str_call_ty), Some(&Type::String));
    }

    #[test]
    fn typed_hir_expression_checker_preserves_field_error_message() {
        let source = r#"
model Customer {
    name: string
}

fn bad(customer: Customer) -> string {
    return customer.missing
}
"#;

        let program = crate::parse_source(source).unwrap();
        let mut checker = Checker::new();
        let diagnostic = checker.check_diagnostic(&program).unwrap_err();

        assert_eq!(diagnostic.message, "Campo 'Customer.missing' nao existe");
    }

    #[test]
    fn checker_uses_hir_operation_args_for_route_static_calls() {
        let source = r#"
model Customer {
    name: string
    score: int
}

route GET /customers ?(score: int = 10, limit: int = 5, offset: int = 0) {
    return Customer::where("score", score, limit, offset)
}
"#;

        let program = crate::parse_source(source).unwrap();
        let hir = lower_program(&program);
        let mut checker = Checker::new();
        checker.check_diagnostic(&program).unwrap();
        let metadata = checker.checked_hir_metadata();

        assert_eq!(
            checker.typed_hir_operation_arg_hits(),
            4,
            "route static-call args should be ensured once through HirExprId/source AST"
        );
        assert_eq!(
            checker.typed_hir_model_op_validator_hits(),
            3,
            "lookup and pagination validators should infer route static-call args through HIR without duplicate post-processing"
        );

        let static_call = hir
            .exprs
            .iter()
            .find(|expr| {
                matches!(
                    &expr.kind,
                    HirExprKind::StaticCall {
                        ty,
                        method,
                        ..
                    } if *ty == "Customer" && *method == "where"
                )
            })
            .unwrap();
        let HirExprKind::StaticCall { args, .. } = &static_call.kind else {
            unreachable!();
        };
        let value_arg = args[1];
        let value_ty = metadata.expr_type(value_arg).unwrap();
        assert_eq!(metadata.ty(value_ty), Some(&Type::Int));
        let limit_arg = args[2];
        let limit_ty = metadata.expr_type(limit_arg).unwrap();
        assert_eq!(metadata.ty(limit_ty), Some(&Type::Int));
        let offset_arg = args[3];
        let offset_ty = metadata.expr_type(offset_arg).unwrap();
        assert_eq!(metadata.ty(offset_ty), Some(&Type::Int));

        let query_symbol = hir
            .symbols
            .iter()
            .find(|symbol| symbol.kind == HirSymbolKind::QueryParameter && symbol.name == "score")
            .unwrap()
            .id;
        assert_eq!(metadata.expr_symbol(value_arg), Some(query_symbol));
    }

    #[test]
    fn checker_uses_hir_model_op_validators_for_advanced_and_composite_filters() {
        let source = r#"
model Customer {
    name: string
    status: string
    score: int
}

route GET /customers/compare ?(min_score: int = 10, limit: int = 5, offset: int = 0) {
    return Customer::where_compare_page("score", ">=", min_score, limit, offset)
}

route GET /customers/range ?(min_score: int = 10, max_score: int = 99) {
    return Customer::where_between("score", min_score, max_score)
}

route GET /customers/composite ?(score: int = 10, status: string = "active", limit: int = 5, offset: int = 0) {
    return Customer::where_all_page("score", score, "status", status, limit, offset)
}
"#;

        let program = crate::parse_source(source).unwrap();
        let hir = lower_program(&program);
        let mut checker = Checker::new();
        checker.check_diagnostic(&program).unwrap();
        let metadata = checker.checked_hir_metadata();

        assert_eq!(
            checker.typed_hir_model_op_validator_hits(),
            9,
            "advanced/composite model-op validators should infer route static-call args through HIR without duplicate post-processing"
        );

        let static_call_args = |method: &str| -> Vec<_> {
            let expr = hir
                .exprs
                .iter()
                .find(|expr| {
                    matches!(
                        &expr.kind,
                        HirExprKind::StaticCall { ty, method: candidate, .. }
                            if *ty == "Customer" && *candidate == method
                    )
                })
                .unwrap();
            let HirExprKind::StaticCall { args, .. } = &expr.kind else {
                unreachable!();
            };
            args.clone()
        };

        let compare_args = static_call_args("where_compare_page");
        assert_hir_arg_type_and_symbol(&hir, &metadata, compare_args[2], &Type::Int, "min_score");
        assert_hir_arg_type_and_symbol(&hir, &metadata, compare_args[3], &Type::Int, "limit");
        assert_hir_arg_type_and_symbol(&hir, &metadata, compare_args[4], &Type::Int, "offset");

        let range_args = static_call_args("where_between");
        assert_hir_arg_type_and_symbol(&hir, &metadata, range_args[1], &Type::Int, "min_score");
        assert_hir_arg_type_and_symbol(&hir, &metadata, range_args[2], &Type::Int, "max_score");

        let composite_args = static_call_args("where_all_page");
        assert_hir_arg_type_and_symbol(&hir, &metadata, composite_args[1], &Type::Int, "score");
        assert_hir_arg_type_and_symbol(&hir, &metadata, composite_args[3], &Type::String, "status");
    }

    #[test]
    fn checker_links_model_operation_field_args_to_hir_field_symbols() {
        let source = r#"
model Customer {
    name: string
    status: string
    score: int
}

route GET /customers/compare ?(min_score: int = 10, limit: int = 5, offset: int = 0) {
    return Customer::where_compare_page("score", ">=", min_score, "name", "asc", limit, offset)
}

route GET /customers/composite ?(status: string = "active") {
    return Customer::where_all("status", status, "name", "Ana")
}
"#;

        let program = crate::parse_source(source).unwrap();
        let hir = lower_program(&program);
        let mut checker = Checker::new();
        checker.check_diagnostic(&program).unwrap();
        let metadata = checker.checked_hir_metadata();

        assert!(
            checker.typed_hir_expr_symbol_hits() >= 4,
            "model operation field-name args should consume linked HirExprId symbol metadata"
        );

        let static_call_args = |method: &str| -> Vec<_> {
            let expr = hir
                .exprs
                .iter()
                .find(|expr| {
                    matches!(
                        &expr.kind,
                        HirExprKind::StaticCall { ty, method: candidate, .. }
                            if *ty == "Customer" && *candidate == method
                    )
                })
                .unwrap();
            let HirExprKind::StaticCall { args, .. } = &expr.kind else {
                unreachable!();
            };
            args.clone()
        };

        let compare_args = static_call_args("where_compare_page");
        assert_hir_model_field_arg_metadata(&hir, &metadata, compare_args[0], "score", &Type::Int);
        assert_hir_model_field_arg_metadata(
            &hir,
            &metadata,
            compare_args[3],
            "name",
            &Type::String,
        );

        let composite_args = static_call_args("where_all");
        assert_hir_model_field_arg_metadata(
            &hir,
            &metadata,
            composite_args[0],
            "status",
            &Type::String,
        );
        assert_hir_model_field_arg_metadata(
            &hir,
            &metadata,
            composite_args[2],
            "name",
            &Type::String,
        );
    }

    #[test]
    fn checker_links_run_workflow_literal_to_hir_workflow_symbol() {
        let source = r#"
workflow Billing {
    step preparar {
        print("Preparar fatura")
    }
}

run_workflow("Billing")
"#;

        let program = crate::parse_source(source).unwrap();
        let hir = lower_program(&program);
        let mut checker = Checker::new();
        checker.check_diagnostic(&program).unwrap();
        let metadata = checker.checked_hir_metadata();

        assert!(
            checker.typed_hir_expr_symbol_hits() >= 1,
            "run_workflow literal should consume linked HirExprId symbol metadata"
        );

        let workflow_symbol = hir
            .symbols
            .iter()
            .find(|symbol| symbol.kind == HirSymbolKind::Workflow && symbol.name == "Billing")
            .unwrap()
            .id;
        let run_workflow_call = hir
            .exprs
            .iter()
            .find(|expr| {
                matches!(&expr.kind, HirExprKind::Call { name, .. } if *name == "run_workflow")
            })
            .unwrap();
        let HirExprKind::Call { args, .. } = &run_workflow_call.kind else {
            unreachable!();
        };

        let workflow_arg = args[0];
        assert_eq!(metadata.expr_symbol(workflow_arg), Some(workflow_symbol));
        let workflow_arg_ty = metadata.expr_type(workflow_arg).unwrap();
        assert_eq!(metadata.ty(workflow_arg_ty), Some(&Type::String));

        let call_ty = metadata.expr_type(run_workflow_call.id).unwrap();
        assert_eq!(metadata.ty(call_ty), Some(&Type::Void));
    }

    #[test]
    fn checker_links_auth_declaration_and_route_guard_references() {
        let source = r#"
model User {
    email: string unique
    role: string
}

auth Session {
    model: User
    identity: email
    role: role
}

route GET /me auth(Session, role: "admin") {
    return "ok"
}
"#;

        let program = crate::parse_source(source).unwrap();
        let hir = lower_program(&program);
        let mut checker = Checker::new();
        checker.check_diagnostic(&program).unwrap();
        let metadata = checker.checked_hir_metadata();

        assert!(
            checker.typed_hir_reference_hits() >= 4,
            "auth declarations and route guards should consume linked HirRefId metadata"
        );

        assert_hir_reference_symbol(
            &hir,
            &metadata,
            HirReferenceKind::AuthModel,
            "User",
            HirSymbolKind::Model,
            "User",
        );
        assert_hir_reference_symbol(
            &hir,
            &metadata,
            HirReferenceKind::AuthIdentityField,
            "email",
            HirSymbolKind::ModelField,
            "email",
        );
        assert_hir_reference_symbol(
            &hir,
            &metadata,
            HirReferenceKind::AuthRoleField,
            "role",
            HirSymbolKind::ModelField,
            "role",
        );
        assert_hir_reference_symbol(
            &hir,
            &metadata,
            HirReferenceKind::RouteAuthGuard,
            "Session",
            HirSymbolKind::Auth,
            "Session",
        );
    }

    #[test]
    fn checker_links_object_literal_field_references() {
        let source = r#"
model Customer {
    name: string
    active: bool
}

route GET /literal {
    return Customer { name: "Ana", active: true }
}
"#;

        let program = crate::parse_source(source).unwrap();
        let hir = lower_program(&program);
        let mut checker = Checker::new();
        checker.check_diagnostic(&program).unwrap();
        let metadata = checker.checked_hir_metadata();

        assert!(
            checker.typed_hir_reference_hits() >= 2,
            "object literal validation should consume linked HirReference metadata"
        );

        let fields = hir
            .exprs
            .iter()
            .find_map(|expr| match &expr.kind {
                HirExprKind::Object { model, fields } if *model == "Customer" => Some(fields),
                _ => None,
            })
            .unwrap();

        let name_field = fields.iter().find(|field| field.name == "name").unwrap();
        let name_symbol = assert_hir_reference_id_symbol(
            &hir,
            &metadata,
            name_field.field_ref,
            HirReferenceKind::ObjectField,
            "name",
            HirSymbolKind::ModelField,
            "name",
        );
        let name_ty = metadata.symbol_type(name_symbol).unwrap();
        assert_eq!(metadata.ty(name_ty), Some(&Type::String));

        let active_field = fields.iter().find(|field| field.name == "active").unwrap();
        let active_symbol = assert_hir_reference_id_symbol(
            &hir,
            &metadata,
            active_field.field_ref,
            HirReferenceKind::ObjectField,
            "active",
            HirSymbolKind::ModelField,
            "active",
        );
        let active_ty = metadata.symbol_type(active_symbol).unwrap();
        assert_eq!(metadata.ty(active_ty), Some(&Type::Bool));
    }

    fn assert_hir_arg_type_and_symbol(
        hir: &HirProgram<'_>,
        metadata: &crate::hir::HirCheckedMetadata,
        expr: crate::hir::HirExprId,
        expected_ty: &Type,
        expected_symbol_name: &str,
    ) {
        let ty = metadata.expr_type(expr).unwrap();
        assert_eq!(metadata.ty(ty), Some(expected_ty));

        let symbol = metadata
            .expr_symbol(expr)
            .and_then(|symbol| hir.symbol(symbol))
            .unwrap();
        assert_eq!(symbol.kind, HirSymbolKind::QueryParameter);
        assert_eq!(symbol.name, expected_symbol_name);
    }

    fn assert_hir_model_field_arg_metadata(
        hir: &HirProgram<'_>,
        metadata: &crate::hir::HirCheckedMetadata,
        expr: crate::hir::HirExprId,
        expected_field_name: &str,
        expected_field_ty: &Type,
    ) {
        let expr_ty = metadata.expr_type(expr).unwrap();
        assert_eq!(metadata.ty(expr_ty), Some(&Type::String));

        let expected_symbol = hir
            .symbols
            .iter()
            .find(|symbol| {
                symbol.kind == HirSymbolKind::ModelField && symbol.name == expected_field_name
            })
            .unwrap()
            .id;
        assert_eq!(metadata.expr_symbol(expr), Some(expected_symbol));

        let field_ty = metadata.symbol_type(expected_symbol).unwrap();
        assert_eq!(metadata.ty(field_ty), Some(expected_field_ty));
    }

    fn assert_hir_reference_id_symbol(
        hir: &HirProgram<'_>,
        metadata: &crate::hir::HirCheckedMetadata,
        reference_id: crate::hir::HirRefId,
        reference_kind: HirReferenceKind,
        reference_name: &str,
        expected_symbol_kind: HirSymbolKind,
        expected_symbol_name: &str,
    ) -> crate::hir::HirSymbolId {
        let reference = hir.reference(reference_id).unwrap();
        assert_eq!(reference.kind, reference_kind);
        assert_eq!(reference.name, reference_name);

        let symbol_id = metadata.reference_symbol(reference.id).unwrap();
        let symbol = hir.symbol(symbol_id).unwrap();
        assert_eq!(symbol.kind, expected_symbol_kind);
        assert_eq!(symbol.name, expected_symbol_name);
        symbol_id
    }

    fn assert_hir_reference_symbol(
        hir: &HirProgram<'_>,
        metadata: &crate::hir::HirCheckedMetadata,
        reference_kind: HirReferenceKind,
        reference_name: &str,
        expected_symbol_kind: HirSymbolKind,
        expected_symbol_name: &str,
    ) {
        let reference = hir
            .references
            .iter()
            .find(|reference| reference.kind == reference_kind && reference.name == reference_name)
            .unwrap();
        let symbol = metadata
            .reference_symbol(reference.id)
            .and_then(|symbol| hir.symbol(symbol))
            .unwrap();
        assert_eq!(symbol.kind, expected_symbol_kind);
        assert_eq!(symbol.name, expected_symbol_name);
    }
}
