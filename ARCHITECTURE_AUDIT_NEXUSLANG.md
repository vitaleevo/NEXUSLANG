# NEXUSLANG Architectural Audit

Date: 2026-05-27

Scope: full repository audit of the Rust language core, CLI, semantic checker,
interpreter, HTTP runtime, storage/auth runtime, OpenAPI tooling, package
manager, playground/WASM bridge, tests, roadmap, and continuity memory.

Validation observed during this audit:

```text
cd /home/alexandre/Nesusang
NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh
PASS: fmt, check, clippy, cargo test, storage compatibility validation,
model operation contract docs validation, node --check, HTTP/auth/storage
smokes, and OpenAPI validation.
Cargo test coverage observed in the gate: 35 lib tests, 7 CLI
package-manager tests, and 156 core tests.
```

## Executive Verdict

NEXUSLANG is not yet a Rust-like production compiler. It is currently an
ERP-first interpreted language and application runtime implemented in Rust.
That is not a bad thing. The project has real vertical slices: lexer, parser,
AST, semantic checker, interpreter, CLI, formatter, linter, WASM playground,
HTTP route runtime, JSON/SQLite storage, OpenAPI generation, package-manager
MVP, native auth, release packaging, and a strong regression suite.

The hard truth: the system has grown by adding feature cases directly into
the AST/checker/router/OpenAPI/storage layers. The biggest architectural risk
is not missing syntax; it is missing compiler layers. A first general HIR and
module graph now exist, but there is still no full resolver-owned typed HIR, no
MIR, no persistent/incremental source database or LSP-grade module resolver, no standard
library boundary, no backend, and no production runtime contract. The code
works because the supported surface is narrow and heavily tested, not because
the architecture is ready to scale.

Current best classification:

- Language category: interpreted ERP DSL with general-purpose scripting pieces.
- Compiler maturity: front-end prototype with useful semantic checking.
- Runtime maturity: internal/dev runtime with serious ERP/API features, not a
  production server platform yet.
- Framework readiness: good for demos and controlled internal prototypes;
  not ready for enterprise frameworks, SaaS platforms, plugin systems, or
  dependency-injected modular applications without major compiler/runtime work.

## Scores

| Area | Score | Meaning |
| --- | ---: | --- |
| Architecture score | 98/100 | Good vertical slices, centralized route contracts, initial HIR, resolver integration, consumed checked HIR metadata, explicit lexical HIR scopes, scoped checker binding lookups, typed binding metadata, typed-HIR expression context, dedicated checker submodules for HIR expression checking, AST expression fallback inference, shared type/filter rules, base type-helper isolation, binding-resolution isolation, program-flow isolation, symbol-lookup isolation, HIR symbol indexing, local checker scope state, typed-HIR metadata/cache isolation, typed-HIR metadata owner/writer API, hardened typed-HIR metadata store/write boundary, explicit expression metadata production helpers, centralized symbol/binding metadata writers, HIR operation arguments, model operation validation, auth static operation validation, auth declaration validation, route static-call dispatch, route expression validation, route declaration validation, invoice declaration validation, model declaration validation, and workflow declaration validation, HIR-backed operation argument adapters, consolidated model/auth operation HIR argument context, typed-HIR argument API coverage for lookup/ordering/pagination/advanced/range/composite `ModelStaticOperation` arguments, shared source+HIR argument wrappers for model/auth operations, isolated source diagnostics inside typed argument wrappers, and HIR-consuming operation validators; checker/runtime layering is still weak. |
| Compiler maturity score | 76/100 | Lexer/parser/checker, initial HIR, lexical `HirScopeId` frames, scoped local binding lookup, minimum symbol resolver, TypeIds, expression and symbol metadata, typed binding metadata, typed-HIR expression-context consumption, a `HirExprId`/`HirExprKind` expression checker module, graph-aware modules/imports/exports, local path dependency imports, minimal `SourceDatabase` with declaration ranges, checker `DiagnosticOwner` metadata, dedicated `checker/expr.rs`, `checker/type_rules.rs`, `checker/type_core.rs`, `checker/binding_resolution.rs`, `checker/program_flow.rs`, `checker/symbol_lookup.rs`, `checker/symbols.rs`, `checker/scope.rs`, `checker/hir_metadata.rs`, `checker/typed_hir_pass.rs`, `checker/hir_args.rs`, `checker/auth_static_ops.rs`, `checker/auth_decl.rs`, `checker/function_decl.rs`, `checker/statement_decl.rs`, `checker/stmt.rs`, `checker/route_static_ops.rs`, `checker/route_expr.rs`, `checker/route_decl.rs`, `checker/invoice_decl.rs`, `checker/model_decl.rs`, and `checker/workflow_decl.rs` modules, HIR/source adapters for route static-call arguments, `HirOperationContext`, `CheckedHirOperationArg`, `CheckedHirModelOperationArgs`, `CheckedHirAuthOperationArgs`, and HIR-backed model/auth operation validation exist; no standalone typed HIR production pass, MIR, optimizer, backend, persistent/incremental source database, ownership type system, or advanced type system. |
| Runtime maturity score | 53/100 | HTTP/storage/auth/OpenAPI exist with stronger contracts; raw sequential HTTP, no async runtime, no scheduler, limited safety envelope. |
| Tooling maturity score | 46/100 | CLI/fmt/lint/playground/package MVP exist; no LSP, debugger, real package registry, or workspace tooling. |
| Framework readiness score | 36/100 | Route/model/auth primitives are promising and local modules/path packages now exist; still missing registry-backed packages, stdlib breadth, DI/plugin contracts, concurrency, migrations, and source-database tooling. |

