# CURRENT_TASKS.md - Tarefas atuais

Este arquivo registra o foco imediato para continuar sem reler todo o
repositorio.

## Status atual

Fase 11.62 concluida em 2026-05-28: foi decidido nao promover `0.2.0` stable
imediatamente; o caminho escolhido foi hardening pre-stable curto. O PR #3
documentou a decisao em `meta/STABLE_0_2_0_DECISION.md`, moveu o CI para
actions Node 24 pinadas por SHA, passou com duas jobs `quality` e CodeRabbit
verdes, foi mergeado em `main` por
`e86d3c4121914d75d6736e29f5e842929dcd39f9` e teve quality gate local e CI
remoto pos-merge verdes. `v0.1.1` continua stable/latest; `v0.2.0-rc.2`
continua como public pre-release.

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
- [x] Reconfirmar checks finais do PR #1 no head `a8ee64a`.
- [x] Confirmar que threads ainda abertos eram antigos/`isOutdated=true` e sem
  review bloqueante atual.
- [x] Mergear PR #1 em `main`.
- [x] Atualizar `main` local para `bcedf2c`.
- [x] Rodar `NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh` em `main`.
- [x] Rodar `NEXUS_PUBLIC_RELEASE_TAG=v0.2.0-rc.1 ./scripts/validate-public-release-install.sh`.
- [x] Confirmar `v0.2.0-rc.1` como pre-release publico e `v0.1.1` como stable/latest.
- [x] Criar branch `codex/prepare-nexuslang-0.2.0-rc.2`.
- [x] Atualizar `nexuslang-src` e `nexus-lsp` para `0.2.0-rc.2`.
- [x] Atualizar README, release notes, versioning e release docs para RC2.
- [x] Rodar `NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh` em RC2.
- [x] Gerar e validar pacote local `nexuslang-v0.2.0-rc.2-local-release.tar.gz`.
- [x] Pushar branch RC2 e observar GitHub Actions verde.
- [x] Rodar strict release preflight/dry-run com chave mantida.
- [x] Criar e pushar tag assinada `v0.2.0-rc.2`.
- [x] Publicar GitHub Release `v0.2.0-rc.2` como pre-release, nao latest.
- [x] Rodar validacao publica de install contra `v0.2.0-rc.2`.
- [x] Criar PR #2 para mergear RC2 em `main`.
- [x] Corrigir feedback acionavel do CodeRabbit no PR #2 em `4403e94`.
- [x] Observar checks finais do PR #2: duas jobs `quality` PASS e CodeRabbit
  PASS.
- [x] Mergear PR #2 em `main`.
- [x] Atualizar `main` local para `8c243bb`.
- [x] Rodar `NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh` em `main` apos o
  merge.
- [x] Rodar install publico do `v0.2.0-rc.2` apos o merge.
- [x] Decidir explicitamente que `0.2.0` stable nao sera publicado antes de
  hardening pre-stable.
- [x] Criar `meta/STABLE_0_2_0_DECISION.md` com riscos e gates de stable.
- [x] Atualizar GitHub Actions para refs `v6` Node 24 pinadas por SHA.
- [x] Criar PR #3 para decisao/hardening de stable.
- [x] Corrigir feedback/nitpick valido do CodeRabbit no PR #3.
- [x] Observar checks finais do PR #3: duas jobs `quality` PASS e CodeRabbit
  PASS.
- [x] Mergear PR #3 em `main`.
- [x] Rodar quality gate local e observar CI remoto pos-merge em `main`.

## Validacao executada

