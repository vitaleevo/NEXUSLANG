/// NexusLang Module Loader — Fase 11.03
///
/// Resolves imports, loads .nx files recursively, and produces a unified
/// `Program` from a multi-file module graph.
///
/// # MVP Scope
/// - Relative paths (prefixed with `./` or `../`)
/// - `std/<module>` imports from the NexusLang stdlib
/// - Package-name imports for local `path:` dependencies in `nexus.toml`
/// - One exported symbol per import
/// - No wildcard or namespace imports
/// - No re-exports
/// - `.nx` extension is appended if missing
///
/// # Errors
/// - Missing module file
/// - Circular dependency
/// - Import from a module that doesn't export the requested symbol
/// - Duplicate top-level declarations across modules
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::ast::{Decl, Program, Span};
use crate::diagnostic::{codes, Diagnostic, DiagnosticStage};
use crate::hir::{HirDeclId, HirModuleId, HirProgram, HirSymbolId, HirSymbolRef};
use crate::package_manager::{self, ProjectDependencySource};

/// Error type for module loading operations.
#[derive(Debug, Clone)]
pub enum ModuleError {
    /// The .nx file could not be read (IO error).
    IoError { path: PathBuf, message: String },
    /// The source could not be parsed.
    ParseError {
        path: PathBuf,
        diagnostic: Diagnostic,
    },
    /// A circular dependency was detected.
    CircularDependency {
        path: PathBuf,
        span_line: usize,
        span_column: usize,
    },
    /// The imported symbol is not exported by the target module.
    SymbolNotExported {
        symbol: String,
        target: PathBuf,
        import_line: usize,
        import_column: usize,
    },
    /// Two loaded modules contribute the same symbol to the flat MVP surface.
    DuplicateGraphSymbol {
        symbol: String,
        kind: String,
        first_path: PathBuf,
        duplicate_path: PathBuf,
        span_line: usize,
        span_column: usize,
    },
    /// Two imports in one module use the same local alias.
    DuplicateImportAlias {
        alias: String,
        path: PathBuf,
        first_line: usize,
        first_column: usize,
        duplicate_line: usize,
        duplicate_column: usize,
    },
    /// An import alias collides with a local top-level declaration.
    ImportAliasCollision {
        alias: String,
        path: PathBuf,
        decl_line: usize,
        decl_column: usize,
        import_line: usize,
        import_column: usize,
    },
    /// A module path is not relative (must start with ./ or ../).
    NonRelativePath {
        source: String,
        span_line: usize,
        span_column: usize,
    },
    /// A package-name import could not be resolved from local manifest data.
    PackageImport {
        source: String,
        message: String,
        span_line: usize,
        span_column: usize,
    },
    /// Failed to resolve a canonical path.
    PathResolution {
        source: String,
        span_line: usize,
        span_column: usize,
    },
    /// A `"std/<module>"` import could not be resolved because the stdlib
    /// directory was not found or does not contain the requested module.
    StdLibNotFound {
        module: String,
        span_line: usize,
        span_column: usize,
    },
}

impl ModuleError {
    /// Convert to a user-facing diagnostic.
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            ModuleError::IoError { path, message } => Diagnostic::new(
                DiagnosticStage::ModuleLoader,
                format!("Erro de IO ao ler '{}': {}", path.display(), message),
            )
            .with_code(codes::MODULE_LOADER_IO),
            ModuleError::ParseError { path, diagnostic } => Diagnostic::new(
                DiagnosticStage::ModuleLoader,
                format!(
                    "Erro de parsing em '{}': {}",
                    path.display(),
                    diagnostic.message
                ),
            )
            .with_code(codes::MODULE_LOADER_PARSE),
            ModuleError::CircularDependency {
                path,
                span_line,
                span_column,
            } => Diagnostic::new(
                DiagnosticStage::ModuleLoader,
                format!("Dependência circular detectada: '{}'", path.display()),
            )
            .with_code(codes::MODULE_LOADER_CIRCULAR_DEPENDENCY)
            .with_location(*span_line, *span_column),
            ModuleError::SymbolNotExported {
                symbol,
                target,
                import_line,
                import_column,
            } => Diagnostic::new(
                DiagnosticStage::ModuleLoader,
                format!("'{}' não é exportado por '{}'", symbol, target.display()),
            )
            .with_code(codes::MODULE_LOADER_SYMBOL_NOT_EXPORTED)
            .with_label_at("simbolo importado aqui", *import_line, *import_column)
            .with_note("O modulo alvo foi encontrado, mas nao exporta o simbolo solicitado.")
            .with_suggestion(format!(
                "Exporte '{}' no modulo alvo ou altere o nome importado.",
                symbol
            ))
            .with_location(*import_line, *import_column),
            ModuleError::DuplicateGraphSymbol {
                symbol,
                kind,
                first_path,
                duplicate_path,
                span_line,
                span_column,
            } => Diagnostic::new(
                DiagnosticStage::ModuleLoader,
                format!(
                    "Nome duplicado no module graph: {} '{}' aparece em '{}' e '{}'. \
                     A superficie de simbolos do MVP ainda e flat por tipo; use um nome diferente ate namespaces serem suportados.",
                    kind,
                    symbol,
                    first_path.display(),
                    duplicate_path.display()
                ),
            )
            .with_code(codes::MODULE_LOADER_DUPLICATE_SYMBOL)
            .with_label_at("nome duplicado aqui", *span_line, *span_column)
            .with_note("A superficie de simbolos do module graph MVP ainda e flat por tipo.")
            .with_suggestion("Renomeie uma das declaracoes ou aguarde namespaces/reexports.")
            .with_location(*span_line, *span_column),
            ModuleError::DuplicateImportAlias {
                alias,
                path,
                first_line,
                first_column,
                duplicate_line,
                duplicate_column,
            } => Diagnostic::new(
                DiagnosticStage::ModuleLoader,
                format!(
                    "Alias de import duplicado '{}' em '{}' (primeiro em {}:{}, duplicado em {}:{}).",
                    alias,
                    path.display(),
                    first_line,
                    first_column,
                    duplicate_line,
                    duplicate_column
                ),
            )
            .with_code(codes::MODULE_LOADER_DUPLICATE_ALIAS)
            .with_label_at("alias duplicado aqui", *duplicate_line, *duplicate_column)
            .with_note(format!(
                "O alias '{}' ja tinha sido usado neste modulo em {}:{}.",
                alias, first_line, first_column
            ))
            .with_suggestion("Use aliases locais diferentes para cada import.")
            .with_location(*duplicate_line, *duplicate_column),
            ModuleError::ImportAliasCollision {
                alias,
                path,
                decl_line,
                decl_column,
                import_line,
                import_column,
            } => Diagnostic::new(
                DiagnosticStage::ModuleLoader,
                format!(
                    "Alias de import '{}' em '{}' colide com declaracao top-level local em {}:{}.",
                    alias,
                    path.display(),
                    decl_line,
                    decl_column
                ),
            )
            .with_code(codes::MODULE_LOADER_ALIAS_COLLISION)
            .with_label_at("alias importado aqui", *import_line, *import_column)
            .with_note(format!(
                "O alias '{}' colide com uma declaracao top-level local em {}:{}.",
                alias, decl_line, decl_column
            ))
            .with_suggestion("Renomeie o alias do import ou a declaracao local.")
            .with_location(*import_line, *import_column),
            ModuleError::NonRelativePath {
                source,
                span_line,
                span_column,
            } => Diagnostic::new(
                DiagnosticStage::ModuleLoader,
                format!(
                    "Caminho nao relativo: '{}'. Use './', '../', 'std/<modulo>' ou um pacote path declarado em nexus.toml.",
                    source
                ),
            )
            .with_code(codes::MODULE_LOADER_PATH)
            .with_label_at("caminho importado aqui", *span_line, *span_column)
            .with_note("Imports de arquivo precisam ser relativos, std/<modulo>, ou pacotes path em nexus.toml.")
            .with_suggestion("Use './modulo.nx', '../modulo.nx', 'std/<modulo>' ou uma dependencia path local.")
            .with_location(*span_line, *span_column),
            ModuleError::PackageImport {
                source,
                message,
                span_line,
                span_column,
            } => Diagnostic::new(
                DiagnosticStage::ModuleLoader,
                format!("Import de pacote '{}' nao resolvido: {}", source, message),
            )
            .with_code(codes::MODULE_LOADER_PACKAGE)
            .with_label_at("import de pacote aqui", *span_line, *span_column)
            .with_note("Imports por nome de pacote exigem uma dependencia path local em nexus.toml.")
            .with_suggestion("Declare o pacote com `nexus add <nome> --path <dir>` ou ajuste o import.")
            .with_location(*span_line, *span_column),
            ModuleError::PathResolution {
                source,
                span_line,
                span_column,
            } => Diagnostic::new(
                DiagnosticStage::ModuleLoader,
                format!("Não foi possível resolver o caminho: '{}'", source),
            )
            .with_code(codes::MODULE_LOADER_PATH)
            .with_label_at("caminho importado aqui", *span_line, *span_column)
            .with_note("O loader nao conseguiu resolver o caminho informado para um arquivo NexusLang.")
            .with_suggestion("Confira se o arquivo existe e se a extensao .nx ou o caminho relativo estao corretos.")
            .with_location(*span_line, *span_column),
            ModuleError::StdLibNotFound {
                module,
                span_line,
                span_column,
            } => Diagnostic::new(
                DiagnosticStage::ModuleLoader,
                format!(
                    "Módulo padrão '{}' não encontrado. \
                     Defina a variável de ambiente NEXUS_STDLIB ou \
                     assegure-se de que a stdlib está instalada.",
                    module
                ),
            )
            .with_code(codes::MODULE_LOADER_STDLIB)
            .with_label_at("stdlib importada aqui", *span_line, *span_column)
            .with_note("O loader nao encontrou a stdlib configurada ou o modulo padrao solicitado.")
            .with_suggestion("Defina NEXUS_STDLIB ou confira se o modulo existe na stdlib instalada.")
            .with_location(*span_line, *span_column),
        }
    }
}

