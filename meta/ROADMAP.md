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

A linha atual esta em estabilizacao de release/producao. O PR
`https://github.com/vitaleevo/NEXUSLANG/pull/1` foi mergeado em `main` pelo
commit `bcedf2c feat(release): prepare NexusLang 0.2.0-rc.1`, e o `main`
pos-merge passou no gate completo com clippy estrito. O pre-release publico
`v0.2.0-rc.1` continua publicado, instalavel e validado, mas representa uma fase
historica anterior as correcoes pos-publicacao agora presentes em `main`.
O foco imediato e publicar um novo artefato/tag RC pos-merge (`v0.2.0-rc.2`
recomendado) ou aprovar explicitamente um plano de promocao para `0.2.0`
estavel. A release stable/latest continua sendo `v0.1.1`.

## Trilhas proximas

1. Release/producao: preparar novo RC pos-merge (`v0.2.0-rc.2`) ou plano
   controlado de `0.2.0` estavel a partir do `main` validado.
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
