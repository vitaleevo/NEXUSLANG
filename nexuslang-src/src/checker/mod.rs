use std::cell::Cell;
#[cfg(test)]
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diagnostic::{
    checker_code_for_message, codes, Diagnostic, DiagnosticOwner, DiagnosticStage,
};
use crate::hir;
use crate::hir::{HirDeclBody, HirDeclId, HirDeclKind, HirModuleId, HirSymbolKind, HirSymbolRef};
use crate::module_loader::{self, ModuleGraph};

mod auth_decl;
mod auth_static_ops;
mod binding_resolution;
mod expr;
mod function_decl;
mod hir_args;
mod hir_expr;
mod hir_metadata;
mod invoice_decl;
mod model_decl;
mod model_ops;
mod program_flow;
mod resolver;
mod route_decl;
mod route_expr;
mod route_static_ops;
mod scope;
mod statement_decl;
mod stmt;
mod symbol_lookup;
mod symbols;
mod type_core;
mod type_rules;
mod typed_hir_pass;
mod workflow_decl;

use resolver::ResolvedProgram;
use scope::Scope;
use symbols::CheckerSymbols;
use type_core::{ensure_assignable, type_name};
use typed_hir_pass::TypedHirMetadataStore;

type CheckResult<T> = Result<T, Diagnostic>;

#[derive(Debug, Clone)]
struct FunctionSig {
    params: Vec<(String, Type)>,
    return_type: Type,
}

pub struct Checker {
    functions: HashMap<String, FunctionSig>,
    models: HashMap<String, Vec<Field>>,
    auths: HashMap<String, AuthConfig>,
    workflows: HashSet<String>,
    symbols: CheckerSymbols,
    hir_metadata: TypedHirMetadataStore,
    /// Stores the cross-module import resolutions after
    /// `check_with_module_graph` completes, keyed by `HirDeclId`.
    /// Empty (default) for single-file checking.
    import_resolutions: HashMap<HirDeclId, HirSymbolRef>,
    /// Maps import alias names to their original declaration names
    /// (e.g. `import User as Usuario` → "Usuario" → "User").
    /// Used by `hir_model_fields` and other lookup functions to find
    /// the original HIR decl when an alias is used.
    import_aliases: HashMap<String, String>,
    /// Optional module ownership for each merged-program declaration.
    ///
    /// Empty in single-file checking. Populated during graph-aware checking so
    /// checker diagnostics can carry the exact declaration/module owner.
    diagnostic_decl_modules: Vec<HirModuleId>,
    current_diagnostic_owner: Cell<Option<DiagnosticOwner>>,
    #[cfg(test)]
    hir_metadata_cache_hits: RefCell<usize>,
    #[cfg(test)]
    scoped_hir_binding_hits: RefCell<usize>,
    #[cfg(test)]
    typed_hir_binding_hits: RefCell<usize>,
    #[cfg(test)]
    typed_hir_expr_context_hits: RefCell<usize>,
    #[cfg(test)]
    typed_hir_expr_symbol_hits: RefCell<usize>,
    #[cfg(test)]
    typed_hir_expression_checker_hits: RefCell<usize>,
    #[cfg(test)]
    typed_hir_operation_arg_hits: RefCell<usize>,
    #[cfg(test)]
    typed_hir_model_op_validator_hits: RefCell<usize>,
    #[cfg(test)]
    typed_hir_reference_hits: RefCell<usize>,
}

impl Default for Checker {
    fn default() -> Self {
        Self::new()
    }
}

impl Checker {
    pub fn new() -> Self {
        Checker {
            functions: HashMap::new(),
            models: HashMap::new(),
            auths: HashMap::new(),
            workflows: HashSet::new(),
            symbols: CheckerSymbols::default(),
            hir_metadata: TypedHirMetadataStore::default(),
            import_resolutions: HashMap::new(),
            import_aliases: HashMap::new(),
            diagnostic_decl_modules: Vec::new(),
            current_diagnostic_owner: Cell::new(None),
            #[cfg(test)]
            hir_metadata_cache_hits: RefCell::new(0),
            #[cfg(test)]
            scoped_hir_binding_hits: RefCell::new(0),
            #[cfg(test)]
            typed_hir_binding_hits: RefCell::new(0),
            #[cfg(test)]
            typed_hir_expr_context_hits: RefCell::new(0),
            #[cfg(test)]
            typed_hir_expr_symbol_hits: RefCell::new(0),
            #[cfg(test)]
            typed_hir_expression_checker_hits: RefCell::new(0),
            #[cfg(test)]
            typed_hir_operation_arg_hits: RefCell::new(0),
            #[cfg(test)]
            typed_hir_model_op_validator_hits: RefCell::new(0),
            #[cfg(test)]
            typed_hir_reference_hits: RefCell::new(0),
        }
    }

    pub fn check(&mut self, program: &Program) -> Result<(), String> {
        self.check_diagnostic(program)
            .map_err(|diagnostic| diagnostic.to_string())
    }

