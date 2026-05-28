# MEMORY.md - NexusLang architectural decisions

Canonical continuity remains in `MEMORIA_NEXUSLANG.md`. This file exists as
the short English-named memory requested for architecture decisions.

Last updated: 2026-05-28

## 2026-05-28 - PR post-feedback CI passed; RC status clarified

- Commit `38b64e6 fix(rc): address automated review feedback` is on
  `origin/codex/prepare-nexuslang-0.2.0-rc`; PR #1 is open, ready for review,
  mergeable, and had two remote `quality` jobs plus CodeRabbit pass on that
  head.
- CodeRabbit left one residual documentation comment in `meta/ROADMAP.md`; the
  roadmap now clarifies that public `v0.2.0-rc.1` is a historical pre-release
  and does not include the post-publication fixes on the current PR head.
- Architectural release decision: do not treat the published `v0.2.0-rc.1`
  assets as representing the current PR head. Use a new RC/tag or explicit
  post-merge validation before any stable `0.2.0`.
- Final local validation for this phase passed:
  `CARGO_TARGET_DIR=/tmp/nexuslang-target-codex NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo test --workspace --all-targets`.

## 2026-05-28 - PR feedback hardening passed locally

- PR #1 was marked ready for review (`isDraft=false`) and remains open,
  mergeable, and unmerged.
- Automated PR feedback was addressed locally across module loading, checker/HIR,
  diagnostics, LSP metadata/diagnostics, README/release docs, and tests.
- Dependency module imports are now retained during merged-program construction
  so aliases used inside transitive modules survive checker and runtime paths.
- Runtime multi-module diagnostics now preserve entry path/module metadata for
  JSON/LSP tooling while `.err` sidecars stay deterministic by comparing the
  core diagnostic text.
- Local validation passed after the feedback fixes:
  `CARGO_TARGET_DIR=/tmp/nexuslang-target-codex cargo test -p nexus-lsp`,
  `CARGO_TARGET_DIR=/tmp/nexuslang-target-codex cargo test -p nexuslang --test core`,
  and `NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh`.
- Next work is committing/pushing the feedback fixes, watching CI/CodeRabbit on
  the new PR head, and deciding merge only if no blockers remain.

## 2026-05-28 - Public RC pre-release validated

- `v0.2.0-rc.1` is now published as a public GitHub pre-release, not as a
  stable latest release.
- The public release initially lacked `nexuslang-release-public-key.asc` and
  `nexuslang-release-signing-key.fingerprint`; public install validation caught
  the missing assets with a 404, then passed after both assets were uploaded.
- `NEXUS_PUBLIC_RELEASE_TAG=v0.2.0-rc.1 ./scripts/validate-public-release-install.sh`
  passed, including GPG verification, checksum verification, package smoke,
  auth smoke, storage backup/restore smoke, playground syntax, and HTTP asset
  fetches.
- PR #1 remains a draft and has not been merged. Next work is PR/feedback
  review and deciding whether the RC branch can leave draft/merge toward a
  future stable `0.2.0`.

## 2026-05-28 - RC draft release created

- Signed annotated tag `v0.2.0-rc.1` was created with the NexusLang release key
  and pushed to `origin`.
- The tag points to validated commit `75c5ef8fb8b67494d741d3231965d81ba1ef33b7`
  from `codex/prepare-nexuslang-0.2.0-rc`.
- A GitHub Release draft was created for `v0.2.0-rc.1` as a pre-release, with
  the local package archive, `.sha256`, and both detached `.asc` signatures
  attached.
- This was superseded by the public RC pre-release validation phase above.

## 2026-05-28 - RC PR, CI, and strict dry-run passed

- Branch `codex/prepare-nexuslang-0.2.0-rc` was pushed to
  `origin/codex/prepare-nexuslang-0.2.0-rc`.
- Draft PR `https://github.com/vitaleevo/NEXUSLANG/pull/1` targets `main`
  from the RC branch.
