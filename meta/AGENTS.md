# AGENTS.md - Guia para agentes

Este arquivo orienta agentes que forem trabalhar no projeto NexusLang.
Use-o como ponto de partida operacional junto com a memoria principal.

## Ordem de leitura

1. Leia `MEMORIA_NEXUSLANG.md` na raiz do projeto.
2. Leia `meta/CURRENT_TASKS.md` para entender o foco imediato.
3. Leia `meta/ROADMAP.md` para contexto de produto e proximas fases.
4. Abra apenas os arquivos citados pela tarefa atual antes de expandir a
   investigacao.

## Principios do projeto

- NexusLang e uma linguagem ERP-first, focada em fluxos de negocio reais.
- O core Rust em `nexuslang-src` e a fonte de verdade para sintaxe,
  semantica, runtime, diagnostics e CLI.
- O playground deve demonstrar capacidades ja suportadas pelo core.
- Mudancas devem ser pequenas, testaveis e alinhadas com os contratos
  existentes.

## Guardrails importantes

- Preserve o contrato JSON v1 de diagnostics e tooling, salvo pedido explicito.
- Preserve a saida textual da CLI, salvo pedido explicito.
- Nao altere a semantica de imports sem uma fase propria de arquitetura.
- Nao introduza registry remoto real no package manager sem decisao explicita.
- Mantenha dependencias de editor/LSP fora do core; use a crate `nexus-lsp`.
- Evite refactors amplos quando a tarefa pedir uma correcao localizada.

## Fluxo recomendado

1. Confirmar o objetivo no arquivo de memoria.
2. Inspecionar os modulos diretamente relacionados.
3. Implementar o menor corte completo.
4. Rodar formatacao e validacoes proporcionais ao risco.
5. Atualizar docs ou memoria quando a fase mudar o estado do projeto.

## Comandos uteis

Execute a partir de `nexuslang-src` quando aplicavel:

```bash
CARGO_TARGET_DIR=/tmp/nexuslang-target-codex cargo check
CARGO_TARGET_DIR=/tmp/nexuslang-target-codex cargo test
CARGO_TARGET_DIR=/tmp/nexuslang-target-codex cargo check -p nexus-lsp
CARGO_TARGET_DIR=/tmp/nexuslang-target-codex cargo test -p nexus-lsp
rustfmt --edition 2021 <arquivo.rs>
```

Use validacoes mais especificas quando a tarefa tocar apenas uma crate, modulo
ou contrato.
