const WASM_URL = './nexuslang-src/web/nexuslang_playground.wasm';

const EXAMPLES = {
  erp: `// erp_basico.nx — exemplo de sistema ERP em NexusLang

model Employee {
    name: string
    salary: money
    department: string
}

model Department {
    name: string
    budget: money
}

fn calcular_bonus(salario: money) -> money {
    return salario * 0.1
}

fn saudar(nome: string) -> string {
    return "Bem-vindo, " + nome
}

workflow Payroll {
    step calcular_salarios
    step aprovar_pagamentos
    step processar_pagamentos
}

route GET /employees {
    return Employee::all()
}

route GET /departments {
    return Department::all()
}

route POST /employees {
    return "Employee criado com sucesso"
}

invoice {
    customer: "Empresa Petrolífera SARL"
    service: "Consultoria ERP"
    total: 750000 kz
    currency: "AOA"
}

print("=== Sistema NexusLang ERP ===")

let salario_base = 300000 kz
let bonus = calcular_bonus(salario_base)
let total = salario_base + bonus

print(saudar("Admin"))
print("Salário base:")
print(salario_base)
print("Bónus (10%):")
print(bonus)
print("Total:")
print(total)

if total > 250000 kz {
    print("Salário acima da média")
} else {
    print("Salário abaixo da média")
}

let contador = 1
while contador <= 3 {
    print("Processando mês:")
    print(contador)
    contador = contador + 1
}

let departamentos = ["TI", "RH", "Financeiro", "Operações"]
for dept in departamentos {
    print(dept)
}`,

  fase3: `// fase3_erp_primitivas.nx — invoice estruturada, workflow executável e route params

model Employee {
    name: string
    salary: money
    department: string
}

workflow Billing {
    step preparar {
        print("Preparando fatura")
    }

    step emitir {
        print("Emitindo documento fiscal")
    }

    step notificar {
        print("Notificando cliente")
    }
}

route GET /employees/:id {
    return "employee " + id
}

invoice {
    customer: "Empresa Petrolífera SARL"
    currency: "AOA"
    tax: 14
    discount: 25000 kz
    item "Consultoria ERP" qty 2 price 150000 kz
    item "Suporte mensal" qty 1 price 75000 kz
}

print("=== Fase 3: ERP primitives ===")
run_workflow("Billing")`,

  fase4: `// fase4_tooling_lint.nx — o core Rust devolve avisos de lint ao playground

model employee {
    FullName: string
}

fn CalcularTotal(valor: money) -> money {
    return valor * 1.14
}

let TotalMensal = CalcularTotal(100000 kz)

print("Total com IVA:")
print(TotalMensal)`,

  fase5: `// fase5_runtime_services.nx — routes HTTP, storage JSON e OpenAPI no CLI/servidor

model Employee {
    name: string
    department: string
}

route GET /health {
    return "ok"
}

route GET /employees {
    return Employee::all()
}

route GET /employees/:id {
    return "employee " + id
}

print("Runtime service pronto")
print("nexus serve examples/runtime_services.nx")`,

  hello: `// hello.nx — primeiro programa NexusLang

let nome = "NexusLang"
const versao = 1
let salario: money = 500000 kz

print("Olá, " + nome + "!")
print("Versão:")
print(versao)
print("Salário de exemplo:")
print(salario)

fn dobrar(x: int) -> int {
    return x * 2
}

let resultado = dobrar(21)
print("Dobro de 21:")
print(resultado)

let lista = [10, 20, 30, 40, 50]
for item in lista {
    print(item)
}`,

  ecommerce: `// e-commerce.nx — plataforma de vendas

model Produto {
    nome: string
    preco: money
    stock: int
}

model Cliente {
    nome: string
    email: string
    saldo: money
}

fn calcular_desconto(preco: money) -> money {
    return preco * 0.15
}

fn aplicar_iva(valor: money) -> money {
    return valor * 1.14
}

workflow Checkout {
    step validar_stock {
        print("Stock validado")
    }
    step processar_pagamento {
        print("Pagamento processado")
    }
    step emitir_fatura {
        print("Fatura emitida")
    }
}

route GET /produtos {
    return Produto::all()
}

route POST /checkout {
    return "Pedido processado com sucesso"
}

invoice {
    customer: "João Silva"
    currency: "AOA"
    tax: 14
    discount: 18750 kz
    item "Pacote ERP" qty 1 price 125000 kz
}

print("=== E-Commerce NexusLang ===")

let preco_base = 125000 kz
let desconto = calcular_desconto(preco_base)
let preco_final = aplicar_iva(preco_base)

print("Preço base:")
print(preco_base)
print("Desconto 15%:")
print(desconto)
print("Com IVA 14%:")
print(preco_final)
run_workflow("Checkout")`,

  banca: `// banca.nx — sistema bancário digital

model Conta {
    titular: string
    saldo: money
    numero: string
}

model Transacao {
    valor: money
    tipo: string
    descricao: string
}

fn calcular_juros(saldo: money) -> money {
    return saldo * 0.035
}

fn verificar_saldo(saldo: money) -> bool {
    return saldo > 0 kz
}

workflow Transferencia {
    step autenticar_cliente {
        print("Cliente autenticado")
    }
    step validar_saldo {
        print("Saldo validado")
    }
    step registar_transacao {
        print("Transação registada")
    }
}

route GET /contas {
    return Conta::all()
}

route POST /transferencia {
    return "Transferência processada"
}

print("=== Banco Digital NexusLang ===")

let saldo_conta = 2500000 kz
let juros_mensais = calcular_juros(saldo_conta)
let novo_saldo = saldo_conta + juros_mensais

print("Saldo actual:")
print(saldo_conta)
print("Juros mensais (3.5%):")
print(juros_mensais)
print("Saldo com juros:")
print(novo_saldo)

if novo_saldo > 1000000 kz {
    print("Cliente Premium")
} else {
    print("Cliente Standard")
}

run_workflow("Transferencia")`
,

  inventory: `// inventory.nx - stock control and replenishment planning

model Product {
    sku: string
    name: string
    stock: int
    min_stock: int
    cost: money
}

model Warehouse {
    name: string
    city: string
    capacity: int
}

model Supplier {
    name: string
    lead_time_days: int
    active: bool
}

fn needs_restock(stock: int, min_stock: int) -> bool {
    return stock < min_stock
}

fn inventory_value(stock: int, unit_cost: money) -> money {
    return unit_cost * stock
}

workflow InventoryReplenishment {
    step audit_stock {
        print("Stock counted")
    }
    step approve_purchase {
        print("Purchase approved")
    }
    step receive_goods {
        print("Goods received and posted")
    }
}

route GET /inventory {
    return Product::all()
}

route GET /inventory/:sku {
    return "product " + sku
}

route POST /purchase_orders {
    return "purchase order created"
}

invoice {
    customer: "Main Warehouse"
    currency: "AOA"
    tax: 14
    item "Barcode scanners" qty 2 price 85000 kz
    item "Storage bins" qty 12 price 4500 kz
}

print("=== Inventory Control ===")

let current_stock = 18
let min_stock = 25
let unit_cost = 12500 kz
let total_value = inventory_value(current_stock, unit_cost)

print("Current stock:")
print(current_stock)
print("Minimum stock:")
print(min_stock)
print("Inventory value:")
print(total_value)

if needs_restock(current_stock, min_stock) {
    print("Reorder required")
} else {
    print("Stock level healthy")
}

let zones = ["A1", "A2", "B1"]
for zone in zones {
    print("Cycle count zone:")
    print(zone)
}

run_workflow("InventoryReplenishment")`,

  crm: `// crm.nx - lead pipeline and sales operations

model Lead {
    name: string
    source: string
    score: int
}

model Deal {
    account: string
    value: money
    stage: string
    probability: float
}

model Activity {
    owner: string
    kind: string
    done: bool
}

fn weighted_pipeline(value: money, probability: float) -> money {
    return value * probability
}

fn hot_lead(score: int) -> bool {
    return score >= 80
}

workflow LeadToCustomer {
    step qualify_lead {
        print("Lead qualified")
    }
    step send_proposal {
        print("Proposal sent")
    }
    step close_deal {
        print("Deal closed")
    }
}

route GET /leads {
    return Lead::all()
}

route GET /deals/:id {
    return "deal " + id
}

route POST /activities {
    return "activity registered"
}

invoice {
    customer: "Nova Retail"
    currency: "AOA"
    tax: 14
    discount: 25000 kz
    item "CRM setup" qty 1 price 300000 kz
    item "Sales automation" qty 1 price 450000 kz
}

print("=== CRM Pipeline ===")

let lead_score = 87
let deal_value = 950000 kz
let probability = 0.65
let forecast = weighted_pipeline(deal_value, probability)

print("Lead score:")
print(lead_score)
print("Weighted forecast:")
print(forecast)

if hot_lead(lead_score) {
    print("Prioritize this lead")
} else {
    print("Nurture sequence")
}

let stages = ["Lead", "Qualified", "Proposal", "Won"]
for stage in stages {
    print("Pipeline stage:")
    print(stage)
}

run_workflow("LeadToCustomer")`,

  payroll: `// payroll_real.nx - monthly payroll close

model Employee {
    name: string
    salary: money
    department: string
    active: bool
}

model PayrollItem {
    employee: string
    gross: money
    net: money
    month: string
}

model Benefit {
    name: string
    amount: money
    taxable: bool
}

fn inss(gross: money) -> money {
    return gross * 0.03
}

fn irt(gross: money) -> money {
    return gross * 0.10
}

fn net_salary(gross: money) -> money {
    return gross - inss(gross) - irt(gross)
}

workflow PayrollClose {
    step collect_timesheets {
        print("Timesheets collected")
    }
    step calculate_deductions {
        print("Deductions calculated")
    }
    step approve_bank_file {
        print("Bank file approved")
    }
    step post_accounting {
        print("Accounting entries posted")
    }
}

route GET /payroll/:month {
    return "payroll " + month
}

route GET /employees {
    return Employee::all()
}

route POST /payroll/close {
    return "payroll closed"
}

print("=== Payroll Close ===")

let gross_salary = 450000 kz
let social_security = inss(gross_salary)
let income_tax = irt(gross_salary)
let net = net_salary(gross_salary)

print("Gross salary:")
print(gross_salary)
print("INSS:")
print(social_security)
print("IRT:")
print(income_tax)
print("Net salary:")
print(net)

let departments = ["Operations", "Finance", "Sales"]
for department in departments {
    print("Payroll reviewed:")
    print(department)
}

run_workflow("PayrollClose")`
};

