use crate::ast::*;

pub fn format_program(program: &Program) -> String {
    let mut out = String::new();

    for (idx, decl) in program.decls.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        format_decl(decl, 0, &mut out);
        out.push('\n');
    }

    out
}

fn format_decl(decl: &Decl, indent: usize, out: &mut String) {
    match decl {
        Decl::Function {
            name,
            params,
            return_type,
            body,
            ..
        } => {
            write_indent(indent, out);
            let params = params
                .iter()
                .map(|(name, ty)| format!("{}: {}", name, format_type(ty)))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("fn {}({})", name, params));
            if *return_type != Type::Void {
                out.push_str(&format!(" -> {}", format_type(return_type)));
            }
            format_block(body, indent, out);
        }
        Decl::Model { name, fields, .. } => {
            write_indent(indent, out);
            out.push_str(&format!("model {} {{\n", name));
            for field in fields {
                write_indent(indent + 1, out);
                out.push_str(&format!("{}: {}", field.name, format_type(&field.ty)));
                if field.unique {
                    out.push_str(" unique");
                }
                if field.index {
                    out.push_str(" index");
                }
                if let Some(min) = &field.min {
                    out.push_str(&format!(" min {}", format_expr(min)));
                }
                if let Some(max) = &field.max {
                    out.push_str(&format!(" max {}", format_expr(max)));
                }
                if let Some(default) = &field.default {
                    out.push_str(&format!(" = {}", format_expr(default)));
                }
                out.push('\n');
            }
            write_indent(indent, out);
            out.push('}');
        }
        Decl::Workflow { name, steps, .. } => {
            write_indent(indent, out);
            out.push_str(&format!("workflow {} {{\n", name));
            for step in steps {
                write_indent(indent + 1, out);
                out.push_str(&format!("step {}", step.name));
                if step.body.is_empty() {
                    out.push('\n');
                } else {
                    format_block(&step.body, indent + 1, out);
                    out.push('\n');
                }
            }
            write_indent(indent, out);
            out.push('}');
        }
        Decl::Auth { config } => {
            write_indent(indent, out);
            out.push_str(&format!("auth {} {{\n", config.name));
            write_indent(indent + 1, out);
            out.push_str(&format!("model: {}\n", config.model));
            write_indent(indent + 1, out);
            out.push_str(&format!("identity: {}\n", config.identity));
            if let Some(role) = &config.role {
                write_indent(indent + 1, out);
                out.push_str(&format!("role: {}\n", role));
            }
            write_indent(indent + 1, out);
            out.push_str(&format!("password_min: {}\n", config.password_min));
            write_indent(indent + 1, out);
            out.push_str(&format!(
                "session_ttl_minutes: {}\n",
                config.session_ttl_minutes
            ));
            write_indent(indent + 1, out);
            out.push_str(&format!("idle_ttl_minutes: {}\n", config.idle_ttl_minutes));
            write_indent(indent, out);
            out.push('}');
        }
        Decl::Route {
            method,
            path,
            query_params,
            auth,
            body,
            ..
        } => {
            write_indent(indent, out);
            out.push_str(&format!("route {} {}", format_method(method), path));
            if !query_params.is_empty() {
                let params = query_params
                    .iter()
                    .map(format_query_param)
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!(" ?({})", params));
            }
            if let Some(guard) = auth {
                out.push_str(&format!(" auth({}", guard.auth));
                if let Some(role) = &guard.role {
                    out.push_str(&format!(r#", role: "{}""#, role));
                }
                out.push(')');
            }
            format_block(body, indent, out);
        }
        Decl::Invoice { fields, items, .. } => {
            write_indent(indent, out);
            out.push_str("invoice {\n");
            for field in fields {
                write_indent(indent + 1, out);
                out.push_str(&format!("{}: {}\n", field.key, format_expr(&field.value)));
            }
            for item in items {
                write_indent(indent + 1, out);
                out.push_str(&format!(
                    "item {} qty {} price {}\n",
                    format_expr(&item.description),
                    format_expr(&item.qty),
                    format_expr(&item.price)
                ));
            }
            write_indent(indent, out);
            out.push('}');
        }
        Decl::Statement(stmt) => format_stmt(stmt, indent, out),
    }
}

fn format_block(stmts: &[Stmt], indent: usize, out: &mut String) {
    out.push_str(" {\n");
    for stmt in stmts {
        format_stmt(stmt, indent + 1, out);
        out.push('\n');
    }
    write_indent(indent, out);
    out.push('}');
}

