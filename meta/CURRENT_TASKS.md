# CURRENT_TASKS.md - Tarefas atuais

Este arquivo registra o foco imediato para continuar sem reler todo o
repositorio.

## Status atual

Fase 11.54 concluida em 2026-05-28: a branch
`codex/prepare-nexuslang-0.2.0-rc` foi pushada para `origin`, o PR draft
`https://github.com/vitaleevo/NEXUSLANG/pull/1` foi criado, o CI remoto
`NexusLang Quality Gate` passou e o strict public-release dry-run passou com
chave GPG mantida. Nenhuma tag, merge ou release publica foi criada ainda.

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
```

Resultado: PASS para LSP, quality gate, package-release, validate-release-package,
diff check, varredura de marcadores, push da branch e criacao do PR draft.
CI remoto PASS. Strict preflight PASS. Strict dry-run PASS. O `gh` no
PowerShell ainda nao esta autenticado, mas o `gh` no WSL esta autenticado e foi
usado pelo strict flow.

## Proxima fase recomendada

Fase 11.55: preparar tag/release draft ou pre-release do RC `0.2.0-rc.1`.
Confirmar o head validado, criar tag `v0.2.0-rc.1`, anexar pacote/checksum/
assinaturas e publicar como draft/pre-release conforme politica do projeto.

## Arquivos para abrir primeiro na proxima fase

- `MEMORIA_NEXUSLANG.md`
- `meta/CURRENT_TASKS.md`
- `RELEASE_NOTES.md`
- `scripts/release-dry-run-strict.sh`
- `scripts/sign-release-artifacts.sh`
- `GITHUB_RELEASE.md`

## Riscos de compatibilidade

- Nao publicar release estavel; este alvo e RC/pre-release.
- Nao prometer registry remoto real enquanto `PACKAGE_MANAGER.md` ainda o
  define como contrato futuro.
- `0.2.0-rc.1` continua RC em PR draft ate decisao de tag/release.