let currentTab = 'output';
let wasmExports = null;
let wasmReady = null;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

function switchTab(name, el) {
  document.querySelectorAll('.tab').forEach(tab => tab.classList.remove('active'));
  document.querySelectorAll('.tab-content').forEach(tab => tab.classList.remove('active'));
  el.classList.add('active');
  document.getElementById('tab-' + name).classList.add('active');
  currentTab = name;
}

function activateOutputTab() {
  const outputTab = document.querySelector('.tab');
  switchTab('output', outputTab);
}

function loadExample() {
  const key = document.getElementById('exampleSelect').value;
  document.getElementById('editor').value = EXAMPLES[key] || EXAMPLES.erp;
  onEditorInput();
}

function clearEditor() {
  document.getElementById('editor').value = '';
  document.getElementById('outputLines').innerHTML = '';
  document.getElementById('tokenLines').innerHTML = '';
  document.getElementById('astLines').innerHTML = '';
  document.getElementById('erpLines').innerHTML = '';
  document.getElementById('emptyState').style.display = 'flex';
  resetStatuses();
  onEditorInput();
}

function handleKey(e) {
  if (e.key === 'Tab') {
    e.preventDefault();
    const textarea = e.target;
    const start = textarea.selectionStart;
    const end = textarea.selectionEnd;
    textarea.value = textarea.value.slice(0, start) + '    ' + textarea.value.slice(end);
    textarea.selectionStart = textarea.selectionEnd = start + 4;
    onEditorInput();
  }
}

