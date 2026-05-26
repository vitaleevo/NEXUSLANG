use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage_and_exit(1);
    }

    let command = &args[1];

    match command.as_str() {
        "help" | "--help" | "-h" => {
            print_usage_and_success();
        }
        "run" => {
            let file_path = required_arg(&args, 2, "run <ficheiro.nx>");
            let source = read_source(file_path);
            if let Err(e) = nexuslang::run_source(&source) {
                eprintln!("Erro de execução: {}", e);
                process::exit(1);
            }
        }
        "check" => {
            let file_path = required_arg(&args, 2, "check <ficheiro.nx>");
            let source = read_source(file_path);
            if let Err(e) = nexuslang::check_source(&source) {
                eprintln!("Erro de validação: {}", e);
                process::exit(1);
            }
            println!("OK: '{}' é válido", file_path);
        }
        "tokens" => {
            let file_path = required_arg(&args, 2, "tokens <ficheiro.nx>");
            let source = read_source(file_path);
            let tokens = match nexuslang::tokens_source_spanned_diagnostic(&source) {
                Ok(tokens) => tokens,
                Err(e) => {
                    eprintln!("Erro de lexing: {}", e);
                    process::exit(1);
                }
            };
            println!("Tokens em '{}':", file_path);
            for (tok, line, column) in &tokens {
                println!("  {:>4}:{:<3} │ {:?}", line, column, tok);
            }
        }
        "ast" => {
            let file_path = required_arg(&args, 2, "ast <ficheiro.nx>");
            let source = read_source(file_path);
            match nexuslang::ast_source(&source) {
                Ok(prog) => {
                    println!("AST de '{}':", file_path);
                    println!("{:#?}", prog);
                }
                Err(e) => {
                    eprintln!("Erro de parsing: {}", e);
                    process::exit(1);
                }
            }
        }
        "fmt" => {
            let file_path = required_arg(&args, 2, "fmt <ficheiro.nx> [--write]");
            let source = read_source(file_path);
            let formatted = match nexuslang::fmt_source(&source) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Erro de formatação: {}", e);
                    process::exit(1);
                }
            };

            if args.iter().any(|arg| arg == "--write") {
                if let Err(e) = fs::write(file_path, formatted) {
                    eprintln!("Erro ao escrever '{}': {}", file_path, e);
                    process::exit(1);
                }
                println!("Formatado: {}", file_path);
            } else {
                print!("{}", formatted);
            }
        }
        "lint" => {
            let file_path = required_arg(&args, 2, "lint <ficheiro.nx>");
            let source = read_source(file_path);
            match nexuslang::lint_source(&source) {
                Ok(warnings) if warnings.is_empty() => {
                    println!("OK: '{}' sem avisos", file_path);
                }
                Ok(warnings) => {
                    println!("Avisos em '{}':", file_path);
                    for warning in warnings {
                        println!("  {}: {}", warning.code, warning.message);
                    }
                }
                Err(e) => {
                    eprintln!("Erro de lint: {}", e);
                    process::exit(1);
                }
            }
        }
        "serve" => {
            let file_path = required_arg(&args, 2, "serve <ficheiro.nx> [addr]");
            let addr = args.get(3).map(|s| s.as_str()).unwrap_or("127.0.0.1:5050");
            if let Err(e) = nexuslang::server::serve_file(file_path, addr) {
                eprintln!("Erro no servidor: {}", e);
                process::exit(1);
            }
        }
        "repl" => run_repl(),
        "new" => {
            let project_name = required_arg(&args, 2, "new <project>");
            if let Err(e) = create_project(project_name) {
                eprintln!("Erro ao criar projeto: {}", e);
                process::exit(1);
            }
        }
        cmd => {
            eprintln!("Comando desconhecido: '{}'", cmd);
            print_usage_and_exit(1);
        }
    }
}

fn print_usage_and_success() -> ! {
    let mut stdout = io::stdout();
    print_usage(&mut stdout);
    process::exit(0);
}

fn print_usage_and_exit(code: i32) -> ! {
    let mut stderr = io::stderr();
    print_usage(&mut stderr);
    process::exit(code);
}

