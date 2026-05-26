# NexusLang Artifact Signing

NexusLang currently produces a SHA-256 checksum for each local release archive.
This document defines the signing path for public artifacts.

## Current Integrity Check

Every package build creates:

```text
dist/nexuslang-v<version>-local-release.tar.gz
dist/nexuslang-v<version>-local-release.tar.gz.sha256
```

Validate the checksum before extracting:

```bash
sha256sum -c dist/nexuslang-v<version>-local-release.tar.gz.sha256
```

## Signing Tool

Use:

```bash
./scripts/sign-release-artifacts.sh
```

This is a publisher/source-tree step. Ordinary package users only need to
verify published signatures and checksums.

The script signs both the archive and its `.sha256` file with detached ASCII
GPG signatures:

```text
dist/nexuslang-v<version>-local-release.tar.gz.asc
dist/nexuslang-v<version>-local-release.tar.gz.sha256.asc
```

To choose a specific signing key:

```bash
NEXUS_RELEASE_SIGNING_KEY="<fingerprint-or-key-id>" ./scripts/sign-release-artifacts.sh
```

The script verifies the generated signatures before exiting.

For the strict public-release dry-run, the signing key is mandatory:

```bash
NEXUS_RELEASE_SIGNING_KEY="<fingerprint-or-key-id>" ./scripts/release-dry-run-strict.sh
```

Strict mode rejects ephemeral dry-run keys and requires the current commit to
have successful GitHub Actions before signing.

## Verify A Signed Release

With the public release key imported:

```bash
gpg --verify nexuslang-v<version>-local-release.tar.gz.asc nexuslang-v<version>-local-release.tar.gz
gpg --verify nexuslang-v<version>-local-release.tar.gz.sha256.asc nexuslang-v<version>-local-release.tar.gz.sha256
sha256sum -c nexuslang-v<version>-local-release.tar.gz.sha256
```

## Key Policy

- Private keys must never be committed to the repository.
- Public signing keys and fingerprints should be published with public release
  notes when NexusLang moves beyond local/internal releases.
- CI signing should use repository secrets or a dedicated release environment,
  not plaintext keys in workflow files.
- A public release is not considered fully signed unless the archive, checksum,
  source tag, and release notes all identify the same version.

## Current Status

The `0.1.0` strict release dry-run passed with a maintained local release key:

```text
3237F7CC5CE2514FC9671BB93CB6808B55385273
```

The public key was exported locally to:

```text
dist/nexuslang-release-public-key.asc
```

The final local dry-run script can still create ephemeral dry-run signatures
when no maintained key is present; those signatures are only for mechanical
testing and are not public release signatures.

The current source checkout is preparing `0.1.1`; public `0.1.1` signatures
should use the same maintained release key unless the release notes explicitly
document a rotation.
