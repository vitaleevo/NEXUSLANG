# NexusLang Development Roadmap

NexusLang is an ERP-first programming language. The current core is a Rust
lexer, parser, semantic checker, and interpreter, plus a standalone HTML
playground.

## Current baseline

- `nexus run <file.nx>` parses, checks, and executes a program.
- `nexus check <file.nx>` validates a program without executing it.
- `nexus tokens <file.nx>` prints lexer output.
- `nexus ast <file.nx>` prints parser output.
- `nexus install`, `nexus add <package>`, `nexus add <package> --path <dir>`,
  `nexus add <package> --registry <package@version>`, and `nexus update`
  provide the first package-manager MVP using `nexus.toml`, `nexus.lock`, and
  `.nexus/packages/`.
- Supported ERP primitives: `model`, `workflow`, `route`, `auth`, `invoice`,
  `money`.
- Supported language primitives: functions, `let`, `const`, `if`, `while`,
  `for`, arrays, strings, numbers, booleans, and static model calls.

## Post-0.1 release line

Current judgement: `v0.1.1` is published and publicly install-validated for
evaluation, demos, and QA, with public artifact checksum/signature verification
in place. The next work should reduce real user friction and narrow
compatibility risk before adding broad new language surface.

Current source line: post-`v0.1.1` local development, with Package Manager MVP
changes present locally but not included in the published `v0.1.1` tag.

### 0.1.1 maintenance focus

- Validate the published GitHub Release install path from a clean temporary
  directory. DONE
- Keep `scripts/validate-public-release-install.sh` as the repeatable
  post-release check for archive download, fingerprint, GPG signatures,
  checksum, extraction, packaged smoke, and playground asset smoke.
- Polish public install docs so they start from GitHub Release assets, not only
  locally built `dist/` artifacts.
- Make Linux/WSL package expectations explicit until cross-platform installers
  exist.
- Define the JSON/SQLite storage compatibility policy more concretely,
  especially backup, migration, and schema-change expectations. DONE in
  `COMPATIBILITY.md`, validated by
  `scripts/validate-storage-compatibility-policy.sh`,
  `storage_schema_evolution_allows_additive_optional_and_defaulted_fields`,
  and `sqlite_storage_matches_json_storage_for_crud_and_critical_filters`.
- Add one or two small public examples that show realistic inventory/CRM
  workflows without depending on unstable storage guarantees. STARTED with
  `storage_backup_restore_inventory.nx`, `STORAGE_BACKUP_RESTORE.md`, and
  `scripts/smoke-storage-backup-restore.sh`.
- Add a local package-manager MVP for project manifests and lockfiles. DONE
  with `nexus.toml`, `nexus.lock`, `nexus install`, `nexus add <package>`,
  `nexus update`, generated local `.nexus/packages/` cache, and CLI tests.
- Harden the package-manager MVP with local path dependencies, stronger
  `nexus.toml` validation, safe stale-cache cleanup, and an initial registry
  declaration contract. DONE locally; needs commit/push and CI observation.

Real risks to retire in `0.1.1`:

- The first package is still local-platform oriented; there are no Windows or
  macOS installers yet.
- The language package manager is still MVP-level: path dependencies and
  registry declarations are supported, but no remote downloads, semantic
  version solver, package publishing, transitive dependency resolution, or
  dependency signature/checksum verification exists yet.
- JSON/SQLite persistence works for the supported QA flows, but migrations and
  long-term schema compatibility are limited to the documented `0.1.x` policy:
  additive optional/defaulted fields are supported, while renames, removals,
  required fields without defaults, type changes, and physical SQLite schema
  assumptions remain breaking/experimental.
- `index` remains declarative metadata and does not create physical indexes.
- The playground is distributed as static package assets, not hosted as a
  public web product.
- Public release validation now exists, but it should stay part of every
  release handoff so regressions are caught after upload, not only before tag.
- Backup/restore is now documented and smoked for JSON storage, but SQLite
  remains a behavioral parity backend without a stable public `nexus serve`
  selection flag.
