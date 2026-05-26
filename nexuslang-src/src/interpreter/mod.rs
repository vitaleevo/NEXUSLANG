use crate::ast::*;
/// NexusLang Interpreter — executa a AST
use std::collections::{HashMap, HashSet};

/// Valores em runtime
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Money(f64, String),
    Array(Vec<Value>),
    Object(String, Vec<(String, Value)>),
    Nil,
}

impl Value {
    fn currency_matches(left: &str, right: &str) -> Result<(), String> {
        if left.eq_ignore_ascii_case(right) {
            Ok(())
        } else {
            Err(format!("Moedas incompatíveis: {} e {}", left, right))
        }
    }

    fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Integer(n) => *n != 0,
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::Money(v, _) => *v != 0.0,
            Value::Nil => false,
            Value::Array(a) => !a.is_empty(),
            Value::Object(_, _) => true,
        }
    }

    pub fn display(&self) -> String {
        match self {
            Value::Integer(n) => n.to_string(),
            Value::Float(f) => format!("{:.2}", f),
            Value::Str(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Money(v, cur) => format!("{:.2} {}", v, cur.to_uppercase()),
            Value::Array(items) => {
                let parts: Vec<String> = items.iter().map(|v| v.display()).collect();
                format!("[{}]", parts.join(", "))
            }
            Value::Object(model, fields) => {
                let parts = fields
                    .iter()
                    .map(|(name, value)| format!("{}: {}", name, value.display()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{} {{ {} }}", model, parts)
            }
            Value::Nil => "nil".to_string(),
        }
    }

    fn add(&self, other: &Value) -> Result<Value, String> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a + b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(a + *b as f64)),
            (Value::Integer(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),
            (Value::Money(a, ca), Value::Money(b, cb)) => {
                Self::currency_matches(ca, cb)?;
                Ok(Value::Money(a + b, ca.clone()))
            }
            (Value::Str(a), Value::Str(b)) => Ok(Value::Str(format!("{}{}", a, b))),
            (Value::Str(a), other) => Ok(Value::Str(format!("{}{}", a, other.display()))),
            _ => Err(format!("Não é possível somar {:?} e {:?}", self, other)),
        }
    }

    fn sub(&self, other: &Value) -> Result<Value, String> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a - b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
            (Value::Money(a, ca), Value::Money(b, cb)) => {
                Self::currency_matches(ca, cb)?;
                Ok(Value::Money(a - b, ca.clone()))
            }
            _ => Err(format!("Não é possível subtrair {:?} e {:?}", self, other)),
        }
    }

    fn mul(&self, other: &Value) -> Result<Value, String> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a * b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
            (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(a * *b as f64)),
            (Value::Integer(a), Value::Float(b)) => Ok(Value::Float(*a as f64 * b)),
            (Value::Money(a, ca), Value::Float(b)) => Ok(Value::Money(a * b, ca.clone())),
            (Value::Money(a, ca), Value::Integer(b)) => Ok(Value::Money(a * *b as f64, ca.clone())),
            (Value::Float(a), Value::Money(b, cb)) => Ok(Value::Money(a * b, cb.clone())),
            (Value::Integer(a), Value::Money(b, cb)) => Ok(Value::Money(*a as f64 * b, cb.clone())),
            _ => Err(format!(
                "Não é possível multiplicar {:?} e {:?}",
                self, other
            )),
        }
    }

    fn div(&self, other: &Value) -> Result<Value, String> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => {
                if *b == 0 {
                    return Err("Divisão por zero".to_string());
                }
                Ok(Value::Integer(a / b))
            }
            (Value::Float(a), Value::Float(b)) => {
                if *b == 0.0 {
                    return Err("Divisão por zero".to_string());
                }
                Ok(Value::Float(a / b))
            }
            (Value::Float(a), Value::Integer(b)) => {
                if *b == 0 {
                    return Err("Divisão por zero".to_string());
                }
                Ok(Value::Float(a / *b as f64))
            }
            (Value::Integer(a), Value::Float(b)) => {
                if *b == 0.0 {
                    return Err("Divisão por zero".to_string());
                }
                Ok(Value::Float(*a as f64 / b))
            }
            (Value::Money(a, ca), Value::Integer(b)) => {
                if *b == 0 {
                    return Err("Divisão por zero".to_string());
                }
                Ok(Value::Money(a / *b as f64, ca.clone()))
            }
            (Value::Money(a, ca), Value::Float(b)) => {
                if *b == 0.0 {
                    return Err("Divisão por zero".to_string());
                }
                Ok(Value::Money(a / b, ca.clone()))
            }
            _ => Err(format!("Não é possível dividir {:?} por {:?}", self, other)),
        }
    }

    fn compare(&self, other: &Value) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => a.partial_cmp(b),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
            (Value::Float(a), Value::Integer(b)) => a.partial_cmp(&(*b as f64)),
            (Value::Integer(a), Value::Float(b)) => (*a as f64).partial_cmp(b),
            (Value::Money(a, ca), Value::Money(b, cb)) => {
                if ca.eq_ignore_ascii_case(cb) {
                    a.partial_cmp(b)
                } else {
                    None
                }
            }
            (Value::Str(a), Value::Str(b)) => a.partial_cmp(b),
            _ => None,
        }
    }

    fn equals(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Money(a, ca), Value::Money(b, cb)) => a == b && ca == cb,
            (Value::Object(am, af), Value::Object(bm, bf)) => am == bm && af == bf,
            (Value::Nil, Value::Nil) => true,
            _ => false,
        }
    }
}