Post-audit implementation note: phases 8.02a through 9.05 have now reduced
two major architecture risks: duplicated operation contracts across checker,
route HIR, runtime, storage, and OpenAPI; and the absence of any general
lowering layer. `ModelStaticOperation` and `AuthStaticOperation` are
centralized, normalized argument HIR exists for both families, and both have
contract matrices. `src/hir.rs` now lowers checked AST programs into a first
general HIR with stable declaration, symbol, expression, statement context, and lexical scope IDs, and
`checker/resolver.rs` now gives the checker a minimum HIR-backed symbol graph
for top-level declarations, lexical bindings, scope-contained symbols, parent
scope traversal, and visible binding queries. `HirCheckedMetadata` now
records expression `TypeId`s, symbol binding `TypeId`s, and definition/use
links to `HirSymbolId`s for the checked paths already resolved by the checker.
The checker no longer exposes a raw `RefCell<HirCheckedMetadata>`: the private
`TypedHirMetadataStore` lives in `checker/typed_hir_pass.rs`, exposes
read-only snapshot/query access to `checker/hir_metadata.rs`, and keeps
replace/write operations private to the owner path. This
moves the architecture score directionally from about `58/100` to about
`98/100`; checker passes now consume typed-HIR expression context and route
the main statement/route expression checks through
`checker/hir_expr.rs`, a dedicated HIR expression checker for identifiers,
field access, object literals, binary expressions, calls, static calls, and
route return expressions. Route static calls now use `HirOperationArgs` to
pair normalized source AST arguments with `HirExprId`s, so model/auth
operation paths can begin validating and ensuring arguments through HIR without
breaking the existing operation contracts. Model operation validator families
for lookup, pagination, advanced filters, and composite filters now infer
values through this HIR adapter when available and fall back to AST for legacy
callers. `ModelOperationContext` now carries `HirProgram`, `HirOperationArgs`,
and `Scope` as one internal model-op validation context, and the route
static-call path no longer reinfers every normalized model argument after
validator execution. `CheckedHirOperationArg` now provides a shared
source+optional-`HirExprId` wrapper for static operation arguments.
`CheckedHirModelOperationArgs` maps normalized lookup, ordering, pagination,
advanced, range, and composite AST arguments to that wrapper, while
`CheckedHirAuthOperationArgs` maps auth config arguments to the same contract.
Model-op validators now consume those wrappers as the primary argument contract
and auth route static calls use them to preserve source diagnostics and attach
auth symbols by `HirExprId`, preserving public AST contracts and diagnostics.
Those operation-argument contracts now live in `checker/hir_args.rs`, reducing
the responsibility of `checker/hir_expr.rs` to expression validation and
route-expression orchestration. Auth static operation validation now lives in
`checker/auth_static_ops.rs`, which owns AST return inference, auth config
lookup, and HIR auth static-call validation while preserving
`CheckedAuthOperationArgs` and public diagnostics. Auth declaration validation
now lives in `checker/auth_decl.rs`, which owns auth collection, duplicate
config checks, `AuthConfig` storage, HIR auth symbol registration, auth config
validation against model fields, password minimum, and TTL rules while
preserving public diagnostics and leaving `Auth::...` operation checks in
`auth_static_ops.rs`.
AST/HIR route static-call dispatch now lives in `checker/route_static_ops.rs`,
so `checker/mod.rs` and `checker/hir_expr.rs` delegate Auth/Model static-call
decisions through one shared checker layer. Route expression validation now lives in
`checker/route_expr.rs`, which owns AST/HIR route-return inference, expression
ensuring, return-type validation, and the HIR route-expression wrapper path.
Route declaration validation now lives in `checker/route_decl.rs`, which owns
route collection, method/path uniqueness, duplicate path/query parameter
checks, route guards, direct-return shape checks, route/query parameter scope
setup, query parameter type/default validation, and HIR route symbol
registration.
Function declaration validation now lives in `checker/function_decl.rs`, which
owns function signature collection, parameter and return type validation, HIR
parameter symbol setup, body checking, and required-return validation for
non-void functions.
Top-level statement declaration checking now delegates through
`checker/statement_decl.rs`, keeping the program-level statement path separate
from the shared statement checker implementation.
Shared statement and binding checking now lives in `checker/stmt.rs`, which
owns `check_stmts`, `check_stmt`, binding annotations, assignments, returns,
control-flow statement checks, HIR scope switching, and checked binding
metadata.
General AST expression inference and compatibility fallback now lives in
`checker/expr.rs`, which owns `infer_expr`, object literal field validation,
field access, unary/binary fallback, function-call fallback, simple
`Model::all()` static-call fallback, and the optional/comparable/numeric
helpers shared with `checker/hir_expr.rs`.
Shared type/operator/filter rules now live in `checker/type_rules.rs`, which
owns HIR-to-AST binary operator compatibility, ordering support, comparison
operator support, comparison field-type checks, and text filter support used
by `checker/model_ops.rs` and `checker/hir_expr.rs`.
HIR symbol indexing now lives in `checker/symbols.rs`, which owns
`CheckerSymbols`, top-level symbol maps, expression IDs, statement scopes,
statement binding scopes, and model-field symbol indexes consumed by checker
metadata and typed-HIR paths.
Local checker scope state now lives in `checker/scope.rs`, which owns `Scope`,
local vars, const tracking, HIR symbols, current `HirScopeId`, assignment
validation, and local name resolution consumed by statement, route, model,
function, workflow, and typed-HIR argument paths.
Typed-HIR metadata/cache helpers now live in `checker/hir_metadata.rs`, which
queries checked metadata snapshots, expression context lookup, typed
expression/symbol lookup, cache-hit counters, and checked symbol binding types
consumed by AST fallback, typed-HIR expression checking, route expressions,
route static operations, and statement/function/route binding setup.
The typed-HIR metadata owner/writer entrypoint now lives in
`checker/typed_hir_pass.rs`, which owns the private `TypedHirMetadataStore`,
metadata initialization from `HirProgram` counts, typed-HIR test counter resets
before declaration checking starts, expression type/symbol writes, symbol type
writes, and explicit expression metadata production/ensure helpers used by AST
inference, typed-HIR inference, route expression checking, and route
static-call validation. Auth config symbol links and binding/parameter symbol
type metadata also go through the same owner/writer module.
Base type helpers now live in `checker/type_core.rs`, which owns
`ensure_assignable` and `type_name` while preserving assignment compatibility
for `Unknown`, `Optional`, `Array`, `Float <- Int`, and the existing diagnostic
wording consumed across checker submodules.
Binding resolution helpers now live in `checker/binding_resolution.rs`, which
owns scoped binding lookup, assignment through checked symbol metadata,
`HirScopeId`-first symbol resolution with decl/span fallback, visible-binding
fallback, and scoped binding cache-hit accounting.
Program-flow orchestration now lives in `checker/program_flow.rs`, which owns
declaration collection, declaration checking, known-type validation, and
static-default validation while preserving the central `check_diagnostic`
entrypoint in `checker/mod.rs`.
Symbol lookup helpers now live in `checker/symbol_lookup.rs`, and the
typed-HIR/AST expression fallback bridge now lives in `checker/expr.rs`, so
`checker/mod.rs` is reduced to state, construction, the public check entrypoint,
HIR/resolver setup, typed-HIR pass startup, and central diagnostic
construction.
Invoice declaration validation now lives in `checker/invoice_decl.rs`, which
owns required invoice contract checks, duplicate field detection, invoice
field type validation, and invoice item validation while preserving source
spans and public diagnostics.
Model declaration validation now lives in `checker/model_decl.rs`, which owns
model collection, duplicate/reserved-name checks, field type validation,
model field defaults, and `unique`/`index`/`min`/`max` constraints while
preserving `checker/model_ops.rs` for `Model::...` operations.
Workflow declaration validation now lives in `checker/workflow_decl.rs`, which
owns workflow collection, duplicate-name checks, workflow step validation, and
shared `run_workflow` arity/type/existence checks for AST and HIR call paths.
Local bindings for functions, routes, blocks, and loops are resolved through
`HirScopeId` before falling back to legacy decl/span lookup. Identifier and
assignment checks now consume typed binding metadata when a `HirSymbolId` is
available. It still does not remove the larger missing systems: a complete
typed-HIR production pass, full definition/use links for every expression,
modules, stdlib, production runtime, parser recovery, LSP foundations, and
backend strategy.

## Repository Evidence Snapshot

Important implementation hotspots:

- `nexuslang-src/src/lexer/mod.rs`: tokenization, diagnostics, line/column.
- `nexuslang-src/src/parser/mod.rs`: recursive descent parser and precedence chain.
- `nexuslang-src/src/ast/mod.rs`: public AST, types, spans.
- `nexuslang-src/src/checker/mod.rs`: semantic checker orchestration, around
  120 lines.
- `nexuslang-src/src/checker/expr.rs`: general AST expression inference and
  compatibility fallback for non-HIR or legacy expression paths.
- `nexuslang-src/src/checker/type_core.rs`: base assignability and type-name
  helpers shared by checker submodules.
- `nexuslang-src/src/checker/binding_resolution.rs`: scoped binding lookup,
  typed-HIR assignment lookup, HIR symbol binding resolution, and scoped
  binding cache-hit accounting.
- `nexuslang-src/src/checker/program_flow.rs`: checker program-flow
  orchestration for declaration collection/checking, known types, and static
  defaults.
- `nexuslang-src/src/checker/symbol_lookup.rs`: small HIR symbol lookup
  helpers over indexed top-level and model-field symbols.
- `nexuslang-src/src/checker/type_rules.rs`: shared type/operator/filter
  rules for HIR binop compatibility and model operation validation.