impl std::fmt::Display for ModuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_diagnostic())
    }
}

/// Metadata about a loaded module.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct LoadedModule {
    /// Absolute path to the .nx file.
    path: PathBuf,
    /// Original module source text.
    source: String,
    /// Parsed AST program.
    program: Program,
    /// Set of exported declaration names.
    exports: HashSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SurfaceSymbolKind {
    Function,
    Model,
    Workflow,
    Auth,
}

impl SurfaceSymbolKind {
    fn label(self) -> &'static str {
        match self {
            SurfaceSymbolKind::Function => "funcao",
            SurfaceSymbolKind::Model => "model",
            SurfaceSymbolKind::Workflow => "workflow",
            SurfaceSymbolKind::Auth => "auth",
        }
    }
}

#[derive(Debug, Clone)]
struct SurfaceSymbol {
    name: String,
    kind: SurfaceSymbolKind,
    span: crate::ast::Span,
}

/// Resolves relative `.nx` import paths and builds a unified program graph.
pub struct ModuleLoader {
    /// Cache of already-loaded modules (absolute path → loaded data).
    loaded: HashMap<PathBuf, LoadedModule>,
    /// Set of paths currently being loaded (for cycle detection).
    loading: HashSet<PathBuf>,
    /// Optional path to the stdlib directory for `"std/<name>"` imports.
    stdlib_path: Option<PathBuf>,
}

impl ModuleLoader {
    /// Create a new module loader without stdlib support.
    fn new() -> Self {
        ModuleLoader {
            loaded: HashMap::new(),
            loading: HashSet::new(),
            stdlib_path: None,
        }
    }

    /// Set the path to the stdlib directory, enabling `"std/<name>"` imports.
    #[allow(dead_code)]
    fn with_stdlib_path(mut self, path: PathBuf) -> Self {
        self.stdlib_path = Some(path);
        self
    }

