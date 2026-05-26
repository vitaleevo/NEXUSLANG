# Plano de Desenvolvimento — NexusLang

Baseado na análise completa do código-fonte (`21.8k linhas Rust`), roadmap actual,
estado dos testes (210 passando) e próximos passos documentados.

---

## Fase 7.64 — Validação Externa OpenAPI 3.0 (AGORA)

**Objectivo:** Fechar o release do OpenAPI 1.0 com validação externa.

1. **Servir `/openapi.json` no runtime**  
   - Adicionar route `GET /openapi.json` ao servidor HTTP  
   - Testar com `curl http://127.0.0.1:5050/openapi.json`

2. **Validar com ferramenta OpenAPI 3.0 externa**  
   - Opções: `openapi-schema-validator` (Python), `speccy`, `swagger-cli`, ou `redocly-cli`  
   - Criar script `scripts/validate-openapi.sh` que serve o JSON e corre o validador  
   - Adicionar ao CI (GitHub Actions ou script local)

3. **Smoke test com cliente/SDK gerado**  
   - Gerar cliente OpenAPI (ex: `openapi-generator-cli`)  
   - Testar GET/POST/PUT/DELETE contra o servidor a correr  
   - Confirmar que o contrato OpenAPI bate com o runtime real

4. **Corrigir incompatibilidades**  
   - Ajustar `generate_openapi()` se o validador externo encontrar erros  
   - Rodar suite de coerência interna após cada correcção

---

## Fase 8 — Produção Ready

**Objectivo:** Tornar o NexusLang utilizável para projectos reais.

### 8.1 — Split do server (REFACTOR CRÍTICO)

| Problema | Solução |
|----------|---------|
| `server/mod.rs` com 6.7k linhas | Quebrar em módulos |
| Geração OpenAPI misturada com HTTP | Extrair `openapi.rs` (gerador) |
| Testes QA OpenAPI no mesmo ficheiro | Mover para `tests/openapi_qa.rs` |
| Parsing JSON manual | Extrair `json.rs` com parser dedicado |

Estrutura nova:
```
src/server/
├── mod.rs          # Orquestração (serve_file, handle_stream)
├── http.rs         # TcpListener, request/response parsing
├── openapi.rs      # generate_openapi() + helpers
├── storage.rs      # JSON file storage
└── router.rs       # Route matching + dispatch
```

### 8.2 — Backend Storage (pós-JSON)

1. **SQLite via crate leve** (`rusqlite` ou `sqlite3` via C)
   - Substituir JSON filesystem por SQLite
   - Manter JSON como fallback/configurável
   - Migração: `JsonStorage` trait → `SqliteStorage` impl

2. **Índices físicos** para campos `index`
   - `Model::where("email", "x")` usa índice real
   - `Model::where_between("salary", 1000, 5000)` com range scan

3. **Transacções** em `Model::create()` e `Model::update()`
   - Rollback em caso de erro
   - Isolamento para concorrência

### 8.3 — Dependências Estratégicas

Actualmente **zero dependências**. Adicionar selectivamente:

| Crate | Motivo | Risco de não ter |
|-------|--------|------------------|
| `serde` + `serde_json` | Parsing/serialização JSON | Parsing manual frágil |
| Pequeno HTTP server (`matchit` + `tiny_http`) | Substituir TcpListener raw | Segurança, performance |
| `clap` | CLI parsing | `args[1]` frágil |
| `chrono` | `date` type operations | Parsing manual de datas |

**Regra:** Cada dependência deve ser avaliada — manter zero onde o custo é baixo.

### 8.4 — CI/CD Pipeline

```yaml
# .github/workflows/ci.yml
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings
  openapi-validate:
    runs-on: ubuntu-latest
    steps:
      - run: scripts/validate-openapi.sh
  wasm:
    runs-on: ubuntu-latest
    steps:
      - run: scripts/build-playground-wasm.sh
```

### 8.5 — Refactor do Checker

`checker/mod.rs` com 3.8k linhas precisa de split:

```
src/checker/
├── mod.rs           # Checker struct + orquestração
├── models.rs        # Validação de model declarations
├── routes.rs        # Validação de route declarations
├── functions.rs     # Validação de funções
├── types.rs         # Type inference + compatibilidade
└── constraints.rs   # unique, index, min, max
```

### 8.6 — Testes Organizados

`tests/core.rs` com 6.8k linhas → quebrar por módulo:

```
tests/
├── core.rs              # Smoke tests básicos
├── lexer.rs             # Testes de tokenização
├── parser.rs            # Testes de parsing
├── checker.rs           # Testes de validação semântica
├── interpreter.rs       # Testes de runtime
├── server/
│   ├── http.rs          # Testes de servidor HTTP
│   ├── routes.rs        # Testes de route dispatch
│   ├── storage.rs       # Testes de JSON storage
│   └── openapi_qa.rs    # Testes de QA OpenAPI
└── examples.rs          # Testes de integração com exemplos
```

