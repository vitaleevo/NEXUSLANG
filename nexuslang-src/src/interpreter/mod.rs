use crate::ast::*;
/// NexusLang Interpreter — executa a AST
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use sha2::{Digest, Sha256};

use crate::diagnostic::{Diagnostic, DiagnosticStage};

fn runtime_error(message: impl Into<String>) -> Diagnostic {
    let message = message.into();
    let code = crate::diagnostic::runtime_code_for_message(&message);
    let diagnostic = Diagnostic::new(DiagnosticStage::Runtime, &message).with_code(code);
    crate::diagnostic::enrich_runtime_diagnostic(diagnostic, code)
}

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
            (Value::Integer(a), Value::Float(b)) => *a as f64 == *b,
            (Value::Float(a), Value::Integer(b)) => *a == *b as f64,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Money(a, ca), Value::Money(b, cb)) => a == b && ca == cb,
            (Value::Array(a), Value::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| a.equals(b))
            }
            (Value::Object(am, af), Value::Object(bm, bf)) => am == bm && af == bf,
            (Value::Nil, Value::Nil) => true,
            _ => false,
        }
    }
}

fn expect_arg_count(name: &str, args: &[Value], expected: usize) -> Result<(), String> {
    if args.len() == expected {
        Ok(())
    } else {
        Err(format!(
            "{} espera {} argumento(s), recebeu {}",
            name,
            expected,
            args.len()
        ))
    }
}

fn expect_arg_count_range(
    name: &str,
    args: &[Value],
    min: usize,
    max: usize,
) -> Result<(), String> {
    if (min..=max).contains(&args.len()) {
        Ok(())
    } else {
        Err(format!(
            "{} espera {} a {} argumento(s), recebeu {}",
            name,
            min,
            max,
            args.len()
        ))
    }
}

fn optional_assert_message(
    name: &str,
    args: &[Value],
    index: usize,
) -> Result<Option<String>, String> {
    match args.get(index) {
        Some(Value::Str(message)) if !message.is_empty() => Ok(Some(message.clone())),
        Some(Value::Str(_)) | None => Ok(None),
        Some(other) => Err(format!(
            "{} espera string como mensagem, encontrado {}",
            name,
            other.display()
        )),
    }
}

fn assertion_failure(name: &str, details: impl AsRef<str>, message: Option<String>) -> String {
    match message {
        Some(message) => format!("{} falhou: {}; {}", name, message, details.as_ref()),
        None => format!("{} falhou: {}", name, details.as_ref()),
    }
}

fn assert_contains_value(container: &Value, needle: &Value) -> Result<bool, String> {
    match (container, needle) {
        (Value::Str(container), Value::Str(needle)) => Ok(container.contains(needle)),
        (Value::Str(_), other) => Err(format!(
            "assert_contains espera string como valor procurado, encontrado {}",
            other.display()
        )),
        (Value::Array(items), needle) => Ok(items.iter().any(|item| item.equals(needle))),
        (other, _) => Err(format!(
            "assert_contains espera string ou array, encontrado {}",
            other.display()
        )),
    }
}

fn expect_string_arg(name: &str, args: &[Value]) -> Result<String, String> {
    expect_arg_count(name, args, 1)?;
    match &args[0] {
        Value::Str(value) => Ok(value.clone()),
        other => Err(format!(
            "{} espera string, encontrado {}",
            name,
            other.display()
        )),
    }
}

fn expect_int_arg(name: &str, args: &[Value]) -> Result<i64, String> {
    expect_arg_count(name, args, 1)?;
    match args[0] {
        Value::Integer(value) => Ok(value),
        ref other => Err(format!(
            "{} espera int, encontrado {}",
            name,
            other.display()
        )),
    }
}

fn expect_two_strings(name: &str, args: &[Value]) -> Result<(String, String), String> {
    expect_arg_count(name, args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Str(left), Value::Str(right)) => Ok((left.clone(), right.clone())),
        _ => Err(format!("{} espera dois argumentos string", name)),
    }
}