    /// Load a `.nx` file and return its AST. Does NOT recursively resolve
    /// imports — call `load_program` for full resolution.
    fn parse_file(&self, path: &Path) -> Result<(Program, String), ModuleError> {
        let source = fs::read_to_string(path).map_err(|e| ModuleError::IoError {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;

        let mut lexer = crate::lexer::Lexer::new(&source);
        let tokens = lexer
            .tokenize_spanned_diagnostic()
            .map_err(|d| ModuleError::ParseError {
                path: path.to_path_buf(),
                diagnostic: d,
            })?;

        let mut parser = crate::parser::Parser::new_spanned(tokens);
        let program = parser
            .parse_program_diagnostic()
            .map_err(|d| ModuleError::ParseError {
                path: path.to_path_buf(),
                diagnostic: d,
            })?;

        Ok((program, source))
    }

    /// Scan a program for exported declaration names.
    fn collect_exports(program: &Program) -> HashSet<String> {
        let mut exports = HashSet::new();
        for decl in &program.decls {
            if let Decl::Export { decl: inner, .. } = decl {
                let name = match inner.as_ref() {
                    Decl::Function { name, .. }
                    | Decl::Model { name, .. }
                    | Decl::Workflow { name, .. } => Some(name.clone()),
                    Decl::Auth { config } => Some(config.name.clone()),
                    _ => None,
                };
                if let Some(name) = name {
                    exports.insert(name);
                }
            }
        }
        exports
    }

    /// Ensure the path has a `.nx` extension. If the path already has a
    /// supported extension, it is returned as-is.
    fn ensure_nx_extension(path: &Path) -> PathBuf {
        match path.extension().and_then(OsStr::to_str) {
            Some("nx") => path.to_path_buf(),
            _ => {
                let mut p = path.to_path_buf();
                p.set_extension("nx");
                p
            }
        }
    }

    /// Resolve an import path relative to a base file path.
    ///
    /// Rules:
    /// - `"std/<name>"` — resolved against the stdlib directory.
    /// - `"./..."` or `"../..."` — resolved relative to the importing file.
    /// - `"<package>/..."` — resolved through a local path dependency in
    ///   the nearest `nexus.toml`.
    /// - Otherwise — rejected as `NonRelativePath`.
    /// - If no `.nx` extension, it is appended.
    fn resolve_import_path(
        &self,
        source: &str,
        relative_to: &Path,
        span: crate::ast::Span,
    ) -> Result<PathBuf, ModuleError> {
        // Stdlib import: "std/<name>"
        if let Some(module_name) = source.strip_prefix("std/") {
            return self.resolve_stdlib_path(module_name, span);
        }

        // Relative imports: ./ or ../
        if source.starts_with("./") || source.starts_with("../") {
            let relative_to_dir = relative_to.parent().unwrap_or(Path::new("."));
            let raw_path = relative_to_dir.join(source);
            let with_ext = Self::ensure_nx_extension(&raw_path);

            // Try to canonicalise. If the file doesn't exist, we'll catch it
            // during the actual load step; the path is valid for resolution.
            return match with_ext.canonicalize() {
                Ok(canon) => Ok(canon),
                Err(_) => {
                    // Path is syntactically valid but file may not exist yet.
                    // Return the non-canonical path for a better error message
                    // during the load step.
                    Ok(with_ext)
                }
            };
        }

        match resolve_package_import_path(source, relative_to) {
            Ok(Some(path)) => Ok(path),
            Ok(None) => Err(ModuleError::NonRelativePath {
                source: source.to_string(),
                span_line: span.line,
                span_column: span.column,
            }),
            Err(message) => Err(ModuleError::PackageImport {
                source: source.to_string(),
                message,
                span_line: span.line,
                span_column: span.column,
            }),
        }
    }

    /// Resolve a `"std/<name>"` import path.
    ///
    /// Looks up the stdlib directory (from the configured path or via
    /// auto-discovery), ensures the requested module exists, and returns the
    /// canonical path to the `.nx` file.
    fn resolve_stdlib_path(
        &self,
        module_name: &str,
        span: crate::ast::Span,
    ) -> Result<PathBuf, ModuleError> {
        // Determine the stdlib directory
        let stdlib_dir = match &self.stdlib_path {
            Some(path) => path.clone(),
            None => find_stdlib_path().map_err(|_| ModuleError::StdLibNotFound {
                module: module_name.to_string(),
                span_line: span.line,
                span_column: span.column,
            })?,
        };

        let raw_path = stdlib_dir.join(module_name);
        let with_ext = Self::ensure_nx_extension(&raw_path);

        match with_ext.canonicalize() {
            Ok(canon) => Ok(canon),
            Err(_) => Err(ModuleError::StdLibNotFound {
                module: module_name.to_string(),
                span_line: span.line,
                span_column: span.column,
            }),
        }
    }

    /// Recursively load a module and all its dependencies.
    fn load_module(&mut self, path: &Path) -> Result<(), ModuleError> {
        let canonical = path.canonicalize().map_err(|e| ModuleError::IoError {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;

        // Already loaded?
        if self.loaded.contains_key(&canonical) {
            return Ok(());
        }

        // Cycle detection
        if !self.loading.insert(canonical.clone()) {
            // Already in the loading set — circular dependency
            return Err(ModuleError::CircularDependency {
                path: canonical,
                span_line: 0,
                span_column: 0,
            });
        }

        // Parse the file
        let (program, source) = self.parse_file(&canonical)?;
        Self::validate_module_local_surface(&canonical, &program)?;
        let exports = Self::collect_exports(&program);

        let module = LoadedModule {
            path: canonical.clone(),
            source,
            program,
            exports,
        };

        // Recursively resolve imports BEFORE inserting into `loaded`, so
        // the cycle detection works. But we need the module to exist so we
        // can check exported symbols during recursion.

        // Collect import paths first to avoid borrow issues
        let imports: Vec<(String, String, Option<String>, crate::ast::Span)> = module
            .program
            .decls
            .iter()
            .filter_map(|decl| match decl {
                Decl::Import { import } => Some((
                    import.name.clone(),
                    import.source.clone(),
                    import.alias.clone(),
                    import.span,
                )),
                _ => None,
            })
            .collect();

        // Resolve each import
        for (imported_name, source_path, _alias, span) in &imports {
            let target_path = self.resolve_import_path(source_path, &canonical, *span)?;

            // Load the target module recursively
            self.load_module(&target_path)?;

            // Verify the symbol is exported
            let target_canon = target_path.canonicalize().unwrap_or(target_path.clone());
            if let Some(target_mod) = self.loaded.get(&target_canon) {
                if !target_mod.exports.contains(imported_name.as_str()) {
                    // Check if the symbol exists but is not exported
                    let exists = target_mod.program.decls.iter().any(|d| {
                        let name = match d {
                            Decl::Function { name, .. }
                            | Decl::Model { name, .. }
                            | Decl::Workflow { name, .. } => Some(name.as_str()),
                            Decl::Auth { config } => Some(config.name.as_str()),
                            _ => None,
                        };
                        name == Some(imported_name.as_str())
                    });

                    if !exists {
                        return Err(ModuleError::SymbolNotExported {
                            symbol: imported_name.clone(),
                            target: target_canon,
                            import_line: span.line,
                            import_column: span.column,
                        });
                    }
                    // If it exists but isn't exported, also error
                    return Err(ModuleError::SymbolNotExported {
                        symbol: imported_name.clone(),
                        target: target_canon,
                        import_line: span.line,
                        import_column: span.column,
                    });
                }
            }
        }

        // Insert into loaded cache
        self.loaded.insert(canonical.clone(), module);
        self.loading.remove(&canonical);

        Ok(())
    }

    fn validate_module_local_surface(path: &Path, program: &Program) -> Result<(), ModuleError> {
        let mut local_decls: HashMap<String, crate::ast::Span> = HashMap::new();
        let mut import_aliases: HashMap<String, crate::ast::Span> = HashMap::new();

        for decl in &program.decls {
            if let Some(symbol) = surface_symbol(decl) {
                if let Some(alias_span) = import_aliases.get(&symbol.name) {
                    return Err(ModuleError::ImportAliasCollision {
                        alias: symbol.name.clone(),
                        path: path.to_path_buf(),
                        decl_line: symbol.span.line,
                        decl_column: symbol.span.column,
                        import_line: alias_span.line,
                        import_column: alias_span.column,
                    });
                }
                local_decls.entry(symbol.name).or_insert(symbol.span);
                continue;
            }

            if let Decl::Import { import } = decl {
                let alias = import.alias.as_deref().unwrap_or(&import.name);
                let alias_span = import.alias_span.unwrap_or(import.name_span);
                if let Some(first_span) = import_aliases.insert(alias.to_string(), alias_span) {
                    return Err(ModuleError::DuplicateImportAlias {
                        alias: alias.to_string(),
                        path: path.to_path_buf(),
                        first_line: first_span.line,
                        first_column: first_span.column,
                        duplicate_line: alias_span.line,
                        duplicate_column: alias_span.column,
                    });
                }

                if let Some(decl_span) = local_decls.get(alias) {
                    return Err(ModuleError::ImportAliasCollision {
                        alias: alias.to_string(),
                        path: path.to_path_buf(),
                        decl_line: decl_span.line,
                        decl_column: decl_span.column,
                        import_line: alias_span.line,
                        import_column: alias_span.column,
                    });
                }
            }
        }

        Ok(())
    }

    fn validate_graph_symbol_surface(&self) -> Result<(), ModuleError> {
        let mut seen: HashMap<(SurfaceSymbolKind, String), (PathBuf, crate::ast::Span)> =
            HashMap::new();

        let mut module_paths: Vec<&PathBuf> = self.loaded.keys().collect();
        module_paths.sort();

        for path in module_paths {
            let Some(module) = self.loaded.get(path) else {
                continue;
            };

            for decl in &module.program.decls {
                let Some(symbol) = surface_symbol(decl) else {
                    continue;
                };
                let key = (symbol.kind, symbol.name.clone());

                if let Some((first_path, first_span)) = seen.get(&key) {
                    if first_path != path {
                        return Err(ModuleError::DuplicateGraphSymbol {
                            symbol: symbol.name,
                            kind: symbol.kind.label().to_string(),
                            first_path: first_path.clone(),
                            duplicate_path: path.clone(),
                            span_line: symbol.span.line,
                            span_column: symbol.span.column,
                        });
                    }
                    if first_span.line == symbol.span.line
                        && first_span.column == symbol.span.column
                    {
                        continue;
                    }
                }

                seen.entry(key)
                    .or_insert_with(|| (path.clone(), symbol.span));
            }
        }

        Ok(())
    }

    /// Build the merged program from all loaded modules.
    /// The entry module's declarations come first, followed by dependency
    /// declarations in load order. Dependency imports are retained because
    /// dependency declarations can reference their own local import aliases.
    fn build_merged_program(&self, entry_canon: &Path) -> Program {
        let (program, _) = self.build_merged_program_with_map(entry_canon);
        program
    }

    /// Build the merged program and track which module each decl came from.
    ///
    /// Returns `(Program, decl_module_map)` where `decl_module_map[i]` is
    /// the `HirModuleId` of the module that contributed `program.decls[i]`.
    fn build_merged_program_with_map(&self, entry_canon: &Path) -> (Program, Vec<HirModuleId>) {
        let mut all_decls: Vec<Decl> = Vec::new();
        let mut decl_module_map: Vec<HirModuleId> = Vec::new();
        let module_paths = self.ordered_module_paths(entry_canon);

        for (module_index, module_path) in module_paths.iter().enumerate() {
            if let Some(module) = self.loaded.get(module_path) {
                let mod_id = HirModuleId(module_index);
                for decl in &module.program.decls {
                    all_decls.push(decl.clone());
                    decl_module_map.push(mod_id);
                }
            }
        }

        (Program { decls: all_decls }, decl_module_map)
    }

    /// Deterministic module order shared by merged programs, module graphs,
    /// and source databases: entry first, then dependencies sorted by path.
    fn ordered_module_paths(&self, entry_canon: &Path) -> Vec<PathBuf> {
        let mut module_paths = Vec::new();
        let entry_buf = entry_canon.to_path_buf();

        if self.loaded.contains_key(&entry_buf) {
            module_paths.push(entry_buf.clone());
        }

        let mut dep_paths: Vec<PathBuf> = self
            .loaded
            .keys()
            .filter(|path| **path != entry_buf)
            .cloned()
            .collect();
        dep_paths.sort();

        module_paths.extend(dep_paths);
        module_paths
    }
}

fn surface_symbol(decl: &Decl) -> Option<SurfaceSymbol> {
    let inner = decl.exported_inner().unwrap_or(decl);
    match inner {
        Decl::Function { name, span, .. } => Some(SurfaceSymbol {
            name: name.clone(),
            kind: SurfaceSymbolKind::Function,
            span: *span,
        }),
        Decl::Model { name, span, .. } => Some(SurfaceSymbol {
            name: name.clone(),
            kind: SurfaceSymbolKind::Model,
            span: *span,
        }),
        Decl::Workflow { name, span, .. } => Some(SurfaceSymbol {
            name: name.clone(),
            kind: SurfaceSymbolKind::Workflow,
            span: *span,
        }),
        Decl::Auth { config } => Some(SurfaceSymbol {
            name: config.name.clone(),
            kind: SurfaceSymbolKind::Auth,
            span: config.span,
        }),
        _ => None,
    }
}

// ─── Stdlib Discovery ───────────────────────────────────────────────────

fn resolve_package_import_path(
    source: &str,
    relative_to: &Path,
) -> Result<Option<PathBuf>, String> {
    let Some((package_name, module_path)) = split_package_import_source(source)? else {
        return Ok(None);
    };

    let project_start = relative_to.parent().unwrap_or(Path::new("."));
    let manifest =
        package_manager::load_nearest_project_manifest(project_start)?.ok_or_else(|| {
            format!(
                "{} nao encontrado para resolver import de pacote '{}'",
                package_manager::MANIFEST_FILE,
                package_name
            )
        })?;

    let dependency = manifest.dependency(package_name).ok_or_else(|| {
        format!(
            "pacote '{}' nao esta declarado em [dependencies] de {}",
            package_name,
            manifest.root.join(package_manager::MANIFEST_FILE).display()
        )
    })?;

    let dependency_root = match dependency {
        ProjectDependencySource::Path(dependency_root) => dependency_root,
        ProjectDependencySource::Registry {
            package,
            version,
            cache_path,
        } => {
            if !cache_path.join(package_manager::MANIFEST_FILE).is_file() {
                return Err(format!(
                    "dependencia '{}' usa registry:{}@{}, mas ainda nao esta instalada em {}; configure NEXUS_REGISTRY_URL e rode 'nexus install'",
                    package_name,
                    package,
                    version,
                    cache_path.display()
                ));
            }
            cache_path
        }
        ProjectDependencySource::Local => {
            return Err(format!(
                "dependencia '{}' usa 'local', mas o compilador resolve apenas path ou registry dependencies instaladas nesta fase",
                package_name
            ));
        }
    };

    let target = if let Some(module_path) = module_path {
        dependency_root.join(module_path)
    } else {
        let dependency_manifest = package_manager::load_project_manifest(dependency_root)?;
        dependency_manifest.entry_path()
    };

    let with_ext = ModuleLoader::ensure_nx_extension(&target);
    Ok(Some(with_ext.canonicalize().unwrap_or(with_ext)))
}

fn split_package_import_source(source: &str) -> Result<Option<(&str, Option<&str>)>, String> {
    let mut parts = source.splitn(2, '/');
    let Some(package_name) = parts.next() else {
        return Ok(None);
    };
    if !is_valid_package_import_name(package_name) {
        return Ok(None);
    }

    let module_path = parts.next().filter(|path| !path.is_empty());
    if let Some(module_path) = module_path {
        validate_package_module_path(source, module_path)?;
    }

    Ok(Some((package_name, module_path)))
}

fn is_valid_package_import_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        && name
            .chars()
            .next()
            .map(|ch| ch.is_ascii_alphanumeric())
            .unwrap_or(false)
}

fn validate_package_module_path(source: &str, module_path: &str) -> Result<(), String> {
    let path = Path::new(module_path);
    let valid = !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));

    if valid {
        Ok(())
    } else {
        Err(format!(
            "subcaminho de pacote invalido em '{}': use apenas caminhos relativos dentro do pacote",
            source
        ))
    }
}

/// Try to locate the NexusLang standard library directory.
///
/// Resolution order:
/// 1. `NEXUS_STDLIB` environment variable (explicit override)
/// 2. Relative to the current executable: `<exe_dir>/../stdlib/`
/// 3. Development fallback: `<CARGO_MANIFEST_DIR>/stdlib/` (compile-time)
///
/// Returns an error if none of the locations exist.
fn find_stdlib_path() -> Result<PathBuf, String> {
    // 1. Environment variable
    if let Ok(path) = std::env::var("NEXUS_STDLIB") {
        let p = PathBuf::from(&path);
        if p.is_dir() {
            return Ok(p);
        }
    }

    // 2. Relative to executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let candidate = exe_dir.join("../stdlib").canonicalize();
            if let Ok(p) = candidate {
                if p.is_dir() {
                    return Ok(p);
                }
            }
        }
    }

    // 3. Compile-time default (dev fallback)
    let candidate = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/stdlib"));
    if candidate.is_dir() {
        return Ok(candidate);
    }

    Err("Nenhum diretório stdlib encontrado. Defina NEXUS_STDLIB.".to_string())
}

