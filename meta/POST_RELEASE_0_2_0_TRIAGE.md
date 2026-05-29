# NexusLang 0.2.0 Post-Release Triage

Data: 2026-05-29

Release: https://github.com/vitaleevo/NEXUSLANG/releases/tag/v0.2.0

## Verificacao pos-release

| Checagem | Resultado | Evidencia |
| --- | --- | --- |
| Release stable publicada | PASS | `v0.2.0`, `isDraft=false`, `isPrerelease=false`, publicada em 2026-05-28T20:28:11Z |
| Latest release do GitHub | PASS | `gh release view` sem tag retorna `v0.2.0` |
| Assets essenciais | PASS | archive, checksum, assinaturas, chave publica e fingerprint anexados |
| Archive publico | PASS | `nexuslang-v0.2.0-local-release.tar.gz`, 1595212 bytes |
| Checksum do archive | PASS | `7979dc7ad2e24b81c0bf8bb126bebb8147a6feb289b234ee5c5b038b4d238950` |
| Chave de assinatura | PASS | fingerprint `3237F7CC5CE2514FC9671BB93CB6808B55385273` |
| Validacao publica de install | PASS | `NEXUS_PUBLIC_RELEASE_TAG=v0.2.0 ./scripts/validate-public-release-install.sh` passou na fase de publicacao |
| CI remoto da publicacao | PASS | run `26600083912` passou no commit final tagueado `a05bb74a663a4e2e7cc18dd4de7adb25e3f1faeb` |
| CI remoto da memoria/docs | PASS | run `26600474468` passou em `main` no commit `03ac46172170fd668b125e4d452bbfd620e250a5` |
| PRs abertas | PASS | 1 PR aberta (`#8` - 2026-05-29) |
| Issues abertas | PASS | nenhuma issue aberta |

## Diagnostico de progresso apos 0.2.0

| Area | Progresso | Estado |
| --- | ---: | --- |
| Core da linguagem | 86% | Parser, checker, interpreter, diagnostics, runtime e grafo multi-file estao solidos para MVP/stable inicial. Ainda falta maturar HIR tipado e recovery amplo. |
| CLI | 90% | `run`, `check`, `tokens`, `ast`, `docs`, `test`, package commands e flows de release funcionais. Falta polir UX de pacotes remotos e comandos editoriais. |
| Runtime HTTP/ERP | 92% | Rotas, models, auth, storage, OpenAPI e smokes estao verdes. SQLite/migracoes MVP e historico/ledger/smoke SQLite estao em `main`; export/import operacional JSON/SQLite esta implementado em branch e aguarda review/merge. |
| LSP/editor | 68% | Diagnostics multi-file, go-to-definition cross-file, semantic tokens e document symbols MVP existem. Ainda faltam rename, formatting, workspace symbols e code actions. |
| Package/release | 91% | Stable `v0.2.0`, tag assinada, assets assinados, checksum, strict dry-run e install publico passaram. O package manager agora baixa pacotes de registry read-only configurado. |
| Producao real | 88% | Distribuicao publica, package install remoto MVP, `storage-plan` SQLite, ledger e smoke SQLite estao em `main`; export/import operacional esta implementado em branch. Ainda faltam review/merge, observabilidade e hardening operacional para cargas criticas. |
| Playground/public demo | 70% | WASM empacotado e validado, mas o playground ainda nao esta hospedado como experiencia publica continua. |
| Ecossistema/registry | 60% | Registry read-only MVP esta em `main` com metadata, download/cache, checksum opcional, extracao segura e imports instalados. Ainda nao ha HTTPS, publish/auth, assinaturas, transitivas ou solver completo. |

## Trilhas candidatas

| Trilha | Valor | Risco | Decisao |
| --- | --- | --- | --- |
| Registry remoto MVP read-only | Alto | Medio | Concluida nas Fases 11.66/11.67 e mergeada em `main` pelo PR #5. |
| SQLite/migracoes | Alto | Medio/Alto | Fases 11.68-11.71 concluidas e mergeadas em `main`; SQLite tem plano/apply, ledger, idempotencia e smoke backup/restore. |
| Export/import operacional | Alto | Medio | Fase 11.72 implementada em branch com archive logico `nexus.storage.export.v1`, roundtrip JSON/SQLite e package validation; pendente de review/PR/CI/merge. |
| LSP editorial | Medio/Alto | Medio | Bom ganho de DX, mas menos critico que permitir consumo de pacotes remotos depois da stable. |
| Playground hospedado | Medio | Baixo/Medio | Importante para demonstracao publica, mas depende mais de infraestrutura do que do core do produto. |

## Escopo implementado em branch para Fase 11.72

Trilha: storage data tooling.

Objetivo: adicionar export/import operacional de dados para JSON/SQLite, com
contrato CLI minimo, archive logico portavel, roundtrip testado e compatibilidade
com ledger/migracoes SQLite sem iniciar observabilidade ou publish remoto.

Incluido:

- comandos `nexus storage-export` e `nexus storage-import`;
- formato `nexus.storage.export.v1` com `format`, `source_driver`, `models` e
  `auth`;
- import replace-only com `--replace` obrigatorio;
- export SQLite bloqueado quando o `storage-plan` nao esta limpo;
- import SQLite transacional que preserva `nexus_schema_migrations`;
- teste CLI de roundtrip JSON -> export -> SQLite -> export;
- docs de rollback/restore e politica de compatibilidade atualizadas.

Nao incluir nesta fase:

- solver de migracoes semanticas completo;
- rename/drop/type-change automatico;
- transformacao de payloads existentes;
- publish/auth/registry ou outra trilha;
- observabilidade, metricas ou logs operacionais;
- substituicao retroativa de assets da release `v0.2.0`.

## Gates antes de mergear a proxima fase

| Gate | Comando/evidencia |
| --- | --- |
| Qualidade local | `NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh` |
| Testes focados | teste CLI de `storage-export`/`storage-import` e teste CLI de `storage-plan` |
| Docs | `COMPATIBILITY.md` e `STORAGE_BACKUP_RESTORE.md` atualizados |
| Segurança de dados | import replace-only, SQLite transacional e ledger preservado fora do archive |
| Release safety | nenhuma tag/release alterada durante a fase |

## Proximo aviso

Fase 11.72 implementou em branch o export/import operacional de dados
JSON/SQLite. O proximo passo operacional passa a ser review/PR/CI/merge dessa
branch antes de iniciar observabilidade, publish remoto ou outra trilha.

AVISO: O proximo passo e criar/implementar Fase 11.73 - review/PR/CI/merge do export/import operacional de dados JSON/SQLite, com CI remoto verde e validacao pos-merge dos comandos `storage-export`, `storage-import`, `storage-plan` e package smoke antes de iniciar observabilidade, publish remoto ou outra trilha. Antes de iniciar, leia `MEMORIA_NEXUSLANG.md`, `meta/CURRENT_TASKS.md`, `COMPATIBILITY.md`, `STORAGE_BACKUP_RESTORE.md`, `nexuslang-src/src/main.rs`, `nexuslang-src/src/server/storage_backend.rs`, `nexuslang-src/src/server/json.rs`, `nexuslang-src/src/server/sqlite.rs` e `nexuslang-src/tests/cli.rs` para continuar exatamente de onde o projeto parou, entender o que ja foi feito e integrar a solucao com o sistema atual sem reler todo o repositorio.
