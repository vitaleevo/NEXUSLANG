# CURRENT_TASKS.md - Tarefas atuais

Este arquivo registra o foco imediato para continuar sem reler todo o
repositorio.

## Status atual

Fase 11.52 concluida em 2026-05-28: a branch
`codex/prepare-nexuslang-0.2.0-rc` foi pushada para `origin`, mas PR/CI ainda
nao foram criados/observados porque `gh` nao esta autenticado nesta maquina.
O RC local `0.2.0-rc.1` ja passou quality gate, package-release e
validate-release-package.

## Tarefas concluidas

- [x] Confirmar worktree limpo na branch `codex/prepare-nexuslang-0.2.0-rc`.
- [x] Validar LSP com check, test e clippy estrito.
- [x] Rodar `NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh`.
- [x] Gerar pacote local `nexuslang-v0.2.0-rc.1-local-release.tar.gz`.
- [x] Validar pacote em diretorio limpo com `./scripts/validate-release-package.sh`.
- [x] Pushar branch:
  `git push -u origin codex/prepare-nexuslang-0.2.0-rc`.
- [x] Registrar link de PR retornado pelo GitHub:
  `https://github.com/vitaleevo/NEXUSLANG/pull/new/codex/prepare-nexuslang-0.2.0-rc`.
- [x] Confirmar bloqueio de PR/CI automatico: `gh auth status` falha por falta
  de login.

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
```

Resultado: PASS para LSP, quality gate, package-release, validate-release-package,
diff check, varredura de marcadores e push da branch. `gh auth status` falhou
porque nao ha sessao GitHub CLI autenticada; por isso PR/CI ficam para a
proxima fase.

## Proxima fase recomendada

Fase 11.53: criar PR/observar CI do RC `0.2.0-rc.1`. Autenticar `gh` com
`gh auth login` ou abrir o link de PR manualmente, acompanhar checks remotos
ate CI verde e depois rodar strict public-release preflight com chave mantida.

## Arquivos para abrir primeiro na proxima fase

- `MEMORIA_NEXUSLANG.md`
- `meta/CURRENT_TASKS.md`
- `RELEASE_NOTES.md`
- `scripts/release-dry-run-strict.sh`
- `GITHUB_RELEASE.md`

## Riscos de compatibilidade

- Nao publicar tag/release antes de PR/CI verde, strict preflight e assinatura.
- Nao prometer registry remoto real enquanto `PACKAGE_MANAGER.md` ainda o
  define como contrato futuro.
- `0.2.0-rc.1` continua RC local/remoto de branch ate PR/CI e publicacao.
