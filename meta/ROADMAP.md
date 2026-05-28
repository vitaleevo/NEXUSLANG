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

A linha atual esta em estabilizacao de release/producao. O branch
`codex/prepare-nexuslang-0.2.0-rc` ja organiza o trabalho pos-`v0.1.1` em
commits por escopo para o RC local `0.2.0-rc.1`; a quality gate local ampla,
o package local e `validate-release-package.sh` passaram. O foco imediato e
push/PR/CI, seguido de strict public-release preflight com chave de assinatura.

## Trilhas proximas

1. Release/producao: pushar branch, abrir PR ou fluxo equivalente, observar CI
   verde e rodar strict public-release preflight antes de tag/publicacao.
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