    pub fn check_diagnostic(&mut self, program: &Program) -> CheckResult<()> {
        let hir = hir::lower_program(program);
        let resolved = resolver::resolve_program(&hir);
        self.symbols = CheckerSymbols::default();
        self.symbols.index_hir(&hir);
        self.reset_diagnostic_owner_context(&[]);
        self.begin_typed_hir_metadata_pass(&hir);
        self.collect_decls(program, &hir, &resolved)?;
        self.check_decls(program, &hir, &resolved)
    }

    /// Multi-module checking: same as `check_diagnostic` but also resolves
    /// cross-module `import` declarations using the `ModuleGraph` and
    /// `decl_module_map`.
    ///
    /// After lowering, calls `resolve_hir_imports` and patches each
    /// `HirDeclBody::Import.resolved` with the matching `HirSymbolRef`.
    pub fn check_with_module_graph(
        &mut self,
        program: &Program,
        module_graph: &ModuleGraph,
        decl_module_map: &[HirModuleId],
    ) -> CheckResult<()> {
        let mut hir = hir::lower_program(program);

        // Resolve cross-module imports and patch the HIR
        // (do this BEFORE resolve_program to avoid borrow conflicts)
        let import_resolutions =
            module_loader::resolve_hir_imports(&hir, module_graph, decl_module_map);
        for decl in &mut hir.decls {
            if let HirDeclBody::Import {
                ref mut resolved, ..
            } = decl.body
            {
                if let Some(sym_ref) = import_resolutions.get(&decl.id) {
                    *resolved = Some(*sym_ref);
                }
            }
        }

        self.import_resolutions = import_resolutions;

        let resolved = resolver::resolve_program(&hir);
        self.symbols = CheckerSymbols::default();
        self.symbols.index_hir(&hir);
        self.reset_diagnostic_owner_context(decl_module_map);
        self.begin_typed_hir_metadata_pass(&hir);
        self.collect_decls(program, &hir, &resolved)?;
        // Register import aliases: when `import X as Y` is used, the alias
        // name "Y" must be findable in the checker's symbol tables (models,
        // functions, etc.) so that type annotations, object expressions,
        // and call expressions resolve to the right symbol.
        self.register_import_aliases(&hir);
        self.check_decls(program, &hir, &resolved)
    }

    /// Report-oriented multi-module checking.
    ///
    /// This preserves the same loader/import setup used by
    /// `check_with_module_graph`, but accumulates checker diagnostics from
    /// independent declaration bodies where continuing is safe. Global
    /// collection/setup failures still return as a single diagnostic.
    pub fn check_with_module_graph_diagnostics(
        &mut self,
        program: &Program,
        module_graph: &ModuleGraph,
        decl_module_map: &[HirModuleId],
    ) -> Result<(), Vec<Diagnostic>> {
        let mut hir = hir::lower_program(program);

        let import_resolutions =
            module_loader::resolve_hir_imports(&hir, module_graph, decl_module_map);
        for decl in &mut hir.decls {
            if let HirDeclBody::Import {
                ref mut resolved, ..
            } = decl.body
            {
                if let Some(sym_ref) = import_resolutions.get(&decl.id) {
                    *resolved = Some(*sym_ref);
                }
            }
        }

        self.import_resolutions = import_resolutions;

        let resolved = resolver::resolve_program(&hir);
        self.symbols = CheckerSymbols::default();
        self.symbols.index_hir(&hir);
        self.reset_diagnostic_owner_context(decl_module_map);
        self.begin_typed_hir_metadata_pass(&hir);
        self.collect_decls(program, &hir, &resolved)
            .map_err(|diagnostic| vec![diagnostic])?;
        self.register_import_aliases(&hir);
        self.check_decls_collecting_independent_diagnostics(program, &hir, &resolved)
    }

    /// Registers import aliases in the checker's symbol tables.
    ///
    /// For each `HirDeclBody::Import { resolved: Some(sym_ref), .. }`:
    /// 1. Records the alias → original name mapping in `import_aliases`.
    /// 2. Calls `set_top_level` with the *target* symbol's kind (Model,
    ///    Function, Workflow, Auth) so that the alias name (e.g. "Y" in
    ///    `import X as Y`) is findable as the right kind.
    /// 3. Copies the declaration data (model fields, function signatures,
    ///    auth configs, workflow set entries) under the alias name so
    ///    that all checker lookups succeed for the alias.
    fn register_import_aliases(&mut self, hir: &crate::hir::HirProgram<'_>) {
        for decl in &hir.decls {
            if decl.kind != HirDeclKind::Import {
                continue;
            }
            let HirDeclBody::Import {
                resolved: Some(sym_ref),
                ..
            } = &decl.body
            else {
                continue;
            };
            let Some(target_sym) = hir.symbol(sym_ref.symbol) else {
                continue;
            };
            let Some(name) = decl.name else {
                continue;
            };

            // 1. Record alias → original name mapping (for HIR decl lookups)
            self.import_aliases
                .insert(name.to_string(), target_sym.name.to_string());

            // 2. Register the alias name under the target symbol's kind
            self.symbols
                .set_top_level(target_sym.kind, name, decl.symbol);

            // 3. Copy declaration data under the alias name so that
            //    `self.functions`, `self.models`, etc. resolve correctly.
            match target_sym.kind {
                HirSymbolKind::Function => {
                    if let Some(sig) = self.functions.get(target_sym.name) {
                        self.functions.insert(name.to_string(), sig.clone());
                    }
                }
                HirSymbolKind::Model => {
                    if let Some(fields) = self.models.get(target_sym.name) {
                        self.models.insert(name.to_string(), fields.clone());
                    }
                }
                HirSymbolKind::Auth => {
                    if let Some(config) = self.auths.get(target_sym.name) {
                        self.auths.insert(name.to_string(), config.clone());
                    }
                }
                HirSymbolKind::Workflow if self.workflows.contains(target_sym.name) => {
                    self.workflows.insert(name.to_string());
                }
                _ => {}
            }
        }
    }