function onEditorInput() {
  updateLineNumbers();
}

function updateLineNumbers() {
  const lines = document.getElementById('editor').value.split('\n').length;
  const nums = Array.from({ length: lines }, (_, index) => index + 1).join('\n');
  document.getElementById('lineNumbers').textContent = nums;
}

function syncScroll() {
  const editor = document.getElementById('editor');
  document.getElementById('lineNumbers').scrollTop = editor.scrollTop;
}

function resetStatuses() {
  setStatus('lexer', null, 'WASM');
  setStatus('parser', null, 'WASM');
  setStatus('checker', null, 'WASM');
  setStatus('interp', null, 'WASM');
  document.getElementById('statusTime').textContent = '';
}

function setStatus(which, ok, msg) {
  const el = document.getElementById('status' + which.charAt(0).toUpperCase() + which.slice(1));
  const label = which === 'interp' ? 'RUNTIME' : which.toUpperCase();
  const cls = ok === true ? 'status-ok' : ok === false ? 'status-err' : '';
  const mark = ok === true ? '✓' : ok === false ? '✗' : '—';
  el.innerHTML = `<span style="color:var(--text3)">${label}</span>
    <span class="${cls}">${mark} ${escHtml(msg)}</span>`;
}

async function initWasm() {
  if (wasmReady) return wasmReady;

  wasmReady = (async () => {
    const imports = {};
    if (WebAssembly.instantiateStreaming) {
      try {
        const { instance } = await WebAssembly.instantiateStreaming(fetch(WASM_URL), imports);
        wasmExports = instance.exports;
        return wasmExports;
      } catch (error) {
        console.warn('WASM streaming indisponivel, usando fallback:', error);
      }
    }

    const response = await fetch(WASM_URL);
    if (!response.ok) {
      throw new Error(`WASM não encontrado em ${WASM_URL}`);
    }
    const bytes = await response.arrayBuffer();
    const { instance } = await WebAssembly.instantiate(bytes, imports);
    wasmExports = instance.exports;
    return wasmExports;
  })();

  return wasmReady;
}

