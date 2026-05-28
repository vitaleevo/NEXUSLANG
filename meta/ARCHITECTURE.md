# ARCHITECTURE.md - Visao de arquitetura

Este arquivo e uma visao curta. Documentos mais detalhados existem em
`nexuslang-src/MODULES_IMPORTS_ARCHITECTURE.md`,
`nexuslang-src/TYPED_HIR_ARCHITECTURE.md`,
`nexuslang-src/DIAGNOSTICS_JSON_CONTRACT.md` e
`ARCHITECTURE_AUDIT_NEXUSLANG.md`.

## Camadas principais

- Raiz do projeto: docs operacionais, release notes, playground standalone e
  memoria de continuidade.
- `nexuslang-src`: workspace Rust principal, contendo core, CLI, library,
  WebAssembly e crate LSP.
- `nexuslang-src/src`: lexer, parser, AST, checker, HIR, interpreter,
  runtime, module loader, test runner, package manager, docs e WASM bindings.
- `nexuslang-src/nexus-lsp`: crate LSP separada, com `src/lib.rs` para o
  nucleo testavel de snapshots/operacoes e `src/main.rs` como adapter
  `tower-lsp` fino. Diagnostics multi-file passam por `SourceDatabase` e APIs
  do checker/module loader, enquanto `LspCore` guarda apenas grupos de URIs
  publicadas para limpar diagnostics stale sem cache incremental.
  Go-to-definition cross-file tambem usa `SourceDatabase`/`ModuleGraph` de modo
  opt-in para imports/aliases quando os snapshots abertos correspondem ao disco.
  Semantic tokens sao gerados lexicalmente a partir da tokenizacao existente e
  nao dependem de checker ou `SourceDatabase`.
  Document symbols sao gerados a partir do parser/AST do documento atual, com
  filhos ERP aninhados, `selection_range` separado do range de declaracao e
  fallback vazio quando o documento nao parseia.
- `nexuslang-src/examples` e `nexuslang-src/tests`: exemplos e regressao para
  comportamento da linguagem.
- `nexuslang-src/web` e arquivos `nexuslang-playground.*`: experiencia de
  playground e distribuicao browser.

## Fluxo conceitual

```text
source .nx
  -> lexer
  -> parser / AST
  -> semantic checker
  -> HIR and typed metadata
  -> interpreter/runtime, docs, OpenAPI, diagnostics, tests, or tooling views
```

## Contratos importantes

- O core Rust define a semantica.
- O JSON v1 de diagnostics e contrato publico.
- Operacoes estaticas de `model` e `auth` usam descritores centralizados.
- HIR e metadata tipada devem crescer como camada interna estavel, sem quebrar
  compatibilidade externa.
- O LSP consome APIs do core e nao deve inverter a dependencia.
- O LSP pode manter snapshots em memoria. Diagnostics multi-file devem continuar
  passando por `SourceDatabase`/module loader/checker, nao por duplicacao de
  semantica do core.
- A limpeza de diagnostics stale e responsabilidade do adapter/core LSP:
  memorizar URIs publicadas por entry document, emitir batches vazios para URIs
  que saem do grafo e preservar URIs ainda cobertas por outra entrada ativa.
- Navegacao cross-file do LSP deve continuar disk-backed ate uma fase explicita
  de source database incremental; snapshots sujos devem cair para o
  comportamento same-document.
- Semantic tokens devem permanecer uma camada de editor: legenda estavel no LSP,
  classificacao lexical testavel no `LspCore`, sem alterar JSON de diagnostics
  nem saida textual da CLI.
- Document symbols devem permanecer document-local ate uma fase explicita de
  workspace indexing; nao devem criar cache global ou duplicar semantica do
  checker.

## Validacao por area

- Core geral: `cargo check` e `cargo test` em `nexuslang-src`.
- LSP: `cargo check -p nexus-lsp` e `cargo test -p nexus-lsp`.
- Mudancas em examples/tests: `nexus test` quando o binario estiver disponivel.
- Mudancas em docs de contrato: preferir testes que travem fixtures ou saidas
  publicas relevantes.