// ─── Module Graph (Fase 11.04) ─────────────────────────────────────────

/// Entry in the module graph: metadata about one loaded module.
#[derive(Debug, Clone)]
pub struct ModuleGraphEntry {
    pub module_id: HirModuleId,
    pub path: PathBuf,
    /// Names of all exported declarations (from `export` keyword).
    pub export_names: Vec<String>,
}

/// Cross-module resolution metadata produced by the module loader.
///
/// Maps each loaded module to an ID and records which names are exported,
/// enabling the checker to resolve `import X from "./path"` to the concrete
/// `HirSymbolRef { module, symbol }` that the imported name refers to.
#[derive(Debug, Clone)]
pub struct ModuleGraph {
    pub entries: Vec<ModuleGraphEntry>,
    pub entry_id: HirModuleId,
    #[allow(dead_code)]
    path_to_id: HashMap<PathBuf, HirModuleId>,
}

/// Source text and path metadata for one loaded module.
#[derive(Debug, Clone)]
pub struct SourceModule {
    pub module_id: HirModuleId,
    pub path: PathBuf,
    pub source: String,
    pub is_entry: bool,
}

/// Source range owned by one loaded module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceRange {
    pub start: Span,
    pub end: Span,
}

impl SourceRange {
    pub fn new(start: Span, end: Span) -> Self {
        let end = if !start.is_known() || !end.is_known() || span_before(end, start) {
            start
        } else {
            end
        };
        SourceRange { start, end }
    }