async function runCore(source) {
  const api = await initWasm();
  const input = encoder.encode(source);
  const inputPtr = api.nexus_alloc(input.length);
  new Uint8Array(api.memory.buffer).set(input, inputPtr);

  const resultPtr = api.nexus_playground_run(inputPtr, input.length);
  api.nexus_dealloc(inputPtr, input.length);

  const view = new DataView(api.memory.buffer, resultPtr, 4);
  const resultLen = view.getUint32(0, true);
  const resultBytes = new Uint8Array(api.memory.buffer, resultPtr + 4, resultLen).slice();
  api.nexus_free_result(resultPtr);

  return JSON.parse(decoder.decode(resultBytes));
}

async function runCode() {
  const src = document.getElementById('editor').value.trim();
  if (!src) return;

  const btn = document.getElementById('runBtn');
  btn.classList.add('running');
  btn.textContent = '⏳ A executar...';
  const t0 = performance.now();

  try {
    const result = await runCore(src);
    const elapsed = (performance.now() - t0).toFixed(1);
    updateStatusFromResult(result, elapsed);
    renderTokens(result.tokens || []);
    renderAST(result.ast || []);
    renderERP(result.erp || {});

    if (result.ok) {
      renderOutput(result);
    } else {
      renderError(result);
    }

    document.getElementById('statusTime').textContent = `⏱ ${elapsed}ms · Rust/WASM`;
  } catch (error) {
    renderFatalError(error);
    setStatus('interp', false, 'WASM indisponível');
  } finally {
    btn.classList.remove('running');
    btn.textContent = '▶ Executar';
  }
}

function updateStatusFromResult(result, elapsed) {
  const stats = result.stats || { tokens: 0, decls: 0, warnings: 0 };
  if (!result.ok && result.stage === 'lexer') {
    setStatus('lexer', false, formatDiagnosticShort(result));
    setStatus('parser', null, 'â€”');
    setStatus('checker', null, 'â€”');
    setStatus('interp', null, 'â€”');
    return;
  }

  setStatus('lexer', true, `${stats.tokens} tokens`);

  if (!result.ok && result.stage === 'parser') {
    setStatus('parser', false, formatDiagnosticShort(result));
    setStatus('checker', null, '—');
    setStatus('interp', null, '—');
    return;
  }

  setStatus('parser', true, `${stats.decls} declarações`);

  if (!result.ok && result.stage === 'checker') {
    setStatus('checker', false, formatDiagnosticShort(result));
    setStatus('interp', null, '—');
    return;
  }

  const warningText = stats.warnings ? `${stats.warnings} avisos` : 'OK';
  setStatus('checker', true, warningText);

  if (!result.ok) {
    setStatus('interp', false, formatDiagnosticShort(result));
    return;
  }

  setStatus('interp', true, `${elapsed}ms`);
}