/// Função definida pelo utilizador
#[derive(Debug, Clone)]
struct Function {
    params: Vec<(String, Type)>,
    body: Vec<Stmt>,
}

/// Model registado
#[derive(Debug, Clone)]
struct ModelDef {
    fields: Vec<Field>,
}

#[derive(Debug, Clone)]
struct RuntimeInvoiceItem {
    description: String,
    qty: f64,
    price: Value,
    total: Value,
}

#[derive(Debug, Clone)]
struct RuntimeInvoice {
    fields: Vec<(String, Value)>,
    items: Vec<RuntimeInvoiceItem>,
    subtotal: Option<Value>,
    discount: Option<Value>,
    tax_amount: Option<Value>,
    total: Option<Value>,
}

/// Sinal de controlo de fluxo
enum Signal {
    None,
    Return(Value),
}

#[derive(Debug, Default)]
struct Scope {
    values: HashMap<String, Value>,
    constants: HashSet<String>,
}

impl Scope {
    fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    fn define(&mut self, name: String, value: Value, is_const: bool) {
        if is_const {
            self.constants.insert(name.clone());
        } else {
            self.constants.remove(&name);
        }
        self.values.insert(name, value);
    }

    fn assign(&mut self, name: &str, value: Value) -> Result<bool, String> {
        if self.constants.contains(name) {
            return Err(format!("Constante '{}' não pode ser reatribuída", name));
        }
        if self.values.contains_key(name) {
            self.values.insert(name.to_string(), value);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

fn find_field<'a>(fields: &'a [(String, Value)], key: &str) -> Option<&'a Value> {
    fields.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn number_to_f64(value: Value) -> Option<f64> {
    match value {
        Value::Integer(n) => Some(n as f64),
        Value::Float(f) => Some(f),
        _ => None,
    }
}

fn money_parts(value: &Value) -> Option<(f64, String)> {
    match value {
        Value::Money(amount, currency) => Some((*amount, currency.clone())),
        _ => None,
    }
}

fn normalize_tax_rate(rate: f64) -> f64 {
    if rate > 1.0 {
        rate / 100.0
    } else {
        rate
    }
}

fn format_qty(qty: f64) -> String {
    if qty.fract() == 0.0 {
        format!("{}", qty as i64)
    } else {
        format!("{:.2}", qty)
    }
}

fn type_is_optional(ty: &Type) -> bool {
    matches!(ty, Type::Optional(_))
}

pub struct Interpreter {
    globals: HashMap<String, Value>,
    functions: HashMap<String, Function>,
    models: HashMap<String, ModelDef>,
    workflows: HashMap<String, Vec<WorkflowStep>>,
    routes: Vec<(String, String, Vec<String>, Vec<Stmt>)>,
    invoices: Vec<RuntimeInvoice>,
    global_constants: HashSet<String>,
    output: Vec<String>,
    capture_output: bool,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        Self::with_capture(false)
    }

    pub fn new_captured() -> Self {
        Self::with_capture(true)
    }

    fn with_capture(capture_output: bool) -> Self {
        Interpreter {
            globals: HashMap::new(),
            functions: HashMap::new(),
            models: HashMap::new(),
            workflows: HashMap::new(),
            routes: Vec::new(),
            invoices: Vec::new(),
            global_constants: HashSet::new(),
            output: Vec::new(),
            capture_output,
        }
    }

    pub fn output(&self) -> &[String] {
        &self.output
    }

    fn emit(&mut self, line: impl Into<String>) {
        let line = line.into();
        if !self.capture_output {
            println!("{}", line);
        }
        self.output.push(line);
    }

    pub fn run(&mut self, program: &Program) -> Result<(), String> {
        // First pass: register declarations
        for decl in &program.decls {
            self.register_decl(decl)?;
        }

        // Second pass: execute top-level statements
        let mut top_scope = Scope::default();
        for decl in &program.decls {
            if let Decl::Statement(stmt) = decl {
                let signal = self.exec_stmt(stmt, &mut top_scope)?;
                self.sync_globals_from(&top_scope);
                if let Signal::Return(v) = signal {
                    self.emit(v.display());
                }
            }
        }

        // Print registered routes and workflows as a summary.
        let mut summary = Vec::new();
        if !self.models.is_empty() {
            summary.push(String::new());
            summary.push("📦 Models registados:".to_string());
            for (name, model) in &self.models {
                let fields: Vec<String> = model
                    .fields
                    .iter()
                    .map(|f| format!("  {} {:?}", f.name, f.ty))
                    .collect();
                summary.push(format!("  ▸ {} {{ {} }}", name, fields.join(", ")));
            }
        }

        if !self.workflows.is_empty() {
            summary.push(String::new());
            summary.push("⚙️  Workflows registados:".to_string());
            for (name, steps) in &self.workflows {
                let names: Vec<String> = steps.iter().map(|step| step.name.clone()).collect();
                summary.push(format!("  ▸ {} → [{}]", name, names.join(" → ")));
            }
        }

        if !self.routes.is_empty() {
            summary.push(String::new());
            summary.push("🌐 Routes registadas:".to_string());
            for (method, path, params, _) in &self.routes {
                if params.is_empty() {
                    summary.push(format!("  ▸ {} {}", method, path));
                } else {
                    summary.push(format!(
                        "  ▸ {} {} params=[{}]",
                        method,
                        path,
                        params.join(", ")
                    ));
                }
            }
        }

        if !self.invoices.is_empty() {
            summary.push(String::new());
            summary.push("🧾 Invoices:".to_string());
            for inv in &self.invoices {
                summary.push("  ▸ Fatura:".to_string());
                for (k, v) in &inv.fields {
                    summary.push(format!("    {} : {}", k, v.display()));
                }
                for item in &inv.items {
                    summary.push(format!(
                        "    item : {} x {} @ {} = {}",
                        format_qty(item.qty),
                        item.description,
                        item.price.display(),
                        item.total.display()
                    ));
                }
                if let Some(subtotal) = &inv.subtotal {
                    summary.push(format!("    subtotal : {}", subtotal.display()));
                }
                if let Some(discount) = &inv.discount {
                    summary.push(format!("    discount : {}", discount.display()));
                }
                if let Some(tax_amount) = &inv.tax_amount {
                    summary.push(format!("    tax_amount : {}", tax_amount.display()));
                }
                if let Some(total) = &inv.total {
                    summary.push(format!("    total_auto : {}", total.display()));
                }
            }
        }

        for line in summary {
            self.emit(line);
        }

        Ok(())
    }

    fn sync_globals_from(&mut self, scope: &Scope) {
        self.globals = scope.values.clone();
        self.global_constants = scope.constants.clone();
    }

    fn register_decl(&mut self, decl: &Decl) -> Result<(), String> {
        match decl {
            Decl::Function {
                name, params, body, ..
            } => {
                self.functions.insert(
                    name.clone(),
                    Function {
                        params: params.clone(),
                        body: body.clone(),
                    },
                );
            }
            Decl::Model { name, fields, .. } => {
                self.models.insert(
                    name.clone(),
                    ModelDef {
                        fields: fields.clone(),
                    },
                );
            }
            Decl::Workflow { name, steps, .. } => {
                self.workflows.insert(name.clone(), steps.clone());
            }
            Decl::Route {
                method,
                path,
                params,
                body,
                ..
            } => {
                let method_str = match method {
                    HttpMethod::Get => "GET",
                    HttpMethod::Post => "POST",
                    HttpMethod::Put => "PUT",
                    HttpMethod::Delete => "DELETE",
                }
                .to_string();
                self.routes
                    .push((method_str, path.clone(), params.clone(), body.clone()));
            }
            Decl::Invoice { fields, items, .. } => {
                let mut evaluated = Vec::new();
                let mut locals = Scope::default();
                for f in fields {
                    let val = self.eval_expr(&f.value, &mut locals)?;
                    evaluated.push((f.key.clone(), val));
                }
                let invoice = self.build_invoice(evaluated, items, &mut locals)?;
                self.invoices.push(invoice);
            }
            Decl::Statement(_) => {} // handled in run()
        }
        Ok(())
    }

    fn build_invoice(
        &mut self,
        fields: Vec<(String, Value)>,
        items: &[InvoiceItem],
        locals: &mut Scope,
    ) -> Result<RuntimeInvoice, String> {
        let mut runtime_items = Vec::new();
        let mut subtotal: Option<Value> = None;

        for item in items {
            let description = match self.eval_expr(&item.description, locals)? {
                Value::Str(s) => s,
                v => {
                    return Err(format!(
                        "Invoice item description espera string, encontrado {}",
                        v.display()
                    ))
                }
            };
            let qty = number_to_f64(self.eval_expr(&item.qty, locals)?)
                .ok_or_else(|| "Invoice item qty espera int ou float".to_string())?;
            let price = self.eval_expr(&item.price, locals)?;
            let (price_amount, currency) =
                money_parts(&price).ok_or_else(|| "Invoice item price espera money".to_string())?;
            let line_total = Value::Money(price_amount * qty, currency.clone());

            subtotal = Some(match subtotal {
                Some(Value::Money(current, cur)) => {
                    Value::currency_matches(&cur, &currency)?;
                    Value::Money(current + price_amount * qty, cur)
                }
                Some(_) => unreachable!(),
                None => Value::Money(price_amount * qty, currency),
            });

            runtime_items.push(RuntimeInvoiceItem {
                description,
                qty,
                price,
                total: line_total,
            });
        }

        let discount = find_field(&fields, "discount").cloned();
        let tax_rate = find_field(&fields, "tax")
            .and_then(|v| number_to_f64(v.clone()))
            .map(normalize_tax_rate);

        let (tax_amount, total) = match (&subtotal, &discount, tax_rate) {
            (Some(Value::Money(sub, cur)), discount, rate) => {
                let discount_amount = match discount {
                    Some(Value::Money(v, discount_cur)) => {
                        Value::currency_matches(cur, discount_cur)?;
                        *v
                    }
                    Some(v) => {
                        return Err(format!(
                            "Invoice discount espera money, encontrado {}",
                            v.display()
                        ))
                    }
                    None => 0.0,
                };
                let taxable = sub - discount_amount;
                let tax_amount = rate.map(|r| Value::Money(taxable * r, cur.clone()));
                let total = Value::Money(
                    taxable
                        + tax_amount
                            .as_ref()
                            .and_then(money_parts)
                            .map(|(v, _)| v)
                            .unwrap_or(0.0),
                    cur.clone(),
                );
                (tax_amount, Some(total))
            }
            _ => (None, None),
        };

        Ok(RuntimeInvoice {
            fields,
            items: runtime_items,
            subtotal,
            discount,
            tax_amount,
            total,
        })
    }

    fn run_workflow(&mut self, name: &str) -> Result<(), String> {
        let steps = self
            .workflows
            .get(name)
            .cloned()
            .ok_or_else(|| format!("Workflow '{}' não encontrado", name))?;

        self.emit(format!("▶ Workflow {}", name));
        let mut scope = Scope {
            values: self.globals.clone(),
            constants: self.global_constants.clone(),
        };

        for step in steps {
            self.emit(format!("  step {}", step.name));
            match self.exec_block(&step.body, &mut scope)? {
                Signal::Return(v) => self.emit(format!("  return {}", v.display())),
                Signal::None => {}
            }
            self.sync_globals_from(&scope);
        }

        Ok(())
    }

    fn exec_block(&mut self, stmts: &[Stmt], locals: &mut Scope) -> Result<Signal, String> {
        for stmt in stmts {
            let signal = self.exec_stmt(stmt, locals)?;
            match signal {
                Signal::None => {}
                other => return Ok(other),
            }
        }
        Ok(Signal::None)
    }

    fn exec_stmt(&mut self, stmt: &Stmt, locals: &mut Scope) -> Result<Signal, String> {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let val = self.eval_expr(value, locals)?;
                locals.define(name.clone(), val, false);
                Ok(Signal::None)
            }

            Stmt::Const { name, value, .. } => {
                let val = self.eval_expr(value, locals)?;
                locals.define(name.clone(), val, true);
                Ok(Signal::None)
            }

            Stmt::Assign { name, value, .. } => {
                let val = self.eval_expr(value, locals)?;
                if locals.assign(name, val.clone())? {
                    return Ok(Signal::None);
                }
                if self.global_constants.contains(name) {
                    return Err(format!("Constante '{}' não pode ser reatribuída", name));
                }
                if self.globals.contains_key(name) {
                    self.globals.insert(name.clone(), val);
                } else {
                    return Err(format!("Variável '{}' não definida", name));
                }
                Ok(Signal::None)
            }

            Stmt::Return { value, .. } => {
                let val = self.eval_expr(value, locals)?;
                Ok(Signal::Return(val))
            }

            Stmt::Print { value, .. } => {
                let val = self.eval_expr(value, locals)?;
                self.emit(val.display());
                Ok(Signal::None)
            }

            Stmt::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                let cond = self.eval_expr(condition, locals)?;
                if cond.is_truthy() {
                    self.exec_block(then_body, locals)
                } else if let Some(else_stmts) = else_body {
                    self.exec_block(else_stmts, locals)
                } else {
                    Ok(Signal::None)
                }
            }