fn print_usage(out: &mut dyn Write) {
    writeln!(out, "NexusLang 🔷 v{}", env!("CARGO_PKG_VERSION")).ok();
    writeln!(out).ok();
    writeln!(out, "Uso:").ok();
    writeln!(
        out,
        "  nexus run <ficheiro.nx>          — Executar programa"
    )
    .ok();
    writeln!(
        out,
        "  nexus check <ficheiro.nx>        — Validar sem executar"
    )
    .ok();
    writeln!(out, "  nexus fmt <ficheiro.nx> [--write] — Formatar código").ok();
    writeln!(
        out,
        "  nexus lint <ficheiro.nx>         — Analisar estilo e riscos"
    )
    .ok();
    writeln!(
        out,
        "  nexus serve <ficheiro.nx> [addr] — Servir routes HTTP"
    )
    .ok();
    writeln!(
        out,
        "  nexus repl                       — Abrir REPL simples"
    )
    .ok();
    writeln!(
        out,
        "  nexus new <project>              — Criar projeto NexusLang"
    )
    .ok();
    writeln!(
        out,
        "  nexus tokens <ficheiro.nx>       — Ver tokens (debug)"
    )
    .ok();
    writeln!(out, "  nexus ast <ficheiro.nx>          — Ver AST (debug)").ok();
}

fn required_arg<'a>(args: &'a [String], index: usize, usage: &str) -> &'a str {
    match args.get(index) {
        Some(value) => value,
        None => {
            eprintln!("Uso: nexus {}", usage);
            process::exit(1);
        }
    }
}

fn read_source(file_path: &str) -> String {
    match fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Erro ao ler '{}': {}", file_path, e);
            process::exit(1);
        }
    }
}

fn run_repl() {
    println!("NexusLang REPL. Use :quit para sair, :clear para limpar o buffer.");
    let mut buffer = String::new();

    loop {
        print!("nexus> ");
        io::stdout().flush().ok();

        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            eprintln!("Erro ao ler entrada");
            process::exit(1);
        }

        let trimmed = line.trim();
        match trimmed {
            ":quit" | ":exit" => break,
            ":clear" => {
                buffer.clear();
                println!("Buffer limpo");
                continue;
            }
            "" => continue,
            _ => {}
        }

        buffer.push_str(&line);
        if let Err(e) = nexuslang::run_source(&buffer) {
            eprintln!("Erro: {}", e);
            eprintln!("Dica: use :clear para reiniciar o buffer.");
        }
    }
}

fn create_project(project_name: &str) -> Result<(), String> {
    let root = Path::new(project_name);
    if root.exists() {
        return Err(format!("'{}' já existe", project_name));
    }

    fs::create_dir_all(root.join("examples")).map_err(|e| e.to_string())?;

    let main_nx = r#"model Customer {
    name: string
    balance: money
}

workflow Onboarding {
    step criar_cliente {
        print("Criando cliente")
    }
    step notificar {
        print("Notificando cliente")
    }
}

route GET /customers/:id {
    return "customer " + id
}

invoice {
    customer: "Cliente Exemplo"
    currency: "AOA"
    tax: 14
    item "Setup ERP" qty 1 price 250000 kz
}

print("Projeto NexusLang pronto")
run_workflow("Onboarding")
"#;

    let readme = format!(
        "# {}\n\nProjeto criado com `nexus new`.\n\n## Comandos\n\n```bash\nnexus check main.nx\nnexus run main.nx\nnexus lint main.nx\nnexus fmt main.nx --write\n```\n",
        project_name
    );

    fs::write(root.join("main.nx"), main_nx).map_err(|e| e.to_string())?;
    fs::write(root.join("README.md"), readme).map_err(|e| e.to_string())?;
    fs::write(root.join("examples").join("invoice.nx"), main_nx).map_err(|e| e.to_string())?;

    println!("Projeto criado: {}", root.display());
    println!("Próximo passo:");
    println!("  cd {}", root.display());
    println!("  nexus check main.nx");
    Ok(())
}