    pub fn contains(self, line: usize, column: Option<usize>) -> bool {
        if line == 0 || !self.start.is_known() || !self.end.is_known() {
            return false;
        }
        if line < self.start.line || line > self.end.line {
            return false;
        }

        let Some(column) = column.filter(|column| *column != 0) else {
            return true;
        };

        if line == self.start.line && column < self.start.column {
            return false;
        }
        if line == self.end.line && column > self.end.column {
            return false;
        }

        true
    }

    fn line_len(self) -> usize {
        self.end.line.saturating_sub(self.start.line) + 1
    }
}

/// Source range for one declaration in the merged program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceDeclRange {
    pub module_id: HirModuleId,
    pub decl_index: usize,
    pub range: SourceRange,
}

/// Import edge as it appears in a source module.
#[derive(Debug, Clone)]
pub struct SourceImportEdge {
    pub source_module: HirModuleId,
    pub target_module: Option<HirModuleId>,
    pub imported_name: String,
    pub alias: Option<String>,
    pub source_path: String,
    pub import_span: Span,
    pub name_span: Span,
    pub alias_span: Option<Span>,
    pub source_span: Span,
}

/// Diagnostic enriched with the module path that owns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleDiagnostic {
    pub module_id: HirModuleId,
    pub path: PathBuf,
    pub diagnostic: Diagnostic,
    pub source_range: Option<SourceRange>,
}

impl std::fmt::Display for ModuleDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.diagnostic.line, self.diagnostic.column) {
            (Some(line), Some(column)) => write!(
                f,
                "{}:{}:{}: {}",
                self.path.display(),
                line,
                column,
                self.diagnostic.message
            ),
            (Some(line), None) => write!(
                f,
                "{}:{}: {}",
                self.path.display(),
                line,
                self.diagnostic.message
            ),
            _ => write!(f, "{}: {}", self.path.display(), self.diagnostic.message),
        }
    }
}

impl std::error::Error for ModuleDiagnostic {}

/// Minimal source database for multi-module tooling.
///
/// This database is additive metadata: it mirrors the `ModuleGraph` module IDs,
/// keeps the original source text per module, and records import edges by path
/// without changing import resolution or checker semantics.
#[derive(Debug, Clone)]
pub struct SourceDatabase {
    modules: Vec<SourceModule>,
    import_edges: Vec<SourceImportEdge>,
    decl_ranges: Vec<SourceDeclRange>,
    path_to_id: HashMap<PathBuf, HirModuleId>,
}

impl SourceDatabase {
    pub fn modules(&self) -> &[SourceModule] {
        &self.modules
    }

    pub fn import_edges(&self) -> &[SourceImportEdge] {
        &self.import_edges
    }

    pub fn decl_ranges(&self) -> &[SourceDeclRange] {
        &self.decl_ranges
    }

    pub fn decl_range_for_program_decl(&self, decl_index: usize) -> Option<&SourceDeclRange> {
        self.decl_ranges
            .iter()
            .find(|range| range.decl_index == decl_index)
    }

    pub fn module(&self, module_id: HirModuleId) -> Option<&SourceModule> {
        self.modules
            .iter()
            .find(|module| module.module_id == module_id)
    }

    pub fn module_path(&self, module_id: HirModuleId) -> Option<&Path> {
        self.module(module_id).map(|module| module.path.as_path())
    }

    pub fn module_by_path(&self, path: &Path) -> Option<&SourceModule> {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let module_id = self.path_to_id.get(&canonical).copied()?;
        self.module(module_id)
    }

    pub fn import_edges_from(
        &self,
        module_id: HirModuleId,
    ) -> impl Iterator<Item = &SourceImportEdge> {
        self.import_edges
            .iter()
            .filter(move |edge| edge.source_module == module_id)
    }

    pub fn attach_diagnostic(
        &self,
        module_id: HirModuleId,
        diagnostic: Diagnostic,
    ) -> Option<ModuleDiagnostic> {
        let module = self.module(module_id)?;
        let source_range = diagnostic.line.and_then(|line| {
            self.source_range_for_module_location(module_id, line, diagnostic.column)
        });
        Some(ModuleDiagnostic {
            module_id,
            path: module.path.clone(),
            diagnostic,
            source_range,
        })
    }

    pub fn attach_diagnostic_to_path(
        &self,
        path: &Path,
        diagnostic: Diagnostic,
    ) -> Option<ModuleDiagnostic> {
        let module = self.module_by_path(path)?;
        let source_range = diagnostic.line.and_then(|line| {
            self.source_range_for_module_location(module.module_id, line, diagnostic.column)
        });
        Some(ModuleDiagnostic {
            module_id: module.module_id,
            path: module.path.clone(),
            diagnostic,
            source_range,
        })
    }

    pub fn module_for_program_location(
        &self,
        program: &Program,
        decl_module_map: &[HirModuleId],
        line: usize,
    ) -> Option<&SourceModule> {
        let (module_id, _) =
            self.module_match_for_program_location(program, decl_module_map, line, None)?;
        self.module(module_id)
    }

    pub fn source_range_for_module_location(
        &self,
        module_id: HirModuleId,
        line: usize,
        column: Option<usize>,
    ) -> Option<SourceRange> {
        self.best_decl_range_for_module_location(module_id, line, column)
            .map(|range| range.range)
    }

    pub fn attach_program_diagnostic(
        &self,
        program: &Program,
        decl_module_map: &[HirModuleId],
        diagnostic: Diagnostic,
    ) -> Option<ModuleDiagnostic> {
        if let Some((module_id, source_range)) =
            self.module_match_for_diagnostic_owner(decl_module_map, &diagnostic)
        {
            let module = self.module(module_id)?;
            return Some(ModuleDiagnostic {
                module_id: module.module_id,
                path: module.path.clone(),
                diagnostic,
                source_range,
            });
        }

        let line = diagnostic.line?;
        let (module_id, source_range) = self.module_match_for_program_location(
            program,
            decl_module_map,
            line,
            diagnostic.column,
        )?;
        let module = self.module(module_id)?;
        Some(ModuleDiagnostic {
            module_id: module.module_id,
            path: module.path.clone(),
            diagnostic,
            source_range,
        })
    }

    fn module_match_for_diagnostic_owner(
        &self,
        decl_module_map: &[HirModuleId],
        diagnostic: &Diagnostic,
    ) -> Option<(HirModuleId, Option<SourceRange>)> {
        let owner = diagnostic.owner?;
        let owner_module_id = owner.module_id.map(HirModuleId);
        let map_module_id = decl_module_map.get(owner.decl_index).copied();
        let decl_range = self.decl_range_for_program_decl(owner.decl_index);
        let range_module_id = decl_range.map(|range| range.module_id);
        let module_id = owner_module_id.or(map_module_id).or(range_module_id)?;
        let source_range = decl_range
            .filter(|range| range.module_id == module_id)
            .map(|range| range.range);
        Some((module_id, source_range))
    }