- `nexuslang-src/src/checker/symbols.rs`: HIR symbol and source-pointer index
  cache for checker metadata, scoped bindings, and typed-HIR paths.
- `nexuslang-src/src/checker/scope.rs`: local checker scope state for vars,
  const tracking, HIR binding symbols, and current `HirScopeId`.
- `nexuslang-src/src/checker/hir_metadata.rs`: typed-HIR metadata/cache
  helpers for checked metadata snapshots, expression context lookup, cache
  counters, typed expression/symbol lookup, and checked binding types through
  read-only owner-store access.
- `nexuslang-src/src/checker/typed_hir_pass.rs`: typed-HIR metadata
  owner/writer entrypoint and private metadata store for initializing checked
  metadata, resetting typed-HIR test counters, recording expression/symbol
  metadata writes, and producing expression metadata results for AST/HIR
  inference and route paths, plus isolated symbol links and symbol type
  metadata for bindings/parameters.
- `nexuslang-src/src/checker/hir_expr.rs`: dedicated typed-HIR expression
  checker for `HirExprId`/`HirExprKind` inference and route return validation.
- `nexuslang-src/src/checker/hir_args.rs`: shared typed-HIR argument contract
  for model/auth static operations, including source spans and optional
  `HirExprId`s.
- `nexuslang-src/src/checker/auth_static_ops.rs`: dedicated auth static
  operation checker for AST return inference, auth config lookup, and HIR
  static-call validation.
- `nexuslang-src/src/checker/auth_decl.rs`: auth declaration checker for
  collection, duplicate config checks, config storage, HIR symbol registration,
  auth config model lookup, identity/role field validation, password minimum,
  and TTL rules.
- `nexuslang-src/src/checker/function_decl.rs`: function declaration checker
  for signature collection, parameter scope/HIR setup, body checking, and
  required-return validation.
- `nexuslang-src/src/checker/statement_decl.rs`: top-level statement
  declaration checker wrapper.
- `nexuslang-src/src/checker/stmt.rs`: shared statement and binding checker
  for `check_stmts`, `check_stmt`, local binding validation, assignment,
  return, print, expression statement, if/while/for handling, and HIR scope
  switching.
- `nexuslang-src/src/checker/route_static_ops.rs`: shared AST/HIR dispatcher
  for route static calls, delegating model/auth validation to dedicated
  checker submodules.
- `nexuslang-src/src/checker/route_expr.rs`: shared AST/HIR route expression
  validator for route-return inference, expression ensuring, and return-type
  checks.
- `nexuslang-src/src/checker/route_decl.rs`: route declaration checker for
  route collection, method/path uniqueness, duplicate path/query parameter
  checks, auth guards, route body shape, route/query parameter scope setup,
  query parameter type/default validation, and route HIR symbol registration.
- `nexuslang-src/src/checker/invoice_decl.rs`: invoice declaration checker for
  required contract fields, duplicate fields, field typing, and item typing.
- `nexuslang-src/src/checker/model_decl.rs`: model declaration checker for
  collection, duplicate/reserved-name checks, field typing, defaults, and
  `unique`/`index`/`min`/`max` constraints.
- `nexuslang-src/src/checker/workflow_decl.rs`: workflow declaration checker
  for workflow collection, step validation, and shared `run_workflow` checks.
- `nexuslang-src/src/checker/resolver.rs`: minimum HIR-backed resolver for
  top-level declarations, scoped lexical bindings, scope parent links, and
  visible binding queries.
- `nexuslang-src/src/model_ops.rs`: first central contract for static model operations.
- `nexuslang-src/src/hir.rs`: initial general HIR/lowering layer with stable
  declaration, symbol, statement/expression context, lexical scope IDs, and
  checked metadata for expression and binding types.
- `nexuslang-src/src/interpreter/mod.rs`: tree-walking interpreter.
- `nexuslang-src/src/server/router.rs`: HTTP dispatch and route expression evaluation.
- `nexuslang-src/src/server/openapi.rs`: OpenAPI generation and inferred route schemas.
- `nexuslang-src/src/server/storage.rs`: JSON value model, filters, route matching, JSON parser.
- `nexuslang-src/src/server/json.rs`: JSON file storage backend.
- `nexuslang-src/src/server/sqlite.rs`: SQLite backend storing model rows as JSON text.
- `nexuslang-src/src/server/auth.rs`: native auth, Argon2id, sessions, bearer tokens, CSRF, rate limiting.
- `nexuslang-src/src/package_manager.rs`: local package manager MVP.
- `nexuslang-src/src/playground/mod.rs` and `nexuslang-src/src/wasm.rs`: Rust core exposed to the browser.
- `nexuslang-playground.js`: playground UI, examples, WASM loading, diagnostics display.

Size concentration matters:

- `checker/mod.rs`: about 120 lines.
- `checker/expr.rs`: about 421 lines.
- `checker/type_core.rs`: about 39 lines.
- `checker/binding_resolution.rs`: about 76 lines.
- `checker/program_flow.rs`: about 201 lines.
- `checker/symbol_lookup.rs`: about 25 lines.
- `checker/symbols.rs`: about 151 lines.
- `checker/type_rules.rs`: about 90 lines.
- `checker/scope.rs`: about 56 lines.
- `checker/hir_metadata.rs`: about 130 lines.
- `checker/typed_hir_pass.rs`: about 167 lines.
- `checker/model_decl.rs`: about 561 lines.
- `checker/workflow_decl.rs`: about 103 lines.
- `checker/hir_expr.rs`: about 718 lines.
- `checker/auth_decl.rs`: about 114 lines.
- `checker/function_decl.rs`: about 98 lines.
- `checker/statement_decl.rs`: about 18 lines.
- `checker/stmt.rs`: about 210 lines.
- `checker/route_decl.rs`: about 232 lines.
- `checker/invoice_decl.rs`: about 124 lines.
- `checker/route_expr.rs`: about 291 lines.
- `checker/route_static_ops.rs`: about 242 lines.
- `checker/auth_static_ops.rs`: about 138 lines.
- `server/storage.rs`: about 1687 lines.
- `server/openapi.rs`: about 1581 lines.
- `server/router.rs`: about 1559 lines.
- `parser/mod.rs`: about 1127 lines.

This is the clearest architectural smell: feature logic is concentrated in a
few files and duplicated across compiler and runtime layers.

## 1. Lexer / Tokenizer

Already implemented:

- Concrete `Token` enum for literals, identifiers, keywords, built-in types,
  HTTP verbs, operators, delimiters, EOF, and an unused newline variant.
- Spanned tokenization returning `(Token, line, column)`.
- Structured lexer diagnostics for invalid characters, single `&`/`|`, and
  unterminated strings.
- Keyword recognition for language keywords, booleans, nil, primitive types,
  and HTTP methods.
- Basic string escapes and money literals such as `100 kz`.

Missing:

- End spans, byte offsets, source ranges, file IDs, and token IDs.
- Dedicated token stream abstraction with lookahead utilities, checkpoints,
  trivia/comments, or source mapping.
- Proper Unicode policy. Source is stored as `Vec<char>` and identifiers use
  `is_alphanumeric()` after an ASCII-only start check. This is neither fully
  Unicode-compliant nor explicitly ASCII-only.
- Grapheme-aware columns. Current columns count Rust `char`s, not display
  columns or byte offsets.
- Block comments, raw strings, escaped Unicode sequences, numeric separators,
  hex/binary literals, and robust currency/token suffix handling.

Poor design:

- The lexer copies the whole source into `Vec<char>`, increasing memory use
  and losing byte-offset locality.
- Currency suffix matching allocates `Vec<char>` for each currency candidate
  during number lexing.
