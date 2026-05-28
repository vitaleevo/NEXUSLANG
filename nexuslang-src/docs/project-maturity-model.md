# NexusLang — Project Maturity Model A-Z

Status date: 2026-05-28

## Reporting Rule

Every completed phase, release note, or readiness summary must include:

- Current maturity letter(s) per track.
- Percentage per track (never one blended number).
- The next letter or gate blocking progress.

## A-F: MVP Foundation

"Core language exists and is usable from CLI"

| Letter | Milestone | NexusLang Status |
|---|---|---|
| A | Product intent and PRD | ✅ `prd.md` defines the language vision |
| B | Lexer + Parser | ✅ Complete, 0 panics, spanned diagnostics |
| C | Checker + Interpreter | ✅ F1 Core Stability 100%, Diagnostic em todos os estágios |
| D | CLI + Test Runner | ✅ `nexus` binary, 403 testes, sidecars, filters, JSON report |
| E | Module system + Package manager | ✅ Module graph, SourceDatabase, path deps, stdlib, install/add |
| F | MVP language ready | ✅ Core stablilized — API pública congelada para tooling |

**A-F Score: 100%** ✅ Concluído

## F-P: Production Path

"Ferramentas de produção e servidor operacional"

| Letter | Milestone | NexusLang Status |
|---|---|---|
| G | HTTP server | ✅ REST server com OpenAPI, SQLite/JSON storage |
| H | OpenAPI documentation | ✅ Schema generation, contract snapshot tests |
| I | Multi-module diagnostics | ✅ Report, JSON v1, tooling helpers, grouping, source context |
| J | WASM playground | ✅ Playground funcional, WASM build |
| K | Docs generator | ✅ `nexus docs` gera markdown para declarações ERP |
| L | Performance baseline | ⬜ Not started — sem profiling, sem benchmarks |
| M | Error recovery (parser) | ⬜ Not started — parser pára no primeiro erro |
| N | Formatting polish | ⬜ `nexus fmt` básico, sem config options |
| O | LSP server | ✅ **MVP implementado** (diagnostics, hover, completion, goto-def) |
| P | Editor extensions | ⬜ Not started — VS Code / Neovim integration |

**F-P Score: ~65%** — Servidor, docs, diagnostics prontos. Faltam performance, error recovery, editor extensions.

## P-Z: Commercial Product

"Ecosystema e produto comercial escalável"

| Letter | Milestone | NexusLang Status |
|---|---|---|
| P | Editor support stable | ⬜ LSP needs testing + editor configs |
| Q | Language server protocol complete | ⬜ Missing: signature help, references, rename, code actions |
| R | Semantic tokens + syntax highlighting | ⬜ Not started |
| S | Debugger | ⬜ Not started |
| T | Registry (package manager remoto) | ⬜ `nexus package` local only, sem registry |
| U | LSP performance | ⬜ Not profiled |
| V | Multi-file workspace support | ⬜ LSP single-file only |
| W | Embedded DSL tooling | ⬜ Not started |
| X | CI/CD integration | ⬜ Not started |
| Y | Enterprise features | ⬜ Not started |
| Z | Stable commercial release | ⬜ Not started |

**P-Z Score: ~2%** — LSP MVP existe, todo o resto está por fazer.

## Overall NexusLang Position

| Track | Score | Current Letter |
|---|---|---|
| A-F Core Language | 100% | ✅ Complete |
| F-P Production Tooling | ~65% | O (LSP MVP) / P (editor stubs) |
| P-Z Commercial Product | ~2% | P (just started LSP) |

## Current Focus

**LSP Adapter (letter O → P)**

| Sub-task | Status |
|---|---|
| A — Intent/scope | ✅ Defined |
| B — Crate scaffold | ✅ Created |
| C — Core features (diagnostics, hover, completion, goto-def) | ✅ 4/4 implemented |
| D — Editor integration testing | ⬜ Next step |
| E — Hardening (error handling, edge cases) | ⬜ |
| F — MVP ready | ⬜ |

LSP A-F: ~55%

## Update Rule

When any work changes scope or status:

1. Update this document if maturity letter/score changes.
2. Update `MEMORIA_NEXUSLANG.md` at every phase closeout.
3. Update `docs/prd.md` if product requirements change.
4. Never use a single blended percentage — tracks are independent.