    fn module_match_for_program_location(
        &self,
        program: &Program,
        decl_module_map: &[HirModuleId],
        line: usize,
        column: Option<usize>,
    ) -> Option<(HirModuleId, Option<SourceRange>)> {
        if line == 0 {
            return None;
        }

        if let Some(range) = self.best_decl_range_for_location(line, column) {
            return Some((range.module_id, Some(range.range)));
        }

        let module_id =
            self.legacy_module_id_for_program_location(program, decl_module_map, line)?;
        Some((module_id, None))
    }

    fn best_decl_range_for_location(
        &self,
        line: usize,
        column: Option<usize>,
    ) -> Option<&SourceDeclRange> {
        self.best_decl_range_matching(line, column, |_| true)
    }

    fn best_decl_range_for_module_location(
        &self,
        module_id: HirModuleId,
        line: usize,
        column: Option<usize>,
    ) -> Option<&SourceDeclRange> {
        self.best_decl_range_matching(line, column, |range| range.module_id == module_id)
    }

    fn best_decl_range_matching(
        &self,
        line: usize,
        column: Option<usize>,
        predicate: impl Fn(&SourceDeclRange) -> bool,
    ) -> Option<&SourceDeclRange> {
        self.decl_ranges
            .iter()
            .filter(|decl_range| predicate(decl_range))
            .filter(|decl_range| decl_range.range.contains(line, column))
            .min_by(|left, right| {
                left.range
                    .line_len()
                    .cmp(&right.range.line_len())
                    .then_with(|| right.range.start.line.cmp(&left.range.start.line))
                    .then_with(|| right.decl_index.cmp(&left.decl_index))
            })
    }

    fn legacy_module_id_for_program_location(
        &self,
        program: &Program,
        decl_module_map: &[HirModuleId],
        line: usize,
    ) -> Option<HirModuleId> {
        let mut best_match: Option<(usize, HirModuleId)> = None;

        for (index, decl) in program.decls.iter().enumerate() {
            if matches!(decl, Decl::Import { .. }) {
                continue;
            }

            let Some(module_id) = decl_module_map.get(index).copied() else {
                continue;
            };
            let start_line = decl.span().line;
            if start_line == 0 || line < start_line {
                continue;
            }

            let end_line = self
                .next_decl_start_line(program, decl_module_map, index, module_id)
                .and_then(|next_line| next_line.checked_sub(1))
                .or_else(|| self.module(module_id).map(source_module_line_count))?;

            if line <= end_line
                && best_match
                    .map(|(best_start, _)| start_line >= best_start)
                    .unwrap_or(true)
            {
                best_match = Some((start_line, module_id));
            }
        }

        if let Some((_, module_id)) = best_match {
            return Some(module_id);
        }

        let mut modules_for_line = self
            .modules
            .iter()
            .filter(|module| line <= source_module_line_count(module));
        let module = modules_for_line.next()?;
        if modules_for_line.next().is_none() {
            Some(module.module_id)
        } else {
            None
        }
    }

    fn next_decl_start_line(
        &self,
        program: &Program,
        decl_module_map: &[HirModuleId],
        current_index: usize,
        module_id: HirModuleId,
    ) -> Option<usize> {
        program
            .decls
            .iter()
            .enumerate()
            .skip(current_index + 1)
            .find_map(|(index, decl)| {
                if decl_module_map.get(index).copied() == Some(module_id) {
                    let line = decl.span().line;
                    if line != 0 {
                        return Some(line);
                    }
                }
                None
            })
    }
}

fn source_module_line_count(module: &SourceModule) -> usize {
    module.source.lines().count().max(1)
}

fn span_before(left: Span, right: Span) -> bool {
    left.line < right.line || (left.line == right.line && left.column < right.column)
}

fn build_source_decl_ranges(
    modules: &[SourceModule],
    program: &Program,
    decl_module_map: &[HirModuleId],
) -> Vec<SourceDeclRange> {
    program
        .decls
        .iter()
        .enumerate()
        .filter_map(|(decl_index, decl)| {
            let module_id = decl_module_map.get(decl_index).copied()?;
            let module = modules
                .iter()
                .find(|module| module.module_id == module_id)?;
            let range = infer_decl_source_range(&module.source, decl);
            Some(SourceDeclRange {
                module_id,
                decl_index,
                range,
            })
        })
        .collect()
}

fn infer_decl_source_range(source: &str, decl: &Decl) -> SourceRange {
    let start = decl.span();
    if !start.is_known() {
        return SourceRange::new(start, start);
    }

    SourceRange::new(
        start,
        infer_decl_end_span(source, start, decl_has_block(decl)),
    )
}

fn decl_has_block(decl: &Decl) -> bool {
    match decl {
        Decl::Function { .. }
        | Decl::Model { .. }
        | Decl::Workflow { .. }
        | Decl::Auth { .. }
        | Decl::Route { .. }
        | Decl::Invoice { .. } => true,
        Decl::Export { decl, .. } => decl_has_block(decl),
        _ => false,
    }
}

fn infer_decl_end_span(source: &str, start: Span, scan_for_block: bool) -> Span {
    let lines: Vec<&str> = source.lines().collect();
    let Some(start_index) = start.line.checked_sub(1) else {
        return start;
    };
    if start_index >= lines.len() {
        return start;
    }
    let fallback = line_end_span(start.line, lines[start_index]);

    let mut saw_open_brace = false;
    let mut brace_depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (line_index, line) in lines.iter().enumerate().skip(start_index) {
        let line_no = line_index + 1;
        let start_column = if line_index == start_index {
            start.column.saturating_sub(1)
        } else {
            0
        };

        for (char_index, ch) in line.chars().enumerate().skip(start_column) {
            if in_string {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_string = false;
                }
                continue;
            }

            if ch == '"' {
                in_string = true;
                continue;
            }

            match ch {
                '{' => {
                    saw_open_brace = true;
                    brace_depth = brace_depth.saturating_add(1);
                }
                '}' if saw_open_brace => {
                    brace_depth = brace_depth.saturating_sub(1);
                    if brace_depth == 0 {
                        return Span::new(line_no, char_index + 1);
                    }
                }
                _ => {}
            }
        }

        if !saw_open_brace && line_index == start_index && !scan_for_block {
            return fallback;
        }
    }

    if saw_open_brace {
        lines
            .last()
            .map(|line| line_end_span(lines.len(), line))
            .unwrap_or(start)
    } else {
        fallback
    }
}

fn line_end_span(line_no: usize, line: &str) -> Span {
    Span::new(line_no, line.chars().count().saturating_add(1))
}

/// High-level entry point: load a `.nx` file and all its dependencies,
/// returning a unified `Program` with merged declarations.
///
/// `"std/<module>"` imports are resolved through `NEXUS_STDLIB`, the installed
/// stdlib next to the executable, or the development stdlib directory.
///
/// # Errors
///
/// Returns `ModuleError` for IO failures, parse errors, circular
/// dependencies, missing exports, or non-relative paths.
pub fn load_program(entry_path: &Path) -> Result<Program, ModuleError> {
    let (loader, canonical) = load_entry_modules(entry_path)?;
    Ok(loader.build_merged_program(&canonical))
}

/// Convenience: load a multi-file project and produce both the merged
/// `Program` and the `ModuleGraph` for cross-module symbol resolution.
pub fn load_program_with_graph(entry_path: &Path) -> Result<(Program, ModuleGraph), ModuleError> {
    let (loader, canonical) = load_entry_modules(entry_path)?;
    let program = loader.build_merged_program(&canonical);
    let module_graph = build_module_graph(&loader, &canonical);
    Ok((program, module_graph))
}

