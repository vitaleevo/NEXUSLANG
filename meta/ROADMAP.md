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
- Package manager local MVP com `nexus.toml`, `nexus.lock` e dependencias de
  caminho.
- Playground WebAssembly e crate inicial `nexus-lsp` para integracao com
  editores, agora com nucleo testavel em `src/lib.rs` e adapter fino em
  `src/main.rs`, incluindo diagnostics multi-file opt-in via `SourceDatabase`
  quando o snapshot aberto corresponde ao disco, alem de limpeza de diagnostics
  stale quando o grafo muda, ha fallback sujo ou um documento e fechado, e
  go-to-definition cross-file para imports/aliases em grafos disk-backed e
  semantic tokens full-document lexicais, alem de document symbols MVP para o
  documento atual com filhos ERP aninhados.

## Foco imediato

A linha atual esta em estabilizacao de release/producao. O merge deste PR nao
publica tag/release: o pre-release publico `v0.2.0-rc.1` ja foi publicado em uma
fase anterior da trilha e passou validacao de install publico com assinatura,
checksum, package smoke, auth, storage e assets HTTP. O PR
`https://github.com/vitaleevo/NEXUSLANG/pull/1` saiu de draft, segue aberto e
mergeable, e agora contem correcoes pos-publicacao que nao pertencem ao tag nem
aos assets publicados de `v0.2.0-rc.1`. O foco imediato e concluir a revisao do
PR no head atual e, se o merge for aprovado, planejar um novo artefato/tag RC
ou uma validacao pos-merge explicita antes de qualquer `0.2.0` estavel.

## Trilhas proximas

1. Release/producao: concluir revisao do PR #1 no head pos-feedback, decidir
   merge e planejar novo RC/validacao pos-merge antes de `0.2.0` estavel.
2. Diagnostics/tooling API: preservar JSON v1 enquanto melhora APIs internas
   para consumidores de editor.
3. LSP/editor tooling: adicionar workspace symbols, formatting, rename ou code
   actions apenas em fases separadas.
4. HIR/checker: continuar migracoes pequenas para contratos HIR tipados,
   mantendo compatibilidade de mensagens.
5. Playground/docs: demonstrar apenas recursos suportados e manter exemplos
   executaveis.
6. Release hardening: smoke tests, contratos publicos, instalacao e artefatos
   assinados.

## Nao objetivos atuais

- Registry remoto real.
- Mudanca ampla de sintaxe 1.0 sem fase propria.
- Parser recovery completo.
- Byte ranges completos em todos os diagnostics.
- Reescrita do runtime ou do checker.