fn expect_int_array_arg(name: &str, args: &[Value]) -> Result<Vec<i64>, String> {
    expect_arg_count(name, args, 1)?;
    collect_int_array(name, &args[0])
}

fn expect_string_array_arg(name: &str, args: &[Value]) -> Result<Vec<String>, String> {
    expect_arg_count(name, args, 1)?;
    collect_string_array(name, &args[0])
}

fn expect_int_array_and_int(name: &str, args: &[Value]) -> Result<(Vec<i64>, i64), String> {
    expect_arg_count(name, args, 2)?;
    let items = collect_int_array(name, &args[0])?;
    let Value::Integer(needle) = args[1] else {
        return Err(format!("{} espera int como segundo argumento", name));
    };
    Ok((items, needle))
}

fn expect_string_array_and_string(
    name: &str,
    args: &[Value],
) -> Result<(Vec<String>, String), String> {
    expect_arg_count(name, args, 2)?;
    let items = collect_string_array(name, &args[0])?;
    let Value::Str(needle) = &args[1] else {
        return Err(format!("{} espera string como segundo argumento", name));
    };
    Ok((items, needle.clone()))
}

fn collect_int_array(name: &str, value: &Value) -> Result<Vec<i64>, String> {
    let Value::Array(items) = value else {
        return Err(format!("{} espera array de int", name));
    };

    items
        .iter()
        .map(|item| match item {
            Value::Integer(value) => Ok(*value),
            other => Err(format!(
                "{} espera array de int, encontrou {}",
                name,
                other.display()
            )),
        })
        .collect()
}

fn collect_string_array(name: &str, value: &Value) -> Result<Vec<String>, String> {
    let Value::Array(items) = value else {
        return Err(format!("{} espera array de string", name));
    };

    items
        .iter()
        .map(|item| match item {
            Value::Str(value) => Ok(value.clone()),
            other => Err(format!(
                "{} espera array de string, encontrou {}",
                name,
                other.display()
            )),
        })
        .collect()
}

fn expect_money_arg(name: &str, args: &[Value]) -> Result<(f64, String), String> {
    expect_arg_count(name, args, 1)?;
    match &args[0] {
        Value::Money(amount, currency) => Ok((*amount, currency.clone())),
        other => Err(format!(
            "{} espera money, encontrado {}",
            name,
            other.display()
        )),
    }
}

fn expect_two_money(name: &str, args: &[Value]) -> Result<(String, String), String> {
    expect_arg_count(name, args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Money(_, left), Value::Money(_, right)) => Ok((left.clone(), right.clone())),
        _ => Err(format!("{} espera dois argumentos money", name)),
    }
}

fn is_basic_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };

    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains("..")
}

fn parse_iso_date(value: &str) -> Option<(i32, u32, u32)> {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }

    let year = value[0..4].parse::<i32>().ok()?;
    let month = value[5..7].parse::<u32>().ok()?;
    let day = value[8..10].parse::<u32>().ok()?;

    if month == 0 || month > 12 {
        return None;
    }

    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => return None,
    };

    if day == 0 || day > max_day {
        return None;
    }

    Some((year, month, day))
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();

    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch <= '\u{1f}' => {
                let _ = write!(escaped, "\\u{:04x}", ch as u32);
            }
            ch => escaped.push(ch),
        }
    }

    escaped
}

fn json_string(value: &str) -> String {
    format!("\"{}\"", json_escape(value))
}

fn json_is_wrapped(value: &str, open: char, close: char) -> bool {
    let value = value.trim();
    value.starts_with(open) && value.ends_with(close)
}

fn csv_needs_quotes(value: &str) -> bool {
    value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r')
}

fn csv_escape_cell(value: &str) -> String {
    if csv_needs_quotes(value) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn http_status_text(code: i64) -> &'static str {
    match code {
        100 => "Continue",
        101 => "Switching Protocols",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Unknown Status",
    }
}

fn http_method_allows_body(method: &str) -> bool {
    matches!(
        method.trim().to_ascii_uppercase().as_str(),
        "POST" | "PUT" | "PATCH"
    )
}