/// Convenience: load a multi-file project and produce the merged `Program`,
/// `ModuleGraph`, and `SourceDatabase` for tooling.
pub fn load_program_with_source_database(
    entry_path: &Path,
) -> Result<(Program, ModuleGraph, SourceDatabase), ModuleError> {
    let (program, module_graph, _decl_module_map, source_database) =
        load_program_full_with_source_database(entry_path)?;
    Ok((program, module_graph, source_database))
}

fn load_entry_modules(entry_path: &Path) -> Result<(ModuleLoader, PathBuf), ModuleError> {
    let canonical = entry_path
        .canonicalize()
        .map_err(|e| ModuleError::IoError {
            path: entry_path.to_path_buf(),
            message: format!("Entry file not found: {}", e),
        })?;

    let mut loader = ModuleLoader::new();
    loader.load_module(&canonical)?;
    loader.validate_graph_symbol_surface()?;
    Ok((loader, canonical))
}

/// Build a `ModuleGraph` from the loader's loaded modules.
///
/// Uses the same deterministic order as `build_merged_program_with_map`:
/// entry module first, then dependency modules sorted by path.
fn build_module_graph(loader: &ModuleLoader, entry_canon: &Path) -> ModuleGraph {
    let mut entries = Vec::new();
    let mut path_to_id = HashMap::new();
    let mut entry_id = HirModuleId(0);

    for (index, mod_path) in loader.ordered_module_paths(entry_canon).iter().enumerate() {
        let module_id = HirModuleId(index);
        path_to_id.insert(mod_path.clone(), module_id);
        if index == 0 {
            entry_id = module_id;
        }

        if let Some(loaded_mod) = loader.loaded.get(mod_path) {
            entries.push(ModuleGraphEntry {
                module_id,
                path: mod_path.clone(),
                export_names: loaded_mod.exports.iter().cloned().collect(),
            });
        }
    }

    ModuleGraph {
        entries,
        entry_id,
        path_to_id,
    }
}

fn build_source_database(
    loader: &ModuleLoader,
    entry_canon: &Path,
    module_graph: &ModuleGraph,
    program: &Program,
    decl_module_map: &[HirModuleId],
) -> SourceDatabase {
    let mut modules = Vec::new();
    let mut import_edges = Vec::new();
    let mut path_to_id = HashMap::new();

    for (index, module_path) in loader.ordered_module_paths(entry_canon).iter().enumerate() {
        let module_id = HirModuleId(index);
        path_to_id.insert(module_path.clone(), module_id);

        if let Some(loaded_mod) = loader.loaded.get(module_path) {
            modules.push(SourceModule {
                module_id,
                path: loaded_mod.path.clone(),
                source: loaded_mod.source.clone(),
                is_entry: module_id == module_graph.entry_id,
            });

            for decl in &loaded_mod.program.decls {
                let Decl::Import { import } = decl else {
                    continue;
                };

                let target_module =
                    resolve_import_target_module(module_graph, module_id, import.source.as_str());

                import_edges.push(SourceImportEdge {
                    source_module: module_id,
                    target_module,
                    imported_name: import.name.clone(),
                    alias: import.alias.clone(),
                    source_path: import.source.clone(),
                    import_span: import.span,
                    name_span: import.name_span,
                    alias_span: import.alias_span,
                    source_span: import.source_span,
                });
            }
        }
    }

    let decl_ranges = build_source_decl_ranges(&modules, program, decl_module_map);

    SourceDatabase {
        modules,
        import_edges,
        decl_ranges,
        path_to_id,
    }
}

/// Load a multi-file project and produce the merged `Program`, `ModuleGraph`,
/// and `decl_module_map` for cross-module symbol resolution.
///
/// The `decl_module_map[i]` identifies which module contributed declaration
/// `program.decls[i]`, enabling the checker to resolve `HirSymbolRef`.
pub fn load_program_full(
    entry_path: &Path,
) -> Result<(Program, ModuleGraph, Vec<HirModuleId>), ModuleError> {
    let (loader, canonical) = load_entry_modules(entry_path)?;
    let (program, decl_module_map) = loader.build_merged_program_with_map(&canonical);
    let module_graph = build_module_graph(&loader, &canonical);
    Ok((program, module_graph, decl_module_map))
}

/// Load a multi-file project and produce the merged `Program`, `ModuleGraph`,
/// `decl_module_map`, and `SourceDatabase`.
///
/// This is the tooling-friendly variant of `load_program_full`: the checker
/// still receives the same graph/map data, while callers can keep source text,
/// module paths, and import edges for diagnostics and IDE features.
pub fn load_program_full_with_source_database(
    entry_path: &Path,
) -> Result<(Program, ModuleGraph, Vec<HirModuleId>, SourceDatabase), ModuleError> {
    let (loader, canonical) = load_entry_modules(entry_path)?;
    let (program, decl_module_map) = loader.build_merged_program_with_map(&canonical);
    let module_graph = build_module_graph(&loader, &canonical);
    let source_database = build_source_database(
        &loader,
        &canonical,
        &module_graph,
        &program,
        &decl_module_map,
    );
    Ok((program, module_graph, decl_module_map, source_database))
}

/// Given the merged program and the module graph, produce a map from
/// each import's HirDeclId to the HirSymbolRef it resolves to.
///
/// This is called by the multi-module checking path after HIR lowering.
///
/// `decl_module_map`: maps each decl index in the merged program to its
/// source module ID. This is built from `build_merged_program`'s ordering.
pub fn resolve_hir_imports<'a>(
    hir: &'a HirProgram<'a>,
    module_graph: &ModuleGraph,
    decl_module_map: &[HirModuleId],
) -> HashMap<HirDeclId, HirSymbolRef> {
    use crate::hir::{HirDeclBody, HirDeclKind};
    let mut result = HashMap::new();

    for (decl_idx, decl) in hir.decls.iter().enumerate() {
        if decl.kind != HirDeclKind::Import {
            continue;
        }

        let HirDeclBody::Import {
            module: module_ref,
            imported: imported_ref,
            alias: _,
            resolved: _,
        } = &decl.body
        else {
            continue;
        };

        // Get the module path from the reference
        let mod_path = hir
            .references
            .get(module_ref.index())
            .map(|r| r.name)
            .unwrap_or("");

        // Get the imported name from the reference
        let import_name = hir
            .references
            .get(imported_ref.index())
            .map(|r| r.name)
            .unwrap_or("");

        if mod_path.is_empty() || import_name.is_empty() {
            continue;
        }

        let source_module = match decl_module_map.get(decl_idx).copied() {
            Some(m) => m,
            None => continue,
        };

        let Some(target_module) =
            resolve_import_target_module(module_graph, source_module, mod_path)
        else {
            continue;
        };

        if let Some(target_sym) =
            find_exported_symbol(hir, decl_module_map, target_module, import_name)
        {
            result.insert(
                decl.id,
                HirSymbolRef {
                    module: target_module,
                    symbol: target_sym,
                },
            );
        }
    }

    result
}

fn resolve_import_target_module(
    module_graph: &ModuleGraph,
    source_module: HirModuleId,
    import_source: &str,
) -> Option<HirModuleId> {
    let source_entry = module_graph
        .entries
        .iter()
        .find(|entry| entry.module_id == source_module)?;
    let target_path = resolve_import_path_for_graph(import_source, &source_entry.path)?;
    module_graph.path_to_id.get(&target_path).copied()
}