- GitHub Actions `NexusLang Quality Gate` passed for the PR head.
- Strict public-release dry-run passed with maintained signing key
  `3237F7CC5CE2514FC9671BB93CB6808B55385273`, producing signed local artifacts
  and second-environment validation.
- No tag, merge, or public GitHub Release was created in this phase.

## 2026-05-28 - 0.2.0-rc.1 local RC packaging

- The post-`v0.1.1` work is now organized on
  `codex/prepare-nexuslang-0.2.0-rc` with scoped commits and source version
  `0.2.0-rc.1`.
- Local release packaging produced
  `nexuslang-v0.2.0-rc.1-local-release.tar.gz` plus its `.sha256` checksum
  file.
- `validate-release-package.sh` validated the package in a clean temporary
  directory, including CLI/package smoke, stdlib imports, auth smoke, storage
  backup/restore smoke, playground JavaScript syntax, and HTTP asset fetches.
- `v0.1.1` remains the latest published GitHub Release. The RC is not public
  until push/PR/CI, strict release dry-run, signing, and publication pass.

## 2026-05-28 - RC branch commits organized

- `codex/prepare-nexuslang-0.2.0-rc` now carries the post-`v0.1.1` work as
  scoped commits for a local `0.2.0-rc.1` candidate.
- The branch includes docs/handoff, modular checker/diagnostics, runtime auth
  storage/OpenAPI hardening, package manager + stdlib workflows, the
  `nexus-lsp` adapter, refreshed playground WASM, and tighter release package
  gates.
- Local validation passed: `git diff --check`, `git diff --cached --check`,
  `cargo check/test/clippy -p nexus-lsp`, and
  `NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh`.
- The immediate next step is package/preflight:
  `./scripts/package-release.sh` followed by
  `./scripts/validate-release-package.sh`, then push/PR/CI before any strict
  public release dry-run.

## 2026-05-28 - Release RC triage

- The local checkout is post-`v0.1.1` development on `main` at `bf37ed4`, with
  84 pending worktree entries: 34 modified files and 50 untracked files.
- A new triage handoff lives at `meta/RELEASE_RC_TRIAGE.md`, grouping the
  pending work into docs/memory, contracts, LSP, core/checker/HIR,
  runtime/auth/storage/OpenAPI, package manager/stdlib, CLI/test runner,
  playground/WASM, and release scripts.
- `scripts/release-dry-run-strict.sh` requires a clean worktree, pushed HEAD,
  successful CI, and maintained signing key, so the current checkout is blocked
  for public RC until the worktree is intentionally organized.
- The next RC should probably target `0.2.0-rc.1` or `0.2.0` if it includes
  the new LSP, stdlib, package manager, runtime, and tooling surfaces.
- No files were reverted, deleted, staged, or committed during the triage.

## 2026-05-28 - LSP document symbols MVP

- `nexus-lsp` now exposes a document symbols MVP through
  `DocumentSnapshot::document_symbols()` and `LspCore::document_symbols()`.
- The MVP is document-local and parser/AST-backed: it maps functions, models,
  workflows, auth declarations, routes, invoices, imports, exported
  declarations, and top-level bindings to LSP `DocumentSymbol` entries.
- ERP children are included where the AST already has spans: model fields,
  workflow steps, route query params, invoice fields, and invoice items.
- Nested symbols use an enclosing declaration/block `range` when available and
  a separate name-focused `selection_range`, so editor outlines can nest
  children under their parent declaration cleanly.
- Invalid or partially written documents return an empty nested symbol list
  instead of mixing stale symbols with parse errors.
- `src/main.rs` now advertises `document_symbol_provider` while keeping
  `tower-lsp` as a thin adapter over `LspCore`.
- No workspace-wide indexing, source database cache, rename, formatting, code
  actions, or workspace symbols were introduced in this phase.

## 2026-05-28 - LSP semantic tokens MVP

- `LspCore` now exposes full-document semantic tokens through
  `semantic_tokens()` on open document snapshots.
