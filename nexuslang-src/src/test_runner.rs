use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::package_manager;
use crate::runtime_env::{self, NEXUS_DATA_DIR_ENV};
use crate::{
    load_and_run_with_source_database_captured_diagnostic, MultiModuleDiagnostic,
    MultiModuleRunDiagnostic,
};

pub const NEXUS_TEST_JSON_SCHEMA_VERSION: usize = 1;

#[derive(Debug, Clone)]
pub struct NexusTestReport {
    pub target: PathBuf,
    pub cases: Vec<NexusTestCaseResult>,
}

#[derive(Debug, Clone)]
pub struct NexusTestListReport {
    pub target: PathBuf,
    pub cases: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct NexusTestOptions {
    pub update_expected: bool,
    pub update_expected_diagnostic: bool,
    pub name_filter: Option<String>,
    pub timeout: Option<Duration>,
    pub isolate_data: bool,
    pub jobs: usize,
    pub fail_fast: bool,
}

impl Default for NexusTestOptions {
    fn default() -> Self {
        NexusTestOptions {
            update_expected: false,
            update_expected_diagnostic: false,
            name_filter: None,
            timeout: None,
            isolate_data: false,
            jobs: 1,
            fail_fast: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NexusTestCaseResult {
    pub path: PathBuf,
    pub output: Vec<String>,
    pub expected_output: Option<Vec<String>>,
    pub expected_diagnostic: Option<Vec<String>>,
    pub expected_output_updated: Option<PathBuf>,
    pub expected_diagnostic_updated: Option<PathBuf>,
    pub output_mismatch: Option<NexusOutputMismatch>,
    pub diagnostic_mismatch: Option<NexusDiagnosticMismatch>,
    pub diagnostic: Option<MultiModuleDiagnostic>,
    pub timed_out: bool,
    pub isolated_data_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NexusOutputMismatch {
    pub expected: Vec<String>,
    pub actual: Vec<String>,
    pub first_diff: Option<NexusLineDiff>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NexusDiagnosticMismatch {
    pub expected: Vec<String>,
    pub actual: Option<Vec<String>>,
    pub first_diff: Option<NexusLineDiff>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NexusLineDiff {
    pub line: usize,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

impl NexusTestReport {
    pub fn passed(&self) -> usize {
        self.cases.iter().filter(|case| case.passed()).count()
    }

    pub fn failed(&self) -> usize {
        self.cases.iter().filter(|case| !case.passed()).count()
    }

    pub fn total(&self) -> usize {
        self.cases.len()
    }

    pub fn is_success(&self) -> bool {
        self.failed() == 0
    }
}

impl NexusTestCaseResult {
    pub fn passed(&self) -> bool {
        self.diagnostic.is_none()
            && self.output_mismatch.is_none()
            && self.diagnostic_mismatch.is_none()
    }
}

pub fn test_report_json(report: &NexusTestReport) -> String {
    format!(
        "{{\"ok\":{},\"schema_version\":{},\"command\":\"test\",\"target\":{},\"summary\":{},\"cases\":{}}}",
        if report.is_success() { "true" } else { "false" },
        NEXUS_TEST_JSON_SCHEMA_VERSION,
        json_string(report.target.to_string_lossy().as_ref()),
        test_summary_json(report),
        test_cases_json(&report.cases)
    )
}

pub fn test_error_json(message: &str) -> String {
    format!(
        "{{\"ok\":false,\"schema_version\":{},\"command\":\"test\",\"error\":{{\"message\":{}}},\"summary\":{{\"passed\":0,\"failed\":1,\"total\":0}},\"cases\":[]}}",
        NEXUS_TEST_JSON_SCHEMA_VERSION,
        json_string(message)
    )
}

pub fn test_list_report_json(report: &NexusTestListReport) -> String {
    format!(
        "{{\"ok\":true,\"schema_version\":{},\"command\":\"test\",\"mode\":\"list\",\"target\":{},\"summary\":{{\"passed\":0,\"failed\":0,\"total\":{}}},\"cases\":{}}}",
        NEXUS_TEST_JSON_SCHEMA_VERSION,
        json_string(report.target.to_string_lossy().as_ref()),
        report.cases.len(),
        test_list_cases_json(&report.cases)
    )
}

pub fn run_tests_at(target: &Path) -> Result<NexusTestReport, String> {
    run_tests_at_with_options(target, NexusTestOptions::default())
}

pub fn run_tests_at_with_options(
    target: &Path,
    options: NexusTestOptions,
) -> Result<NexusTestReport, String> {
    let files = discover_nx_files(target)?;
    run_test_files(target.to_path_buf(), files, options)
}

pub fn list_tests_at_with_options(
    target: &Path,
    options: &NexusTestOptions,
) -> Result<NexusTestListReport, String> {
    let files = discover_nx_files(target)?;
    Ok(NexusTestListReport {
        target: target.to_path_buf(),
        cases: filter_nx_files(files, options.name_filter.as_deref()),
    })
}

pub fn run_default_tests_from_current_dir() -> Result<NexusTestReport, String> {
    run_default_tests_from_current_dir_with_options(NexusTestOptions::default())
}

pub fn run_default_tests_from_current_dir_with_options(
    options: NexusTestOptions,
) -> Result<NexusTestReport, String> {
    let current = env::current_dir().map_err(|e| e.to_string())?;
    let target = default_test_target_from(&current)?;
    run_tests_at_with_options(&target, options)
}

pub fn list_default_tests_from_current_dir_with_options(
    options: &NexusTestOptions,
) -> Result<NexusTestListReport, String> {
    let current = env::current_dir().map_err(|e| e.to_string())?;
    let target = default_test_target_from(&current)?;
    list_tests_at_with_options(&target, options)
}

pub fn default_test_target_from(start: &Path) -> Result<PathBuf, String> {
    let root = package_manager::load_nearest_project_manifest(start)?
        .map(|manifest| manifest.root)
        .unwrap_or_else(|| start.to_path_buf());

    let tests = root.join("tests");
    if contains_nx_file(&tests) {
        return Ok(tests);
    }

    let examples = root.join("examples");
    if contains_nx_file(&examples) {
        return Ok(examples);
    }

    Err(format!(
        "Nenhum diretorio tests/ ou examples/ com ficheiros .nx encontrado a partir de {}",
        root.display()
    ))
}

fn run_test_files(
    target: PathBuf,
    files: Vec<PathBuf>,
    options: NexusTestOptions,
) -> Result<NexusTestReport, String> {
    let files = filter_nx_files(files, options.name_filter.as_deref());
    let update_expected = options.update_expected;
    let update_expected_diagnostic = options.update_expected_diagnostic;
    let timeout = options.timeout;
    let isolate_data = options.isolate_data;
    let jobs = options.jobs.max(1);
    let fail_fast = options.fail_fast;

    let cases = if jobs == 1 || files.len() <= 1 {
        run_test_files_sequential(
            files,
            update_expected,
            update_expected_diagnostic,
            timeout,
            isolate_data,
            fail_fast,
        )?
    } else {
        run_test_files_parallel(
            files,
            update_expected,
            update_expected_diagnostic,
            timeout,
            isolate_data,
            jobs,
            fail_fast,
        )?
    };

    Ok(NexusTestReport { target, cases })
}

fn run_test_files_sequential(
    files: Vec<PathBuf>,
    update_expected: bool,
    update_expected_diagnostic: bool,
    timeout: Option<Duration>,
    isolate_data: bool,
    fail_fast: bool,
) -> Result<Vec<NexusTestCaseResult>, String> {
    let mut cases = Vec::new();
    for file in files {
        let expected_output = if update_expected {
            None
        } else {
            read_expected_output(&file)?
        };
        let expected_diagnostic = if update_expected_diagnostic {
            None
        } else {
            read_expected_diagnostic(&file)?
        };
        let result = run_test_file(
            file,
            expected_output,
            expected_diagnostic,
            update_expected,
            update_expected_diagnostic,
            timeout,
            isolate_data,
        )?;
        let failed = !result.passed();
        cases.push(result);
        if fail_fast && failed {
            break;
        }
    }

    Ok(cases)
}

fn run_test_files_parallel(
    files: Vec<PathBuf>,
    update_expected: bool,
    update_expected_diagnostic: bool,
    timeout: Option<Duration>,
    isolate_data: bool,
    jobs: usize,
    fail_fast: bool,
) -> Result<Vec<NexusTestCaseResult>, String> {
    let mut cases = Vec::new();
    let mut iter = files.into_iter().enumerate();

    loop {
        let mut handles = Vec::new();
        for _ in 0..jobs {
            let Some((index, file)) = iter.next() else {
                break;
            };
            handles.push(thread::spawn(move || {
                let expected_output = if update_expected {
                    None
                } else {
                    read_expected_output(&file)?
                };
                let expected_diagnostic = if update_expected_diagnostic {
                    None
                } else {
                    read_expected_diagnostic(&file)?
                };
                run_test_file(
                    file,
                    expected_output,
                    expected_diagnostic,
                    update_expected,
                    update_expected_diagnostic,
                    timeout,
                    isolate_data,
                )
                .map(|case| (index, case))
            }));
        }

        if handles.is_empty() {
            break;
        }

        let mut batch = Vec::new();
        for handle in handles {
            match handle.join() {
                Ok(Ok((index, case))) => batch.push((index, case)),
                Ok(Err(e)) => return Err(e),
                Err(_) => return Err("Falha interna ao executar teste em paralelo".to_string()),
            }
        }

        batch.sort_by_key(|(index, _)| *index);
        let batch_failed = batch.iter().any(|(_, case)| !case.passed());
        cases.extend(batch.into_iter().map(|(_, case)| case));
        if fail_fast && batch_failed {
            break;
        }
    }

    Ok(cases)
}

fn run_test_file(
    file: PathBuf,
    expected_output: Option<Vec<String>>,
    expected_diagnostic: Option<Vec<String>>,
    update_expected: bool,
    update_expected_diagnostic: bool,
    timeout: Option<Duration>,
    isolate_data: bool,
) -> Result<NexusTestCaseResult, String> {
    let isolated_data = if isolate_data {
        Some(IsolatedDataDir::create(&file)?)
    } else {
        None
    };
    let isolated_data_dir = isolated_data.as_ref().map(|dir| dir.path.clone());

    match execute_test_file_with_timeout(&file, timeout, isolated_data_dir.as_deref()) {
        TestFileExecution::Completed(run_result) => finalize_test_file_result(
            file,
            expected_output,
            expected_diagnostic,
            update_expected,
            update_expected_diagnostic,
            *run_result,
            isolated_data_dir,
        ),
        TestFileExecution::TimedOut => {
            if let Some(dir) = isolated_data {
                dir.persist();
            }
            timeout_test_file_result(
                file,
                expected_output,
                expected_diagnostic,
                update_expected_diagnostic,
                timeout,
                isolated_data_dir,
            )
        }
    }
}

fn execute_test_file_with_timeout(
    file: &Path,
    timeout: Option<Duration>,
    isolated_data_dir: Option<&Path>,
) -> TestFileExecution {
    let Some(timeout) = timeout else {
        return TestFileExecution::Completed(Box::new(execute_test_file(file, isolated_data_dir)));
    };

    let file_for_thread = file.to_path_buf();
    let isolated_data_dir = isolated_data_dir.map(Path::to_path_buf);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(execute_test_file(
            &file_for_thread,
            isolated_data_dir.as_deref(),
        ));
    });

    match rx.recv_timeout(timeout) {
        Ok(result) => TestFileExecution::Completed(Box::new(result)),
        Err(mpsc::RecvTimeoutError::Timeout) => TestFileExecution::TimedOut,
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            TestFileExecution::Completed(Box::new(Err(MultiModuleRunDiagnostic {
                diagnostic: MultiModuleDiagnostic::runtime("Falha interna ao executar teste"),
                output: Vec::new(),
            })))
        }
    }
}

fn execute_test_file(
    file: &Path,
    isolated_data_dir: Option<&Path>,
) -> Result<Vec<String>, MultiModuleRunDiagnostic> {
    let _env_guard =
        isolated_data_dir.map(|dir| runtime_env::set_thread_var_path(NEXUS_DATA_DIR_ENV, dir));
    load_and_run_with_source_database_captured_diagnostic(file)
}

fn finalize_test_file_result(
    file: PathBuf,
    expected_output: Option<Vec<String>>,
    expected_diagnostic: Option<Vec<String>>,
    update_expected: bool,
    update_expected_diagnostic: bool,
    run_result: Result<Vec<String>, MultiModuleRunDiagnostic>,
    isolated_data_dir: Option<PathBuf>,
) -> Result<NexusTestCaseResult, String> {
    match run_result {
        Ok(output) => {
            let (expected_output, expected_output_updated, output_mismatch) = if update_expected {
                if expected_diagnostic.is_none() {
                    let sidecar = expected_output_path(&file);
                    write_expected_output(&sidecar, &output)?;
                    (Some(output.clone()), Some(sidecar), None)
                } else {
                    (None, None, None)
                }
            } else {
                let output_mismatch = expected_output
                    .as_ref()
                    .and_then(|expected| match_output(expected, &output));
                (expected_output, None, output_mismatch)
            };
            let diagnostic_mismatch =
                expected_diagnostic
                    .as_ref()
                    .map(|expected| NexusDiagnosticMismatch {
                        expected: expected.clone(),
                        first_diff: first_line_diff(expected, &[]),
                        actual: None,
                    });
            Ok(NexusTestCaseResult {
                path: file,
                output,
                expected_output,
                expected_diagnostic,
                expected_output_updated,
                expected_diagnostic_updated: None,
                output_mismatch,
                diagnostic_mismatch,
                diagnostic: None,
                timed_out: false,
                isolated_data_dir,
            })
        }
        Err(run_diagnostic) => {
            let output_mismatch = if update_expected {
                None
            } else {
                expected_output
                    .as_ref()
                    .and_then(|expected| match_output(expected, &run_diagnostic.output))
            };
            let (expected_diagnostic, expected_diagnostic_updated, diagnostic_mismatch) =
                if update_expected_diagnostic {
                    let actual_diagnostic = diagnostic_lines(&run_diagnostic.diagnostic);
                    let sidecar = expected_diagnostic_path(&file);
                    write_expected_diagnostic(&sidecar, &actual_diagnostic)?;
                    (Some(actual_diagnostic), Some(sidecar), None)
                } else {
                    let diagnostic_mismatch = expected_diagnostic.as_ref().and_then(|expected| {
                        match_diagnostic(expected, Some(&run_diagnostic.diagnostic))
                    });
                    (expected_diagnostic, None, diagnostic_mismatch)
                };
            let diagnostic = if expected_diagnostic.is_some() && diagnostic_mismatch.is_none() {
                None
            } else {
                Some(run_diagnostic.diagnostic)
            };
            Ok(NexusTestCaseResult {
                path: file,
                output: run_diagnostic.output,
                expected_output,
                expected_diagnostic,
                expected_output_updated: None,
                expected_diagnostic_updated,
                output_mismatch,
                diagnostic_mismatch,
                diagnostic,
                timed_out: false,
                isolated_data_dir,
            })
        }
    }
}

fn timeout_test_file_result(
    file: PathBuf,
    expected_output: Option<Vec<String>>,
    expected_diagnostic: Option<Vec<String>>,
    update_expected_diagnostic: bool,
    timeout: Option<Duration>,
    isolated_data_dir: Option<PathBuf>,
) -> Result<NexusTestCaseResult, String> {
    let timeout = timeout.expect("timeout result requires timeout");
    let diagnostic = MultiModuleDiagnostic::runtime(format!(
        "Timeout de teste excedido apos {} em '{}'",
        format_timeout_duration(timeout),
        file.display()
    ));
    let (expected_diagnostic, expected_diagnostic_updated, diagnostic_mismatch) =
        if update_expected_diagnostic {
            let actual_diagnostic = diagnostic_lines(&diagnostic);
            let sidecar = expected_diagnostic_path(&file);
            write_expected_diagnostic(&sidecar, &actual_diagnostic)?;
            (Some(actual_diagnostic), Some(sidecar), None)
        } else {
            let diagnostic_mismatch = expected_diagnostic
                .as_ref()
                .and_then(|expected| match_diagnostic(expected, Some(&diagnostic)));
            (expected_diagnostic, None, diagnostic_mismatch)
        };
    let diagnostic = if expected_diagnostic.is_some() && diagnostic_mismatch.is_none() {
        None
    } else {
        Some(diagnostic)
    };
    Ok(NexusTestCaseResult {
        path: file.clone(),
        output: Vec::new(),
        expected_output,
        expected_diagnostic,
        expected_output_updated: None,
        expected_diagnostic_updated,
        output_mismatch: None,
        diagnostic_mismatch,
        diagnostic,
        timed_out: true,
        isolated_data_dir,
    })
}

enum TestFileExecution {
    Completed(Box<Result<Vec<String>, MultiModuleRunDiagnostic>>),
    TimedOut,
}

struct IsolatedDataDir {
    path: PathBuf,
    cleanup: bool,
}

impl IsolatedDataDir {
    fn create(file: &Path) -> Result<Self, String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let name = sanitize_path_segment(
            file.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("case"),
        );
        let path = env::temp_dir().join(format!(
            "nexuslang-test-data-{}-{}-{}",
            process::id(),
            now,
            name
        ));
        fs::create_dir_all(&path)
            .map_err(|e| format!("Nao foi possivel criar '{}': {}", path.display(), e))?;
        Ok(Self {
            path,
            cleanup: true,
        })
    }

    fn persist(mut self) {
        self.cleanup = false;
    }
}

impl Drop for IsolatedDataDir {
    fn drop(&mut self) {
        if self.cleanup {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn sanitize_path_segment(value: &str) -> String {
    let mut out = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if out.is_empty() {
        out.push_str("case");
    }
    out
}

fn format_timeout_duration(timeout: Duration) -> String {
    if timeout.as_millis() < 1000 || timeout.subsec_nanos() != 0 {
        format!("{}ms", timeout.as_millis())
    } else if timeout.as_secs().is_multiple_of(60) {
        format!("{}m", timeout.as_secs() / 60)
    } else {
        format!("{}s", timeout.as_secs())
    }
}

fn filter_nx_files(files: Vec<PathBuf>, name_filter: Option<&str>) -> Vec<PathBuf> {
    let Some(name_filter) = name_filter else {
        return files;
    };
    let normalized_filter = name_filter.to_lowercase();

    files
        .into_iter()
        .filter(|file| file_matches_name_filter(file, &normalized_filter))
        .collect()
}

fn file_matches_name_filter(file: &Path, normalized_filter: &str) -> bool {
    file.to_string_lossy()
        .to_lowercase()
        .contains(normalized_filter)
}

fn discover_nx_files(target: &Path) -> Result<Vec<PathBuf>, String> {
    let metadata = fs::metadata(target)
        .map_err(|e| format!("Nao foi possivel ler '{}': {}", target.display(), e))?;
    let mut files = Vec::new();

    if metadata.is_file() {
        if is_nx_file(target) {
            files.push(target.to_path_buf());
        } else {
            return Err(format!("Alvo '{}' nao e um ficheiro .nx", target.display()));
        }
    } else if metadata.is_dir() {
        collect_nx_files(target, &mut files)?;
    } else {
        return Err(format!(
            "Alvo '{}' nao e ficheiro nem diretorio",
            target.display()
        ));
    }

    files.sort();
    if files.is_empty() {
        Err(format!(
            "Nenhum ficheiro .nx encontrado em '{}'",
            target.display()
        ))
    } else {
        Ok(files)
    }
}

fn collect_nx_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries = fs::read_dir(dir)
        .map_err(|e| format!("Nao foi possivel ler '{}': {}", dir.display(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type().map_err(|e| e.to_string())?;
        if file_type.is_dir() {
            if should_skip_dir(&path) {
                continue;
            }
            collect_nx_files(&path, files)?;
        } else if file_type.is_file() && is_nx_file(&path) {
            files.push(path);
        }
    }

    Ok(())
}

fn contains_nx_file(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    discover_nx_files(path)
        .map(|files| !files.is_empty())
        .unwrap_or(false)
}

fn should_skip_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".git" | ".nexus" | ".nexus-data" | "target")
    )
}

fn is_nx_file(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("nx")
}

fn read_expected_output(path: &Path) -> Result<Option<Vec<String>>, String> {
    let sidecar = expected_output_path(path);
    if !sidecar.is_file() {
        return Ok(None);
    }

    let source = fs::read_to_string(&sidecar)
        .map_err(|e| format!("Nao foi possivel ler '{}': {}", sidecar.display(), e))?;
    Ok(Some(expected_lines_from_source(&source)))
}

fn read_expected_diagnostic(path: &Path) -> Result<Option<Vec<String>>, String> {
    let sidecar = expected_diagnostic_path(path);
    if !sidecar.is_file() {
        return Ok(None);
    }

    let source = fs::read_to_string(&sidecar)
        .map_err(|e| format!("Nao foi possivel ler '{}': {}", sidecar.display(), e))?;
    Ok(Some(expected_lines_from_source(&source)))
}

fn write_expected_output(path: &Path, output: &[String]) -> Result<(), String> {
    fs::write(path, serialize_output_lines(output))
        .map_err(|e| format!("Nao foi possivel escrever '{}': {}", path.display(), e))
}

fn write_expected_diagnostic(path: &Path, diagnostic: &[String]) -> Result<(), String> {
    fs::write(path, serialize_output_lines(diagnostic))
        .map_err(|e| format!("Nao foi possivel escrever '{}': {}", path.display(), e))
}

fn expected_output_path(path: &Path) -> PathBuf {
    path.with_extension("out")
}

fn expected_diagnostic_path(path: &Path) -> PathBuf {
    path.with_extension("err")
}

fn match_output(expected: &[String], actual: &[String]) -> Option<NexusOutputMismatch> {
    if expected == actual {
        None
    } else {
        Some(NexusOutputMismatch {
            expected: expected.to_vec(),
            actual: actual.to_vec(),
            first_diff: first_line_diff(expected, actual),
        })
    }
}

fn match_diagnostic(
    expected: &[String],
    actual: Option<&MultiModuleDiagnostic>,
) -> Option<NexusDiagnosticMismatch> {
    let actual = actual.map(diagnostic_lines);
    if actual.as_deref() == Some(expected) {
        None
    } else {
        Some(NexusDiagnosticMismatch {
            expected: expected.to_vec(),
            first_diff: first_line_diff(expected, actual.as_deref().unwrap_or(&[])),
            actual,
        })
    }
}

fn first_line_diff(expected: &[String], actual: &[String]) -> Option<NexusLineDiff> {
    for index in 0..expected.len().max(actual.len()) {
        let expected_line = expected.get(index);
        let actual_line = actual.get(index);
        if expected_line != actual_line {
            return Some(NexusLineDiff {
                line: index + 1,
                expected: expected_line.cloned(),
                actual: actual_line.cloned(),
            });
        }
    }

    None
}

fn diagnostic_lines(diagnostic: &MultiModuleDiagnostic) -> Vec<String> {
    expected_lines_from_source(&diagnostic.to_string())
}

fn serialize_output_lines(output: &[String]) -> String {
    if output.is_empty() {
        String::new()
    } else {
        format!("{}\n", output.join("\n"))
    }
}

fn expected_lines_from_source(source: &str) -> Vec<String> {
    if source.is_empty() {
        return Vec::new();
    }

    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = normalized
        .split('\n')
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
    if normalized.ends_with('\n') {
        lines.pop();
    }
    lines
}

fn test_summary_json(report: &NexusTestReport) -> String {
    format!(
        "{{\"passed\":{},\"failed\":{},\"total\":{}}}",
        report.passed(),
        report.failed(),
        report.total()
    )
}

fn test_cases_json(cases: &[NexusTestCaseResult]) -> String {
    let mut out = String::from("[");
    for (index, case) in cases.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&test_case_json(case));
    }
    out.push(']');
    out
}

fn test_list_cases_json(cases: &[PathBuf]) -> String {
    let mut out = String::from("[");
    for (index, case) in cases.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"path\":{},\"status\":\"listed\"}}",
            json_string(case.to_string_lossy().as_ref())
        ));
    }
    out.push(']');
    out
}

