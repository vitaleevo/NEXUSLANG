use crate::ast::*;
use crate::formatter::{format_expr, format_method, format_type};

pub fn document_program(program: &Program, title: Option<&str>) -> String {
    let docs = ProgramDocs::from_program(program);
    let title = title.unwrap_or("NexusLang Documentation");
    let mut out = String::new();

    out.push_str("# ");
    out.push_str(title);
    out.push_str("\n\n");

    out.push_str("## Summary\n\n");
    out.push_str(&format!("- Models: {}\n", docs.models.len()));
    out.push_str(&format!("- Functions: {}\n", docs.functions.len()));
    out.push_str(&format!("- Workflows: {}\n", docs.workflows.len()));
    out.push_str(&format!("- Auth configs: {}\n", docs.auths.len()));
    out.push_str(&format!("- Routes: {}\n", docs.routes.len()));
    out.push_str(&format!("- Invoices: {}\n", docs.invoices.len()));

    render_models(&docs.models, &mut out);
    render_functions(&docs.functions, &mut out);
    render_workflows(&docs.workflows, &mut out);
    render_auths(&docs.auths, &mut out);
    render_routes(&docs.routes, &mut out);
    render_invoices(&docs.invoices, &mut out);

    out
}

#[derive(Default)]
struct ProgramDocs<'a> {
    models: Vec<DocModel<'a>>,
    functions: Vec<DocFunction<'a>>,
    workflows: Vec<DocWorkflow<'a>>,
    auths: Vec<&'a AuthConfig>,
    routes: Vec<DocRoute<'a>>,
    invoices: Vec<DocInvoice<'a>>,
}

struct DocModel<'a> {
    name: &'a str,
    fields: &'a [Field],
}

struct DocFunction<'a> {
    name: &'a str,
    params: &'a [(String, Type)],
    return_type: &'a Type,
    statements: usize,
}

struct DocWorkflow<'a> {
    name: &'a str,
    steps: &'a [WorkflowStep],
}

struct DocRoute<'a> {
    method: &'a HttpMethod,
    path: &'a str,
    params: &'a [String],
    query_params: &'a [QueryParam],
    auth: &'a Option<RouteAuthGuard>,
    statements: usize,
}

struct DocInvoice<'a> {
    fields: &'a [InvoiceField],
    items: &'a [InvoiceItem],
}

impl<'a> ProgramDocs<'a> {
    fn from_program(program: &'a Program) -> Self {
        let mut docs = ProgramDocs::default();
        for decl in &program.decls {
            docs.collect_decl(decl);
        }
        docs
    }

    fn collect_decl(&mut self, decl: &'a Decl) {
        match decl {
            Decl::Function {
                name,
                params,
                return_type,
                body,
                ..
            } => self.functions.push(DocFunction {
                name,
                params,
                return_type,
                statements: body.len(),
            }),
            Decl::Model { name, fields, .. } => self.models.push(DocModel { name, fields }),
            Decl::Workflow { name, steps, .. } => self.workflows.push(DocWorkflow { name, steps }),
            Decl::Auth { config } => self.auths.push(config),
            Decl::Route {
                method,
                path,
                params,
                query_params,
                auth,
                body,
                ..
            } => self.routes.push(DocRoute {
                method,
                path,
                params,
                query_params,
                auth,
                statements: body.len(),
            }),
            Decl::Invoice { fields, items, .. } => self.invoices.push(DocInvoice { fields, items }),
            Decl::Export { decl, .. } => self.collect_decl(decl),
            Decl::Import { .. } | Decl::Statement(_) => {}
        }
    }
}

fn render_models(models: &[DocModel<'_>], out: &mut String) {
    if models.is_empty() {
        return;
    }

    out.push_str("\n## Models\n\n");
    for model in models {
        out.push_str("### ");
        out.push_str(&md_text(model.name));
        out.push_str("\n\n");
        out.push_str("| Field | Type | Constraints | Default |\n");
        out.push_str("| --- | --- | --- | --- |\n");
        for field in model.fields {
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                md_cell(&field.name),
                md_cell(&format_type(&field.ty)),
                md_cell(&field_constraints(field)),
                md_cell(
                    &field
                        .default
                        .as_ref()
                        .map(format_expr)
                        .unwrap_or_else(|| "-".to_string())
                )
            ));
        }
        out.push('\n');
    }
}

fn render_functions(functions: &[DocFunction<'_>], out: &mut String) {
    if functions.is_empty() {
        return;
    }

    out.push_str("## Functions\n\n");
    out.push_str("| Function | Parameters | Returns | Statements |\n");
    out.push_str("| --- | --- | --- | ---: |\n");
    for function in functions {
        let params = function
            .params
            .iter()
            .map(|(name, ty)| format!("{}: {}", name, format_type(ty)))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            md_cell(function.name),
            md_cell(empty_dash(&params)),
            md_cell(&format_type(function.return_type)),
            function.statements
        ));
    }
    out.push('\n');
}

