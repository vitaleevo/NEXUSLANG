# CURRENT_TASKS.md - Tarefas atuais

Este arquivo registra o foco imediato para continuar sem reler todo o
repositorio.

## Status atual

Fase 11.49 concluida em 2026-05-28: a triagem de release/producao para o
proximo RC foi registrada em `meta/RELEASE_RC_TRIAGE.md`. O checkout local
continua bloqueado para RC publico porque tem 84 entradas pendentes, sendo 34
arquivos modificados e 50 untracked.

## Tarefas concluidas

- [x] Ler `MEMORIA_NEXUSLANG.md`, `RELEASE.md`, `VERSIONING.md`,
  `PACKAGE_MANAGER.md`, `scripts/release-dry-run-strict.sh` e
  `scripts/package-release.sh`.
- [x] Confirmar branch `main`, HEAD `bf37ed4`, remote GitHub e versao local
  `0.1.1`.
- [x] Inventariar worktree: 84 entradas pendentes, 34 modificadas e 50
  untracked.
- [x] Agrupar mudancas por escopo: docs/memoria, contratos, LSP,
  core/checker/HIR, runtime/auth/storage/OpenAPI, package manager/stdlib,
  CLI/test runner, playground/WASM e release scripts.
- [x] Registrar bloqueadores e sequencia de preparacao no arquivo
  `meta/RELEASE_RC_TRIAGE.md`.
- [x] Confirmar que strict preflight exige worktree limpo antes de RC publico.
- [x] Nao apagar, reverter, stagear ou commitar mudancas locais.

## Validacao executada

```bash
cd /home/alexandre/Nesusang
git status --short
git diff --check
```

Resultado: PASS para `git diff --check`. `git status --short` confirmou o
bloqueio de release: 84 entradas pendentes. A quality gate da fase 11.48 segue
sendo a ultima validacao tecnica ampla; nesta fase a validacao foi de estado
Git/docs.

## Proxima fase recomendada

Fase 11.50: preparar branch e commits do RC por escopo, sem descartar
alteracoes locais: criar branch `codex/prepare-nexuslang-0.2.0-rc`, revisar
staging por blocos, decidir versao alvo e atualizar release notes antes do
package/preflight.

## Arquivos para abrir primeiro na proxima fase

- `MEMORIA_NEXUSLANG.md`
- `MEMORY.md`
- `meta/CURRENT_TASKS.md`
- `meta/RELEASE_RC_TRIAGE.md`
- `RELEASE_NOTES.md`
- `VERSIONING.md`
- `nexuslang-src/Cargo.toml`
- `scripts/package-release.sh`
- `scripts/validate-release-package.sh`

## Riscos de compatibilidade

- Nao descartar mudancas locais nao revisadas.
- Nao cortar release com worktree sujo.
- Nao rodar strict public-release preflight enquanto HEAD nao estiver limpo,
  pushado e com CI verde.
- Nao prometer registry remoto real enquanto `PACKAGE_MANAGER.md` ainda o
  define como contrato futuro.
- A triagem recomenda `0.2.0-rc.1`/`0.2.0` se o RC incluir LSP, stdlib,
  package manager expandido e novas superficies de runtime/tooling.
