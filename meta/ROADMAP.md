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

A release stable `v0.2.0` ja foi publicada com tag assinada, GitHub Release,
assets assinados, checksum, strict public-release dry-run e validacao publica
de install. A triagem pos-release confirmou que nao ha PRs nem issues abertas
e que a `main` esta com CI verde. A proxima linha de produto escolhida e
package registry remoto MVP read-only, porque o package manager ja tem contrato
`registry:<pacote>@<versao>`, mas ainda nao baixa dependencias remotas.

## Trilhas proximas

1. Package registry remoto MVP: implementar leitura/download de dependencias
   `registry:<pacote>@<versao>` com contrato de metadata, cache seguro,
   checksum e testes, sem publish/auth/solver completo nesta fase.
2. SQLite/migracoes: desenhar introspeccao de schema, dry-run e migracoes sem
   quebrar dados persistidos.
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