```bash
cd <repo-root>/nexuslang-src
CARGO_TARGET_DIR=/tmp/nexuslang-target-codex cargo check -p nexus-lsp
CARGO_TARGET_DIR=/tmp/nexuslang-target-codex cargo test -p nexus-lsp
CARGO_TARGET_DIR=/tmp/nexuslang-target-codex cargo clippy -p nexus-lsp -- -D warnings

cd <repo-root>
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
gh pr view 1 -R vitaleevo/NEXUSLANG --json number,state,mergedAt,mergeCommit,url,title
git fetch origin main
git switch main
git pull --ff-only origin main
NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh
NEXUS_PUBLIC_RELEASE_TAG=v0.2.0-rc.1 ./scripts/validate-public-release-install.sh
gh release view v0.2.0-rc.1 -R vitaleevo/NEXUSLANG --json tagName,isDraft,isPrerelease,publishedAt,url,targetCommitish
gh release view -R vitaleevo/NEXUSLANG --json tagName,isDraft,isPrerelease,publishedAt,url,targetCommitish
git switch -c codex/prepare-nexuslang-0.2.0-rc.2
CARGO_TARGET_DIR=/tmp/nexuslang-target-codex cargo check --workspace --all-targets
NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh
./scripts/package-release.sh
./scripts/validate-release-package.sh
git push -u origin codex/prepare-nexuslang-0.2.0-rc.2
gh run watch 26595258834 -R vitaleevo/NEXUSLANG --exit-status --interval 10
NEXUS_RELEASE_SIGNING_KEY=3237F7CC5CE2514FC9671BB93CB6808B55385273 ./scripts/release-dry-run-strict.sh --preflight-only
NEXUS_RELEASE_SIGNING_KEY=3237F7CC5CE2514FC9671BB93CB6808B55385273 ./scripts/release-dry-run-strict.sh
git tag -u 3CB6808B55385273 -m NexusLang-0.2.0-rc.2 v0.2.0-rc.2 HEAD
git tag -v v0.2.0-rc.2
git push origin refs/tags/v0.2.0-rc.2
gh release create v0.2.0-rc.2 -R vitaleevo/NEXUSLANG --title 'NexusLang 0.2.0-rc.2' --notes-file RELEASE_NOTES.md --prerelease --latest=false --verify-tag ...
NEXUS_PUBLIC_RELEASE_TAG=v0.2.0-rc.2 ./scripts/validate-public-release-install.sh
GitHub connector: create PR #2
GitHub/API: observe checks for 4403e94122e7d85deb3e562732cec327c956683f
GitHub connector: merge PR #2
git fetch origin main
git switch main
git pull --ff-only origin main
NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh
NEXUS_PUBLIC_RELEASE_TAG=v0.2.0-rc.2 ./scripts/validate-public-release-install.sh
git ls-remote https://github.com/actions/checkout.git refs/tags/v6
git ls-remote https://github.com/actions/setup-node.git refs/tags/v6
git ls-remote https://github.com/actions/setup-python.git refs/tags/v6
git ls-remote https://github.com/actions/upload-artifact.git refs/tags/v6
git push -u origin codex/stable-0.2.0-hardening-decision
gh pr checks 3 -R vitaleevo/NEXUSLANG --watch --interval 10 --fail-fast
GitHub connector: merge PR #3
git fetch origin main
git switch main
git pull --ff-only origin main
git diff --check
NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh
gh run watch 26598079182 -R vitaleevo/NEXUSLANG --exit-status --interval 10
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
`meta/ROADMAP.md`. Pos-merge, PR #1 esta `MERGED`, `main` esta em
`bcedf2c1d8ef37c4afdf014a99e79fa8d8757e64`, o quality gate completo passou de
novo em `main`, a validacao publica de install do `v0.2.0-rc.1` passou, o
pre-release segue `isDraft=false`/`isPrerelease=true`, e a release latest
estavel continua `v0.1.1`. Para RC2, o commit validado e
`5561a2484e7f5082b9d339f94b02ee5dd8d77be0`, a Actions remota
`26595258834` passou, strict preflight/dry-run passou, a tag assinada
`v0.2.0-rc.2` foi publicada, o GitHub Release publico esta em
`https://github.com/vitaleevo/NEXUSLANG/releases/tag/v0.2.0-rc.2`, e a
validacao publica de install passou. Archive publico RC2:
`8ed601c2751e86ca84c40cbbd0edec9b4f1266d3663299fd83e8b2b4912eea0b`,
1590587 bytes; WASM: 479717 bytes. PR #2 foi criado, feedback valido do
CodeRabbit foi corrigido em `4403e94`, checks finais do PR passaram, e o PR foi
mergeado em `main` por `8c243bb62fd627421e914ccabc4d6caf8daf205a`. Pos-merge,
o quality gate completo e o install publico do `v0.2.0-rc.2` passaram de novo.
Na Fase 11.62, o stable `0.2.0` foi explicitamente adiado para hardening
pre-stable, o PR #3 mergeou a decisao e CI Node 24 pinado por SHA em `main`,
e o quality gate local mais CI remoto pos-merge passaram.

## Proxima fase recomendada

Fase 11.63: criar branch controlada de stable `0.2.0`, trocar a versao fonte,
preparar release notes finais, rodar package validation e strict public-release
dry-run antes de qualquer tag/release estavel.

## Arquivos para abrir primeiro na proxima fase

- `MEMORIA_NEXUSLANG.md`
- `meta/CURRENT_TASKS.md`
- `meta/STABLE_0_2_0_DECISION.md`
- `RELEASE_NOTES.md`
- `GITHUB_RELEASE.md`
- `scripts/release-dry-run-strict.sh`
- Release publico `https://github.com/vitaleevo/NEXUSLANG/releases/tag/v0.2.0-rc.2`

## Riscos de compatibilidade

- Nao promover para release estavel; este alvo continua RC/pre-release.
- Nao prometer registry remoto real enquanto `PACKAGE_MANAGER.md` ainda o
  define como contrato futuro.
- O PR #2 ja foi mergeado e `main` esta alinhado ao RC2; nao promover stable
  sem decisao explicita de risco/producao.
- O hardening de Actions Node 24 ja foi mergeado; se forem usados runners
  self-hosted no futuro, manter Actions Runner `v2.327.1+`.
- Nao publicar `v0.2.0` sem strict dry-run, assinatura e validacao publica de
  install do artefato stable.
