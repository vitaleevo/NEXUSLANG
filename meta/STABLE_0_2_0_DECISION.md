# Stable 0.2.0 Decision

Date: 2026-05-28

## Decision

| Item | Decision |
| --- | --- |
| Promote `0.2.0` stable now | No |
| Continue with RC2 as public pre-release | Yes |
| Run a short pre-stable hardening cycle | Yes |
| Change source version from `0.2.0-rc.2` to `0.2.0` in this phase | No |
| Create or publish `v0.2.0` stable in this phase | No |

## Rationale

RC2 is technically healthy enough to be the base for a stable release, but the
stable label would set broader expectations than the project should claim
today. The right decision is to finish a small hardening cycle first, then run a
dedicated stable release branch if the remaining gates pass.

## Evidence

| Signal | Status | Notes |
| --- | --- | --- |
| RC2 public pre-release | Pass | `v0.2.0-rc.2` is published, signed, and public. |
| RC2 PR merge | Pass | PR #2 merged into `main` by `8c243bb62fd627421e914ccabc4d6caf8daf205a`. |
| Post-merge quality gate | Pass | `NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh` passed on `main`. |
| Public install validation | Pass | `NEXUS_PUBLIC_RELEASE_TAG=v0.2.0-rc.2 ./scripts/validate-public-release-install.sh` passed. |
| Latest stable release | Stable unchanged | `v0.1.1` remains stable/latest. |
| Current source version | RC | `0.2.0-rc.2`. |

## Risk Triage

| Area | Stable impact | Decision |
| --- | --- | --- |
| Core language, CLI, diagnostics, runtime | Low for RC2 scope | Keep as stable candidate surface. |
| Release packaging and signatures | Low | Already strong enough for stable preflight. |
| GitHub Actions Node.js 20 warnings | Medium | Harden before stable by moving first-party actions to Node 24 compatible refs pinned by commit SHA. |
| Package registry | Medium/high | Keep documented as MVP; do not imply remote registry support in `0.2.0`. |
| SQLite physical schema | Medium/high | Keep documented as behavioral parity only; do not promise stable raw schema. |
| LSP/editor tooling | Medium | Keep positioned as MVP; rename/format/code actions are future work. |
| Hosted playground | Medium | Keep positioned as packaged/local playground, not hosted public product. |

## Stable 0.2.0 Entry Criteria

| Gate | Required before `v0.2.0` |
| --- | --- |
| Worktree | Clean stable branch created from updated `main`. |
| CI | GitHub Actions green on the stable branch/head after Node 24 action hardening with pinned action SHAs. |
| Local quality | `NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh` passes. |
| Package | `./scripts/package-release.sh` and `./scripts/validate-release-package.sh` pass for `0.2.0`. |
| Strict release dry-run | `NEXUS_RELEASE_SIGNING_KEY=<key> ./scripts/release-dry-run-strict.sh` passes on pushed stable branch. |
| Release notes | `RELEASE_NOTES.md` has a final `0.2.0` section that clearly carries forward known limits. |
| Public release | `v0.2.0` tag/release is published only after the gates above. |
| Post-release validation | `NEXUS_PUBLIC_RELEASE_TAG=v0.2.0 ./scripts/validate-public-release-install.sh` passes. |

## Hardening Scope Selected

1. Move first-party GitHub Actions from Node 20-era majors to Node 24 compatible refs pinned by commit SHA.
2. Keep source version and public release state on `0.2.0-rc.2`.
3. Preserve the known limits for registry, SQLite physical schema, LSP, and hosted playground.
4. Prepare the next phase as a controlled stable release branch only after this hardening PR is reviewed and merged.

## References Checked

- `actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd`: `v6`
- `actions/setup-node@48b55a011bda9f5d6aeb4c2d9c7362e8dae4041e`: `v6`
- `actions/setup-python@a309ff8b426b58ec0e2a45f0f869d46889d02405`: `v6`
- `actions/upload-artifact@b7c566a772e6b6bfb58ed0dc250532a479d7789f`: `v6`
