# NexusLang Package Manager

This document defines the current local package-manager contract.

## Status

The package manager is an MVP. It supports project manifests, lockfiles, local
cache metadata, local path dependencies, registry dependency declarations, and
a read-only registry install flow when `NEXUS_REGISTRY_URL` is configured.
The compiler can consume `[package].entry` as the default CLI entrypoint and
can resolve package-name imports for local `path:` dependencies and installed
registry dependencies.

Current score: 72/100 after local path dependencies, manifest validation,
safe stale-cache cleanup, read-only registry download/cache, checksum
validation, safe archive extraction, and module-graph integration for installed
registry dependencies are validated.

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
- `registry:<package>@<version>`: records the remote-registry source contract
  in `nexus.toml` and `nexus.lock`. When `NEXUS_REGISTRY_URL` is configured,
  `nexus install`, `nexus add --registry`, and `nexus update` resolve the
  package metadata, download the declared archive, verify `sha256` when
  present, extract it safely into `.nexus/packages/<package>`, and write
  checksum/resolved-path metadata to `nexus.lock`.

## Read-Only Registry MVP

Set `NEXUS_REGISTRY_URL` to a filesystem path, `file://` URL, or plain
`http://` base URL:

```bash
NEXUS_REGISTRY_URL=/opt/nexus-registry nexus install
NEXUS_REGISTRY_URL=file:///opt/nexus-registry nexus update
NEXUS_REGISTRY_URL=http://127.0.0.1:8090 nexus add audit_core --registry audit_core@0.1.0
```

The MVP registry layout is:

```text
<registry-root>/<package>/<version>/nexus-package.toml
<registry-root>/<package>/<version>/<archive>.tar
```

`nexus-package.toml` uses a small key-value contract:

```toml
name = "audit_core"
version = "0.1.0"
archive = "audit_core-0.1.0.tar"
sha256 = "64 lowercase or uppercase hex characters"
```

The archive must be an uncompressed `.tar` with package files at its root,
including a valid `nexus.toml` whose `[package].name` and `[package].version`
match the registry request. Extraction rejects absolute paths, `..` traversal,
hard links, symbolic links, unsupported tar entry types, and checksum
mismatches.

If `NEXUS_REGISTRY_URL` is not configured, registry dependencies keep the old
contract-only behavior: manifest, lockfile, and cache marker are written, but
no remote package is downloaded and the compiler cannot import that dependency
until it is installed from a configured registry.

## Compiler Integration

The module graph resolves imports in this order:

- `std/<module>` from the installed or development stdlib;
- `./...` and `../...` relative to the importing file;
- `<package>` and `<package>/...` through a direct local `path:` dependency in
  the nearest `nexus.toml`;
- `<package>` and `<package>/...` through an installed
  `registry:<package>@<version>` dependency in `.nexus/packages/<package>`.

Only local `path:` dependencies and installed registry dependencies are
compiler inputs in this MVP. `local` entries still write manifest, lockfile,
and cache metadata but are not source roots for imports.

The current module graph keeps a flat symbol surface per declaration kind.
If two loaded modules define the same `fn`, `model`, `workflow`, or `auth`
name with the same kind, `nexus check` rejects the program until namespaces or
package-qualified lookup are introduced.

## Lockfile

`nexus.lock` is deterministic and includes one `[[package]]` block per
dependency, with source kind, source string, version, and path/registry metadata
when available. Installed registry packages include `resolved_path`,
`registry_package`, and `checksum = "sha256:<hex>"` when the registry metadata
provided a checksum.

## Validation

`nexus install`, `nexus add`, and `nexus update` validate:

- known sections only: `[package]` and `[dependencies]`;
- unique manifest keys;
- package names using letters, numbers, `_`, or `-`;
- package versions using letters, numbers, `.`, `-`, or `+`;
- project entry as a relative `.nx` path inside the project;
- dependency source syntax;
- path dependency existence and matching package manifest.
- registry metadata name/version/archive/checksum when `NEXUS_REGISTRY_URL` is
  configured;
- registry archive extraction safety.

## Cache Cleanup

`nexus install` and `nexus update` remove stale direct child directories under
`.nexus/packages/` when they are no longer present in `nexus.toml`. The cleanup
is intentionally scoped to managed package-name directories only.

## Known Limits

- No package publishing command.
- No registry authentication or HTTPS client yet; this MVP supports local
  paths, `file://`, and plain `http://`.
- No semantic version solver.
- No transitive dependency solver; package-name imports are limited to local
  path roots and installed registry roots declared by the nearest package
  manifest.
- No registry package signature verification yet.