- `v0.1.1` has completed commit/push, GitHub Actions observation, strict
  dry-run, tag/release publication, and post-release public install validation.

### 0.2.0 product focus

- Choose one durable ERP vertical slice, such as inventory plus billing or CRM
  plus orders, and make it excellent end to end.
- Decide whether storage migrations, physical indexes, and a stable SQLite
  contract are required before larger runtime features.
- Decide whether docs generation belongs in the CLI as a first-class command
  before expanding documentation UI in the playground.
- Improve runtime diagnostics with structured locations where feasible, so
  server/runtime errors can match lexer/parser/checker diagnostic quality.
- Decide whether to ship a hosted playground or keep the web surface as a
  package-local learning/debugging tool for another release.
- Harden the first native auth slice with rate limiting, CSRF protection for
  cookie-backed unsafe methods, hosted TLS guidance, refresh-token policy, and
  SQLite auth-store parity.

## Phase 1: Core stability

- Keep semantic checks in Rust as the source of truth.
- Expand tests for lexer, parser, checker, and interpreter.
- Add line/column spans to key AST nodes and semantic errors. DONE for checker
  diagnostics and literal spans.
- Stop silently skipping unknown lexer characters. DONE
- Add a small diagnostics type instead of returning plain `String`. STARTED
  with lexer/parser/checker diagnostics and playground JSON.

## Phase 2: Type system

- Support explicit array types, for example `[string]` and `[money]`. DONE
- Support model types, for example `[Employee]`. DONE
- Validate user function parameters and return types. DONE
- Add model instance types and object literals. DONE
- Add field access for model instances, for example `customer.name`. DONE
- Add optional values and fields, for example `string?`. DONE
- Add model field defaults, for example `status: string = "active"`. DONE
- Validate all route and invoice return values. STARTED with direct route
  return contracts and required invoice fields/items.
- Add clearer rules for numeric promotion between `int` and `float`.

## Phase 3: ERP primitives

- Make `invoice` structured with line items, tax, discount, and totals. DONE
- Make `workflow` executable with step bodies. DONE
- Add model field rules such as `required`, `unique`, `min`, `max`. `default`
  is DONE for static model field values; `unique` STARTED for JSON-backed
  `Model::create()` and `Model::update()`; `index` STARTED as declarative
  scalar metadata with an OpenAPI marker; `min`/`max` STARTED for scalar
  model fields, including static default validation and JSON-backed
  `Model::create()`/`Model::update()` enforcement.
- Add route parameters such as `/employees/:id`. DONE
- Add JSON/object values for API responses. DONE for direct model instances in
  route returns.

## Phase 4: Tooling

- Add `nexus fmt`. DONE
- Add `nexus lint`. DONE
- Add `nexus repl`. DONE
- Add `nexus new <project>`. DONE
- Generate docs from `model`, `route`, `workflow`, and `invoice` declarations.

Python is useful here for quick developer scripts, fixtures, and migration
tools. It should not replace the Rust core.

## Phase 5: Runtime services