function formatDiagnosticShort(result) {
  const diagnostic = result.diagnostic || {};
  if (diagnostic.line) {
    return diagnostic.column ? `L${diagnostic.line}:C${diagnostic.column}` : `L${diagnostic.line}`;
  }
  return String(result.message || 'erro').slice(0, 36);
}

function renderOutput(result) {
  const el = document.getElementById('outputLines');
  document.getElementById('emptyState').style.display = 'none';

  const lines = result.output || [];
  let activeKind = 'print';
  const html = [];

  if (!lines.length) {
    html.push('<div class="output-line out-info">Programa executado sem saída.</div>');
  }

  for (const line of lines) {
    const kind = outputKind(line, activeKind);
    if (kind.next) activeKind = kind.next;
    html.push(`<div class="output-line ${kind.cls}">${escHtml(line || ' ')}</div>`);
  }

  for (const warning of result.warnings || []) {
    html.push(`<div class="output-line out-warning">⚠ ${escHtml(warning.code)}: ${escHtml(warning.message)}</div>`);
  }

  el.innerHTML = html.join('');
  activateOutputTab();
}

function outputKind(line, activeKind) {
  if (!line) return { cls: 'out-info' };
  if (line.includes('Models registados')) return { cls: 'out-section', next: 'out-model' };
  if (line.includes('Workflows registados')) return { cls: 'out-section', next: 'out-workflow' };
  if (line.includes('Routes registadas')) return { cls: 'out-section', next: 'out-route' };
  if (line.includes('Invoices')) return { cls: 'out-section', next: 'out-invoice' };
  if (line.startsWith('▶ Workflow')) return { cls: 'out-workflow', next: 'out-workflow' };
  if (line.startsWith('  ') || line.startsWith('    ')) return { cls: activeKind };
  return { cls: 'out-print' };
}

function renderError(result) {
  document.getElementById('emptyState').style.display = 'none';
  const diagnostic = result.diagnostic || {};
  const location = diagnostic.line
    ? `Linha ${diagnostic.line}${diagnostic.column ? `, coluna ${diagnostic.column}` : ''}`
    : null;
  const detail = location
    ? `<div class="output-line out-error">${escHtml(location)}</div>`
    : '';

  document.getElementById('outputLines').innerHTML =
    `<div class="output-line out-error">❌ ${escHtml(result.stage || 'erro')}: ${escHtml(result.message || 'Erro')}</div>${detail}`;

  if (diagnostic.line) {
    focusDiagnostic(diagnostic.line, diagnostic.column || 1);
  }
  activateOutputTab();
}

function renderFatalError(error) {
  document.getElementById('emptyState').style.display = 'none';
  document.getElementById('outputLines').innerHTML =
    `<div class="output-line out-error">❌ ${escHtml(error.message || error)}</div>`;
  activateOutputTab();
}

function focusDiagnostic(line, column) {
  const editor = document.getElementById('editor');
  const index = indexForPosition(editor.value, line, column);
  editor.focus();
  editor.setSelectionRange(index, Math.min(index + 1, editor.value.length));
}

function indexForPosition(source, line, column) {
  let currentLine = 1;
  let currentColumn = 1;
  for (let i = 0; i < source.length; i += 1) {
    if (currentLine === line && currentColumn === column) return i;
    if (source[i] === '\n') {
      currentLine += 1;
      currentColumn = 1;
    } else {
      currentColumn += 1;
    }
  }
  return source.length;
}