fn http_url_encode(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    let mut encoded = String::new();
    for byte in value.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char);
            }
            byte => {
                encoded.push('%');
                encoded.push(HEX[(byte >> 4) as usize] as char);
                encoded.push(HEX[(byte & 0x0f) as usize] as char);
            }
        }
    }

    encoded
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn sha256_hex(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    hex_encode(&digest)
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let max_len = left.len().max(right.len());
    let mut diff = left.len() ^ right.len();

    for index in 0..max_len {
        let left_byte = left.get(index).copied().unwrap_or(0);
        let right_byte = right.get(index).copied().unwrap_or(0);
        diff |= (left_byte ^ right_byte) as usize;
    }

    diff == 0
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn runtime_clock_available() -> bool {
    !cfg!(target_arch = "wasm32")
}

fn unix_millis_now() -> i64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Ok(duration) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        else {
            return 0;
        };
        i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
    }

    #[cfg(target_arch = "wasm32")]
    {
        0
    }
}

fn unix_seconds_now() -> i64 {
    unix_millis_now() / 1000
}

fn env_runtime_available() -> bool {
    !cfg!(target_arch = "wasm32")
}

fn env_get_value(name: &str) -> String {
    #[cfg(not(target_arch = "wasm32"))]
    {
        crate::runtime_env::var_string(name).unwrap_or_default()
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;
        String::new()
    }
}

fn env_has_value(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        crate::runtime_env::var_os(name).is_some()
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;
        false
    }
}

fn path_normalize(value: &str) -> String {
    let value = value.replace('\\', "/");
    let bytes = value.as_bytes();
    let has_drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    let (prefix, rest) = if has_drive {
        (&value[..2], &value[2..])
    } else {
        ("", value.as_str())
    };
    let absolute = rest.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();

    for part in rest.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|last| *last != "..") {
                    parts.pop();
                } else if !absolute {
                    parts.push(part);
                }
            }
            _ => parts.push(part),
        }
    }

    let mut result = String::new();
    result.push_str(prefix);
    if absolute {
        result.push('/');
    }
    result.push_str(&parts.join("/"));

    if result.is_empty() {
        if absolute {
            "/".to_string()
        } else {
            ".".to_string()
        }
    } else {
        result
    }
}

fn path_is_drive_root(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/'
}

fn path_trim_trailing(value: &str) -> String {
    let mut value = path_normalize(value);
    while value.len() > 1 && value.ends_with('/') && !path_is_drive_root(&value) {
        value.pop();
    }
    value
}

fn path_is_absolute(value: &str) -> bool {
    let value = value.replace('\\', "/");
    let bytes = value.as_bytes();
    value.starts_with('/')
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && bytes[2] == b'/')
}

fn path_join(left: &str, right: &str) -> String {
    if right.is_empty() {
        return path_trim_trailing(left);
    }
    if left.is_empty() || path_is_absolute(right) {
        return path_normalize(right);
    }

    let left = path_trim_trailing(left);
    if left == "." {
        return path_normalize(right);
    }

    path_normalize(&format!("{}/{}", left, right))
}

fn path_basename(value: &str) -> String {
    let value = path_trim_trailing(value);
    if value == "/" || path_is_drive_root(&value) {
        return value;
    }

    value.rsplit('/').next().unwrap_or("").to_string()
}

fn path_dirname(value: &str) -> String {
    let value = path_trim_trailing(value);
    if value == "/" || path_is_drive_root(&value) {
        return value;
    }

    match value.rfind('/') {
        Some(0) => "/".to_string(),
        Some(2) if path_is_drive_root(&value[..=2]) => value[..=2].to_string(),
        Some(index) => value[..index].to_string(),
        None => ".".to_string(),
    }
}

fn path_extension(value: &str) -> String {
    let basename = path_basename(value);
    match basename.rfind('.') {
        Some(0) | None => String::new(),
        Some(index) => basename[index + 1..].to_string(),
    }
}

