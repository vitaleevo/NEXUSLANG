# NexusLang Compatibility Contract

This document records what NexusLang treats as stable, release-candidate, or
experimental for the current `0.1.0` local release candidate.

## Compatibility Levels

- Stable: changes require a documented version bump and release note.
- Release candidate: intended to stay coherent, but may still change before
  `1.0.0` with clear notes.
- Experimental: useful, tested locally, but not a long-term public contract.
- Internal: implementation detail with no compatibility promise.

## Language Syntax

Level: release candidate.

The syntax baseline is documented in `nexuslang-src/SYNTAX_1_0.md`. The current
release candidate supports:

- functions with typed params and returns;
- `let` and `const`;
- `if`, `while`, and `for`;
- arrays and optional values;
- `model`, `workflow`, `route`, `invoice`, and `money`;
- model instances and field access;
- typed route params and query params.

Before `1.0.0`, syntax changes may still happen, but breaking changes must be
listed in `RELEASE_NOTES.md`.

## CLI

Level: release candidate.

The public CLI commands for `0.1.0` are:

```text
nexus run <file.nx>
nexus check <file.nx>
nexus fmt <file.nx> [--write]
nexus lint <file.nx>
nexus serve <file.nx> [addr]
nexus repl
nexus new <project>
nexus tokens <file.nx>
nexus ast <file.nx>
nexus --help
```

Patch releases should not remove or rename these commands.

## HTTP Runtime

Level: release candidate.

The runtime contract covers the route shapes validated by tests and OpenAPI QA:

- health endpoint;
- declared route matching;
- path params with safe percent-decoding;
- typed/defaulted/optional query params;
- model create, find, update, delete, lists, filters, ordering, and pagination;
- JSON responses with `400`, `404`, and `409` errors where supported.

Unsupported route expressions remain outside the compatibility contract.

## OpenAPI

Level: release candidate.

OpenAPI generation is stable for the supported NexusLang HTTP subset and is
validated by internal tests plus external OpenAPI 3.0 validation. `x-nexus-*`
extensions are Nexus-specific and should be treated as optional metadata by
external tools.

## Storage

### JSON Storage

Level: experimental.

JSON storage is appropriate for local development, examples, and smoke tests.
Generated `.nexus-data` directories are local runtime data and are excluded
from release packages. Long-term JSON storage compatibility is not frozen.

### SQLite Storage

Level: experimental.

SQLite has parity coverage for critical CRUD and filter flows. The schema and
migration story are not yet a public compatibility contract.

## Playground And WASM

Level: release candidate for the packaged user experience, internal for WASM
exports.

The package promises that `nexuslang-playground.html`,
`nexuslang-playground.js`, and
`nexuslang-src/web/nexuslang_playground.wasm` load together when served from
the package root. Raw WebAssembly exports are internal implementation details.

## Package Layout

Level: release candidate.

The release package should contain:

- `bin/nexus`
- `nexuslang-playground.html`
- `nexuslang-playground.js`
- `nexuslang-src/web/nexuslang_playground.wasm`
- `examples/*.nx`
- `docs/README.md`
- `docs/RELEASE.md`
- `docs/RELEASE_NOTES.md`
- `docs/VERSIONING.md`
- `docs/COMPATIBILITY.md`
- `docs/SIGNING.md`
- `PACKAGE_MANIFEST.txt`
- `scripts/smoke-package.sh`

## Breaking Change Rules

Before `1.0.0`, breaking changes are allowed only when they are intentional,
documented, and covered by tests or release validation. After `1.0.0`, breaking
changes require a major version bump.
