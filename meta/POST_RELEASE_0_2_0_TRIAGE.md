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
| Runtime HTTP/ERP | 82% | Rotas, models, auth, storage, OpenAPI e smokes estao verdes. SQLite fisico ainda precisa plano de schema/migracoes antes de producao forte com dados persistentes. |
| LSP/editor | 68% | Diagnostics multi-file, go-to-definition cross-file, semantic tokens e document symbols MVP existem. Ainda faltam rename, formatting, workspace symbols e code actions. |
| Package/release | 91% | Stable `v0.2.0`, tag assinada, assets assinados, checksum, strict dry-run e install publico passaram. O package manager agora baixa pacotes de registry read-only configurado. |
| Producao real | 75% | Distribuicao publica e package install remoto MVP estao fortes; faltam SQLite/migracoes e hardening operacional para uso de longo prazo. |
| Playground/public demo | 70% | WASM empacotado e validado, mas o playground ainda nao esta hospedado como experiencia publica continua. |
| Ecossistema/registry | 60% | Registry read-only MVP esta em `main` com metadata, download/cache, checksum opcional, extracao segura e imports instalados. Ainda nao ha HTTPS, publish/auth, assinaturas, transitivas ou solver completo. |

## Trilhas candidatas

| Trilha | Valor | Risco | Decisao |
| --- | --- | --- | --- |
| Registry remoto MVP read-only | Alto | Medio | Concluida nas Fases 11.66/11.67 e mergeada em `main` pelo PR #5. |
| SQLite/migracoes | Alto | Medio/Alto | Escolhida para Fase 11.68; precisa design de schema, introspeccao e migracoes sem quebrar dados. |
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

Fases 11.66/11.67 concluiram o registry remoto MVP read-only e o PR #5 foi
mergeado em `main` pelo commit `637065994c04cc211a00297b6ea64d7c75be6bf7`.
O proximo passo operacional passou a ser SQLite/migracoes MVP.

AVISO: O proximo passo e criar/implementar Fase 11.68 - SQLite/migracoes MVP com introspeccao de schema, plano/dry-run de migracoes e testes de compatibilidade entre JSON/SQLite, sem alterar dados existentes nem prometer ORM completo. Antes de iniciar, leia `MEMORIA_NEXUSLANG.md`, `meta/CURRENT_TASKS.md`, `meta/POST_RELEASE_0_2_0_TRIAGE.md`, `nexuslang-src/src/server/sqlite.rs`, `nexuslang-src/src/server/storage_backend.rs` e `nexuslang-src/tests/core.rs` para continuar exatamente de onde o projeto parou, entender o que ja foi feito e integrar a solucao com o sistema atual sem reler todo o repositorio.