fn format_stmt(stmt: &Stmt, indent: usize, out: &mut String) {
    write_indent(indent, out);
    match stmt {
        Stmt::Let {
            name, ty, value, ..
        } => {
            out.push_str("let ");
            out.push_str(name);
            if let Some(ty) = ty {
                out.push_str(&format!(": {}", format_type(ty)));
            }
            out.push_str(&format!(" = {}", format_expr(value)));
        }
        Stmt::Const {
            name, ty, value, ..
        } => {
            out.push_str("const ");
            out.push_str(name);
            if let Some(ty) = ty {
                out.push_str(&format!(": {}", format_type(ty)));
            }
            out.push_str(&format!(" = {}", format_expr(value)));
        }
        Stmt::Assign { name, value, .. } => {
            out.push_str(&format!("{} = {}", name, format_expr(value)));
        }
        Stmt::Return { value, .. } => out.push_str(&format!("return {}", format_expr(value))),
        Stmt::Print { value, .. } => out.push_str(&format!("print({})", format_expr(value))),
        Stmt::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            out.push_str(&format!("if {}", format_expr(condition)));
            format_block(then_body, indent, out);
            if let Some(else_body) = else_body {
                out.push_str(" else");
                format_block(else_body, indent, out);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            out.push_str(&format!("while {}", format_expr(condition)));
            format_block(body, indent, out);
        }
        Stmt::For {
            var,
            iterable,
            body,
            ..
        } => {
            out.push_str(&format!("for {} in {}", var, format_expr(iterable)));
            format_block(body, indent, out);
        }
        Stmt::ExprStmt { expr, .. } => out.push_str(&format_expr(expr)),
    }
}

fn format_expr(expr: &Expr) -> String {
    match expr {
        Expr::Integer { value, .. } => value.to_string(),
        Expr::Float { value, .. } => trim_float(*value),
        Expr::StringLit { value, .. } => format!("{:?}", value),
        Expr::Bool { value, .. } => value.to_string(),
        Expr::Money {
            value, currency, ..
        } => format!("{} {}", trim_float(*value), currency),
        Expr::Array { items, .. } => {
            let items = items.iter().map(format_expr).collect::<Vec<_>>().join(", ");
            format!("[{}]", items)
        }
        Expr::Object { model, fields, .. } => {
            let fields = fields
                .iter()
                .map(|field| format!("{}: {}", field.name, format_expr(&field.value)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{} {{ {} }}", model, fields)
        }
        Expr::FieldAccess { object, field, .. } => format!("{}.{}", format_expr(object), field),
        Expr::Nil { .. } => "nil".to_string(),
        Expr::Ident { name, .. } => name.clone(),
        Expr::BinOp {
            left, op, right, ..
        } => {
            format!(
                "{} {} {}",
                format_expr(left),
                format_binop(op),
                format_expr(right)
            )
        }
        Expr::UnaryOp { op, expr, .. } => {
            let op = match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
            };
            format!("{}{}", op, format_expr(expr))
        }
        Expr::Call { name, args, .. } => {
            let args = args.iter().map(format_expr).collect::<Vec<_>>().join(", ");
            format!("{}({})", name, args)
        }
        Expr::StaticCall {
            ty, method, args, ..
        } => {
            let args = args.iter().map(format_expr).collect::<Vec<_>>().join(", ");
            format!("{}::{}({})", ty, method, args)
        }
    }
}

fn format_type(ty: &Type) -> String {
    match ty {
        Type::String => "string".to_string(),
        Type::Int => "int".to_string(),
        Type::Float => "float".to_string(),
        Type::Bool => "bool".to_string(),
        Type::Money => "money".to_string(),
        Type::Date => "date".to_string(),
        Type::Array(inner) => format!("[{}]", format_type(inner)),
        Type::Optional(inner) => format!("{}?", format_type(inner)),
        Type::Model(name) => name.clone(),
        Type::Nil => "nil".to_string(),
        Type::Void => "void".to_string(),
        Type::Unknown => "unknown".to_string(),
    }
}

fn format_query_param(param: &QueryParam) -> String {
    let mut out = format!("{}: {}", param.name, format_type(&param.ty));
    if let Some(default) = &param.default {
        out.push_str(&format!(" = {}", format_expr(default)));
    }
    out
}

fn format_method(method: &HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Delete => "DELETE",
    }
}

fn format_binop(op: &BinOp) -> &'static str {
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

fn trim_float(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

fn write_indent(indent: usize, out: &mut String) {
    for _ in 0..indent {
        out.push_str("    ");
    }
}