- `Token::Newline` exists but the lexer does not emit it. That is a contract
  smell.
- `tokenize_spanned()` hides diagnostics by returning only EOF on error,
  preserving compatibility but making the non-diagnostic API unsafe for new
  code.

Risks:

- Unicode identifiers and diagnostics will behave inconsistently for real
  international code.
- Future tooling such as LSP, formatter diffs, editor highlights, and source
  maps will need byte ranges. Retrofitting them later will be painful.
- Performance is acceptable for small scripts, but the design is allocation
  heavy for large codebases.

Production improvements:

- Replace line/column-only spans with `Span { file_id, start_byte, end_byte,
  start_line, start_col, end_line, end_col }`.
- Keep a compact token type plus interned identifier/literal storage.
- Define an explicit identifier policy: ASCII-only for 1.0, or Unicode XID
  with `unicode-ident`.
- Keep trivia/comment channels if the formatter/LSP must preserve comments.
- Remove or deprecate non-diagnostic tokenization APIs from compiler flows.

## 2. Parser

Already implemented:

- Hand-written recursive descent parser.
- Precedence climbing by explicit functions: `or`, `and`, equality,
  comparison, additive, multiplicative, unary, postfix, primary.
- Parses functions, models, workflows, auth declarations, routes, query params,
  invoices, let/const/assign/return/print/if/while/for, arrays, object
  literals, calls, static calls, field access, and optional types.
- Enforces comma-separated lists for key syntax forms.
- Emits structured parser diagnostics with line/column.

Missing:

- Syntax error recovery. The parser stops at the first error.
- Parser synchronization points for declarations, statements, blocks, and list
  delimiters.
- A grammar document that maps directly to parser functions.
- Parser-level AST validation separation. Some syntax decisions are mixed with
  semantic assumptions, especially route paths and auth fields.
- Incremental parsing support for LSP/editor usage.

Poor design:

- Route path parsing is a special-case token loop that allows identifiers,
  `/`, `:`, `-`, `in`, and `auth` in specific contexts. This is fragile.
- `expect()` compares discriminants only, which is convenient but too coarse
  for tokens carrying values.
- Parser and AST are tightly coupled; there is no concrete syntax tree or
  lossless representation for formatting.
- Error messages include debug token formatting, which is useful internally but
  not stable UX.

Risks:

- Adding new syntax will keep increasing parser-specific hacks.
- LSP and editor experience will be poor without recovery and partial ASTs.
- Formatter evolution will be limited because comments/trivia are discarded.

Production improvements:

- Introduce a parser recovery strategy: recover to `}`, declaration starters,
  statement starters, `,`, and `)`.
- Add parser tests per grammar category instead of only integration tests.
- Create a `syntax.md` grammar matching parser functions.
- Consider Pratt parsing for expressions if operators grow, but the current
  precedence-chain approach is fine for the present operator set.
- Split parser modules by declaration, statement, expression, type, and route
  syntax.

## 3. Abstract Syntax Tree

Already implemented:

- Concrete Rust enums and structs for declarations, statements, expressions,
  types, model fields, auth configs, route guards, query params, invoice items,
  and workflows.
- Spans are attached to many nodes, including literals, statements,
  declarations, fields, calls, static calls, and field access.
- AST is simple, cloneable, serializable through custom playground rendering,
  and easy to pattern match.
- Initial general HIR exists in `nexuslang-src/src/hir.rs`. It lowers a checked
  AST into `HirProgram` with stable `HirDeclId`, `HirSymbolId`, and
  `HirExprId`, indexes top-level declarations, local bindings, route/query
  parameters, model fields, workflow steps, invoice fields, and checked
  expressions.

Missing:

- End spans and file identity.
- Node IDs or stable handles.
- Visitor/fold/traversal traits.
- Resolver-owned typed HIR with definition/use links.
- Symbol references from AST nodes to resolved definitions.
- Memory-efficient representation for larger programs.

Poor design:

- All AST fields are public and freely mutable from any module.
- Large values such as names, fields, bodies, and args are cloned repeatedly
  across checker, interpreter, playground, storage, and server.
- Type and syntax are mixed in places. For example, `Type::Model(String)` is
  both a syntactic type name and a semantically resolved model type.
- The AST is forced to serve parser output, semantic analysis, interpreter,
  HTTP runtime, OpenAPI, and playground JSON.

Risks:

- Every feature changes many downstream consumers.
- Refactors will stay expensive until the new HIR is connected to resolver,
  type checker, runtime contracts, and later typed IR stages.
- Advanced features such as modules, generics, traits, methods, closures, and
  ownership will make the current AST too overloaded.

Production improvements:

- Keep AST immutable enough for parser output and migrate semantic phases onto
  the new HIR instead of raw AST.
- Store identifiers as interned symbols, not raw `String`s everywhere.
- Add `NodeId` and `SourceMap`.
- Add visitor/fold traits for formatter, linter, checker, and docs.
- Make AST construction private enough that invariants can be enforced.

## 4. Semantic Analysis

Already implemented:

- Declaration collection for functions, models, workflows, and auth configs.
- Minimum HIR-backed resolver integration. The checker now lowers a program to
  HIR, builds a `ResolvedProgram`, stores `HirSymbolId`s for top-level
  declarations, and attaches symbol IDs to function params, route/query params,
  `let`, `const`, and `for` bindings inside checker scopes.
- Initial checked HIR metadata. `HirCheckedMetadata` stores interned
  expression type metadata through `HirTypeId` and definition/use links from
  checked expressions to `HirSymbolId` where the current checker already
  resolves the symbol.
- Incremental HIR metadata consumption. The checker now reads cached
  `HirCheckedMetadata` for resolved identifiers, field access, calls, static
  calls, and route return expressions before falling back to the legacy AST
  paths.
- Duplicate checks for models, fields, functions, workflows, routes, auth, and
  route/query params.
- Scope checking for variables and constants.
- Function parameter/return checking.
- Model instance validation: unknown fields, missing fields, defaults,
  optional fields, field access.
- Route contract validation: single direct return, supported HTTP subset,
  method restrictions for model operations.
- Invoice contract validation.
- Auth declaration and route guard validation.
- Static model operation validation for many route operations.
- Structured checker diagnostics with spans.

Missing:

- Full name-resolution phase. The checker now has a minimum symbol graph,
  initial use links, and metadata cache consumption, but type checking still
  primarily operates over AST and legacy maps.
- Symbol table tree with nested lexical scopes and complete definition/use
  links.
- Full typed HIR consumed by checker/runtime contracts.
- Module/import/export resolution.
- Control-flow graph.
- Definite assignment analysis.
- Lifetime analysis.
- Borrow/ownership analysis.
- Exhaustiveness analysis.
- Effect analysis for route/storage/auth calls.

Poor design:

- `checker/mod.rs` is now a small checker shell and the typed-HIR metadata
  write boundary is hardened behind a private owner-store, but production is
  still triggered by checker validation paths instead of by one standalone
  HIR-walking production pass.
- Scopes are cloned and passed around as simple maps. Block scoping is weak:
  `if`, `while`, and `for` bodies use the same mutable scope, so bindings can
  leak in ways a production language should define explicitly.
- The checker has long duplicate chains for static model methods. The new
  `model_ops.rs` is the right direction but is not yet used everywhere.
- Route semantics are checked in the checker, executed in router/storage, and
  described again in OpenAPI.

Risks:

- Semantic drift between checker, router, OpenAPI, storage, and docs.
- Hard-to-fix bugs when adding new model operations.
- Advanced language features will cause exponential checker growth.

Production improvements:

- Split semantic analysis into: declaration collection, name resolution,
  type inference/checking, control-flow validation, route/model contract
  validation, and lint/effect checks.