- Add `nexus serve <file.nx>`. DONE
- Serve declared `route` blocks over HTTP. DONE
- Generate OpenAPI from route declarations. DONE for paths, params, model
  response schemas, optional fields, defaults, model arrays, and request bodies
  for `Model::create()`/`Model::update()`, `400` invalid-body responses for
  `Model::create()`/`Model::update()`, plus `404` responses for
  `Model::find()`/`Model::update()`/`Model::delete()` and `409` responses for
  unique conflicts using a reusable `NexusError` schema, reserved internal
  component-name validation, stable `operationId` values, stable resource
  tags and top-level tags, reusable path/query parameter components, reusable
  request body components, reusable success response components,
  grouped OpenAPI path items for multiple methods on the same route path,
  duplicate route method+path validation,
  compact golden QA coverage for the OpenAPI 1.0 contract,
  generated OpenAPI JSON parseability QA,
  generated OpenAPI minimum structure QA,
  generated OpenAPI Path Item/Operation structure QA,
  generated OpenAPI internal component reference QA,
  generated OpenAPI operationId uniqueness/tag consistency QA,
  generated OpenAPI reusable component minimum-structure QA,
  generated OpenAPI model-schema semantic QA,
  generated OpenAPI operation/component contract consistency QA,
  generated OpenAPI 1.0 coherence-suite QA,
  unique/index/min/max field extensions, plus typed
  `Model::where()`/`Model::where_not()`/
  `Model::where_in()`/`Model::where_not_in()`/
  `Model::where_not_in_optional()`/`Model::where_all()`/`Model::where_any()` array
  responses via reusable `NexusList_<Model>` schemas, total-count page response envelopes for simple, `where_in`,
  `where_not_in`, `where_not_in_optional`, `where_in_optional`, exclusion, OR, and advanced filters via reusable `NexusPage_<Model>` schemas, composite-filter extension markers,
  exclusion-filter extension markers, or-filter extension markers, pagination extension markers,
  total-count extension markers,
  ordering extension markers, optional-filter extension markers,
  in-filter extension markers,
  comparison-filter extension markers, text-filter extension markers,
  range-filter extension markers, typed/optional/defaulted query params
  including `money` and simple arrays, and `400` responses for query param
  validation errors.
- Add JSON file storage first, then SQLite. JSON FILE DONE; typed create via
  `Model::create()`, typed read via `Model::find()`, typed filter via
  `Model::where()` and `Model::where_all()`, paginated list routes via
  `Model::all(limit, offset)`, `Model::where(..., limit, offset)`, and
  `Model::where_all(..., limit, offset)`, total-count paged list routes via
  `Model::page()`, `Model::where_page()`, `Model::where_in_page()`,
  `Model::where_not_page()`, `Model::where_not_in_page()`,
  `Model::where_not_in_optional_page()`, `Model::where_in_optional_page()`,
  `Model::where_any_page()` and advanced `*_page` filters,
  ordered list routes via
  `Model::all("field", "asc|desc")`, `Model::where(..., "field", "asc|desc")`,
  exclusion filters via `Model::where_not()`/`Model::where_not_in()`/
  `Model::where_not_in_optional()`, optional list filters via `Model::where_optional()`, in-list filters via
  `Model::where_in()`/`Model::where_not_in()`/
  `Model::where_not_in_optional()`/`Model::where_in_optional()`, OR filters via
  `Model::where_any()`, comparison list filters via `Model::where_compare()`,
  text list filters via `Model::where_text()` including simple case-insensitive
  operators, range filters via
  `Model::where_between()`, and
  `Model::where_all(..., "field", "asc|desc", limit, offset)`, and typed update
  via `Model::update()`, plus typed delete via `Model::delete()` STARTED.

Go is a good option if NexusLang later needs a separate high-performance API
gateway or deployment helper, but Rust can handle the first server runtime.

## Phase 5.5: Native Auth And Secure Backend

- Add `auth` declarations bound to a model identity field. DONE for the first
  JSON-backed runtime slice.
- Require auth identity fields to be `string unique`. DONE.
- Add route guards with `auth(Name)` and `auth(Name, role: "admin")`. DONE.
- Add `Auth::register()`, `Auth::login()`, `Auth::logout()`, and
  `Auth::user()` route returns. DONE.
- Store passwords with Argon2id and per-password salts. DONE.
- Issue opaque server-side session cookies and revocable bearer tokens. DONE.
- Generate OpenAPI security schemes plus `401` and `403` responses for guarded
  routes. DONE.
- Add an executable auth example in `examples/auth_secure_crm.nx`. DONE.
- Add production hardening: rate limits, CSRF tokens for cookie sessions,
  SQLite auth-store parity, secret rotation, and reverse-proxy/TLS deployment
  docs.

## Phase 6: Web product

- Replace the duplicated JavaScript interpreter in the playground with the Rust
  core compiled to WebAssembly. DONE
