# NexusLang Package Manager

This document defines the current local package-manager contract.

## Status

The package manager is a local MVP. It supports project manifests, lockfiles,
local cache metadata, local path dependencies, and registry dependency
declarations. It does not download packages from a remote registry yet.
The compiler can consume `[package].entry` as the default CLI entrypoint and
can resolve package-name imports for local `path:` dependencies.

Current score: 60/100 after local path dependencies, manifest validation,
safe stale-cache cleanup, the initial registry contract, and module-graph
integration for local path dependencies are validated.

## Files

- `nexus.toml`: project manifest.
- `nexus.lock`: generated lockfile. Do not edit by hand.
- `.nexus/packages/`: local managed package cache metadata.

## Commands

```bash
nexus install
nexus add <package>
nexus add <package> --path <dir>
nexus add <package> --registry <package@version>
nexus update
nexus check
nexus run
nexus serve --addr <addr>
```

When `check`, `run`, or `serve` are executed without an explicit source file,
the CLI loads the nearest `nexus.toml` and uses `[package].entry`.

## Manifest

```toml
[package]
name = "acme-erp"
version = "0.1.0"
entry = "main.nx"

[dependencies]
crm_core = "local"
billing-core = "path:../billing_core"
audit_core = "registry:audit_core@0.1.0"
```

## Dependency Sources

- `local`: creates a local managed cache marker.
- `path:<dir>`: points to a sibling or local directory containing its own
  `nexus.toml`. The dependency name must match `[package].name` in that
  manifest. This source can be imported by package name, for example
  `import build_invoice from "billing-core"` for the dependency entrypoint or
  `import Customer from "crm_core/models"` for a dependency submodule.
- `registry:<package>@<version>`: records the future remote-registry contract
  in `nexus.toml` and `nexus.lock`. The current implementation does not
  download remote packages, and registry dependencies cannot be imported by
  the compiler yet.

## Compiler Integration

The module graph resolves imports in this order:

- `std/<module>` from the installed or development stdlib;
- `./...` and `../...` relative to the importing file;
- `<package>` and `<package>/...` through a direct local `path:` dependency in
  the nearest `nexus.toml`.

Only `path:` dependencies are compiler inputs in this MVP. `local` and
`registry:` entries still write manifest, lockfile, and cache metadata but are
not source roots for imports.

The current module graph keeps a flat symbol surface per declaration kind.
If two loaded modules define the same `fn`, `model`, `workflow`, or `auth`
name with the same kind, `nexus check` rejects the program until namespaces or
package-qualified lookup are introduced.

## Lockfile

`nexus.lock` is deterministic and includes one `[[package]]` block per
dependency, with source kind, source string, version, and path/registry metadata
when available.

## Validation

`nexus install`, `nexus add`, and `nexus update` validate:

- known sections only: `[package]` and `[dependencies]`;
- unique manifest keys;
- package names using letters, numbers, `_`, or `-`;
- package versions using letters, numbers, `.`, `-`, or `+`;
- project entry as a relative `.nx` path inside the project;
- dependency source syntax;
- path dependency existence and matching package manifest.

## Cache Cleanup

`nexus install` and `nexus update` remove stale direct child directories under
`.nexus/packages/` when they are no longer present in `nexus.toml`. The cleanup
is intentionally scoped to managed package-name directories only.

## Known Limits

- No remote registry downloads.
- No package publishing command.
- No semantic version solver.
- No transitive dependency solver; package-name imports are limited to local
  path roots declared by the nearest package manifest.
- No per-dependency checksum or signature verification yet.
