# CURRENT_TASKS.md - Tarefas atuais

Este arquivo registra o foco imediato para continuar sem reler todo o
repositorio.

## Status atual

Fase 11.58 concluida em 2026-05-28: as correcoes pos-feedback foram commitadas
e pushadas, o PR #1 esta fora de draft, aberto, mergeable e com CI/CodeRabbit
verdes no head pos-feedback. O unico comentario novo foi documental em
`meta/ROADMAP.md` e foi corrigido nesta fase. Nenhum merge foi feito; `v0.1.1`
continua sendo a linha estavel/latest, e o `v0.2.0-rc.1` publico nao representa
as correcoes pos-publicacao do PR.

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
- [x] Observar CI verde no head atual do PR apos o handoff de draft release.
- [x] Publicar `v0.2.0-rc.1` como pre-release publico sem marcar como latest.
- [x] Corrigir assets ausentes para validacao publica:
  `nexuslang-release-public-key.asc` e
  `nexuslang-release-signing-key.fingerprint`.
- [x] Rodar install publico contra `v0.2.0-rc.1` com sucesso.
- [x] Marcar PR #1 como pronto para revisao (`isDraft=false`).
- [x] Revisar comentarios Codex/CodeRabbit do PR.
- [x] Corrigir feedback acionavel de module loader, checker/HIR, diagnostics,
  LSP, README e release docs.
- [x] Rodar `NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh` novamente com
  sucesso apos as correcoes.
- [x] Pushar commit `38b64e6 fix(rc): address automated review feedback`.
- [x] Observar CI remoto pos-feedback no PR #1: duas jobs `quality` PASS e
  CodeRabbit PASS.
- [x] Corrigir comentario documental residual do CodeRabbit em
  `meta/ROADMAP.md`, separando o RC publico historico do head atual do PR.

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
gh run watch 26586404432 --exit-status
gh release edit v0.2.0-rc.1 --draft=false --prerelease --latest=false --verify-tag
gh release upload v0.2.0-rc.1 dist/nexuslang-release-public-key.asc dist/nexuslang-release-signing-key.fingerprint
NEXUS_PUBLIC_RELEASE_TAG=v0.2.0-rc.1 ./scripts/validate-public-release-install.sh
gh pr ready 1
gh api repos/vitaleevo/NEXUSLANG/pulls/1/comments
sha256sum dist/nexuslang-v0.2.0-rc.1-local-release.tar.gz
CARGO_TARGET_DIR=/tmp/nexuslang-target-codex cargo test -p nexus-lsp
CARGO_TARGET_DIR=/tmp/nexuslang-target-codex cargo test -p nexuslang --test core -- --nocapture
CARGO_TARGET_DIR=/tmp/nexuslang-target-codex NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh
CARGO_TARGET_DIR=/tmp/nexuslang-target-codex cargo clippy --workspace --all-targets -- -D warnings
CARGO_TARGET_DIR=/tmp/nexuslang-target-codex cargo test --workspace --all-targets
gh pr checks 1 -R vitaleevo/NEXUSLANG --watch --interval 10 --fail-fast
gh pr view 1 -R vitaleevo/NEXUSLANG --json number,title,state,isDraft,url,headRefOid,mergeable,reviewDecision,statusCheckRollup,latestReviews,comments
```

Resultado: PASS para LSP, quality gate, package-release, validate-release-package,
diff check, varredura de marcadores, push da branch e criacao do PR draft.
CI remoto PASS. Strict preflight PASS. Strict dry-run PASS. O `gh` no
PowerShell ainda nao esta autenticado, mas o `gh` no WSL esta autenticado e foi
usado pelo strict flow. Tag assinada PASS. Release draft/pre-release PASS.
Publicacao do pre-release PASS. Install publico PASS depois de adicionar chave
publica e fingerprint como assets. O archive publico validado tem SHA-256
`3d1f376e81aa855c69db3da70674811098169d3aaec8d19cbf50fc36bcbe91d5` e
1582178 bytes. A revisao de feedback automatizado tambem passou localmente:
`nexus-lsp` 25/25, `core.rs` 266/266, lib 78/78, quality gate completo PASS,
Clippy workspace PASS e `cargo test --workspace --all-targets` PASS. CI remoto
pos-feedback PASS; CodeRabbit PASS com um comentario documental corrigido em
`meta/ROADMAP.md`.

## Proxima fase recomendada

Fase 11.59: decisao de merge e novo RC/pos-merge. Observar checks finais do PR
#1 apos a clarificacao documental de `meta/ROADMAP.md`, decidir merge com
criterios objetivos e preparar `v0.2.0-rc.2` ou validacao pos-merge antes de
qualquer `0.2.0` estavel.

## Arquivos para abrir primeiro na proxima fase

- `MEMORIA_NEXUSLANG.md`
- `meta/CURRENT_TASKS.md`
- `RELEASE_NOTES.md`
- `GITHUB_RELEASE.md`
- PR `https://github.com/vitaleevo/NEXUSLANG/pull/1`

## Riscos de compatibilidade

- Nao promover para release estavel; este alvo continua RC/pre-release.
- Nao prometer registry remoto real enquanto `PACKAGE_MANAGER.md` ainda o
  define como contrato futuro.
- O PR saiu de draft, mas ainda precisa de CI/CodeRabbit no novo head apos
  push antes de qualquer merge/main/`0.2.0` estavel.
- GitHub Actions avisou sobre futura migracao de actions Node.js 20 para
  Node.js 24; isso nao bloqueou o RC, mas deve entrar no hardening de CI.