- Extend the minimum resolver with lexical scope frames and complete resolved
  identifier uses, then make more checker passes consume typed expression
  metadata as their primary path instead of fallback/cache.
- Replace string-method dispatch with typed operation descriptors.
- Centralize route/model operation signatures once and generate checker,
  router, and OpenAPI behavior from that contract.
- Add a real lexical scope stack and decide whether block bindings leak.

## 5. Type System

Already implemented:

- Static checking for primitives: string, int, float, bool, money, date.
- Arrays, optional types, model types, nil, void, and unknown.
- Local inference from initializer expressions.
- Function signatures with explicit parameter and return types.
- Numeric promotion from int to float in selected contexts.
- Optional handling where `nil` is assignable only to optional types.
- Model-field defaults and min/max/unique/index constraint typing.

Missing:

- Generics.
- Traits/interfaces.
- Enums/sum types.
- Type aliases.
- Structs independent of ERP models.
- First-class function types.
- Closures/lambdas.
- Methods and associated functions beyond static model/auth calls.
- Nullable ergonomics such as unwrap, match, map, or `?` operator.
- Ownership, borrowing, references, move/copy rules, lifetimes.
- Currency-aware `money(AOA)` style type parameters.

Poor design:

- `Type::Unknown` can make checks permissive if it escapes into later phases.
- Model names are raw strings, not resolved type IDs.
- Optional assignability is lenient: `Optional<T>` accepts `T`, which is fine,
  but the language has no explicit operators for safely using optional values.
- `date` is mostly a string-like contract, not a real date type.

Risks:

- The type system is enough for current ERP demos, but not enough for large
  libraries.
- Without generics and traits, collections and standard-library APIs will be
  duplicated or dynamically typed.
- Without ownership or references, "Rust-inspired internally" remains an
  implementation fact, not a language property.

Production improvements:

- Build a type context with `TypeId`, interned symbols, and resolved model IDs.
- Add type aliases and enums before traits/generics.
- Add optional-safe operations before more nullable APIs are exposed.
- Decide whether ownership is a language feature. If yes, design it before
  references, closures, async, and iterators.
- Add a formal type-system spec and negative test matrix.

## 6. Control Flow

Already implemented:

- `if/else`, `while`, and `for item in array`.
- Basic function return checking for non-void functions.
- Runtime execution for loops and conditionals.

Missing:

- `break`, `continue`.
- `match` and pattern matching.
- `loop`.
- Iterators and iterator protocols.
- Async control flow.
- Coroutines/futures.
- Defer/finally/drop semantics.
- CFG-based reachability and definite return analysis.

Poor design:

- Return analysis is syntactic and shallow. It checks whether any statement in
  a block guarantees return, and whether both branches of an `if` return. There
  is no CFG.
- `for` accepts arrays statically, but the interpreter falls back to iterating
  a non-array as a single-item vector in runtime paths. The checker usually
  prevents this, but the runtime behavior is semantically loose.

Risks:

- More control-flow features will be hard to validate without CFG.
- Async, iterators, and pattern matching will need a new lowering layer.

Production improvements:

- Introduce CFG construction for functions and route bodies.
- Add `break`/`continue` before `match`.
- Add pattern matching only after algebraic data types exist.
- Keep route bodies intentionally restricted, but represent that restriction
  in HIR/effects instead of ad hoc AST checks.

## 7. Functions

Already implemented:

- Function declarations with typed parameters and optional return type.
- User-defined functions in checker and interpreter.
- Recursion should be possible because signatures are collected before bodies.
- Built-ins: `print`, `len`, `str`, `run_workflow`.

Missing:

- Closures and lambdas.
- Function values.
- Overloading.
- Methods.
- Async functions.
- Default arguments.
- Named arguments.
- Variadics.
- Generic functions.
- Visibility/export rules.

Poor design:

- Built-ins are hard-coded in checker and interpreter.
- Function bodies are cloned into runtime function records.
- No call graph, recursion limits, stack control, tail-call policy, or effect
  system.

Risks:

- Standard library growth will become a pile of special cases.
- Framework abstractions need function values, closures, modules, and generic
  APIs. They are absent.

Production improvements:

- Add a built-in registry shared by checker and interpreter.
- Add function symbols and call graph metadata in HIR.
- Implement closures only after lexical scope capture rules are formalized.
- Avoid overloading until generics/traits or a clear dispatch model exists.

## 8. Memory Management

Already implemented:

- Host implementation benefits from Rust memory safety.
- NEXUSLANG runtime values are owned Rust enum values and cloned when needed.
- No language-level pointers or references exist, which avoids many memory
  safety problems.
- WASM bridge exposes explicit allocation/deallocation functions for browser
  interop.

Missing:

- Defined stack/heap model for the language.
- Garbage collection or reference counting strategy.
- Ownership/move semantics.
- Borrow checking.
- Lifetime model.
- Destructors/drop/finalizers.
- Memory leak prevention model for future heap values.

Poor design:

- The interpreter is clone-heavy. Objects, arrays, fields, scopes, and globals
  are copied often.
- WASM exports use unsafe raw pointers. This is expected for ABI boundaries,
  but the contract relies on JavaScript freeing returned buffers correctly.
- There is no explicit runtime memory budget, recursion limit, or allocation
  accounting.

Risks:

- Current memory design is fine for scripts, but not for long-running SaaS or
  ERP services.
- Adding references later without an ownership/GC decision will create a large
  redesign.

Production improvements:

- Decide: value language with GC, ownership language, or hybrid.
- If staying interpreted, consider `Rc`/arena-backed values and copy-on-write
  structures.
- Add recursion and allocation limits for server execution.
- Add a memory model section to the language spec before introducing closures,
  references, async, or iterators.

## 9. Error Handling

Already implemented:

- Structured diagnostics for lexer, parser, and checker.
- Stable v1 diagnostic codes by lexer, parser, module-loader, checker, and
  runtime error family, plus optional severity metadata in structured payloads.
- Structured diagnostics and JSON v1 can carry optional labels, notes, and
  suggestions without changing human text rendering.
- Parser import/export; checker type, symbol, argument, model, route, auth,
  workflow, and invoice; module-loader missing exports, duplicate names/aliases,
  alias collisions, path/package/stdlib resolution; and runtime division,
  undefined-variable/function, model, and workflow diagnostics already populate
  those richer metadata fields.
- Minimal multi-module diagnostic reports can group diagnostics by path/module
  for tooling and are exposed through opt-in `nexus check --json-report` and
  `nexus run --json-report`. These report modes can collect checker
  diagnostics from independent function, route, workflow, and invoice
  declaration bodies while preserving the first-error `check --json` and
  `run --json` shapes. The report API also exposes in-memory helpers for
  querying by path/module, stage, severity, and group, plus a summary with
  counts by stage/severity, affected paths/modules, and flags such as
  `has_errors`. A flattened in-memory tooling view exposes diagnostic/group
  indexes, path/module, stage, severity, code, message, line/column, and source
  range when available. An opt-in source-context view can use `SourceDatabase`
  to attach source-line snippets and highlight columns for diagnostics with
  source owners. A documented pre-LSP contract matrix now locks the public
  report, filter/group, summary, flattened view, source-context, ordering, and
  JSON-boundary expectations. Fixture-backed Rust examples cover checker,
  module-loader, and runtime consumption of filters, groups, summary,
  flattened items, source snippets, captured output, and JSON v1 stability.
- CLI and playground show line/column where available.
- Runtime errors are surfaced in the playground as runtime-stage diagnostics.
- HTTP errors map selected message prefixes to HTTP statuses.
- Auth/runtime return clear JSON error objects.

Missing:

- Runtime diagnostics with source spans.
- Richer typed runtime errors.
- Full multi-error collection and parser recovery.
- Language-level `Result`/`Error` type.
- Exceptions or panic model.
- Stack traces.
- Standardized error contract for libraries.

Poor design:

- Runtime errors are still mostly `String`.
- HTTP status mapping depends on string prefixes such as `Nao autorizado`,
  `Conflito`, and `Muitas requisicoes`.
- Parser and checker stop at first error.

Risks:

- Developer experience will plateau quickly.
- Localization and stable tooling still need typed runtime errors and broader
  producer coverage for richer metadata.
- HTTP behavior can change accidentally if error text changes.

Production improvements:

- Continue expanding labels, notes, and suggestions across parser/checker/runtime
  producers, and add typed runtime error kinds.
- Add `RuntimeError` with optional span and structured kind.
- Replace string-prefix HTTP status mapping with typed runtime errors.
- Add parser/checker recovery for multi-error reporting.
- Define language-level `Result<T, E>` before adding exceptions or panic.

## 10. Runtime Architecture

Already implemented:

- Tree-walking interpreter for scripts, functions, workflows, invoices, and
  static model call summaries.
- HTTP route runtime via `nexus serve`.
- Route matching, path/query param binding, OpenAPI endpoint, health endpoint.
- JSON and SQLite storage backends.
- Native auth with Argon2id, opaque sessions, bearer tokens, CSRF, rate limits.
- WASM playground runtime path using the same Rust core.

Missing:

- Async runtime.
- Scheduler.
- Threading model.
- Module loader.
- Plugin loader.
- Runtime configuration system.
- Middleware pipeline.
- Request body streaming and robust HTTP parsing.
- TLS termination.
- Observability: logs, metrics, traces.

Poor design:

- HTTP server is raw `TcpListener`, sequential, and reads one fixed 8192-byte
  buffer.
- Request parsing is manual and incomplete.
- `nexus serve` exposes stable storage driver selection for JSON and SQLite.
- Route evaluation is another AST interpreter, separate from the main
  interpreter.

Risks:

- Not production-safe as a public HTTP server.
- Large bodies, slow clients, chunked encoding, keep-alive, malformed headers,
  and concurrency are not handled at production level.
- Runtime semantics can diverge between script interpreter and route evaluator.

Production improvements:

- Put the embedded server behind a real HTTP library or keep it explicitly dev
  only and build a production adapter.
- Add typed `RuntimeError`.
- Extend storage driver configuration beyond the current JSON/SQLite selection
  flag when production deployment needs richer connection settings.
- Create a shared route-operation IR used by checker, router, storage, and
  OpenAPI.
- Add transactions for create/update/delete.

## 11. Module System

Already implemented:

- Implicit file modules for `.nx` files.
- `import Name [as Alias] from "./module.nx"`, `import Name from "std/name"`,
  and package-name imports from local `path:` dependencies.
- `export` wrappers for named declarations supported by the current MVP:
  model, function, workflow, and auth.
- Recursive module loading with cycle detection, export validation, `.nx`
  extension inference, and deterministic merged programs.
- `ModuleGraph`, `HirModuleId`, and `HirSymbolRef` for cross-module import
  metadata.
- HIR import resolution is path-aware: it resolves against the importing
  module path instead of selecting the first module that exports the requested
  name.
- User-facing `nexus check`, `nexus run`, and `nexus serve` now use the
  graph-aware loader for file and manifest entrypoints.
- A small stdlib path exists with `std/math`.
- `nexus.toml` can identify a project entry and local path dependencies; those
  path dependencies now feed package-name import resolution in the module
  graph.
- The module graph enforces the current MVP symbol-surface contract: duplicate
  import aliases, alias collisions with local top-level declarations, and
  duplicate `fn`/`model`/`workflow`/`auth` names with the same kind across
  loaded modules are rejected before checker merge.
- A minimal `SourceDatabase` now mirrors `ModuleGraph` IDs, stores canonical
  paths and source text, records import edges and declaration ranges, and can
  attach diagnostics to module IDs or paths for future tooling.
- Checker diagnostics from merged graph-loaded programs can now carry explicit
  declaration/module owner metadata and map back to their owning module path
  and declaration range for display. Public structured diagnostic APIs expose
  that payload for tooling, while the CLI renders imported-module checker
  errors with that path and can emit them as versioned JSON via
  `nexus check --json` and `nexus run --json`. JSON run mode captures program
  stdout into an `output` array and preserves partial output on runtime
  diagnostics. The JSON formatter is public and covered for module-loader,
  checker, and runtime diagnostic variants. Diagnostics also carry optional
  code/severity metadata, including a granular v1 code catalog by
  lexer/parser/module-loader/checker/runtime error family. Optional labels,
  notes, and suggestions are now available in the structured payload and are
  populated for broader checker/module-loader/runtime producers without
  changing textual rendering. A minimal diagnostic report API and opt-in
  `nexus check --json-report` / `nexus run --json-report` modes can group
  diagnostics by path/module for tooling and collect independent checker
  declaration diagnostics across function, route, workflow, and invoice bodies
  while regular CLI JSON remains first-error compatible. Tooling helpers on the
  report support path/module, stage, severity, and group navigation without
  changing JSON v1, and `summary()` exposes in-memory counts, affected
  paths/modules, and error/warning flags for tooling dashboards. `tooling_view()`
  and `tooling_items()` provide flattened in-memory items for UI/CLI/editor
  consumers without serializing those fields into JSON v1. Opt-in source
  context can enrich those items with source line snippets from
  `SourceDatabase` when available. The pre-LSP tooling contract is now
  documented as a stability matrix and backed by a single contract test that
  guards Rust helper fields, source-context behavior, first-error JSON, and
  report JSON boundaries without adding LSP concepts.

Missing:

- Namespaces.
- Full package-level compilation and persistent/incremental source database.
- Registry-backed package imports.
- Dependency solving beyond direct local path roots.
- Re-exports, wildcard imports, namespace imports, module-qualified names, route
  exports, and invoice exports.
- Visibility and privacy beyond explicit exported/non-exported top-level
  symbols.

Poor design:

- Package integration is still loader-level; the current `SourceDatabase` is
  a successful in-memory foundation with minimal source ranges, not yet
  persistent, incremental, or byte-range precise.
- The duplicate-name policy is intentionally conservative: the MVP still uses
  a flat merged symbol surface by declaration kind instead of namespace or
  package-qualified lookup.
- `fmt`, `lint`, `tokens`, `ast`, and `repl` are still mostly single-file or
  entry-local tooling paths; only check/run currently use the structured
  multi-module diagnostic path and CLI JSON output.

Risks:

- Registry declarations may look installable even though only local `path:`
  dependencies feed the compiler.
- Larger ERP apps will need more package-aware/incremental source databases,
  clearer duplicate name ergonomics, and tooling that understands module
  graphs.

Production improvements:

- Evolve the minimal `SourceDatabase` into an LSP/cache-ready database with
  package roots, invalidation, byte ranges, diagnostic collections, and richer
  rendering.
- Define allowed cyclic references, if any, beyond the current rejection model.
- Replace the flat duplicate-name rule with namespace/package-qualified lookup
  once the source database and tooling model exist.

## 12. Package Manager

Already implemented:

- `nexus.toml`, `nexus.lock`, `.nexus/packages/`.
- `nexus install`, `nexus add`, `nexus add --path`, `nexus add --registry`,
  and `nexus update`.
- Manifest validation for sections, package names, versions, entry paths, and
  path dependencies.
- Deterministic lockfile entries.
- Safe stale cache cleanup scoped to package-name directories.
- Initial registry declaration contract.
- Compiler integration for local `path:` dependencies through package-name
  imports and manifest entrypoints.