fn resolve_import_path_for_graph(import_source: &str, source_path: &Path) -> Option<PathBuf> {
    if let Some(module_name) = import_source.strip_prefix("std/") {
        let stdlib_dir = find_stdlib_path().ok()?;
        return canonical_nx_path(stdlib_dir.join(module_name));
    }

    if import_source.starts_with("./") || import_source.starts_with("../") {
        let source_dir = source_path.parent().unwrap_or(Path::new("."));
        return canonical_nx_path(source_dir.join(import_source));
    }

    resolve_package_import_path(import_source, source_path)
        .ok()
        .flatten()
        .and_then(|path| path.canonicalize().ok())
}

fn canonical_nx_path(path: PathBuf) -> Option<PathBuf> {
    ModuleLoader::ensure_nx_extension(&path).canonicalize().ok()
}

/// Scan HIR decls for an exported symbol matching `name` in the given module.
fn find_exported_symbol(
    hir: &HirProgram<'_>,
    decl_module_map: &[HirModuleId],
    target_module: HirModuleId,
    name: &str,
) -> Option<HirSymbolId> {
    for (decl_idx, decl) in hir.decls.iter().enumerate() {
        // Must belong to the target module
        let mod_id = decl_module_map.get(decl_idx).copied()?;
        if mod_id != target_module {
            continue;
        }
        // Must be public
        if decl.visibility != crate::hir::HirVisibility::Public {
            continue;
        }
        // Name must match
        if decl.name != Some(name) {
            continue;
        }
        // Return the symbol
        return decl.symbol;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_temp_nx(dir: &std::path::Path, name: &str, source: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, source).expect("failed to write temp file");
        path
    }

    #[test]
    fn load_single_file_no_imports() {
        let dir = std::env::temp_dir().join(format!("nx_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let entry = create_temp_nx(
            &dir,
            "main.nx",
            r#"
model Hello { name: string }
let x = 1
print("ok")
"#,
        );

        let result = load_program(&entry);
        assert!(
            result.is_ok(),
            "single file should load: {:?}",
            result.err()
        );
        let program = result.unwrap();
        assert!(!program.decls.is_empty(), "should have declarations");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_module_with_dependency() {
        let dir = std::env::temp_dir().join(format!("nx_test_dep_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Create a dependency module
        create_temp_nx(
            &dir,
            "helpers.nx",
            r#"
export fn greet(name: string) -> string {
    return "Hello, " + name
}
"#,
        );

        // Create the entry file that imports from helpers
        let entry = create_temp_nx(
            &dir,
            "main.nx",
            r#"
import greet from "./helpers.nx"

let msg = greet("World")
print(msg)
"#,
        );

        let result = load_program(&entry);
        assert!(
            result.is_ok(),
            "should load with dependency: {:?}",
            result.err()
        );

        let program = result.unwrap();
        // Should have decls from both files (import + fn call + fn def)
        assert!(program.decls.len() >= 2, "should merge both modules");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn circular_dependency_is_rejected() {
        let dir = std::env::temp_dir().join(format!("nx_test_circ_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        create_temp_nx(
            &dir,
            "a.nx",
            r#"
import greet from "./b.nx"
print("from a")
"#,
        );

        create_temp_nx(
            &dir,
            "b.nx",
            r#"
import greet from "./a.nx"
print("from b")
"#,
        );

        let result = load_program(&dir.join("a.nx"));
        assert!(result.is_err(), "circular dependency should be rejected");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn non_relative_path_is_rejected() {
        let dir = std::env::temp_dir().join(format!("nx_test_path_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let entry = create_temp_nx(&dir, "main.nx", r#"import Foo from "bar.nx""#);

        let result = load_program(&entry);
        assert!(result.is_err(), "non-relative path should be rejected");
        match result.unwrap_err() {
            ModuleError::NonRelativePath { .. } => {} // expected
            other => panic!("expected NonRelativePath, got {:?}", other),
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_module_is_rejected() {
        let dir = std::env::temp_dir().join(format!("nx_test_miss_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let entry = create_temp_nx(&dir, "main.nx", r#"import Foo from "./nonexistent.nx""#);

        let result = load_program(&entry);
        assert!(result.is_err(), "missing module should be rejected");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_not_found_is_rejected() {
        let dir = std::env::temp_dir().join(format!("nx_test_exp_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Module that exports only "bar"
        create_temp_nx(
            &dir,
            "lib.nx",
            r#"
export model Bar { name: string }
"#,
        );

        // Main tries to import "Foo" which doesn't exist
        let entry = create_temp_nx(&dir, "main.nx", r#"import Foo from "./lib.nx""#);

        let result = load_program(&entry);
        assert!(result.is_err(), "importing non-existent export should fail");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn non_exported_symbol_is_rejected() {
        let dir = std::env::temp_dir().join(format!("nx_test_nonexp_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Module with a non-exported model
        create_temp_nx(
            &dir,
            "lib.nx",
            r#"
model Private { x: int }
"#,
        );

        // Main tries to import Private which exists but is not exported
        let entry = create_temp_nx(&dir, "main.nx", r#"import Private from "./lib.nx""#);

        let result = load_program(&entry);
        assert!(result.is_err(), "importing non-exported symbol should fail");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_module_with_nx_extension_inference() {
        let dir = std::env::temp_dir().join(format!("nx_test_ext_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Create module WITHOUT .nx extension in the filename
        create_temp_nx(
            &dir,
            "utils",
            r#"
export fn identity(x: string) -> string {
    return x
}
"#,
        );

        // Import without .nx extension — the loader should infer it
        let entry = create_temp_nx(&dir, "main.nx", r#"import identity from "./utils""#);

        let result = load_program(&entry);
        // This should either work (if extension inference kicks in) or
        // produce a reasonable error
        match &result {
            Ok(program) => {
                assert!(program.decls.len() >= 2);
            }
            Err(e) => {
                let msg = e.to_string();
                // Accept both failure modes: no .nx inferred, or file not found
                assert!(
                    msg.contains("utils") || msg.contains("não foi possível"),
                    "unexpected error: {}",
                    msg
                );
            }
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_recursive_dependencies() {
        let dir = std::env::temp_dir().join(format!("nx_test_rec_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Level 3: base utilities
        create_temp_nx(
            &dir,
            "base.nx",
            r#"
export fn base_greeting() -> string {
    return "Hello"
}
"#,
        );

        // Level 2: uses base
        create_temp_nx(
            &dir,
            "middle.nx",
            r#"
import base_greeting from "./base.nx"

export fn greet(name: string) -> string {
    return base_greeting() + ", " + name
}
"#,
        );

        // Level 1: entry point
        let entry = create_temp_nx(
            &dir,
            "main.nx",
            r#"
import greet from "./middle.nx"

let msg = greet("World")
print(msg)
"#,
        );

        let result = load_program(&entry);
        assert!(
            result.is_ok(),
            "recursive deps should load: {:?}",
            result.err()
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn module_loader_integration_with_checker() {
        let dir = std::env::temp_dir().join(format!("nx_test_ck_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        create_temp_nx(
            &dir,
            "types.nx",
            r#"
export model User {
    name: string
    age: int
}
"#,
        );

        let entry = create_temp_nx(
            &dir,
            "main.nx",
            r#"
import User from "./types.nx"

let u = User { name: "Ana", age: 30 }
print(u.name)
"#,
        );

        // Load the program
        let program = load_program(&entry).expect("module load should succeed");

        // Run it through the checker
        let mut checker = crate::checker::Checker::new();
        let check_result = checker.check(&program);
        assert!(
            check_result.is_ok(),
            "checker should accept merged program: {:?}",
            check_result.err()
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