fn test_case_json(case: &NexusTestCaseResult) -> String {
    let ok = case.passed();
    format!(
        "{{\"path\":{},\"ok\":{},\"status\":{},\"timed_out\":{},\"isolated_data_dir\":{},\"output\":{},\"expected_output\":{},\"expected_diagnostic\":{},\"expected_output_updated\":{},\"expected_diagnostic_updated\":{},\"output_mismatch\":{},\"diagnostic_mismatch\":{},\"diagnostics\":{}}}",
        json_string(case.path.to_string_lossy().as_ref()),
        if ok { "true" } else { "false" },
        json_string(if ok { "passed" } else { "failed" }),
        if case.timed_out { "true" } else { "false" },
        optional_path_json(case.isolated_data_dir.as_deref()),
        string_array_json(&case.output),
        optional_string_array_json(case.expected_output.as_deref()),
        optional_string_array_json(case.expected_diagnostic.as_deref()),
        optional_path_json(case.expected_output_updated.as_deref()),
        optional_path_json(case.expected_diagnostic_updated.as_deref()),
        output_mismatch_json(case.output_mismatch.as_ref()),
        diagnostic_mismatch_json(case.diagnostic_mismatch.as_ref()),
        case_diagnostics_json(case)
    )
}

fn output_mismatch_json(mismatch: Option<&NexusOutputMismatch>) -> String {
    mismatch
        .map(|mismatch| {
            format!(
                "{{\"expected\":{},\"actual\":{},\"first_diff\":{}}}",
                string_array_json(&mismatch.expected),
                string_array_json(&mismatch.actual),
                line_diff_json(mismatch.first_diff.as_ref())
            )
        })
        .unwrap_or_else(|| "null".to_string())
}