---

## Fase 9 — ERP Primitivas Avançadas

### 9.1 — Invoice completa

```nexus
invoice {
    customer: "Cliente"
    currency: "AOA"
    items: [
        { desc: "Setup", qty: 1, price: 250000 kz },
        { desc: "Suporte", qty: 12, price: 50000 kz }
    ]
    tax: 14
    discount: 50000 kz
    notes: "Pagamento em 30 dias"
}
```

- Line items com IVA e desconto
- Cálculo automático de totais
- Geração de PDF (via template)
- `invoice` como tipo reutilizável

### 9.2 — Workflow Executável

```nexus
workflow EmployeeOnboarding {
    step criar_employee { Employee::create({ name, salary, department }) }
    step enviar_email { send_email(employee.email, "Bem-vindo") }
    step configurar_acessos { grant_access(employee.id, "default") }
}
```

- Steps com corpos executáveis (nomes apenas já funciona)
- Transições entre steps (success → próximo, fail → rollback)
- Estado persistente entre steps
- Workflow como tipo (reutilizável, parametrizável)

### 9.3 — Multi-moeda e Câmbio

```nexus
model Invoice {
    amount: money(AOA)
    exchange_rate: float
    amount_usd: money(USD) = amount * exchange_rate
}
```

- Suporte a `money(USD)`, `money(EUR)`, `money(AOA)`
- Conversão entre moedas
- Taxas de câmbio como tipo built-in

### 9.4 — Relacionamentos entre Models

```nexus
model Employee {
    name: string
    department: Department  // belongs_to
}

model Department {
    name: string
    employees: [Employee]   // has_many
}
```

- Navegação: `employee.department.name`
- Lazy loading vs eager loading
- Integração com storage (FKs)

---

## Fase 10 — Produto Web

### 10.1 — Hub NexusLang

- Website com playground integrado
- Documentação da sintaxe e exemplos interactivos
- Dashboard de projectos (create/edit/run .nx files)
- Sharing de snippets

### 10.2 — Hosted Runtime

- `nexus deploy` — deploy de projectos para cloud
- Runtime hosted com Supabase/Postgres (em vez de JSON)
- Rate limiting, auth, logging
- API Gateway para routes NexusLang

### 10.3 — Marketplace de Modelos

- Templates de modelos ERP: CRM, Inventory, HR, Sales, Accounting
- Import/export de schemas
- Comunidade de contribuição

---

## Timeline Sugerida

| Timeline | Fase | Entregável |
|----------|------|------------|
| **Semana 1-2** | 7.64 | OpenAPI validado externamente + smoke test |
| **Semana 3-4** | 8.1-8.3 | Split server + dependências iniciais |
| **Semana 5-6** | 8.4-8.6 | CI/CD + split tests + split checker |
| **Semana 7-8** | 8.2 | SQLite backend + índices + transacções |
| **Semana 9-12** | 9.1-9.4 | ERP avançado (invoice, workflow, money, relations) |
| **Q2** | 10 | Produto web + hosted runtime |

---

## Riscos e Mitigações

| Risco | Impacto | Mitigação |
|-------|---------|-----------|
| Server HTTP manual (6.7k) | Segurança, manutenção | Split + substituir por crate testada |
| Zero dependências | Reinventar roda | Adicionar serde, tiny_http, chrono |
| JSON storage | Sem transacções | SQLite na Fase 8.2 |
| Checker monolítico (3.8k) | Difícil evolutir | Split em módulos |
| Testes num ficheiro (6.8k) | Difícil navegar | Split por módulo |
| WASM não recompilado | Playground desactualizado | CI build automático |
| Sem CI/CD | Regressões manuais | GitHub Actions na Fase 8.4 |

---

## Comandos de Verificação

```bash
# Sempre antes de commitar
cargo fmt
cargo clippy -- -D warnings
cargo test

# Build WASM (quando alterar core)
bash scripts/build-playground-wasm.sh

# Validar OpenAPI
bash scripts/validate-openapi.sh

# Servir playground local
python3 -m http.server 8091
# Abrir http://127.0.0.1:8091/nexuslang-playground.html
```

---

## Próximo Passo Imediato

**Fase 7.64 — Validação externa OpenAPI 3.0**

AVISO: O próximo passo é implementar a validação externa OpenAPI 3.0 e smoke test de cliente gerado. Antes de iniciar, ler `MEMORIA_NEXUSLANG.md` para continuar exactamente de onde o projecto parou.