- Keep the playground as a learning/debugging surface. DONE
- Add examples for payroll, inventory, billing, banking, e-commerce, and CRM. DONE
- Add focused playground examples for Phases 3, 4, and 5. DONE
- Surface Rust diagnostics with line/column in the UI when available. DONE
- Reduce WASM size and prefer streaming playground loading with fallback. DONE
- Generate playground docs from model, route, workflow, and invoice data. DONE

Ruby on Rails is useful if NexusLang becomes a hosted product with accounts,
projects, billing, teams, templates, and a marketplace. It is not necessary for
the language core.

## Phase 7: 1.0 target

- Stable CLI.
- Stable syntax. STARTED
  - Document the 1.0 syntax baseline in `SYNTAX_1_0.md`. DONE
  - Reject missing commas in function parameters, call arguments, static call
    arguments, and array literals. DONE
  - Reject non-`step` tokens inside workflow declarations. DONE
  - Require route paths to start with `/`. DONE
- Strong semantic checker. STARTED
  - Require non-void functions to return on every checked path. DONE
  - Reject value returns in functions without declared return type. DONE
  - Require routes to contain one direct HTTP return expression. DONE
  - Restrict route return expressions to the HTTP subset currently supported by
    the server runtime. DONE
  - Require invoices to declare `customer`, `currency`, and at least one
    structured item or `total`. DONE
  - Reject duplicate invoice fields. DONE
  - Validate model instance object literals against declared model fields. DONE
  - Allow routes to return checked model instances as JSON objects. DONE
  - Validate model field access expressions against declared model fields. DONE
  - Reject `nil` for non-optional types and allow omitted optional model
    fields. DONE
  - Validate model field defaults and fill omitted defaulted fields. DONE
  - Generate OpenAPI schemas from route return models, optional fields, and
    defaults. DONE
  - Validate initial model field constraint `unique`. STARTED for scalar
    fields in JSON-backed `Model::create()` and `Model::update()`.
  - Validate initial model field constraint `index`. STARTED for scalar and
    optional scalar fields as declarative metadata.
  - Validate initial model field constraints `min`/`max`. STARTED for string
    length, int/float numeric, money, date, optional scalar fields, and static
    defaults.
- HTTP routes. STARTED with path params and typed query params, including
  optional/defaulted query params, `money` query params, simple array query
  params, optional typed model filters, typed `where_in`, `where_not_in`,
  `where_not_in_optional`, and `where_in_optional` filters, typed exclusion filters, typed OR filters, and typed comparison/text/range
  filters, including simple case-insensitive text operators, plus explicit total-count paged list responses for simple,
  `where_in`, `where_not_in`, `where_not_in_optional`, `where_in_optional`, exclusion, OR, and advanced filters.
- CRUD over models. STARTED with typed `POST` create, typed `GET`
  find/filter/exclusion-filter/optional-filter/comparison-filter/text-filter/case-insensitive-text-filter/range-filter/
  composite-filter/or-filter/in-filter/optional-in-filter/ordered and
  paginated list, total-count page envelopes for simple, `where_in`,
  `where_not_in`, `where_not_in_optional`, `where_in_optional`, exclusion, OR, and advanced filters,
  query-driven list controls, `min`/`max` field validation in create/update,
  typed `PUT` update, and typed `DELETE` delete over JSON storage.