fn path_stem(value: &str) -> String {
    let basename = path_basename(value);
    match basename.rfind('.') {
        Some(0) | None => basename,
        Some(index) => basename[..index].to_string(),
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

    pub fn run(&mut self, program: &Program) -> Result<(), Diagnostic> {
        // First pass: register declarations
        for decl in &program.decls {
            self.register_decl(decl).map_err(runtime_error)?;
        }

        // Register import aliases: when `import X as Y` is used in the merged
        // program, copy the model / function / workflow entry for "X" under
        // the alias name "Y" so that runtime lookups (object construction,
        // static calls, function calls) resolve correctly.
        for decl in &program.decls {
            if let Decl::Import {
                import:
                    ImportDecl {
                        name,
                        alias: Some(alias),
                        ..
                    },
            } = decl
            {
                if let Some(model_def) = self.models.get(name) {
                    self.models.insert(alias.clone(), model_def.clone());
                }
                if let Some(func) = self.functions.get(name) {
                    self.functions.insert(alias.clone(), func.clone());
                }
                if let Some(steps) = self.workflows.get(name) {
                    self.workflows.insert(alias.clone(), steps.clone());
                }
            }
        }

        // Second pass: execute top-level statements
        let mut top_scope = Scope::default();
        for decl in &program.decls {
            if let Decl::Statement(stmt) = decl {
                let signal = self
                    .exec_stmt(stmt, &mut top_scope)
                    .map_err(runtime_error)?;
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
            Decl::Auth { .. } => {}
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
            Decl::Import { .. } => {
                // Imports are not executable in single-file mode.
            }
            Decl::Export { decl: inner, .. } => {
                self.register_decl(inner)?;
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
            "assert_true" => {
                expect_arg_count_range(name, &args, 1, 2)?;
                let message = optional_assert_message(name, &args, 1)?;
                match &args[0] {
                    Value::Bool(true) => return Ok(Value::Nil),
                    Value::Bool(false) => {
                        return Err(assertion_failure(
                            name,
                            "esperado true, recebido false",
                            message,
                        ));
                    }
                    other => {
                        return Err(format!(
                            "assert_true espera bool, encontrado {}",
                            other.display()
                        ))
                    }
                }
            }
            "assert_eq" => {
                expect_arg_count_range(name, &args, 2, 3)?;
                let message = optional_assert_message(name, &args, 2)?;
                let actual = &args[0];
                let expected = &args[1];
                if actual.equals(expected) {
                    return Ok(Value::Nil);
                }
                return Err(assertion_failure(
                    name,
                    format!(
                        "esperado {}, recebido {}",
                        expected.display(),
                        actual.display()
                    ),
                    message,
                ));
            }
            "assert_ne" => {
                expect_arg_count_range(name, &args, 2, 3)?;
                let message = optional_assert_message(name, &args, 2)?;
                let actual = &args[0];
                let expected = &args[1];
                if !actual.equals(expected) {
                    return Ok(Value::Nil);
                }
                return Err(assertion_failure(
                    name,
                    format!("valor nao deveria ser {}", actual.display()),
                    message,
                ));
            }
            "assert_contains" => {
                expect_arg_count_range(name, &args, 2, 3)?;
                let message = optional_assert_message(name, &args, 2)?;
                let container = &args[0];
                let needle = &args[1];
                if assert_contains_value(container, needle)? {
                    return Ok(Value::Nil);
                }
                return Err(assertion_failure(
                    name,
                    format!(
                        "esperado conter {}, recebido {}",
                        needle.display(),
                        container.display()
                    ),
                    message,
                ));
            }
            "__std_string_contains" => {
                let (s, needle) = expect_two_strings(name, &args)?;
                return Ok(Value::Bool(s.contains(&needle)));
            }
            "__std_string_starts_with" => {
                let (s, prefix) = expect_two_strings(name, &args)?;
                return Ok(Value::Bool(s.starts_with(&prefix)));
            }
            "__std_string_ends_with" => {
                let (s, suffix) = expect_two_strings(name, &args)?;
                return Ok(Value::Bool(s.ends_with(&suffix)));
            }
            "__std_string_to_upper" => {
                let s = expect_string_arg(name, &args)?;
                return Ok(Value::Str(s.to_uppercase()));
            }
            "__std_string_to_lower" => {
                let s = expect_string_arg(name, &args)?;
                return Ok(Value::Str(s.to_lowercase()));
            }
            "__std_string_trim" => {
                let s = expect_string_arg(name, &args)?;
                return Ok(Value::Str(s.trim().to_string()));
            }
            "__std_array_contains_int" => {
                let (items, needle) = expect_int_array_and_int(name, &args)?;
                return Ok(Value::Bool(items.contains(&needle)));
            }
            "__std_array_contains_string" => {
                let (items, needle) = expect_string_array_and_string(name, &args)?;
                return Ok(Value::Bool(items.iter().any(|item| item == &needle)));
            }
            "__std_array_first_int" => {
                let items = expect_int_array_arg(name, &args)?;
                return items
                    .first()
                    .copied()
                    .map(Value::Integer)
                    .ok_or_else(|| format!("{} nao aceita array vazio", name));
            }
            "__std_array_first_string" => {
                let items = expect_string_array_arg(name, &args)?;
                return items
                    .first()
                    .cloned()
                    .map(Value::Str)
                    .ok_or_else(|| format!("{} nao aceita array vazio", name));
            }
            "__std_array_last_int" => {
                let items = expect_int_array_arg(name, &args)?;
                return items
                    .last()
                    .copied()
                    .map(Value::Integer)
                    .ok_or_else(|| format!("{} nao aceita array vazio", name));
            }
            "__std_array_last_string" => {
                let items = expect_string_array_arg(name, &args)?;
                return items
                    .last()
                    .cloned()
                    .map(Value::Str)
                    .ok_or_else(|| format!("{} nao aceita array vazio", name));
            }
            "__std_array_reverse_int" => {
                let mut items = expect_int_array_arg(name, &args)?;
                items.reverse();
                return Ok(Value::Array(
                    items.into_iter().map(Value::Integer).collect(),
                ));
            }
            "__std_array_reverse_string" => {
                let mut items = expect_string_array_arg(name, &args)?;
                items.reverse();
                return Ok(Value::Array(items.into_iter().map(Value::Str).collect()));
            }
            "__std_validation_is_blank" => {
                let s = expect_string_arg(name, &args)?;
                return Ok(Value::Bool(s.trim().is_empty()));
            }
            "__std_validation_is_email" => {
                let s = expect_string_arg(name, &args)?;
                return Ok(Value::Bool(is_basic_email(&s)));
            }
            "__std_json_escape" => {
                let s = expect_string_arg(name, &args)?;
                return Ok(Value::Str(json_escape(&s)));
            }
            "__std_json_string" => {
                let s = expect_string_arg(name, &args)?;
                return Ok(Value::Str(json_string(&s)));
            }
            "__std_json_is_object" => {
                let s = expect_string_arg(name, &args)?;
                return Ok(Value::Bool(json_is_wrapped(&s, '{', '}')));
            }
            "__std_json_is_array" => {
                let s = expect_string_arg(name, &args)?;
                return Ok(Value::Bool(json_is_wrapped(&s, '[', ']')));
            }
            "__std_csv_needs_quotes" => {
                let s = expect_string_arg(name, &args)?;
                return Ok(Value::Bool(csv_needs_quotes(&s)));
            }
            "__std_csv_escape_cell" => {
                let s = expect_string_arg(name, &args)?;
                return Ok(Value::Str(csv_escape_cell(&s)));
            }
            "__std_http_status_text" => {
                let code = expect_int_arg(name, &args)?;
                return Ok(Value::Str(http_status_text(code).to_string()));
            }
            "__std_http_method_allows_body" => {
                let method = expect_string_arg(name, &args)?;
                return Ok(Value::Bool(http_method_allows_body(&method)));
            }
            "__std_http_url_encode" => {
                let s = expect_string_arg(name, &args)?;
                return Ok(Value::Str(http_url_encode(&s)));
            }
            "__std_crypto_sha256_hex" => {
                let s = expect_string_arg(name, &args)?;
                return Ok(Value::Str(sha256_hex(&s)));
            }
            "__std_crypto_constant_time_eq" => {
                let (left, right) = expect_two_strings(name, &args)?;
                return Ok(Value::Bool(constant_time_eq(&left, &right)));
            }
            "__std_crypto_is_sha256_hex" => {
                let s = expect_string_arg(name, &args)?;
                return Ok(Value::Bool(is_sha256_hex(&s)));
            }
            "__std_time_runtime_clock_available" => {
                expect_arg_count(name, &args, 0)?;
                return Ok(Value::Bool(runtime_clock_available()));
            }
            "__std_time_unix_seconds" => {
                expect_arg_count(name, &args, 0)?;
                return Ok(Value::Integer(unix_seconds_now()));
            }
            "__std_time_unix_millis" => {
                expect_arg_count(name, &args, 0)?;
                return Ok(Value::Integer(unix_millis_now()));
            }
            "__std_env_runtime_available" => {
                expect_arg_count(name, &args, 0)?;
                return Ok(Value::Bool(env_runtime_available()));
            }
            "__std_env_get" => {
                let key = expect_string_arg(name, &args)?;
                return Ok(Value::Str(env_get_value(&key)));
            }
            "__std_env_has" => {
                let key = expect_string_arg(name, &args)?;
                return Ok(Value::Bool(env_has_value(&key)));
            }
            "__std_path_join" => {
                let (left, right) = expect_two_strings(name, &args)?;
                return Ok(Value::Str(path_join(&left, &right)));
            }
            "__std_path_basename" => {
                let value = expect_string_arg(name, &args)?;
                return Ok(Value::Str(path_basename(&value)));
            }
            "__std_path_dirname" => {
                let value = expect_string_arg(name, &args)?;
                return Ok(Value::Str(path_dirname(&value)));
            }
            "__std_path_extension" => {
                let value = expect_string_arg(name, &args)?;
                return Ok(Value::Str(path_extension(&value)));
            }
            "__std_path_stem" => {
                let value = expect_string_arg(name, &args)?;
                return Ok(Value::Str(path_stem(&value)));
            }
            "__std_path_normalize" => {
                let value = expect_string_arg(name, &args)?;
                return Ok(Value::Str(path_normalize(&value)));
            }
            "__std_path_is_absolute" => {
                let value = expect_string_arg(name, &args)?;
                return Ok(Value::Bool(path_is_absolute(&value)));
            }
            "__std_date_is_iso_date" => {
                let s = expect_string_arg(name, &args)?;
                return Ok(Value::Bool(parse_iso_date(&s).is_some()));
            }
            "__std_date_year" => {
                let s = expect_string_arg(name, &args)?;
                return Ok(Value::Integer(
                    parse_iso_date(&s).map(|(year, _, _)| year).unwrap_or(0) as i64,
                ));
            }
            "__std_date_month" => {
                let s = expect_string_arg(name, &args)?;
                return Ok(Value::Integer(
                    parse_iso_date(&s).map(|(_, month, _)| month).unwrap_or(0) as i64,
                ));
            }
            "__std_date_day" => {
                let s = expect_string_arg(name, &args)?;
                return Ok(Value::Integer(
                    parse_iso_date(&s).map(|(_, _, day)| day).unwrap_or(0) as i64,
                ));
            }
            "__std_money_is_positive" => {
                let (amount, _) = expect_money_arg(name, &args)?;
                return Ok(Value::Bool(amount > 0.0));
            }
            "__std_money_is_zero" => {
                let (amount, _) = expect_money_arg(name, &args)?;
                return Ok(Value::Bool(amount == 0.0));
            }
            "__std_money_same_currency" => {
                let (left, right) = expect_two_money(name, &args)?;
                return Ok(Value::Bool(left.eq_ignore_ascii_case(&right)));
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
