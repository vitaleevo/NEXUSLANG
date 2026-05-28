# RELEASE_RC_TRIAGE.md - Triagem para proximo RC

Data: 2026-05-28

Objetivo: organizar o estado local pos-`v0.1.1` para preparar um proximo
release candidate sem descartar mudancas locais e sem cortar release a partir
de um worktree sujo.

## Snapshot do checkout

| Item | Valor |
|---|---|
| Branch | `main` |
| HEAD | `bf37ed4` |
| Remote | `https://github.com/vitaleevo/NEXUSLANG.git` |
| Versao atual em `Cargo.toml` | `0.1.1` |
| Entradas pendentes | 84 |
| Arquivos modificados | 34 |
| Arquivos untracked | 50 |
| Status de RC publico agora | Bloqueado |

Motivo do bloqueio: `scripts/release-dry-run-strict.sh` exige worktree limpo.
O checkout atual tem mudancas locais amplas, incluindo fontes, docs, LSP,
stdlib, scripts de release e WASM gerado.

## Agrupamento do worktree

| Escopo | Arquivos principais | Acao recomendada |
|---|---|---|
| Handoff/docs operacionais | `MEMORIA_NEXUSLANG.md`, `MEMORY.md`, `meta/*`, `README.md`, `ARCHITECTURE_AUDIT_NEXUSLANG.md` | Rastrear como pacote de documentacao e memoria do RC |
| Docs de contrato | `DIAGNOSTICS_JSON_CONTRACT.md`, `MODULES_IMPORTS_ARCHITECTURE.md`, `TYPED_HIR_ARCHITECTURE.md`, `AUTH_OPERATIONS.md`, `MODEL_OPERATIONS.md`, `docs/project-maturity-model.md` | Rastrear junto das APIs que documentam |
| LSP/editor | `nexuslang-src/nexus-lsp/*`, workspace `Cargo.toml`, `Cargo.lock` | Rastrear como feature de tooling; validar crate separada |
| Core/checker/HIR | `src/checker/*`, `src/hir.rs`, `src/route_hir.rs`, `src/diagnostics/*`, `tests/core.rs` | Revisar como pacote grande de arquitetura interna |
| Runtime HTTP/auth/storage/OpenAPI | `src/server/*`, `src/auth_ops.rs`, `src/model_ops.rs`, smokes HTTP/auth/storage | Rastrear com testes e docs de contrato |
| Package manager/stdlib | `PACKAGE_MANAGER.md`, `src/package_manager.rs`, `tests/cli_package_manager.rs`, `stdlib/*`, exemplos multi-modulo | Rastrear como escopo proprio; registry remoto continua fora |
| CLI/test runner | `src/main.rs`, `src/test_runner.rs`, `tests/cli.rs` | Rastrear junto do comportamento user-facing |
| Playground/WASM | `src/playground/mod.rs`, `web/nexuslang_playground.wasm` | Confirmar se WASM foi regenerado a partir do source atual |
| Release scripts | `scripts/package-release.sh`, `scripts/quality-gate.sh`, `scripts/validate-*.sh`, `scripts/smoke-auth.sh` | Revalidar cadeia de release antes de confiar em novo pacote |

## Blocos sugeridos para commits/RC

| Ordem | Bloco | Conteudo | Validacao minima |
|---:|---|---|---|
| 1 | Docs/memoria/meta | Handoff, roadmap curto, arquitetura curta, triagem RC | `git diff --check` |
| 2 | Core modular/HIR/diagnostics | Modularizacao checker, HIR, diagnostics, module loader | `cargo test`, `quality-gate.sh` |
| 3 | Runtime/auth/storage/OpenAPI | Servidor, auth nativo, storage, OpenAPI, smokes | `quality-gate.sh`, smokes HTTP/auth/storage |
| 4 | Package manager/stdlib | Manifest/lock/path deps, stdlib, exemplos multi-modulo | `cargo test`, package tests, CLI stdlib checks |
| 5 | LSP/editor | `nexus-lsp`, semantic tokens, document symbols | `cargo check/test/clippy -p nexus-lsp` |
| 6 | Release packaging | Scripts, package validation, docs publicas | `package-release.sh`, `validate-release-package.sh` |

## Decisao de versao

Recomendacao: tratar o proximo RC como `0.2.0-rc.1` ou preparar uma linha
`0.2.0`, nao `0.1.2`, porque o checkout local inclui superficie publica nova:

- crate LSP;
- package manager mais amplo;
- stdlib inicial;
- runtime/auth/storage/OpenAPI expandidos;
- APIs/documentos de diagnostics e module graph.

`0.1.2` so faria sentido se o RC fosse reduzido a fixes/documentacao sem
introduzir essas superficies.

## Bloqueadores para RC

| Bloqueador | Severidade | Resolucao |
|---|---|---|
| Worktree sujo | Alta | Criar branch de preparacao e separar commits por escopo |
| Muitos arquivos criticos untracked | Alta | Decidir rastreamento e adicionar ao Git conscientemente |
| Versao ainda `0.1.1` | Alta | Atualizar versao e release notes antes de pacote publico |
| WASM modificado | Media | Confirmar build/procedencia antes de incluir |
| Registry remoto ausente | Media | Manter como limite explicito; nao prometer ecossistema remoto |
| Strict preflight impossivel agora | Alta | So rodar depois de worktree limpo, HEAD pushado e CI verde |

## Sequencia recomendada

1. Criar uma branch de preparacao, por exemplo
   `codex/prepare-nexuslang-0.2.0-rc`.
2. Revisar `git status --short` e staged sets por bloco, sem misturar escopos.
3. Adicionar primeiro `nexuslang-src/nexus-lsp/`, `meta/`, docs de contrato e
   novos modulos que ja sao requeridos pelo build/testes.
4. Decidir versao alvo e atualizar `nexuslang-src/Cargo.toml`,
   `RELEASE_NOTES.md`, `VERSIONING.md` e docs de release.
5. Rodar:

```bash
NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh
./scripts/package-release.sh
./scripts/validate-release-package.sh
```

6. So depois de commit/push/CI verde, rodar:

```bash
NEXUS_RELEASE_SIGNING_KEY=<fingerprint> ./scripts/release-dry-run-strict.sh
```

## Nao fazer

- Nao usar `git reset --hard`.
- Nao apagar untracked para "limpar" sem revisar.
- Nao cortar release de `main` enquanto ha mudancas locais.
- Nao prometer remote registry antes de implementar downloads, publish, solver
  e verificacao por dependencia.