- OpenAPI generation. DONE for model-based response contracts, create/update
  request bodies, create/update `400` invalid-body responses,
  find/update/delete `404` responses, unique/index/min/max extensions,
  unique conflict `409` responses, reusable `NexusError` error schema, reserved
  internal component-name validation, stable `operationId` values, stable
  resource tags and top-level tags, reusable path/query parameter components,
  reusable request body components, reusable success response components,
  grouped OpenAPI path items for multiple methods on the same route path,
  duplicate route method+path validation,
  compact golden QA coverage for the OpenAPI 1.0 contract,
  generated OpenAPI JSON parseability QA,
  generated OpenAPI minimum structure QA,
  generated OpenAPI Path Item/Operation structure QA,
  generated OpenAPI internal component reference QA,
  generated OpenAPI operationId uniqueness/tag consistency QA,
  generated OpenAPI reusable component minimum-structure QA,
  generated OpenAPI model-schema semantic QA,
  generated OpenAPI operation/component contract consistency QA,
  generated OpenAPI 1.0 coherence-suite QA, plus
  where/where_not/where_in/where_not_in_optional/where_all/where_any array
  responses via reusable `NexusList_<Model>` schemas,
  total-count page response envelopes for simple, `where_in`,
  `where_not_in`, `where_not_in_optional`, `where_in_optional`, exclusion, OR, and advanced filters using reusable `NexusPage_<Model>` schemas,
  composite-filter extension markers, exclusion-filter extension markers, or-filter extension markers, pagination extension markers,
  total-count extension markers, ordering extension markers, optional-filter
  extension markers, in-filter extension markers, comparison-filter extension
  markers, text-filter extension markers, range-filter extension markers, and
  typed/optional/defaulted query params including `money` and simple arrays,
  plus `400` responses for query param validation errors.
- Playground powered by the same Rust core as the CLI.
- Documentation and examples for real ERP workflows.

## OpenAPI 1.0 release readiness

Current judgement: READY WITH RISK for an internal release candidate. The
OpenAPI 1.0 subset is implemented for the supported route/runtime shapes and
has regression QA for golden fragments, JSON parseability, root/components
structure, Path Items, operations, reusable components, internal refs,
`operationId` uniqueness, tag consistency, operation/component contract
consistency, an aggregate coherence suite, external OpenAPI 3.0 validation
via Python, and a smoke test suite against the real HTTP server.

Remaining release risks:

- `x-nexus-*` extension semantics are Nexus-specific and may be ignored by
  external tooling.
- JSON file storage is still the first backend; `index` remains declarative and
  does not create physical indexes.
- Python validator is lightweight and does not cover the full OpenAPI 3.0 spec.

Release checklist:

- DONE: OpenAPI route paths, params, responses, request bodies, reusable
  schemas and reusable components are generated for the supported HTTP subset.
- DONE: Semantic guards reject reserved internal component names and duplicate
  route method+path declarations.
- DONE: Compact OpenAPI golden snapshot covers representative 1.0 contract
  fragments.
- DONE: Internal QA covers parseability, minimum structure, reusable component
  structure, `$ref` resolution, `operationId` uniqueness and tag consistency.
- DONE: Internal QA validates operation request bodies, `200`/`201`/`400`/
  `404`/`409` responses and success schemas against the real route contracts
  and reusable components.
- DONE: The aggregate OpenAPI 1.0 coherence suite runs the central structural
  validations as one release-readiness gate.
- DONE: External OpenAPI 3.0 validation via `scripts/validate-openapi.sh`
  (Python `openapi-schema-validator` + structural checks).
- DONE: Smoke test suite via `scripts/smoke-test.sh` covering all CRUD, listagens,
  filtros (where_in, where_not, where_optional, compare, range, text, OR) e
  respostas de erro (400, 404, 409).
- DONE: Representative OpenAPI QA example file `examples/openapi_qa.nx` covering
  todos os tipos de route, filtro e query params suportados pelo alvo 1.0.
- DONE: Lightweight Python validator `scripts/validate-openapi.py` que verifica:
  - `openapi` version, `info`, `paths`, `components`
  - estrutura minima de operations (`operationId`, `responses`)
  - resolução de todos os `$ref` internos
  - schemas, parameters, requestBodies, responses
- DONE: Publish release notes documenting the supported subset and JSON storage
  limitations.
- DONE: Document version/tag policy for local release artifacts.
- DONE: Document language/runtime/storage compatibility levels.
- DONE: Document and script the GPG signing path for release artifacts.
- DONE: Add final local dry-run and Docker-based second-environment validation.
- DONE: Add strict GitHub/GPG/remote-CI release dry-run gate.