function renderTokens(tokens) {
  const el = document.getElementById('tokenLines');
  const colorMap = {
    let: 'tk-keyword', const: 'tk-keyword', fn: 'tk-keyword', return: 'tk-keyword',
    if: 'tk-keyword', else: 'tk-keyword', while: 'tk-keyword', for: 'tk-keyword',
    in: 'tk-keyword', model: 'tk-keyword', workflow: 'tk-keyword', step: 'tk-keyword',
    route: 'tk-keyword', invoice: 'tk-keyword', print: 'tk-keyword',
    GET: 'tk-keyword', POST: 'tk-keyword', PUT: 'tk-keyword', DELETE: 'tk-keyword',
    String: 'tk-literal', Bool: 'tk-literal', Integer: 'tk-literal', Float: 'tk-literal',
    Money: 'tk-money', Ident: 'tk-ident',
    string: 'tk-type', int: 'tk-type', float: 'tk-type', bool: 'tk-type', money: 'tk-type', date: 'tk-type',
    Arrow: 'tk-op', Eq: 'tk-op', NotEq: 'tk-op', Lt: 'tk-op', LtEq: 'tk-op',
    Gt: 'tk-op', GtEq: 'tk-op', And: 'tk-op', Or: 'tk-op', Plus: 'tk-op',
    Minus: 'tk-op', Star: 'tk-op', Slash: 'tk-op', Percent: 'tk-op', Not: 'tk-op',
    Assign: 'tk-op', ColonColon: 'tk-op'
  };

  el.innerHTML = tokens.map(token => {
    const cls = colorMap[token.type] || 'tk-punct';
    const value = token.value === undefined ? '' : ` = ${escHtml(String(token.value))}`;
    const currency = token.currency ? ` ${escHtml(token.currency)}` : '';
    return `<div class="token-line">
      <span class="token-line-num">${token.line}:${token.column}</span>
      <span class="token-type ${cls}">${escHtml(token.type)}</span>
      <span class="token-value">${value}${currency}</span>
    </div>`;
  }).join('');
}

function renderAST(nodes) {
  const el = document.getElementById('astLines');
  const labelMap = {
    Function: ['FUNÇÃO', 'ast-fn'],
    Model: ['MODEL', 'ast-model-l'],
    Workflow: ['WORKFLOW', 'ast-workflow-l'],
    Route: ['ROUTE', 'ast-route-l'],
    Invoice: ['INVOICE', 'ast-invoice-l'],
    Statement: ['STMT', 'ast-stmt-l']
  };

  el.innerHTML = nodes.map(node => {
    const [label, cls] = labelMap[node.kind] || [node.kind, 'ast-stmt-l'];
    const children = (node.children || [])
      .map(child => `<div class="ast-field">▸ <span>${escHtml(child)}</span></div>`)
      .join('');
    return `<div class="ast-decl">
      <div class="ast-label ${cls}">${escHtml(label)}</div>
      <div class="ast-field"><b>${escHtml(node.summary || node.name || node.kind)}</b></div>
      ${children}
    </div>`;
  }).join('');
}

function renderERP(erp) {
  const el = document.getElementById('erpLines');
  const models = erp.models || [];
  const workflows = erp.workflows || [];
  const routes = erp.routes || [];
  const invoices = erp.invoices || [];

  if (!models.length && !workflows.length && !routes.length && !invoices.length) {
    el.innerHTML = '<div class="empty-state"><div class="empty-diamond"></div><span>Nenhuma documentacao ERP gerada</span></div>';
    return;
  }

  el.innerHTML = [
    renderDocSummary(models, workflows, routes, invoices),
    renderModelDocs(models),
    renderRouteDocs(routes),
    renderWorkflowDocs(workflows),
    renderInvoiceDocs(invoices)
  ].filter(Boolean).join('');
}

function renderDocSummary(models, workflows, routes, invoices) {
  const metrics = [
    ['Models', models.length],
    ['Routes', routes.length],
    ['Workflows', workflows.length],
    ['Invoices', invoices.length]
  ];
  return `<div class="doc-summary">${metrics.map(([label, count]) => `
    <div class="doc-metric">
      <strong>${count}</strong>
      <span>${escHtml(label)}</span>
    </div>
  `).join('')}</div>`;
}

function renderModelDocs(models) {
  if (!models.length) return '';
  const cards = models.map(model => {
    const fields = (model.fields || []).map(field => `
      <div class="doc-row">
        <span class="doc-code">${escHtml(field.name)}</span>
        <span style="color:${typeColor(field.type)}">${escHtml(field.type)}</span>
      </div>
    `).join('');
    return `<div class="doc-card">
      <div class="doc-card-head">
        <span class="erp-badge erp-model">MODEL</span>
        <span class="doc-name">${escHtml(model.name)}</span>
      </div>
      ${fields || '<div class="doc-muted">Sem campos</div>'}
      <div class="doc-muted" style="margin-top:8px">Collection API: <span class="doc-code">${escHtml(model.name)}::all()</span></div>
    </div>`;
  }).join('');
  return sectionTitle('Models') + cards;
}