            Stmt::While {
                condition, body, ..
            } => {
                loop {
                    let cond = self.eval_expr(condition, locals)?;
                    if !cond.is_truthy() {
                        break;
                    }
                    if let Signal::Return(v) = self.exec_block(body, locals)? {
                        return Ok(Signal::Return(v));
                    }
                }
                Ok(Signal::None)
            }

            Stmt::For {
                var,
                iterable,
                body,
                ..
            } => {
                let items = self.eval_expr(iterable, locals)?;
                let arr = match items {
                    Value::Array(a) => a,
                    v => vec![v],
                };
                for item in arr {
                    locals.define(var.clone(), item, false);
                    if let Signal::Return(v) = self.exec_block(body, locals)? {
                        return Ok(Signal::Return(v));
                    }
                }
                Ok(Signal::None)
            }

            Stmt::ExprStmt { expr, .. } => {
                self.eval_expr(expr, locals)?;
                Ok(Signal::None)
            }
        }
    }

    fn eval_expr(&mut self, expr: &Expr, locals: &mut Scope) -> Result<Value, String> {
        match expr {
            Expr::Integer { value, .. } => Ok(Value::Integer(*value)),
            Expr::Float { value, .. } => Ok(Value::Float(*value)),
            Expr::StringLit { value, .. } => Ok(Value::Str(value.clone())),
            Expr::Bool { value, .. } => Ok(Value::Bool(*value)),
            Expr::Money {
                value, currency, ..
            } => Ok(Value::Money(*value, currency.clone())),
            Expr::Nil { .. } => Ok(Value::Nil),

            Expr::Array { items, .. } => {
                let mut vals = Vec::new();
                for item in items {
                    vals.push(self.eval_expr(item, locals)?);
                }
                Ok(Value::Array(vals))
            }

            Expr::Object { model, fields, .. } => {
                let mut vals = Vec::new();
                for field in fields {
                    vals.push((field.name.clone(), self.eval_expr(&field.value, locals)?));
                }
                if let Some(model_fields) = self
                    .models
                    .get(model)
                    .map(|model_def| model_def.fields.clone())
                {
                    let mut ordered = Vec::new();
                    for field in &model_fields {
                        if let Some(pos) = vals.iter().position(|(name, _)| name == &field.name) {
                            ordered.push(vals.remove(pos));
                        } else if let Some(default) = &field.default {
                            ordered.push((field.name.clone(), self.eval_expr(default, locals)?));
                        } else if type_is_optional(&field.ty) {
                            ordered.push((field.name.clone(), Value::Nil));
                        }
                    }
                    ordered.extend(vals);
                    vals = ordered;
                }
                Ok(Value::Object(model.clone(), vals))
            }

            Expr::FieldAccess { object, field, .. } => match self.eval_expr(object, locals)? {
                Value::Object(model, fields) => fields
                    .into_iter()
                    .find(|(name, _)| name == field)
                    .map(|(_, value)| value)
                    .ok_or_else(|| format!("Campo '{}.{}' nao existe", model, field)),
                other => Err(format!(
                    "Acesso a campo '{}' espera model instance, encontrado {}",
                    field,
                    other.display()
                )),
            },

            Expr::Ident { name, .. } => {
                if let Some(v) = locals.get(name) {
                    return Ok(v.clone());
                }
                if let Some(v) = self.globals.get(name) {
                    return Ok(v.clone());
                }
                Err(format!("Variável '{}' não definida", name))
            }

            Expr::BinOp {
                left, op, right, ..
            } => {
                let lv = self.eval_expr(left, locals)?;
                let rv = self.eval_expr(right, locals)?;

                match op {
                    BinOp::Add => lv.add(&rv),
                    BinOp::Sub => lv.sub(&rv),
                    BinOp::Mul => lv.mul(&rv),
                    BinOp::Div => lv.div(&rv),
                    BinOp::Mod => match (&lv, &rv) {
                        (Value::Integer(_), Value::Integer(0)) => {
                            Err("Módulo por zero".to_string())
                        }
                        (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a % b)),
                        _ => Err("Módulo apenas funciona com inteiros".to_string()),
                    },
                    BinOp::Eq => Ok(Value::Bool(lv.equals(&rv))),
                    BinOp::NotEq => Ok(Value::Bool(!lv.equals(&rv))),
                    BinOp::Lt => Ok(Value::Bool(
                        lv.compare(&rv).map(|o| o.is_lt()).unwrap_or(false),
                    )),
                    BinOp::LtEq => Ok(Value::Bool(
                        lv.compare(&rv).map(|o| o.is_le()).unwrap_or(false),
                    )),
                    BinOp::Gt => Ok(Value::Bool(
                        lv.compare(&rv).map(|o| o.is_gt()).unwrap_or(false),
                    )),
                    BinOp::GtEq => Ok(Value::Bool(
                        lv.compare(&rv).map(|o| o.is_ge()).unwrap_or(false),
                    )),
                    BinOp::And => Ok(Value::Bool(lv.is_truthy() && rv.is_truthy())),
                    BinOp::Or => Ok(Value::Bool(lv.is_truthy() || rv.is_truthy())),
                }
            }

            Expr::UnaryOp { op, expr, .. } => {
                let val = self.eval_expr(expr, locals)?;
                match op {
                    UnaryOp::Neg => match val {
                        Value::Integer(n) => Ok(Value::Integer(-n)),
                        Value::Float(f) => Ok(Value::Float(-f)),
                        Value::Money(v, c) => Ok(Value::Money(-v, c)),
                        _ => Err("Operador unário negativo inválido".to_string()),
                    },
                    UnaryOp::Not => Ok(Value::Bool(!val.is_truthy())),
                }
            }

            Expr::Call { name, args, .. } => {
                let mut eval_args = Vec::new();
                for arg in args {
                    eval_args.push(self.eval_expr(arg, locals)?);
                }
                self.call_function(name, eval_args)
            }

            Expr::StaticCall { ty, method, .. } => {
                // ERP static calls like Employee::all()
                let model_name = ty.clone();
                let method_name = method.clone();
                if self.models.contains_key(&model_name) {
                    Ok(Value::Array(vec![Value::Str(format!(
                        "{}.{}() → lista de registos",
                        model_name, method_name
                    ))]))
                } else {
                    Err(format!("Model '{}' não encontrado", model_name))
                }
            }
        }
    }

    fn call_function(&mut self, name: &str, args: Vec<Value>) -> Result<Value, String> {
        // Built-in functions
        match name {
            "print" => {
                if let Some(v) = args.first() {
                    self.emit(v.display());
                }
                return Ok(Value::Nil);
            }
            "len" => match args.first() {
                Some(Value::Array(a)) => return Ok(Value::Integer(a.len() as i64)),
                Some(Value::Str(s)) => return Ok(Value::Integer(s.chars().count() as i64)),
                Some(v) => return Err(format!("len() não aceita {}", v.display())),
                None => return Err("len() recebe exatamente 1 argumento".to_string()),
            },
            "str" => {
                if let Some(v) = args.first() {
                    return Ok(Value::Str(v.display()));
                }
                return Ok(Value::Str(String::new()));
            }
            "run_workflow" => {
                let Some(Value::Str(name)) = args.first() else {
                    return Err("run_workflow() espera o nome do workflow".to_string());
                };
                self.run_workflow(name)?;
                return Ok(Value::Nil);
            }
            _ => {}
        }

        // User-defined function
        let func = self
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| format!("Função '{}' não definida", name))?;

        let mut fn_locals = Scope::default();
        for (i, (param_name, _)) in func.params.iter().enumerate() {
            fn_locals.define(
                param_name.clone(),
                args.get(i).cloned().unwrap_or(Value::Nil),
                false,
            );
        }

        match self.exec_block(&func.body, &mut fn_locals)? {
            Signal::Return(v) => Ok(v),
            _ => Ok(Value::Nil),
        }
    }
}
