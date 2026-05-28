# CURRENT_TASKS.md - Tarefas atuais

Este arquivo registra o foco imediato para continuar sem reler todo o
repositorio.

## Status atual

Fase 11.55 concluida em 2026-05-28: a tag assinada `v0.2.0-rc.1` foi criada e
pushada para `origin`, apontando para o commit validado
`75c5ef8fb8b67494d741d3231965d81ba1ef33b7`. Um GitHub Release draft foi criado
para essa tag como pre-release, com pacote, checksum e assinaturas anexados.
O draft ainda nao foi publicado como pre-release publico; nenhum merge foi feito.

## Tarefas concluidas

- [x] Confirmar worktree limpo na branch `codex/prepare-nexuslang-0.2.0-rc`.
- [x] Validar LSP com check, test e clippy estrito.
- [x] Rodar `NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh`.
- [x] Gerar pacote local `nexuslang-v0.2.0-rc.1-local-release.tar.gz`.
- [x] Validar pacote em diretorio limpo com `./scripts/validate-release-package.sh`.
- [x] Pushar branch:
  `git push -u origin codex/prepare-nexuslang-0.2.0-rc`.
- [x] Criar PR draft para `main`:
  `https://github.com/vitaleevo/NEXUSLANG/pull/1`.
- [x] Observar CI remoto `NexusLang Quality Gate` verde no PR.
- [x] Confirmar chave GPG mantida
  `3237F7CC5CE2514FC9671BB93CB6808B55385273`.
- [x] Rodar strict public-release preflight.
- [x] Rodar strict public-release dry-run completo.
- [x] Confirmar que nenhuma tag/release foi publicada automaticamente.
- [x] Criar tag assinada `v0.2.0-rc.1` com a chave NexusLang.
- [x] Pushar `v0.2.0-rc.1` para `origin`.
- [x] Criar GitHub Release draft marcado como pre-release.
- [x] Anexar pacote, checksum e assinaturas ao draft.
- [x] Confirmar que `v0.2.0-rc.1` nao e a release `latest` e continua draft.

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
git push -u origin codex/prepare-nexuslang-0.2.0-rc
gh auth status
GitHub connector: create draft pull request
GitHub Actions: NexusLang Quality Gate
NEXUS_RELEASE_SIGNING_KEY=3237F7CC5CE2514FC9671BB93CB6808B55385273 ./scripts/release-dry-run-strict.sh --preflight-only
NEXUS_RELEASE_SIGNING_KEY=3237F7CC5CE2514FC9671BB93CB6808B55385273 ./scripts/release-dry-run-strict.sh
git tag -u 3CB6808B55385273 -m NexusLang-0.2.0-rc.1 v0.2.0-rc.1 HEAD
git push origin refs/tags/v0.2.0-rc.1
gh release create v0.2.0-rc.1 --draft --prerelease --verify-tag ...
gh release view v0.2.0-rc.1 --json tagName,isDraft,isPrerelease,assets
```

Resultado: PASS para LSP, quality gate, package-release, validate-release-package,
diff check, varredura de marcadores, push da branch e criacao do PR draft.
CI remoto PASS. Strict preflight PASS. Strict dry-run PASS. O `gh` no
PowerShell ainda nao esta autenticado, mas o `gh` no WSL esta autenticado e foi
usado pelo strict flow. Tag assinada PASS. Release draft/pre-release PASS.

## Proxima fase recomendada

Fase 11.56: revisar/publicar o pre-release do RC `0.2.0-rc.1` e validar o
install publico. Publicar o draft somente quando a revisao humana/operacional
estiver ok; depois rodar `NEXUS_PUBLIC_RELEASE_TAG=v0.2.0-rc.1
./scripts/validate-public-release-install.sh`.

## Arquivos para abrir primeiro na proxima fase

- `MEMORIA_NEXUSLANG.md`
- `meta/CURRENT_TASKS.md`
- `RELEASE_NOTES.md`
- `scripts/release-dry-run-strict.sh`
- `scripts/sign-release-artifacts.sh`
- `scripts/validate-public-release-install.sh`
- `GITHUB_RELEASE.md`

## Riscos de compatibilidade

- Nao publicar release estavel; este alvo continua RC/pre-release.
- Nao prometer registry remoto real enquanto `PACKAGE_MANAGER.md` ainda o
  define como contrato futuro.
- O draft ainda nao e publico; validacao de install publico so faz sentido
  depois de publicar o pre-release.
