use crate::ast::*;
use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, DiagnosticStage};
use crate::interpreter::Interpreter;
use crate::lexer::{Lexer, Token};
use crate::linter::{lint_program, LintWarning};
use crate::parser::Parser;

pub fn run_playground_json(source: &str) -> String {
    let mut lexer = Lexer::new(source);
    let tokens = match lexer.tokenize_spanned_diagnostic() {
        Ok(tokens) => tokens,
        Err(diagnostic) => {
            return response_json(ResponseParts {
                ok: false,
                diagnostic,
                token_count: 0,
                decl_count: 0,
                warning_count: 0,
                tokens_json: "[]",
                ast_json: "[]",
                erp_json: "{}",
                warnings_json: "[]",
                output_json: "[]",
            });
        }
    };
    let tokens_json = tokens_json(&tokens);
    let token_count = tokens
        .iter()
        .filter(|(token, _, _)| *token != Token::Eof)
        .count();

    let mut parser = Parser::new_spanned(tokens.clone());
    let program = match parser.parse_program_diagnostic() {
        Ok(program) => program,
        Err(diagnostic) => {
            return response_json(ResponseParts {
                ok: false,
                diagnostic,
                token_count,
                decl_count: 0,
                warning_count: 0,
                tokens_json: &tokens_json,
                ast_json: "[]",
                erp_json: "{}",
                warnings_json: "[]",
                output_json: "[]",
            });
        }
    };

    let ast_json = ast_json(&program);
    let erp_json = erp_json(&program);
    let decl_count = program.decls.len();

    let mut checker = Checker::new();
    if let Err(diagnostic) = checker.check_diagnostic(&program) {
        return response_json(ResponseParts {
            ok: false,
            diagnostic,
            token_count,
            decl_count,
            warning_count: 0,
            tokens_json: &tokens_json,
            ast_json: &ast_json,
            erp_json: &erp_json,
            warnings_json: "[]",
            output_json: "[]",
        });
    }

    let warnings = lint_program(&program);
    let warnings_json = warnings_json(&warnings);

    let mut interpreter = Interpreter::new_captured();
    if let Err(message) = interpreter.run(&program) {
        return response_json(ResponseParts {
            ok: false,
            diagnostic: Diagnostic::new(DiagnosticStage::Runtime, message),
            token_count,
            decl_count,
            warning_count: warnings.len(),
            tokens_json: &tokens_json,
            ast_json: &ast_json,
            erp_json: &erp_json,
            warnings_json: &warnings_json,
            output_json: &string_array_json(interpreter.output()),
        });
    }

    response_json(ResponseParts {
        ok: true,
        diagnostic: Diagnostic::new(DiagnosticStage::Runtime, "OK"),
        token_count,
        decl_count,
        warning_count: warnings.len(),
        tokens_json: &tokens_json,
        ast_json: &ast_json,
        erp_json: &erp_json,
        warnings_json: &warnings_json,
        output_json: &string_array_json(interpreter.output()),
    })
}

struct ResponseParts<'a> {
    ok: bool,
    diagnostic: Diagnostic,
    token_count: usize,
    decl_count: usize,
    warning_count: usize,
    tokens_json: &'a str,
    ast_json: &'a str,
    erp_json: &'a str,
    warnings_json: &'a str,
    output_json: &'a str,
}

fn response_json(parts: ResponseParts<'_>) -> String {
    let stage = parts.diagnostic.stage.as_str();
    let message = parts.diagnostic.to_string();
    format!(
        "{{\"ok\":{},\"stage\":\"{}\",\"message\":{},\"diagnostic\":{},\"stats\":{{\"tokens\":{},\"decls\":{},\"warnings\":{}}},\"tokens\":{},\"ast\":{},\"erp\":{},\"warnings\":{},\"output\":{}}}",
        if parts.ok { "true" } else { "false" },
        stage,
        json_string(&message),
        diagnostic_json(&parts.diagnostic),
        parts.token_count,
        parts.decl_count,
        parts.warning_count,
        parts.tokens_json,
        parts.ast_json,
        parts.erp_json,
        parts.warnings_json,
        parts.output_json
    )
}