fn diagnostic_mismatch_json(mismatch: Option<&NexusDiagnosticMismatch>) -> String {
    mismatch
        .map(|mismatch| {
            format!(
                "{{\"expected\":{},\"actual\":{},\"first_diff\":{}}}",
                string_array_json(&mismatch.expected),
                optional_string_array_json(mismatch.actual.as_deref()),
                line_diff_json(mismatch.first_diff.as_ref())
            )
        })
        .unwrap_or_else(|| "null".to_string())
}

fn line_diff_json(diff: Option<&NexusLineDiff>) -> String {
    diff.map(|diff| {
        format!(
            "{{\"line\":{},\"expected\":{},\"actual\":{}}}",
            diff.line,
            optional_string_json(diff.expected.as_deref()),
            optional_string_json(diff.actual.as_deref())
        )
    })
    .unwrap_or_else(|| "null".to_string())
}

fn case_diagnostics_json(case: &NexusTestCaseResult) -> String {
    let mut out = String::from("[");
    if let Some(diagnostic) = &case.diagnostic {
        out.push_str(&diagnostic_payload_json(diagnostic));
    }
    out.push(']');
    out
}

fn diagnostic_payload_json(error: &MultiModuleDiagnostic) -> String {
    format!(
        "{{\"code\":{},\"severity\":{},\"stage\":{},\"message\":{},\"line\":{},\"column\":{},\"path\":{},\"module_id\":{},\"text\":{}}}",
        optional_string_json(error.diagnostic.code.as_deref()),
        error
            .diagnostic
            .severity
            .map(|severity| json_string(severity.as_str()))
            .unwrap_or_else(|| "null".to_string()),
        json_string(error.diagnostic.stage.as_str()),
        json_string(&error.diagnostic.message),
        optional_usize_json(error.diagnostic.line),
        optional_usize_json(error.diagnostic.column),
        optional_path_json(error.path.as_deref()),
        optional_usize_json(error.module_id.map(|module_id| module_id.0)),
        json_string(&error.to_string())
    )
}

