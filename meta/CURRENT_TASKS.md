# CURRENT_TASKS.md - Tarefas atuais

Este arquivo registra o foco imediato para continuar sem reler todo o
repositorio.

## Status atual

Fase 11.51 concluida em 2026-05-28: o RC local `0.2.0-rc.1` foi empacotado e
validado em diretorio limpo. A branch `codex/prepare-nexuslang-0.2.0-rc` tem
commits por escopo, quality gate local aprovada, pacote local gerado e
`validate-release-package.sh` aprovado. Ainda falta push/PR/CI e strict
public-release preflight antes de tag/publicacao.

## Tarefas concluidas

- [x] Criar branch `codex/prepare-nexuslang-0.2.0-rc`.
- [x] Atualizar linha local para `0.2.0-rc.1`, mantendo `v0.1.1` como release
  publica mais recente.
- [x] Separar commits por escopo:
  - `8ec9321 docs: prepare 0.2.0 rc handoff`
  - `71e1a3c refactor: modularize core diagnostics and checker`
  - `9fce40b feat: harden runtime auth storage and openapi`
  - `ac9f9ec feat: add local packages and stdlib workflows`
  - `bf49b7c feat: add NexusLang LSP adapter`
  - `9c2c606 feat: refresh playground wasm artifact`
  - `1fae863 build: tighten release packaging gates`
- [x] Validar LSP com check, test e clippy estrito.
- [x] Rodar quality gate ampla com clippy.
- [x] Gerar pacote local `nexuslang-v0.2.0-rc.1-local-release.tar.gz`.
- [x] Gerar checksum do pacote local.
- [x] Validar pacote em diretorio limpo com `validate-release-package.sh`.
- [x] Confirmar ausencia de marcadores pendentes reais fora dos diretorios
  ignorados.

## Validacao executada

```bash
cd /home/alexandre/Nesusang/nexuslang-src
CARGO_TARGET_DIR=/tmp/nexuslang-target-codex cargo check -p nexus-lsp
CARGO_TARGET_DIR=/tmp/nexuslang-target-codex cargo test -p nexus-lsp
CARGO_TARGET_DIR=/tmp/nexuslang-target-codex cargo clippy -p nexus-lsp -- -D warnings

cd /home/alexandre/Nesusang
NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh
./scripts/package-release.sh
./scripts/validate-release-package.sh
git diff --check
rg de marcadores pendentes no workspace, ignorando diretorios de build
```

Resultado: PASS. O LSP passou com 23 testes. A quality gate passou com fmt,
check all-targets com warnings como erro, clippy all-targets, testes Rust,
smokes HTTP/auth/storage, validacao OpenAPI e contratos. O pacote
`nexuslang-v0.2.0-rc.1-local-release.tar.gz` foi gerado com checksum `.sha256`
ao lado do artefato e validado em diretorio limpo.

## Proxima fase recomendada

Fase 11.52: push/PR/CI e strict public-release preflight do RC `0.2.0-rc.1`.
Pushar a branch, abrir PR ou fluxo equivalente, observar CI verde, e so depois
rodar `NEXUS_RELEASE_SIGNING_KEY=<fingerprint> ./scripts/release-dry-run-strict.sh`.

## Arquivos para abrir primeiro na proxima fase

- `MEMORIA_NEXUSLANG.md`
- `meta/CURRENT_TASKS.md`
- `RELEASE_NOTES.md`
- `VERSIONING.md`
- `scripts/release-dry-run-strict.sh`
- `scripts/sign-release-artifacts.sh`
- `GITHUB_RELEASE.md`

## Riscos de compatibilidade

- Nao publicar release antes de push, CI verde, strict preflight e assinatura.
- Nao prometer registry remoto real enquanto `PACKAGE_MANAGER.md` ainda o
  define como contrato futuro.
- `0.2.0-rc.1` e RC local ate publicacao; `v0.1.1` continua sendo a release
  publica validada.
- O strict preflight depende de chave de assinatura mantida e acesso GitHub/CI.