fn diagnostic_json(diagnostic: &Diagnostic) -> String {
    format!(
        "{{\"stage\":\"{}\",\"message\":{},\"line\":{},\"column\":{}}}",
        diagnostic.stage.as_str(),
        json_string(&diagnostic.message),
        option_number_json(diagnostic.line),
        option_number_json(diagnostic.column)
    )
}

fn option_number_json(value: Option<usize>) -> String {
    value
        .map(|number| number.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn tokens_json(tokens: &[(Token, usize, usize)]) -> String {
    let mut items = Vec::new();
    for (token, line, column) in tokens {
        if *token == Token::Eof {
            continue;
        }

        let mut fields = vec![
            format!("\"type\":{}", json_string(token_kind(token))),
            format!("\"line\":{}", line),
            format!("\"column\":{}", column),
        ];

        match token_value(token) {
            Some(TokenDisplayValue::Text(value)) => {
                fields.push(format!("\"value\":{}", json_string(&value)));
            }
            Some(TokenDisplayValue::Money { amount, currency }) => {
                fields.push(format!("\"value\":{}", json_string(&amount)));
                fields.push(format!("\"currency\":{}", json_string(&currency)));
            }
            None => {}
        }

        items.push(format!("{{{}}}", fields.join(",")));
    }
    format!("[{}]", items.join(","))
}

enum TokenDisplayValue {
    Text(String),
    Money { amount: String, currency: String },
}

fn token_value(token: &Token) -> Option<TokenDisplayValue> {
    match token {
        Token::Integer(value) => Some(TokenDisplayValue::Text(value.to_string())),
        Token::Float(value) => Some(TokenDisplayValue::Text(number_string(*value))),
        Token::StringLit(value) => Some(TokenDisplayValue::Text(value.clone())),
        Token::Bool(value) => Some(TokenDisplayValue::Text(value.to_string())),
        Token::Money(amount, currency) => Some(TokenDisplayValue::Money {
            amount: number_string(*amount),
            currency: currency.clone(),
        }),
        Token::Ident(value) => Some(TokenDisplayValue::Text(value.clone())),
        _ => None,
    }
}

fn token_kind(token: &Token) -> &'static str {
    match token {
        Token::Integer(_) => "Integer",
        Token::Float(_) => "Float",
        Token::StringLit(_) => "String",
        Token::Bool(_) => "Bool",
        Token::Money(_, _) => "Money",
        Token::Nil => "nil",
        Token::Ident(_) => "Ident",
        Token::Let => "let",
        Token::Const => "const",
        Token::Fn => "fn",
        Token::Return => "return",
        Token::If => "if",
        Token::Else => "else",
        Token::While => "while",
        Token::For => "for",
        Token::In => "in",
        Token::Model => "model",
        Token::Workflow => "workflow",
        Token::Step => "step",
        Token::Route => "route",
        Token::Auth => "auth",
        Token::Invoice => "invoice",
        Token::Print => "print",
        Token::TypeString => "string",
        Token::TypeInt => "int",
        Token::TypeFloat => "float",
        Token::TypeBool => "bool",
        Token::TypeMoney => "money",
        Token::TypeDate => "date",
        Token::Get => "GET",
        Token::Post => "POST",
        Token::Put => "PUT",
        Token::Delete => "DELETE",
        Token::Plus => "Plus",
        Token::Minus => "Minus",
        Token::Star => "Star",
        Token::Slash => "Slash",
        Token::Percent => "Percent",
        Token::Eq => "Eq",
        Token::NotEq => "NotEq",
        Token::Lt => "Lt",
        Token::LtEq => "LtEq",
        Token::Gt => "Gt",
        Token::GtEq => "GtEq",
        Token::And => "And",
        Token::Or => "Or",
        Token::Not => "Not",
        Token::Assign => "Assign",
        Token::Arrow => "Arrow",
        Token::ColonColon => "ColonColon",
        Token::LParen => "LParen",
        Token::RParen => "RParen",
        Token::LBrace => "LBrace",
        Token::RBrace => "RBrace",
        Token::LBracket => "LBracket",
        Token::RBracket => "RBracket",
        Token::Comma => "Comma",
        Token::Colon => "Colon",
        Token::Semicolon => "Semicolon",
        Token::Dot => "Dot",
        Token::Question => "Question",
        Token::Slash2 => "Slash2",
        Token::Eof => "EOF",
        Token::Newline => "Newline",
    }
}

fn ast_json(program: &Program) -> String {
    let items: Vec<String> = program.decls.iter().map(decl_json).collect();
    format!("[{}]", items.join(","))
}

fn decl_json(decl: &Decl) -> String {
    match decl {
        Decl::Function {
            name,
            params,
            return_type,
            body,
            ..
        } => {
            let params_text = params
                .iter()
                .map(|(name, ty)| format!("{}: {}", name, type_string(ty)))
                .collect::<Vec<_>>()
                .join(", ");
            object_json(&[
                ("kind", json_string("Function")),
                ("name", json_string(name)),
                (
                    "summary",
                    json_string(&format!(
                        "fn {}({}) -> {}",
                        name,
                        params_text,
                        type_string(return_type)
                    )),
                ),
                (
                    "children",
                    string_vec_json(vec![
                        format!("{} parametros", params.len()),
                        format!("{} statements", body.len()),
                    ]),
                ),
            ])
        }
        Decl::Model { name, fields, .. } => object_json(&[
            ("kind", json_string("Model")),
            ("name", json_string(name)),
            ("summary", json_string(&format!("model {}", name))),
            (
                "children",
                string_vec_json(fields.iter().map(model_field_string).collect()),
            ),
        ]),
        Decl::Workflow { name, steps, .. } => object_json(&[
            ("kind", json_string("Workflow")),
            ("name", json_string(name)),
            ("summary", json_string(&format!("workflow {}", name))),
            (
                "children",
                string_vec_json(
                    steps
                        .iter()
                        .map(|step| format!("step {} ({} statements)", step.name, step.body.len()))
                        .collect(),
                ),
            ),
        ]),
        Decl::Auth { config } => object_json(&[
            ("kind", json_string("Auth")),
            ("name", json_string(&config.name)),
            (
                "summary",
                json_string(&format!("auth {} -> {}", config.name, config.model)),
            ),
            (
                "children",
                string_vec_json(vec![
                    format!("identity: {}", config.identity),
                    format!(
                        "role: {}",
                        config.role.clone().unwrap_or_else(|| "none".to_string())
                    ),
                    format!("password_min: {}", config.password_min),
                ]),
            ),
        ]),
        Decl::Route {
            method,
            path,
            params,
            query_params,
            body,
            ..
        } => object_json(&[
            ("kind", json_string("Route")),
            ("name", json_string(path)),
            (
                "summary",
                json_string(&format!("{} {}", method_string(method), path)),
            ),
            (
                "children",
                string_vec_json(vec![
                    format!("params: {}", params.join(", ")),
                    format!("query: {}", query_params_summary(query_params)),
                    format!("{} statements", body.len()),
                ]),
            ),
        ]),
        Decl::Invoice { fields, items, .. } => object_json(&[
            ("kind", json_string("Invoice")),
            ("name", json_string("invoice")),
            ("summary", json_string("invoice")),
            (
                "children",
                string_vec_json(
                    fields
                        .iter()
                        .map(|field| format!("field {}", field.key))
                        .chain(std::iter::once(format!("{} items", items.len())))
                        .collect(),
                ),
            ),
        ]),
        Decl::Statement(stmt) => object_json(&[
            ("kind", json_string("Statement")),
            ("name", json_string(stmt_kind(stmt))),
            ("summary", json_string(stmt_kind(stmt))),
            ("children", string_vec_json(Vec::new())),
        ]),
    }
}

fn erp_json(program: &Program) -> String {
    let mut models = Vec::new();
    let mut workflows = Vec::new();
    let mut routes = Vec::new();
    let mut invoices = Vec::new();

    for decl in &program.decls {
        match decl {
            Decl::Model { name, fields, .. } => {
                let field_json = fields
                    .iter()
                    .map(|field| {
                        object_json(&[
                            ("name", json_string(&field.name)),
                            ("type", json_string(&type_string(&field.ty))),
                            (
                                "default",
                                field
                                    .default
                                    .as_ref()
                                    .map(expr_string)
                                    .map(|value| json_string(&value))
                                    .unwrap_or_else(|| "null".to_string()),
                            ),
                        ])
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                models.push(object_json(&[
                    ("name", json_string(name)),
                    ("fields", format!("[{}]", field_json)),
                ]));
            }
            Decl::Workflow { name, steps, .. } => {
                let step_json = steps
                    .iter()
                    .map(|step| {
                        object_json(&[
                            ("name", json_string(&step.name)),
                            ("statements", step.body.len().to_string()),
                        ])
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                workflows.push(object_json(&[
                    ("name", json_string(name)),
                    ("steps", format!("[{}]", step_json)),
                ]));
            }
            Decl::Route {
                method,
                path,
                params,
                query_params,
                auth,
                ..
            } => {
                routes.push(object_json(&[
                    ("method", json_string(method_string(method))),
                    ("path", json_string(path)),
                    ("params", string_array_json(params)),
                    ("queryParams", query_params_json(query_params)),
                    (
                        "auth",
                        auth.as_ref()
                            .map(|guard| json_string(&guard.auth))
                            .unwrap_or_else(|| "null".to_string()),
                    ),
                ]));
            }
            Decl::Invoice { fields, items, .. } => {
                let field_keys = fields
                    .iter()
                    .map(|field| field.key.clone())
                    .collect::<Vec<_>>();
                invoices.push(object_json(&[
                    ("fields", string_vec_json(field_keys)),
                    ("items", items.len().to_string()),
                ]));
            }
            Decl::Function { .. } | Decl::Auth { .. } | Decl::Statement(_) => {}
        }
    }

    object_json(&[
        ("models", format!("[{}]", models.join(","))),
        ("workflows", format!("[{}]", workflows.join(","))),
        ("routes", format!("[{}]", routes.join(","))),
        ("invoices", format!("[{}]", invoices.join(","))),
    ])
}

fn model_field_string(field: &Field) -> String {
    let mut text = format!("{}: {}", field.name, type_string(&field.ty));
    if field.unique {
        text.push_str(" unique");
    }
    if field.index {
        text.push_str(" index");
    }
    if let Some(min) = &field.min {
        text.push_str(&format!(" min {}", expr_string(min)));
    }
    if let Some(max) = &field.max {
        text.push_str(&format!(" max {}", expr_string(max)));
    }
    if let Some(default) = &field.default {
        text.push_str(&format!(" = {}", expr_string(default)));
    }
    text
}

fn expr_string(expr: &Expr) -> String {
    match expr {
        Expr::Integer { value, .. } => value.to_string(),
        Expr::Float { value, .. } => number_string(*value),
        Expr::StringLit { value, .. } => format!("{:?}", value),
        Expr::Bool { value, .. } => value.to_string(),
        Expr::Money {
            value, currency, ..
        } => format!("{} {}", number_string(*value), currency),
        Expr::Array { items, .. } => {
            let items = items.iter().map(expr_string).collect::<Vec<_>>().join(", ");
            format!("[{}]", items)
        }
        Expr::Object { model, fields, .. } => {
            let fields = fields
                .iter()
                .map(|field| format!("{}: {}", field.name, expr_string(&field.value)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{} {{ {} }}", model, fields)
        }
        Expr::FieldAccess { object, field, .. } => format!("{}.{}", expr_string(object), field),
        Expr::Nil { .. } => "nil".to_string(),
        Expr::Ident { name, .. } => name.clone(),
        Expr::BinOp {
            left, op, right, ..
        } => format!(
            "{} {} {}",
            expr_string(left),
            binop_string(op),
            expr_string(right)
        ),
        Expr::UnaryOp { op, expr, .. } => {
            let op = match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
            };
            format!("{}{}", op, expr_string(expr))
        }
        Expr::Call { name, args, .. } => {
            let args = args.iter().map(expr_string).collect::<Vec<_>>().join(", ");
            format!("{}({})", name, args)
        }
        Expr::StaticCall {
            ty, method, args, ..
        } => {
            let args = args.iter().map(expr_string).collect::<Vec<_>>().join(", ");
            format!("{}::{}({})", ty, method, args)
        }
    }
}

fn binop_string(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Eq => "==",
        BinOp::NotEq => "!=",
        BinOp::Lt => "<",
        BinOp::LtEq => "<=",
        BinOp::Gt => ">",
        BinOp::GtEq => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
    }
}

fn warnings_json(warnings: &[LintWarning]) -> String {
    let items = warnings
        .iter()
        .map(|warning| {
            object_json(&[
                ("code", json_string(warning.code)),
                ("message", json_string(&warning.message)),
            ])
        })
        .collect::<Vec<_>>();
    format!("[{}]", items.join(","))
}

fn type_string(ty: &Type) -> String {
    match ty {
        Type::String => "string".to_string(),
        Type::Int => "int".to_string(),
        Type::Float => "float".to_string(),
        Type::Bool => "bool".to_string(),
        Type::Money => "money".to_string(),
        Type::Date => "date".to_string(),
        Type::Array(inner) => format!("[{}]", type_string(inner)),
        Type::Optional(inner) => format!("{}?", type_string(inner)),
        Type::Model(name) => name.clone(),
        Type::Nil => "nil".to_string(),
        Type::Void => "void".to_string(),
        Type::Unknown => "unknown".to_string(),
    }
}

fn method_string(method: &HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Delete => "DELETE",
    }
}

fn query_params_summary(params: &[QueryParam]) -> String {
    if params.is_empty() {
        return "(none)".to_string();
    }
    params
        .iter()
        .map(|param| {
            let mut out = format!("{}: {}", param.name, type_string(&param.ty));
            if let Some(default) = &param.default {
                out.push_str(&format!(" = {}", expr_string(default)));
            }
            out
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn query_params_json(params: &[QueryParam]) -> String {
    let items = params
        .iter()
        .map(|param| {
            object_json(&[
                ("name", json_string(&param.name)),
                ("type", json_string(&type_string(&param.ty))),
                ("required", query_param_required(param).to_string()),
                (
                    "default",
                    param
                        .default
                        .as_ref()
                        .map(expr_string)
                        .map(|value| json_string(&value))
                        .unwrap_or_else(|| "null".to_string()),
                ),
            ])
        })
        .collect::<Vec<_>>();
    format!("[{}]", items.join(","))
}

fn query_param_required(param: &QueryParam) -> bool {
    param.default.is_none() && !matches!(param.ty, Type::Optional(_))
}

fn stmt_kind(stmt: &Stmt) -> &'static str {
    match stmt {
        Stmt::Let { .. } => "let",
        Stmt::Const { .. } => "const",
        Stmt::Assign { .. } => "assign",
        Stmt::Return { .. } => "return",
        Stmt::Print { .. } => "print",
        Stmt::If { .. } => "if",
        Stmt::While { .. } => "while",
        Stmt::For { .. } => "for",
        Stmt::ExprStmt { .. } => "expr",
    }
}

fn number_string(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

fn object_json(fields: &[(&str, String)]) -> String {
    let inner = fields
        .iter()
        .map(|(key, value)| format!("\"{}\":{}", key, value))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{}}}", inner)
}

fn string_array_json(values: &[String]) -> String {
    let items = values
        .iter()
        .map(|value| json_string(value))
        .collect::<Vec<_>>();
    format!("[{}]", items.join(","))
}

fn string_vec_json(values: Vec<String>) -> String {
    string_array_json(&values)
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}