function renderRouteDocs(routes) {
  if (!routes.length) return '';
  const methodColors = { GET: 'var(--accent2)', POST: 'var(--number)', PUT: 'var(--money)', DELETE: 'var(--accent3)' };
  const cards = routes.map(route => {
    const params = route.params?.length
      ? route.params.map(param => `<span class="doc-chip">${escHtml(param)}</span>`).join('')
      : '<span class="doc-muted">Sem parametros</span>';
    return `<div class="doc-card">
      <div class="doc-card-head">
        <span class="erp-badge erp-route" style="color:${methodColors[route.method] || 'var(--accent)'}">${escHtml(route.method)}</span>
        <span class="doc-name doc-code">${escHtml(route.path)}</span>
      </div>
      <div class="doc-muted" style="margin-bottom:6px">Parametros</div>
      <div class="doc-flow">${params}</div>
    </div>`;
  }).join('');
  return sectionTitle('Routes') + cards;
}

function renderWorkflowDocs(workflows) {
  if (!workflows.length) return '';
  const cards = workflows.map(workflow => {
    const steps = (workflow.steps || []).map((step, index, arr) => {
      const arrow = index < arr.length - 1 ? '<span class="doc-muted">-&gt;</span>' : '';
      const actions = Number(step.statements || 0);
      return `<span class="doc-chip">${escHtml(step.name)} <span class="doc-muted">(${actions})</span></span>${arrow}`;
    }).join('');
    return `<div class="doc-card">
      <div class="doc-card-head">
        <span class="erp-badge erp-workflow">WORKFLOW</span>
        <span class="doc-name">${escHtml(workflow.name)}</span>
      </div>
      <div class="doc-flow">${steps || '<span class="doc-muted">Sem steps</span>'}</div>
    </div>`;
  }).join('');
  return sectionTitle('Workflows') + cards;
}

function renderInvoiceDocs(invoices) {
  if (!invoices.length) return '';
  const cards = invoices.map((invoice, index) => {
    const fields = (invoice.fields || []).map(field => `<span class="doc-chip">${escHtml(field)}</span>`).join('');
    return `<div class="doc-card">
      <div class="doc-card-head">
        <span class="erp-badge erp-invoice">INVOICE</span>
        <span class="doc-name">Invoice ${index + 1}</span>
      </div>
      <div class="doc-muted" style="margin-bottom:6px">Campos</div>
      <div class="doc-flow">${fields || '<span class="doc-muted">Sem campos</span>'}</div>
      <div class="doc-muted" style="margin-top:8px">${invoice.items || 0} structured items</div>
    </div>`;
  }).join('');
  return sectionTitle('Invoices') + cards;
}

function typeColor(type) {
  const colors = {
    money: 'var(--money)',
    string: 'var(--string)',
    int: 'var(--number)',
    float: 'var(--number)',
    bool: 'var(--accent)',
    date: 'var(--type)'
  };
  return colors[type] || 'var(--text2)';
}

function sectionTitle(label) {
  return `<div style="margin:0 0 10px 0;font-size:11px;color:var(--text2);letter-spacing:1px;text-transform:uppercase;font-weight:600">${label}</div>`;
}

function escHtml(value) {
  return String(value).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

window.switchTab = switchTab;
window.loadExample = loadExample;
window.clearEditor = clearEditor;
window.handleKey = handleKey;
window.onEditorInput = onEditorInput;
window.syncScroll = syncScroll;
window.runCode = runCode;

document.addEventListener('DOMContentLoaded', () => {
  document.getElementById('editor').value = EXAMPLES.erp;
  resetStatuses();
  onEditorInput();
  initWasm()
    .then(() => setStatus('interp', true, 'WASM pronto'))
    .catch(error => {
      setStatus('interp', false, 'WASM indisponível');
      renderFatalError(error);
    });
});
