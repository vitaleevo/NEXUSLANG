# ROADMAP.md - Visao resumida

Este arquivo resume a direcao do projeto. O roadmap detalhado do core vive em
`nexuslang-src/ROADMAP.md`; a continuidade operacional vive em
`MEMORIA_NEXUSLANG.md`.

## Norte do produto

NexusLang deve ser uma linguagem ERP-first para workflows, modelos, rotas,
auth, invoices, money, storage, tooling e pequenos servicos de negocio.

## Baseline atual

- CLI Rust `nexus` com `run`, `check`, `tokens`, `ast`, `docs` e `test`.
- Parser, checker semantico, interpreter, runtime HTTP e diagnostics
  estruturados.
- Primitivas ERP: `model`, `workflow`, `route`, `auth`, `invoice`, `money`.
- Storage JSON/SQLite e contratos de operacoes estaticas para model/auth.
- Package manager MVP com `nexus.toml`, `nexus.lock`, dependencias de caminho
  e registry read-only via `NEXUS_REGISTRY_URL`, incluindo metadata, checksum,
  extracao segura e cache local para imports instalados.
- Playground WebAssembly e crate inicial `nexus-lsp` para integracao com
  editores, agora com nucleo testavel em `src/lib.rs` e adapter fino em
  `src/main.rs`, incluindo diagnostics multi-file opt-in via `SourceDatabase`
  quando o snapshot aberto corresponde ao disco, alem de limpeza de diagnostics
  stale quando o grafo muda, ha fallback sujo ou um documento e fechado, e
  go-to-definition cross-file para imports/aliases em grafos disk-backed e
  semantic tokens full-document lexicais, alem de document symbols MVP para o
  documento atual com filhos ERP aninhados.

## Foco imediato

A release stable `v0.2.0` ja foi publicada com tag assinada, GitHub Release,
assets assinados, checksum, strict public-release dry-run e validacao publica
de install. As Fases 11.66/11.67 implementaram, revisaram e mergearam o
package registry remoto MVP read-only em `main` pelo PR #5. As Fases
11.68/11.69 implementaram, revisaram e mergearam o SQLite/migracoes MVP em
`main` pelo PR #6, com `storage-plan`, introspeccao de schema, dry-run sem criar
banco novo, aplicacao segura de DDL, blockers conservadores e CI verde. As
Fases 11.70/11.71 implementaram, revisaram e mergearam o historico/ledger de
migracoes SQLite e o smoke operacional SQLite de backup/restore. As Fases
11.72/11.73 implementaram, revisaram e mergearam o export/import operacional de
dados JSON/SQLite pelo PR #8, com archive logico `nexus.storage.export.v1`,
import replace-only e validacao pos-merge. O foco imediato passa a ser
observabilidade operacional basica de runtime/storage antes de producao pesada
ou outra trilha.

## Trilhas proximas

1. Observabilidade operacional: logs/metricas/health de runtime e storage, com
   smokes ampliados para datasets maiores.
2. Registry hardening posterior: avaliar HTTPS, assinaturas, dependencias
   transitivas, publish/auth e solver em fases separadas.
3. LSP/editor tooling: adicionar workspace symbols, formatting, rename ou code
   actions apenas em fases separadas.
4. Diagnostics/tooling API: preservar JSON v1 enquanto melhora APIs internas
   para consumidores de editor.
5. HIR/checker: continuar migracoes pequenas para contratos HIR tipados,
   mantendo compatibilidade de mensagens.
6. Playground/docs: demonstrar apenas recursos suportados e manter exemplos
   executaveis.
7. Release hardening: smoke tests, contratos publicos, instalacao e artefatos
   assinados.

## Nao objetivos atuais

- Registry remoto com publish, auth, solver completo ou hospedagem central.
- Mudanca ampla de sintaxe 1.0 sem fase propria.
- Parser recovery completo.
- Byte ranges completos em todos os diagnostics.
- Reescrita do runtime ou do checker.