fn optional_string_array_json(values: Option<&[String]>) -> String {
    values
        .map(string_array_json)
        .unwrap_or_else(|| "null".to_string())
}

fn string_array_json(values: &[String]) -> String {
    let mut out = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&json_string(value));
    }
    out.push(']');
    out
}

fn optional_path_json(path: Option<&Path>) -> String {
    path.map(|path| json_string(path.to_string_lossy().as_ref()))
        .unwrap_or_else(|| "null".to_string())
}

fn optional_string_json(value: Option<&str>) -> String {
    value.map(json_string).unwrap_or_else(|| "null".to_string())
}

fn optional_usize_json(value: Option<usize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
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
            '\u{0c}' => out.push_str("\\f"),
            ch if ch < '\u{20}' => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(name: &str) -> Self {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos();
            let path = env::temp_dir().join(format!(
                "nexuslang-test-runner-{}-{}-{}",
                name,
                std::process::id(),
                now
            ));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn run_tests_at_runs_discovered_nx_files() {
        let temp = TempDir::new("discover");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(tests.join("a.nx"), "print(\"a\")\n").expect("write a");
        fs::write(tests.join("b.nx"), "print(\"b\")\n").expect("write b");

        let report = run_tests_at(&tests).expect("run tests");

        assert_eq!(report.total(), 2);
        assert_eq!(report.passed(), 2);
        assert_eq!(report.failed(), 0);
        assert_eq!(report.cases[0].output, ["a"]);
        assert_eq!(report.cases[1].output, ["b"]);
    }

    #[test]
    fn run_tests_at_matches_optional_out_sidecar() {
        let temp = TempDir::new("sidecar-match");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(tests.join("smoke.nx"), "print(\"ok\")\nprint(2)\n").expect("write source");
        fs::write(tests.join("smoke.out"), "ok\n2\n").expect("write sidecar");

        let report = run_tests_at(&tests).expect("run tests");

        assert_eq!(report.total(), 1);
        assert_eq!(report.passed(), 1);
        assert_eq!(
            report.cases[0].expected_output.as_ref().expect("expected"),
            &["ok".to_string(), "2".to_string()]
        );
        assert!(report.cases[0].output_mismatch.is_none());
    }

    #[test]
    fn run_tests_at_reports_out_sidecar_mismatch() {
        let temp = TempDir::new("sidecar-mismatch");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(tests.join("smoke.nx"), "print(\"actual\")\n").expect("write source");
        fs::write(tests.join("smoke.out"), "expected\n").expect("write sidecar");

        let report = run_tests_at(&tests).expect("run tests");

        assert_eq!(report.total(), 1);
        assert_eq!(report.passed(), 0);
        assert_eq!(report.failed(), 1);
        assert!(report.cases[0].diagnostic.is_none());
        assert_eq!(
            report.cases[0].output_mismatch.as_ref().expect("mismatch"),
            &NexusOutputMismatch {
                expected: vec!["expected".to_string()],
                actual: vec!["actual".to_string()],
                first_diff: Some(NexusLineDiff {
                    line: 1,
                    expected: Some("expected".to_string()),
                    actual: Some("actual".to_string()),
                }),
            }
        );
    }

    #[test]
    fn run_tests_at_matches_optional_err_sidecar() {
        let temp = TempDir::new("err-sidecar-match");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(
            tests.join("failing.nx"),
            "print(\"before\")\nprint(10 / 0)\n",
        )
        .expect("write source");
        fs::write(tests.join("failing.out"), "before\n").expect("write output sidecar");
        fs::write(tests.join("failing.err"), "Divisão por zero\n").expect("write err sidecar");

        let report = run_tests_at(&tests).expect("run tests");

        assert_eq!(report.total(), 1);
        assert_eq!(report.passed(), 1);
        assert_eq!(report.failed(), 0);
        assert_eq!(report.cases[0].output, ["before"]);
        assert_eq!(
            report.cases[0]
                .expected_diagnostic
                .as_ref()
                .expect("expected diagnostic"),
            &["Divisão por zero".to_string()]
        );
        assert!(report.cases[0].diagnostic.is_none());
        assert!(report.cases[0].diagnostic_mismatch.is_none());
    }

    #[test]
    fn run_tests_at_reports_err_sidecar_mismatch_when_program_succeeds() {
        let temp = TempDir::new("err-sidecar-missing");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(tests.join("smoke.nx"), "print(\"ok\")\n").expect("write source");
        fs::write(tests.join("smoke.err"), "Divisão por zero\n").expect("write err sidecar");

        let report = run_tests_at(&tests).expect("run tests");

        assert_eq!(report.total(), 1);
        assert_eq!(report.passed(), 0);
        assert_eq!(report.failed(), 1);
        assert!(report.cases[0].diagnostic.is_none());
        assert_eq!(
            report.cases[0]
                .diagnostic_mismatch
                .as_ref()
                .expect("diagnostic mismatch"),
            &NexusDiagnosticMismatch {
                expected: vec!["Divisão por zero".to_string()],
                actual: None,
                first_diff: Some(NexusLineDiff {
                    line: 1,
                    expected: Some("Divisão por zero".to_string()),
                    actual: None,
                }),
            }
        );
        let json = test_report_json(&report);
        assert!(
            json.contains(
                r#""diagnostic_mismatch":{"expected":["Divisão por zero"],"actual":null,"first_diff":{"line":1,"expected":"Divisão por zero","actual":null}}"#
            ),
            "json: {json}"
        );
    }

    #[test]
    fn run_tests_at_with_update_writes_out_sidecar_and_passes() {
        let temp = TempDir::new("update-sidecar");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(tests.join("smoke.nx"), "print(\"fresh\")\nprint(7)\n").expect("write source");

        let report = run_tests_at_with_options(
            &tests,
            NexusTestOptions {
                update_expected: true,
                ..Default::default()
            },
        )
        .expect("run tests");

        assert_eq!(report.total(), 1);
        assert_eq!(report.passed(), 1);
        assert_eq!(
            fs::read_to_string(tests.join("smoke.out")).expect("read sidecar"),
            "fresh\n7\n"
        );
        assert_eq!(
            report.cases[0]
                .expected_output_updated
                .as_ref()
                .expect("updated"),
            &tests.join("smoke.out")
        );
        assert!(report.cases[0].output_mismatch.is_none());
    }

    #[test]
    fn run_tests_at_with_update_replaces_mismatched_out_sidecar() {
        let temp = TempDir::new("update-existing-sidecar");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(tests.join("smoke.nx"), "print(\"actual\")\n").expect("write source");
        fs::write(tests.join("smoke.out"), "stale\n").expect("write stale sidecar");

        let report = run_tests_at_with_options(
            &tests,
            NexusTestOptions {
                update_expected: true,
                ..Default::default()
            },
        )
        .expect("run tests");

        assert_eq!(report.passed(), 1);
        assert_eq!(
            fs::read_to_string(tests.join("smoke.out")).expect("read sidecar"),
            "actual\n"
        );
    }

    #[test]
    fn run_tests_at_with_update_does_not_write_sidecar_on_runtime_failure() {
        let temp = TempDir::new("update-runtime-failure");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(
            tests.join("failing.nx"),
            "print(\"before\")\nprint(10 / 0)\n",
        )
        .expect("write source");

        let report = run_tests_at_with_options(
            &tests,
            NexusTestOptions {
                update_expected: true,
                ..Default::default()
            },
        )
        .expect("run tests");

        assert_eq!(report.failed(), 1);
        assert!(report.cases[0].diagnostic.is_some());
        assert!(!tests.join("failing.out").exists());
    }

    #[test]
    fn run_tests_at_with_update_err_writes_err_sidecar_and_passes() {
        let temp = TempDir::new("update-err");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(
            tests.join("failing.nx"),
            "print(\"before\")\nprint(10 / 0)\n",
        )
        .expect("write source");

        let report = run_tests_at_with_options(
            &tests,
            NexusTestOptions {
                update_expected_diagnostic: true,
                ..Default::default()
            },
        )
        .expect("run tests");

        assert_eq!(report.total(), 1);
        assert_eq!(report.passed(), 1);
        assert_eq!(
            fs::read_to_string(tests.join("failing.err")).expect("read err sidecar"),
            "Divisão por zero\n"
        );
        assert_eq!(
            report.cases[0]
                .expected_diagnostic_updated
                .as_ref()
                .expect("updated"),
            &tests.join("failing.err")
        );
        assert_eq!(
            report.cases[0]
                .expected_diagnostic
                .as_ref()
                .expect("expected diagnostic"),
            &["Divisão por zero".to_string()]
        );
        assert!(report.cases[0].diagnostic.is_none());
    }

    #[test]
    fn run_tests_at_with_update_err_does_not_write_err_sidecar_on_success() {
        let temp = TempDir::new("update-err-success");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(tests.join("smoke.nx"), "print(\"ok\")\n").expect("write source");

        let report = run_tests_at_with_options(
            &tests,
            NexusTestOptions {
                update_expected_diagnostic: true,
                ..Default::default()
            },
        )
        .expect("run tests");

        assert_eq!(report.total(), 1);
        assert_eq!(report.passed(), 1);
        assert!(!tests.join("smoke.err").exists());
        assert!(report.cases[0].expected_diagnostic_updated.is_none());
    }

    #[test]
    fn run_tests_at_with_update_and_update_err_does_not_write_out_for_failure() {
        let temp = TempDir::new("update-output-and-err-failure");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(
            tests.join("failing.nx"),
            "print(\"before\")\nprint(10 / 0)\n",
        )
        .expect("write source");

        let report = run_tests_at_with_options(
            &tests,
            NexusTestOptions {
                update_expected: true,
                update_expected_diagnostic: true,
                ..Default::default()
            },
        )
        .expect("run tests");

        assert_eq!(report.total(), 1);
        assert_eq!(report.passed(), 1);
        assert!(!tests.join("failing.out").exists());
        assert_eq!(
            fs::read_to_string(tests.join("failing.err")).expect("read err sidecar"),
            "Divisão por zero\n"
        );
    }

    #[test]
    fn run_tests_at_with_name_filter_runs_only_matching_files() {
        let temp = TempDir::new("name-filter");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(tests.join("smoke_invoice.nx"), "print(\"selected\")\n").expect("write selected");
        fs::write(tests.join("failing_inventory.nx"), "print(10 / 0)\n").expect("write failing");

        let report = run_tests_at_with_options(
            &tests,
            NexusTestOptions {
                name_filter: Some("invoice".to_string()),
                ..Default::default()
            },
        )
        .expect("run tests");

        assert_eq!(report.total(), 1);
        assert_eq!(report.passed(), 1);
        assert_eq!(report.cases[0].output, ["selected"]);
        assert_eq!(report.cases[0].path, tests.join("smoke_invoice.nx"));
    }

    #[test]
    fn list_tests_at_with_options_filters_without_execution_or_update() {
        let temp = TempDir::new("list-filter");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(tests.join("smoke_invoice.nx"), "print(10 / 0)\n").expect("write selected");
        fs::write(tests.join("smoke_inventory.nx"), "print(\"inventory\")\n")
            .expect("write skipped");

        let report = list_tests_at_with_options(
            &tests,
            &NexusTestOptions {
                update_expected: true,
                name_filter: Some("invoice".to_string()),
                ..Default::default()
            },
        )
        .expect("list tests");

        assert_eq!(report.target, tests);
        assert_eq!(report.cases, [report.target.join("smoke_invoice.nx")]);
        assert!(!report.target.join("smoke_invoice.out").exists());
    }

    #[test]
    fn run_tests_at_with_jobs_preserves_sorted_case_order() {
        let temp = TempDir::new("jobs-order");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(tests.join("b.nx"), "print(\"b\")\n").expect("write b");
        fs::write(tests.join("a.nx"), "print(\"a\")\n").expect("write a");

        let report = run_tests_at_with_options(
            &tests,
            NexusTestOptions {
                jobs: 2,
                ..Default::default()
            },
        )
        .expect("run tests");

        assert_eq!(report.total(), 2);
        assert_eq!(report.passed(), 2);
        assert_eq!(report.cases[0].path, tests.join("a.nx"));
        assert_eq!(report.cases[1].path, tests.join("b.nx"));
        assert_eq!(report.cases[0].output, ["a"]);
        assert_eq!(report.cases[1].output, ["b"]);
    }

    #[test]
    fn run_tests_at_with_update_and_name_filter_updates_only_matching_sidecars() {
        let temp = TempDir::new("update-name-filter");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(tests.join("smoke_billing.nx"), "print(\"billing\")\n").expect("write selected");
        fs::write(tests.join("smoke_inventory.nx"), "print(\"inventory\")\n")
            .expect("write skipped");

        let report = run_tests_at_with_options(
            &tests,
            NexusTestOptions {
                update_expected: true,
                name_filter: Some("billing".to_string()),
                ..Default::default()
            },
        )
        .expect("run tests");

        assert_eq!(report.total(), 1);
        assert_eq!(report.passed(), 1);
        assert_eq!(
            fs::read_to_string(tests.join("smoke_billing.out")).expect("read sidecar"),
            "billing\n"
        );
        assert!(!tests.join("smoke_inventory.out").exists());
    }

    #[test]
    fn test_report_json_serializes_success_summary_case_and_output() {
        let temp = TempDir::new("json-success");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(tests.join("smoke.nx"), "print(\"ok\")\n").expect("write source");
        fs::write(tests.join("smoke.out"), "ok\n").expect("write sidecar");

        let report = run_tests_at(&tests).expect("run tests");
        let json = test_report_json(&report);

        assert!(json.contains(r#""ok":true"#), "json: {json}");
        assert!(json.contains(r#""schema_version":1"#), "json: {json}");
        assert!(json.contains(r#""command":"test""#), "json: {json}");
        assert!(
            json.contains(r#""summary":{"passed":1,"failed":0,"total":1}"#),
            "json: {json}"
        );
        assert!(json.contains(r#""path":"#), "json: {json}");
        assert!(json.contains("smoke.nx"), "json: {json}");
        assert!(json.contains(r#""output":["ok"]"#), "json: {json}");
        assert!(json.contains(r#""expected_output":["ok"]"#), "json: {json}");
        assert!(json.contains(r#""diagnostics":[]"#), "json: {json}");
    }

    #[test]
    fn test_report_json_serializes_mismatch_and_failure_status() {
        let temp = TempDir::new("json-mismatch");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(tests.join("smoke.nx"), "print(\"actual\")\n").expect("write source");
        fs::write(tests.join("smoke.out"), "expected\n").expect("write sidecar");

        let report = run_tests_at(&tests).expect("run tests");
        let json = test_report_json(&report);

        assert!(json.contains(r#""ok":false"#), "json: {json}");
        assert!(
            json.contains(r#""summary":{"passed":0,"failed":1,"total":1}"#),
            "json: {json}"
        );
        assert!(json.contains(r#""status":"failed""#), "json: {json}");
        assert!(
            json.contains(
                r#""output_mismatch":{"expected":["expected"],"actual":["actual"],"first_diff":{"line":1,"expected":"expected","actual":"actual"}}"#
            ),
            "json: {json}"
        );
    }

    #[test]
    fn test_error_json_serializes_discovery_errors_for_tooling() {
        let json = test_error_json("Nenhum ficheiro .nx encontrado");

        assert!(json.contains(r#""ok":false"#), "json: {json}");
        assert!(
            json.contains(r#""error":{"message":"Nenhum ficheiro .nx encontrado"}"#),
            "json: {json}"
        );
        assert!(json.contains(r#""cases":[]"#), "json: {json}");
    }

    #[test]
    fn test_list_report_json_serializes_filtered_cases_for_tooling() {
        let temp = TempDir::new("json-list");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(tests.join("b.nx"), "print(\"b\")\n").expect("write b");
        fs::write(tests.join("a.nx"), "print(\"a\")\n").expect("write a");

        let report =
            list_tests_at_with_options(&tests, &NexusTestOptions::default()).expect("list tests");
        let json = test_list_report_json(&report);

        assert!(json.contains(r#""ok":true"#), "json: {json}");
        assert!(json.contains(r#""mode":"list""#), "json: {json}");
        assert!(
            json.contains(r#""summary":{"passed":0,"failed":0,"total":2}"#),
            "json: {json}"
        );
        assert!(json.contains(r#""status":"listed""#), "json: {json}");
        let a_index = json.find("a.nx").expect("a listed");
        let b_index = json.find("b.nx").expect("b listed");
        assert!(
            a_index < b_index,
            "json list should preserve sorted order: {json}"
        );
    }

    #[test]
    fn expected_lines_preserve_intentional_empty_output_lines() {
        assert_eq!(expected_lines_from_source(""), Vec::<String>::new());
        assert_eq!(expected_lines_from_source("\n"), vec!["".to_string()]);
        assert_eq!(
            expected_lines_from_source("a\n\n"),
            vec!["a".to_string(), "".to_string()]
        );
        assert_eq!(expected_lines_from_source("a\r\nb\r\n"), vec!["a", "b"]);
    }

    #[test]
    fn serialize_output_lines_roundtrips_expected_lines() {
        assert_eq!(serialize_output_lines(&[]), "");
        assert_eq!(serialize_output_lines(&["".to_string()]), "\n");
        assert_eq!(
            expected_lines_from_source(&serialize_output_lines(&["a".to_string(), "".to_string()])),
            vec!["a".to_string(), "".to_string()]
        );
    }

    #[test]
    fn match_output_reports_first_divergent_line() {
        let mismatch = match_output(
            &["same".to_string(), "expected".to_string()],
            &["same".to_string(), "actual".to_string()],
        )
        .expect("mismatch");

        assert_eq!(
            mismatch.first_diff,
            Some(NexusLineDiff {
                line: 2,
                expected: Some("expected".to_string()),
                actual: Some("actual".to_string()),
            })
        );
    }

    #[test]
    fn run_tests_at_records_runtime_failures_without_stopping() {
        let temp = TempDir::new("failure");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(tests.join("a.nx"), "print(\"before\")\nprint(10 / 0)\n").expect("write a");
        fs::write(tests.join("b.nx"), "print(\"after\")\n").expect("write b");

        let report = run_tests_at(&tests).expect("run tests");

        assert_eq!(report.total(), 2);
        assert_eq!(report.passed(), 1);
        assert_eq!(report.failed(), 1);
        assert_eq!(report.cases[0].output, ["before"]);
        assert!(report.cases[0].diagnostic.is_some());
        assert_eq!(report.cases[1].output, ["after"]);
        assert!(report.cases[1].passed());
    }

    #[test]
    fn run_tests_at_with_fail_fast_stops_after_first_failure() {
        let temp = TempDir::new("fail-fast");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(
            tests.join("a_fail.nx"),
            "print(\"before\")\nprint(10 / 0)\n",
        )
        .expect("write failing");
        fs::write(tests.join("b_after.nx"), "print(\"after\")\n").expect("write after");

        let report = run_tests_at_with_options(
            &tests,
            NexusTestOptions {
                fail_fast: true,
                ..Default::default()
            },
        )
        .expect("run tests");

        assert_eq!(report.total(), 1);
        assert_eq!(report.passed(), 0);
        assert_eq!(report.failed(), 1);
        assert_eq!(report.cases[0].path, tests.join("a_fail.nx"));
        assert_eq!(report.cases[0].output, ["before"]);
        assert!(report.cases[0].diagnostic.is_some());
    }

    #[test]
    fn run_tests_at_with_fail_fast_continues_after_expected_err_match() {
        let temp = TempDir::new("fail-fast-expected-err");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(
            tests.join("a_expected_err.nx"),
            "print(\"expected\")\nprint(10 / 0)\n",
        )
        .expect("write expected failure");
        fs::write(tests.join("a_expected_err.err"), "Divisão por zero\n")
            .expect("write err sidecar");
        fs::write(
            tests.join("b_unexpected_err.nx"),
            "print(\"unexpected\")\nprint(10 / 0)\n",
        )
        .expect("write unexpected failure");
        fs::write(tests.join("c_after.nx"), "print(\"after\")\n").expect("write after");

        let report = run_tests_at_with_options(
            &tests,
            NexusTestOptions {
                fail_fast: true,
                ..Default::default()
            },
        )
        .expect("run tests");

        assert_eq!(report.total(), 2);
        assert_eq!(report.passed(), 1);
        assert_eq!(report.failed(), 1);
        assert_eq!(report.cases[0].path, tests.join("a_expected_err.nx"));
        assert!(report.cases[0].passed());
        assert_eq!(report.cases[1].path, tests.join("b_unexpected_err.nx"));
        assert!(!report.cases[1].passed());
    }

    #[test]
    fn run_tests_at_with_jobs_fail_fast_stops_after_failed_batch() {
        let temp = TempDir::new("jobs-fail-fast");
        let tests = temp.path.join("tests");
        fs::create_dir_all(&tests).expect("create tests");
        fs::write(
            tests.join("a_fail.nx"),
            "print(\"before\")\nprint(10 / 0)\n",
        )
        .expect("write failing");
        fs::write(tests.join("b_pass.nx"), "print(\"same batch\")\n").expect("write batch pass");
        fs::write(tests.join("c_after.nx"), "print(\"after\")\n").expect("write after");

        let report = run_tests_at_with_options(
            &tests,
            NexusTestOptions {
                jobs: 2,
                fail_fast: true,
                ..Default::default()
            },
        )
        .expect("run tests");

        assert_eq!(report.total(), 2);
        assert_eq!(report.passed(), 1);
        assert_eq!(report.failed(), 1);
        assert_eq!(report.cases[0].path, tests.join("a_fail.nx"));
        assert_eq!(report.cases[1].path, tests.join("b_pass.nx"));
        assert_eq!(report.cases[1].output, ["same batch"]);
    }

    #[test]
    fn default_test_target_prefers_tests_over_examples() {
        let temp = TempDir::new("default");
        fs::write(
            temp.path.join(package_manager::MANIFEST_FILE),
            "[package]\nname = \"default-test\"\nversion = \"0.1.0\"\nentry = \"main.nx\"\n\n[dependencies]\n",
        )
        .expect("write manifest");
        fs::write(temp.path.join("main.nx"), "print(\"main\")\n").expect("write main");
        fs::create_dir_all(temp.path.join("tests")).expect("create tests");
        fs::create_dir_all(temp.path.join("examples")).expect("create examples");
        fs::write(
            temp.path.join("tests").join("smoke.nx"),
            "print(\"test\")\n",
        )
        .expect("write test");
        fs::write(
            temp.path.join("examples").join("example.nx"),
            "print(\"example\")\n",
        )
        .expect("write example");

        let target = default_test_target_from(&temp.path).expect("default target");

        assert_eq!(target, temp.path.join("tests"));
    }
}