- The LSP adapter advertises `textDocument/semanticTokens/full` and returns the
  core-produced token stream without adding editor protocol code to the compiler
  core.
- The MVP legend is intentionally small and stable: `keyword`, `type`,
  `string`, `number`, `variable`, and custom `erpSymbol`.
- Token classification reuses `tokens_source_spanned`; ERP declaration tokens
  such as `model`, `route`, `auth`, `workflow`, `step`, and `invoice` map to
  `erpSymbol`.
- This remains lexical highlighting only. It does not require checker state,
  persistent `SourceDatabase`, rename, formatting, code actions, or workspace
  indexing.

## 2026-05-28 - LSP cross-file go-to-definition

- `LspCore::goto_definition()` now attempts an opt-in cross-file path before
  falling back to the existing same-document definition helper.
- The cross-file path is disk-backed: the entry snapshot must match disk, and
  every loaded module that is open in the editor must match its
  `SourceDatabase` source text.
- Import alias usages and import names resolve through `SourceDatabase`
  import edges and `ModuleGraph` export metadata to the exported declaration in
  the target module.
- Dirty entry snapshots and dirty imported-module snapshots keep the previous
  same-document fallback, so the LSP does not mix unsaved editor text with a
  disk-loaded graph.
- No persistent or incremental source database was introduced; the LSP still
  asks the module loader for a fresh graph only for opt-in editor operations.

## 2026-05-28 - LSP stale diagnostics cleanup

- `LspCore` now remembers the last diagnostic publication group per entry URI,
  as a set of module/file URIs, without introducing a persistent
  `SourceDatabase`.
- When an entry falls back from multi-file diagnostics to single-document
  diagnostics, or when its import graph no longer includes a previously loaded
  module, the LSP emits empty batches for stale URIs so editors clear old
  diagnostics.
- `did_close` uses `close_document_publish_batches()` to remove the snapshot
  and publish empty batches for the closed entry's previous diagnostic group.
- Stale clears are suppressed for a URI that is still covered by another active
  entry document, avoiding accidental removal of diagnostics owned by a
  different open graph.
- The adapter remains thin: `tower-lsp` publishes the batches returned by
  `LspCore`, while module loading and checker semantics stay in the core.

## 2026-05-28 - LSP multi-file diagnostics bridge

- `LspCore::diagnostic_publish_batches_for()` now attempts an opt-in
  multi-file diagnostics path when the opened entry snapshot maps to a local
  file and still matches disk.
- The LSP bridge reuses `module_loader::load_program_full_with_source_database`
  and `check_with_source_database_diagnostic_report`, so editor diagnostics use
  core/module-loader/checker semantics instead of duplicating them.
- Multi-file publication emits one batch per loaded module, which lets the
  editor clear diagnostics for imported files when the project becomes clean.
- Dirty open snapshots fall back to single-document diagnostics to avoid mixing
  unsaved editor text with a disk-loaded module graph.
- The JSON v1 contract remains unchanged; URI grouping and document versions
  are LSP adapter concerns only.
- This remains a non-incremental bridge: no persistent source database,
  cross-file go-to-definition, semantic tokens, formatting, rename, code
  actions, or workspace symbols yet.

## 2026-05-28 - LSP core extraction

- `nexuslang-src/nexus-lsp/src/lib.rs` is now the testable LSP core. It owns
  `DocumentSnapshot`, `LspCore`, diagnostic conversion, hover, completion, and
  same-document definition helpers.
- `nexuslang-src/nexus-lsp/src/main.rs` is now only the `tower-lsp` transport
  adapter. It maps editor lifecycle events into `LspCore` and publishes
  results.
- The compiler/runtime core remains independent from `tower-lsp`; editor
  protocol code stays inside the separate `nexus-lsp` crate.
- LSP behavior for that phase stayed intentionally limited to full-document
  sync, single-document diagnostics, hover, completion, and same-document
  go-to-definition.