    /// Returns the cross-module import resolutions recorded by the most
    /// recent `check_with_module_graph` call.
    ///
    /// Each entry maps `HirDeclId` (an import decl) to the
    /// `HirSymbolRef { module, symbol }` it resolved to.
    /// Returns an empty map if the checker was used in single-file mode.
    pub fn checked_import_resolutions(&self) -> &HashMap<HirDeclId, HirSymbolRef> {
        &self.import_resolutions
    }

    fn reset_diagnostic_owner_context(&mut self, decl_module_map: &[HirModuleId]) {
        self.diagnostic_decl_modules.clear();
        self.diagnostic_decl_modules
            .extend_from_slice(decl_module_map);
        self.current_diagnostic_owner.set(None);
    }

    pub(super) fn enter_diagnostic_decl_owner(&self, decl_index: usize) {
        let owner = self
            .diagnostic_decl_modules
            .get(decl_index)
            .map(|module_id| DiagnosticOwner::new(decl_index).with_module_id(module_id.0))
            .unwrap_or_else(|| DiagnosticOwner::new(decl_index));
        self.current_diagnostic_owner.set(Some(owner));
    }

    pub(super) fn clear_diagnostic_decl_owner(&self) {
        self.current_diagnostic_owner.set(None);
    }

    fn error(&self, span: Span, message: impl Into<String>) -> Diagnostic {
        let message = message.into();
        let code = checker_code_for_message(&message);
        let diagnostic = Diagnostic::new(DiagnosticStage::Checker, message)
            .with_code(code)
            .with_span(span);
        let diagnostic = enrich_checker_diagnostic(diagnostic, code, span);
        if let Some(owner) = self.current_diagnostic_owner.get() {
            diagnostic.with_owner(owner)
        } else {
            diagnostic
        }
    }
}

fn enrich_checker_diagnostic(diagnostic: Diagnostic, code: &str, span: Span) -> Diagnostic {
    match code {
        codes::CHECKER_TYPE => diagnostic
            .with_label_at("origem do erro de tipo", span.line, span.column)
            .with_note("O checker compara o tipo inferido com o tipo esperado neste ponto.")
            .with_suggestion("Ajuste a anotacao de tipo ou o valor produzido pela expressao."),
        codes::CHECKER_SYMBOL => diagnostic
            .with_label_at("referencia de simbolo", span.line, span.column)
            .with_note("O checker nao encontrou uma declaracao, binding ou import visivel para este nome.")
            .with_suggestion("Declare o simbolo antes do uso, importe-o ou corrija o nome."),
        codes::CHECKER_ARGUMENT => diagnostic
            .with_label_at("argumentos verificados aqui", span.line, span.column)
            .with_note("A chamada ou operacao recebeu uma quantidade ou forma de argumentos incompatível.")
            .with_suggestion("Ajuste os argumentos para combinar com a assinatura esperada."),
        codes::CHECKER_MODEL => diagnostic
            .with_label_at("uso de model aqui", span.line, span.column)
            .with_note("Operacoes de model exigem que o model e seus campos estejam declarados e visiveis.")
            .with_suggestion("Declare ou importe o model esperado, ou corrija o nome do model/campo."),
        codes::CHECKER_ROUTE => diagnostic
            .with_label_at("route verificada aqui", span.line, span.column)
            .with_note("Routes precisam respeitar o contrato de metodo, path, parametros e retorno.")
            .with_suggestion("Revise a declaracao da route e os valores retornados."),
        codes::CHECKER_AUTH => diagnostic
            .with_label_at("auth verificado aqui", span.line, span.column)
            .with_note("Configuracoes de auth precisam referenciar modelos e campos validos.")
            .with_suggestion("Revise o auth, identity, roles e campos relacionados."),
        codes::CHECKER_WORKFLOW => diagnostic
            .with_label_at("workflow verificado aqui", span.line, span.column)
            .with_note("Workflows chamados ou declarados precisam existir no programa carregado.")
            .with_suggestion("Declare o workflow esperado ou corrija o nome usado."),
        codes::CHECKER_INVOICE => diagnostic
            .with_label_at("invoice verificado aqui", span.line, span.column)
            .with_note("Invoices precisam declarar os campos obrigatorios e itens validos.")
            .with_suggestion("Revise customer, currency, item/total e tipos dos valores."),
        _ => diagnostic,
    }
}
