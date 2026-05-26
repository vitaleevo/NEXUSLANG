# NexusLang Package Manager

This document defines the current local package-manager contract.

## Status

The package manager is a local MVP. It supports project manifests, lockfiles,
local cache metadata, local path dependencies, and registry dependency
declarations. It does not download packages from a remote registry yet.

Current score: 50/100 after local path dependencies, manifest validation,
safe stale-cache cleanup, and the initial registry contract are validated.

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
```

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
  manifest.
- `registry:<package>@<version>`: records the future remote-registry contract
  in `nexus.toml` and `nexus.lock`. The current implementation does not
  download remote packages.

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
- No transitive dependency resolution.
- No per-dependency checksum or signature verification yet.