Missing:

- Remote download.
- Package publishing.
- Registry server protocol.
- Semantic version solver.
- Transitive dependencies.
- Checksums/signatures per dependency.
- Dependency source trust model.
- Package isolation/sandboxing.

Poor design:

- TOML parsing is hand-rolled and supports only a narrow subset.
- Registry dependencies are declarations only; they create metadata but do not
  fetch usable code.
- Package cache markers are metadata only; compiler resolution reads local path
  manifests directly.

Risks:

- Users may assume registry dependencies work as real installations.
- Security model is not ready for third-party packages.

Production improvements:

- Use a real TOML parser when dependency policy allows it.
- Define registry protocol, package archive format, checksum/signature model,
  and trust roots.
- Resolve registry packages into a module graph after local path roots remain
  stable.
- Add lockfile integrity and reproducible install tests.

## 13. Standard Library

Already implemented:

- Minimal built-ins: `print`, `len`, `str`, `run_workflow`.
- Domain runtime services exist internally: HTTP, JSON parsing, storage,
  OpenAPI, auth, crypto for auth.

Missing:

- Language-visible collections beyond arrays.
- Maps/dictionaries.
- Filesystem.
- Networking.
- HTTP client.
- JSON APIs.
- Date/time operations.
- Async utilities.
- Crypto APIs.
- Testing utilities.
- String/array methods.

Poor design:

- Runtime services are compiler/runtime internals, not a designed standard
  library.
- Built-ins are hard-coded in multiple places.

Risks:

- Users cannot build real apps without escaping into runtime-specific
  primitives.
- Standard library expansion will become duplicated checker/interpreter cases.

Production improvements:

- Create a standard-library registry with typed function signatures and
  runtime implementations.
- Start with `std::string`, `std::array`, `std::json`, `std::time`,
  `std::http`, `std::test`.
- Keep ERP primitives separate from general standard library modules.

## 14. Tooling

Already implemented:

- CLI commands: run, check, tokens, ast, fmt, lint, serve, repl, new, install,
  add, update.
- Formatter.
- Linter with naming/style warnings.
- WASM playground with tokens, AST, ERP docs, diagnostics, output, examples.
- Quality gate scripts covering fmt/check/test/node/OpenAPI/smoke/auth/storage.
- Release packaging and validation scripts.

Missing:

- LSP.
- Debugger.
- Real test runner for `.nx` tests.
- Hot reload.
- Watch mode.
- Coverage.
- Profiling tools.
- Formatter idempotency corpus.
- Structured CLI framework.

Poor design:

- CLI parsing is manual over `env::args()`.
- Formatter is AST-based and not comment-preserving.
- Linter has no spans and no rule configuration.
- REPL accumulates a text buffer and reruns everything.

Risks:

- Developer experience will lag behind language features.
- Larger projects will be painful without LSP and test tooling.

Production improvements:

- Add `clap` or a small structured CLI layer.
- Add LSP after spans and parser recovery are improved.
- Add `nexus test` with `.nx` test files and assertions.
- Add `nexus watch` for check/serve/playground workflows.
- Add lint rule IDs, severity, spans, and configuration.

## 15. Compiler Backend

Already implemented:

- No compiler backend for NEXUSLANG programs.
- WASM support exists only for compiling the Rust core to browser WASM.
- The execution backend is a tree-walking interpreter.

Missing:

- HIR/MIR/IR generation.
- Bytecode.
- LLVM integration.
- Native binary generation.
- Optimization passes.
- NEXUSLANG-to-WASM compilation.
- Incremental compilation.

Poor design:

- Runtime and OpenAPI generation operate directly on AST.
- There is no stable lowered representation for execution or code generation.

Risks:

- Native performance and deployment will not improve without a backend path.
- Attempting LLVM now would amplify architectural debt because there is no IR.

Production improvements:

- Do not start with LLVM.
- First add HIR, then a simple bytecode or typed interpreter IR.
- Only after HIR/MIR stabilizes, evaluate LLVM/Cranelift/WASM codegen.
- Add optimization passes only after semantics are explicit.

## 16. Concurrency Model

Already implemented:

- No language-level concurrency.
- Runtime server handles requests sequentially.
- SQLite enables WAL/busy timeout, but request execution is not concurrent.

Missing:

- Async/await.
- Actors.
- Threads.
- Green threads.
- Channels.
- Mutexes/locks.
- Race-prevention model.
- Send/sync-like type properties.

Poor design:

- There is no concurrency design document.
- Auth/storage updates are not wrapped in a general transaction model.

Risks:

- Production API workloads require concurrency.
- Adding async without ownership/effects/module design will create language
  instability.

Production improvements:

- Decide the concurrency story: async tasks, actors, or sync-first.
- Add storage transactions first.
- Add request concurrency at the host runtime before exposing language-level
  concurrency.
- If Rust-like safety is a goal, define data-race prevention before threads.

## 17. Security And Safety

Already implemented:

- Rust host implementation gives memory safety in safe code.
- Native auth uses Argon2id, per-password salts, opaque session/bearer tokens,
  token hashing at rest, CSRF for cookie-backed unsafe methods, and rate
  limiting by auth identity.
- Checker prevents many invalid route/storage shapes before runtime.
- Storage validates model fields and constraints on create/update.

Missing:

- Language-level unsafe blocks.
- Overflow policy.
- Thread safety model.
- Null-safety ergonomics beyond optional/nil checks.
- Capability/security model for filesystem/network APIs.
- Full HTTP hardening.
- Secret rotation, password reset, MFA, auth audit log.

Poor design:

- Raw HTTP server is not a security boundary.
- HTTP status mapping by string prefix is brittle.
- JSON parsing/serialization is manual.
- No request size limits beyond fixed read buffer behavior.

Risks:

- Public exposure of the built-in server is risky without a reverse proxy and
  strict deployment guidance.
- Future stdlib APIs could accidentally expose unsafe capabilities without a
  permissions model.

Production improvements:

- Keep built-in server as dev/internal until replaced/hardened.
- Add typed errors and typed HTTP statuses.
- Introduce request size limits and robust HTTP parsing.
- Add security review gates for auth, package registry, and stdlib IO.
- Define overflow behavior: checked, wrapping, trapping, or profile-dependent.

## 18. Framework Readiness

Current capability:

- Web APIs: partially capable for CRUD-style JSON APIs over declared models.
- ERP prototypes: strong for demos and internal proof-of-concepts.
- OpenAPI-first internal APIs: promising and well tested.

Not ready yet:

- Enterprise frameworks.
- Large SaaS systems.
- Multi-module ERP platforms.
- Plugin systems.
- Dependency injection.
- Third-party package ecosystems.
- Background jobs, queues, scheduled tasks, workers.
- Database migrations and physical indexes.

Poor design:

- Framework concepts would currently have to be baked into the compiler/runtime
  as special cases.
- No module boundaries, no package resolution into compiler inputs, no generic
  abstractions, and no reflection/metadata layer.

Risks:

- The language may become a pile of ERP keywords instead of a scalable platform.
- Without an IR and module system, every framework feature will touch lexer,
  parser, AST, checker, runtime, OpenAPI, playground, docs, and tests.

Production improvements:

- Build a metadata/HIR layer for models, routes, auth, workflows, invoices.
- Add modules and packages before plugin systems.
- Add dependency injection only after interfaces/traits or a component model
  exists.
- Add migrations and storage indexes before promising serious ERP platforms.

## 19. Architectural Quality

Strengths:

- Clear Rust core as source of truth.
- Strong regression test count for current maturity.
- Good release/QA discipline.
- ERP-first product direction is coherent.
- OpenAPI and auth are unusually advanced for a small language prototype.
- WASM playground uses the Rust core rather than duplicating a JS interpreter.

Weaknesses:

- Monolithic checker.
- Large router/OpenAPI/storage files.
- HIR/lowering boundary has started, but checker/runtime still mostly consume
  raw AST.
- Repeated static model operation logic across checker, router, storage, and
  OpenAPI.
- Manual JSON and HTTP implementations.
- No module graph or package integration.
- AST is used as the universal representation.

SOLID/layering assessment:

- Single responsibility: weak in checker/router/OpenAPI/storage.
- Open/closed: weak because adding one model operation requires edits in many
  places.
- Interface segregation: partial in storage backend, weak elsewhere.
- Dependency inversion: weak; high-level language semantics depend directly on
  AST and string matching.
- Separation of concerns: improving, but still not production-grade.

Production improvements:

- Integrate the initial HIR with a resolver and typed operation metadata.
- Split checker into modules.
- Split route/runtime operation execution from HTTP transport.
- Move tests into domain-focused files.
- Generate docs/OpenAPI from typed metadata, not raw AST pattern matching.

## 20. Performance Analysis

Already acceptable:

- Current scripts and examples are small, and tests run quickly.
- Rust implementation is fast enough for the current feature set.
- WASM size has already been optimized through release profile settings.

Bottlenecks:

- Lexer copies source to `Vec<char>`.
- Parser clones tokens and AST values.
- AST stores many `String`s and boxes without interning.
- Checker clones model fields and scopes.
- Interpreter clones values, globals, object fields, functions, workflow steps.
- JSON storage reads/parses/writes whole model files.
- SQLite stores records as JSON text in a generic `data` column, so filtering
  scans and parses all rows.
- OpenAPI generation builds JSON through string concatenation.
- HTTP server is sequential.

Risks:

- Performance will degrade sharply with larger data or source files.
- Physical `index` metadata does not create actual indexes.
- Route filters and OpenAPI inference are duplicated and hard to optimize.

Production improvements:

- Add symbols/interning for identifiers.
- Use the new HIR compact IDs as the basis for resolver/type-checker caches.
- Add physical SQLite columns/indexes or generated schema for indexed fields.
- Use serde/serde_json or a robust internal JSON writer/parser.
- Add benchmarks: lex/parse/check, route dispatch, create/find/filter, OpenAPI
  generation, WASM payload size.
- Add a bytecode or typed interpreter IR before native compilation.

## Most Critical Missing Systems

1. Complete typed HIR consumed as the primary input by resolver, checker,
   route contracts, and runtime metadata.
2. Central route/model/auth operation contracts used by checker, router,
   OpenAPI, storage, docs, and playground. STARTED for model/auth route
   operations; still missing as a general compiler-wide contract.
3. Module system and multi-file compilation.
4. Real name resolver and symbol table graph.
5. Runtime structured errors with source spans and typed HTTP status mapping.
6. Standard library boundary and built-in registry.
7. Production HTTP/runtime layer or explicit adapter to one.
8. Real package registry/install/publish/solver integrated with modules.
9. LSP foundation: byte spans, parser recovery, labels/notes/suggestions, and
   diagnostic collections.
10. Storage migrations, physical indexes, and transactions.

## Roadmap: Experimental To Production-Ready

### Phase A: Stabilize Current Contracts

Goal: stop semantic drift before adding features.

- Finish applying `ModelStaticOperation` across checker route validation,
  router execution, and OpenAPI generation. DONE for the current route model
  operation surface.
- Add a single operation descriptor table for method name, args shape, route
  method requirement, return type, storage behavior, OpenAPI behavior. DONE
  for current model operations and Auth operations.
- Keep all current tests passing.
- Add focused tests proving checker/router/OpenAPI agree for every operation.
  DONE for current model operations and Auth operations.

### Phase B: Compiler Front-End Hardening

Goal: make parser/AST/diagnostics ready for tools.

- Add full source spans with byte ranges and end positions.
- Add parser recovery.
- Expand labels/notes/suggestions throughout remaining producers. Basic v1
  codes/severities, optional metadata containers, and broader populated
  parser/checker/module-loader/runtime producers are in place.
- Expand the minimal diagnostic report into real multi-error collection when
  parser/checker recovery is available.
- Add AST visitor/fold utilities.
- Split parser modules.
- Add lexer/parser test suites.

### Phase C: HIR And Semantic Architecture

Goal: stop using raw AST as the universal compiler/runtime contract.

- Lower AST to initial HIR. STARTED with `src/hir.rs`.
- Add minimum resolver-owned symbol IDs. STARTED with
  `checker/resolver.rs`.
- Add initial type IDs and definition/use links. STARTED with
  `HirCheckedMetadata`.
- Add module IDs and complete definition/use links.
- Split checker into resolver, type checker, control-flow checker, and
  route/model contract checker.
- Add CFG for function return/reachability analysis.
- Decide block scope semantics and fix leaks intentionally.

### Phase D: Modules And Packages

Goal: make projects larger than one file.

- Add `import`/`export`.
- Build source database and module graph. STARTED with in-memory
  `SourceDatabase` plus graph-aware imports.
- Integrate `nexus.toml` entry/dependencies with compiler inputs. STARTED for
  entrypoints and direct local `path:` dependencies.
- Detect dependency cycles.
- Keep package manager local first, then add registry downloads.

### Phase E: Runtime Contract And Storage Productionization

Goal: make APIs and ERP runtime dependable.

- Replace string-prefix errors with typed runtime errors.
- Add storage transactions.
- Add stable SQLite selection/config.
- Implement physical indexes or document/index migration boundaries.
- Replace raw HTTP server or harden it behind a clear production adapter.
- Add observability and request limits.

### Phase F: Standard Library And Tooling

Goal: let users build without compiler special cases.

- Add built-in/std registry shared by checker/interpreter.
- Add `nexus test`.
- Add LSP.
- Add watch/hot reload for check/serve.
- Add formatter idempotency tests and linter spans/config.

### Phase G: Backend Strategy

Goal: choose execution architecture after semantics stabilize.

- First build typed interpreter IR or bytecode.
- Add optimization passes on IR.
- Only then evaluate WASM/native/LLVM/Cranelift.
- Keep tree-walking interpreter as debug/reference backend if useful.

### Phase H: Framework Platform

Goal: support real SaaS/ERP frameworks.

- Add traits/interfaces or a component model.
- Add migrations, relations, background jobs, queues, schedulers.
- Add plugin system after module/package isolation.
- Add dependency injection only after interfaces/components exist.
- Add security capability model for IO, network, crypto, package code.

## Recommended Next Priorities

Immediate priority:

1. Complete definition/use links for all identifier-like references, including
   model fields, auth configs, workflows, and future modules.
2. Decide whether the hardened typed-HIR metadata writer is enough for the next
   release line or should evolve into a standalone HIR-walking production pass.
3. Split the checker into resolver, type checker, route contract checker, and
   validation passes.

After that:

1. Introduce full spans with byte ranges.
2. Continue splitting checker orchestration into smaller pass owners.
3. Add definition/use links and call graph metadata before adding more surface
   syntax.
4. Add module graph design before real package registry work.
5. Add typed runtime errors before expanding HTTP/auth/storage.

Hard no-go for now:

- Do not start LLVM/native codegen yet.
- Do not add async/await yet.
- Do not add a plugin system yet.
- Do not promise enterprise framework readiness yet.

NEXUSLANG has a strong prototype core. The production path is not "add more
syntax"; it is "add the missing compiler layers and runtime contracts so the
syntax can scale without turning every feature into cross-file duplication."
