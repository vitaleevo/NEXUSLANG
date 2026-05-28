use nexuslang::module_loader::SourceDatabase;
use nexuslang::{
    load_and_check_with_source_database_diagnostic_report,
    load_and_run_with_source_database_captured_diagnostic_report, MultiModuleDiagnosticReport,
};
use std::path::PathBuf;

fn main() {
    let Some(entry) = std::env::args().nth(1) else {
        eprintln!("usage: cargo run --example diagnostic_report_tooling -- <file.nx>");
        return;
    };
    let entry = PathBuf::from(entry);

    let source_database = match load_and_check_with_source_database_diagnostic_report(&entry) {
        Ok(checked) => {
            println!("check: ok");
            Some(checked.source_database)
        }
        Err(report) => {
            print_report("check", &report, None, &[]);
            return;
        }
    };

    match load_and_run_with_source_database_captured_diagnostic_report(&entry) {
        Ok(output) => {
            println!("run: ok");
            for line in output {
                println!("program: {line}");
            }
        }
        Err(run_report) => print_report(
            "run",
            &run_report.report,
            source_database.as_ref(),
            &run_report.output,
        ),
    }
}

fn print_report(
    command: &str,
    report: &MultiModuleDiagnosticReport,
    source_database: Option<&SourceDatabase>,
    output: &[String],
) {
    let view = report.tooling_view_with_source_context(source_database);
    let summary = &view.summary;
    println!(
        "{command}: diagnostics={} has_errors={} paths={} modules={}",
        summary.total,
        summary.has_errors,
        summary.paths.len(),
        summary.module_ids.len()
    );
    println!(
        "{command}: groups={} flattened_items={}",
        view.groups.len(),
        view.items.len()
    );

    for item in view.items {
        println!(
            "{command}: item#{} group={} path={:?} module={:?} stage={} code={:?} message={}",
            item.item.diagnostic_index,
            item.item.group_index,
            item.item.path.as_deref(),
            item.item.module_id,
            item.item.stage.as_str(),
            item.item.code.as_deref(),
            item.item.message
        );
        if let Some(context) = item.source_context {
            println!(
                "{command}: source line={} column={:?} highlight={:?}..{:?} text={}",
                context.line,
                context.column,
                context.highlight_start_column,
                context.highlight_end_column,
                context.line_text.trim()
            );
        }
    }

    if !output.is_empty() {
        println!("{command}: captured_output_lines={}", output.len());
    }
}
