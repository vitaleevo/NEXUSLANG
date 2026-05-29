# NexusLang 0.2.0 Post-Release Triage

Data: 2026-05-28

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
| PRs abertas | PASS | nenhuma PR aberta |
| Issues abertas | PASS | nenhuma issue aberta |

## Diagnostico de progresso apos 0.2.0

| Area | Progresso | Estado |
| --- | ---: | --- |
| Core da linguagem | 86% | Parser, checker, interpreter, diagnostics, runtime e grafo multi-file estao solidos para MVP/stable inicial. Ainda falta maturar HIR tipado e recovery amplo. |
| CLI | 90% | `run`, `check`, `tokens`, `ast`, `docs`, `test`, package commands e flows de release funcionais. Falta polir UX de pacotes remotos e comandos editoriais. |
| Runtime HTTP/ERP | 86% | Rotas, models, auth, storage, OpenAPI e smokes estao verdes. SQLite agora tem plano/dry-run de migracao conservador, mas ainda falta historico/versionamento de migracoes e smoke operacional SQLite amplo. |
| LSP/editor | 68% | Diagnostics multi-file, go-to-definition cross-file, semantic tokens e document symbols MVP existem. Ainda faltam rename, formatting, workspace symbols e code actions. |
| Package/release | 91% | Stable `v0.2.0`, tag assinada, assets assinados, checksum, strict dry-run e install publico passaram. O package manager agora baixa pacotes de registry read-only configurado. |
| Producao real | 80% | Distribuicao publica, package install remoto MVP e primeiro plano de migracoes SQLite estao fortes; faltam historico de migracoes, backup/restore SQLite amplo, observabilidade e hardening operacional. |
| Playground/public demo | 70% | WASM empacotado e validado, mas o playground ainda nao esta hospedado como experiencia publica continua. |
| Ecossistema/registry | 60% | Registry read-only MVP esta em `main` com metadata, download/cache, checksum opcional, extracao segura e imports instalados. Ainda nao ha HTTPS, publish/auth, assinaturas, transitivas ou solver completo. |

## Trilhas candidatas

| Trilha | Valor | Risco | Decisao |
| --- | --- | --- | --- |
| Registry remoto MVP read-only | Alto | Medio | Concluida nas Fases 11.66/11.67 e mergeada em `main` pelo PR #5. |
| SQLite/migracoes | Alto | Medio/Alto | Fase 11.68 implementada localmente com introspeccao, dry-run, aplicacao segura e blockers; falta review/merge/CI e hardening posterior. |
| LSP editorial | Medio/Alto | Medio | Bom ganho de DX, mas menos critico que permitir consumo de pacotes remotos depois da stable. |
| Playground hospedado | Medio | Baixo/Medio | Importante para demonstracao publica, mas depende mais de infraestrutura do que do core do produto. |

## Escopo recomendado para Fase 11.66

Trilha: package registry remoto MVP read-only.

Objetivo: permitir que o package manager baixe dependencias declaradas como
`registry:<pacote>@<versao>` a partir de um registry configuravel e fixture
local/remoto controlado, gravando lock/cache de forma deterministica e segura.

Incluir:

- contrato minimo de registry index/metadata;
- URL/base registry configuravel por ambiente ou manifest, com default seguro
  para testes;
- download de archive de dependencia declarada;
- verificacao de checksum quando o metadata fornecer hash;
- extracao segura para `.nexus/packages/<pacote>`;
- lockfile deterministico com origem, versao, checksum e caminho resolvido;
- testes de sucesso, checksum invalido, path traversal, metadata invalido e
  pacote inexistente;
- documentacao em `PACKAGE_MANAGER.md`.

Nao incluir nesta fase:

- comando de publish;
- autenticacao;
- solver semantico completo;
- dependencias transitivas;
- hospedagem de registry central;
- substituicao retroativa de assets da release `v0.2.0`.

## Gates antes de mergear a proxima fase

| Gate | Comando/evidencia |
| --- | --- |
| Qualidade local | `NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh` |
| Testes focados | `cargo test -p nexuslang --test cli_package_manager` |
| Docs | `PACKAGE_MANAGER.md` atualizado com limites e exemplos |
| Segurança de arquivo | testes cobrindo checksum e path traversal |
| Release safety | nenhuma tag/release alterada durante a fase |

## Proximo aviso

Fase 11.68 implementou localmente o SQLite/migracoes MVP em branch controlada,
com `storage-plan`, introspeccao e blockers conservadores. O proximo passo
operacional passou a ser review/PR/CI/merge dessa fase.

AVISO: O proximo passo e criar/implementar Fase 11.69 - review/PR/CI/merge do SQLite/migracoes MVP, com validacao remota do `storage-plan` e quality gate verde antes de iniciar historico/versionamento de migracoes ou outra trilha. Antes de iniciar, leia `MEMORIA_NEXUSLANG.md`, `meta/CURRENT_TASKS.md`, `COMPATIBILITY.md`, `STORAGE_BACKUP_RESTORE.md` e os arquivos alterados da branch `codex/sqlite-migrations-mvp` para continuar exatamente de onde o projeto parou, entender o que ja foi feito e integrar a solucao com o sistema atual sem reler todo o repositorio.