fn render_workflows(workflows: &[DocWorkflow<'_>], out: &mut String) {
    if workflows.is_empty() {
        return;
    }

    out.push_str("## Workflows\n\n");
    for workflow in workflows {
        out.push_str("### ");
        out.push_str(&md_text(workflow.name));
        out.push_str("\n\n");
        out.push_str("| Step | Statements |\n");
        out.push_str("| --- | ---: |\n");
        for step in workflow.steps {
            out.push_str(&format!(
                "| {} | {} |\n",
                md_cell(&step.name),
                step.body.len()
            ));
        }
        out.push('\n');
    }
}

fn render_auths(auths: &[&AuthConfig], out: &mut String) {
    if auths.is_empty() {
        return;
    }

    out.push_str("## Auth\n\n");
    out.push_str("| Auth | Model | Identity | Role | Password min | Session TTL | Idle TTL |\n");
    out.push_str("| --- | --- | --- | --- | ---: | ---: | ---: |\n");
    for auth in auths {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            md_cell(&auth.name),
            md_cell(&auth.model),
            md_cell(&auth.identity),
            md_cell(auth.role.as_deref().unwrap_or("-")),
            auth.password_min,
            auth.session_ttl_minutes,
            auth.idle_ttl_minutes
        ));
    }
    out.push('\n');
}

fn render_routes(routes: &[DocRoute<'_>], out: &mut String) {
    if routes.is_empty() {
        return;
    }

    out.push_str("## Routes\n\n");
    for route in routes {
        out.push_str("### ");
        out.push_str(format_method(route.method));
        out.push(' ');
        out.push_str(&md_text(route.path));
        out.push_str("\n\n");
        out.push_str(&format!(
            "- Params: {}\n",
            md_text(&join_or_dash(route.params))
        ));
        out.push_str(&format!(
            "- Query: {}\n",
            md_text(&query_params_summary(route.query_params))
        ));
        out.push_str(&format!(
            "- Auth: {}\n",
            md_text(&route_auth_summary(route.auth))
        ));
        out.push_str(&format!("- Statements: {}\n\n", route.statements));
    }
}

fn render_invoices(invoices: &[DocInvoice<'_>], out: &mut String) {
    if invoices.is_empty() {
        return;
    }

    out.push_str("## Invoices\n\n");
    for (index, invoice) in invoices.iter().enumerate() {
        out.push_str(&format!("### Invoice {}\n\n", index + 1));
        let fields = invoice
            .fields
            .iter()
            .map(|field| field.key.clone())
            .collect::<Vec<_>>();
        out.push_str(&format!("- Fields: {}\n", md_text(&join_or_dash(&fields))));
        out.push_str(&format!("- Items: {}\n\n", invoice.items.len()));
    }
}

fn field_constraints(field: &Field) -> String {
    let mut constraints = Vec::new();
    if field.unique {
        constraints.push("unique".to_string());
    }
    if field.index {
        constraints.push("index".to_string());
    }
    if let Some(min) = &field.min {
        constraints.push(format!("min {}", format_expr(min)));
    }
    if let Some(max) = &field.max {
        constraints.push(format!("max {}", format_expr(max)));
    }
    if constraints.is_empty() {
        "-".to_string()
    } else {
        constraints.join(", ")
    }
}

fn query_params_summary(params: &[QueryParam]) -> String {
    if params.is_empty() {
        return "-".to_string();
    }

    params
        .iter()
        .map(|param| {
            let mut out = format!("{}: {}", param.name, format_type(&param.ty));
            if let Some(default) = &param.default {
                out.push_str(&format!(" = {}", format_expr(default)));
            }
            out
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn route_auth_summary(auth: &Option<RouteAuthGuard>) -> String {
    match auth {
        Some(guard) => match &guard.role {
            Some(role) => format!("{} role {}", guard.auth, role),
            None => guard.auth.clone(),
        },
        None => "-".to_string(),
    }
}

fn join_or_dash(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        items.join(", ")
    }
}

fn empty_dash(value: &str) -> &str {
    if value.is_empty() {
        "-"
    } else {
        value
    }
}

fn md_text(value: &str) -> String {
    value.replace('\\', "\\\\").replace(['\n', '\r'], " ")
}

fn md_cell(value: &str) -> String {
    md_text(value).replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_checked_source;

    #[test]
    fn documents_erp_declarations() {
        let source = r#"
model Customer {
    email: string unique index
    balance: money = 0 kz
}

fn label(name: string) -> string {
    return "Cliente " + name
}

workflow Onboarding {
    step criar {
        print("ok")
    }
}

route GET /customers/:email ?(active: bool = true) {
    return Customer::find("email", email)
}

invoice {
    customer: "Ana"
    currency: "AOA"
    item "Setup" qty 1 price 250000 kz
}
"#;

        let program = parse_checked_source(source).expect("program checks");
        let docs = document_program(&program, Some("ERP Docs"));

        assert!(docs.contains("# ERP Docs"));
        assert!(docs.contains("| email | string | unique, index | - |"));
        assert!(docs.contains("| balance | money | - | 0 kz |"));
        assert!(docs.contains("| label | name: string | string | 1 |"));
        assert!(docs.contains("### Onboarding"));
        assert!(docs.contains("### GET /customers/:email"));
        assert!(docs.contains("- Query: active: bool = true"));
        assert!(docs.contains("### Invoice 1"));
        assert!(docs.contains("- Items: 1"));
    }
}
