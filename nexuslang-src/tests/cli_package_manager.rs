use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

struct TempProject {
    path: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("nexuslang-{}-{}-{}", name, std::process::id(), now));
        fs::create_dir_all(&path).expect("create temp project");
        Self { path }
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn nexus() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nexus"));
    command.env_remove("NEXUS_REGISTRY_URL");
    command
}

fn run_nexus(current_dir: &Path, args: &[&str]) -> Output {
    nexus()
        .current_dir(current_dir)
        .args(args)
        .output()
        .expect("run nexus")
}

fn run_nexus_with_registry(current_dir: &Path, args: &[&str], registry_root: &Path) -> Output {
    nexus()
        .current_dir(current_dir)
        .env("NEXUS_REGISTRY_URL", registry_root)
        .args(args)
        .output()
        .expect("run nexus with registry")
}

fn run_nexus_with_registry_url(current_dir: &Path, args: &[&str], registry_url: &str) -> Output {
    nexus()
        .current_dir(current_dir)
        .env("NEXUS_REGISTRY_URL", registry_url)
        .args(args)
        .output()
        .expect("run nexus with registry url")
}

fn assert_success(output: Output) {
    if !output.status.success() {
        panic!(
            "command failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn assert_success_stdout(output: Output) -> String {
    if !output.status.success() {
        panic!(
            "command failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn assert_failure(output: Output) -> String {
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{:02x}", byte)).collect()
}

fn write_registry_package(
    registry_root: &Path,
    name: &str,
    version: &str,
    files: &[(&str, &str)],
) -> String {
    let version_dir = registry_root.join(name).join(version);
    fs::create_dir_all(&version_dir).expect("create registry version dir");
    let archive_name = format!("{name}-{version}.tar");
    let archive = tar_archive(files);
    let checksum = sha256_hex(&archive);
    fs::write(version_dir.join(&archive_name), &archive).expect("write registry archive");
    fs::write(
        version_dir.join("nexus-package.toml"),
        format!(
            "name = \"{name}\"\nversion = \"{version}\"\narchive = \"{archive_name}\"\nsha256 = \"{checksum}\"\n"
        ),
    )
    .expect("write registry metadata");
    checksum
}

fn write_registry_package_with_archive(
    registry_root: &Path,
    name: &str,
    version: &str,
    archive_name: &str,
    archive: &[u8],
    checksum: &str,
) {
    let version_dir = registry_root.join(name).join(version);
    fs::create_dir_all(&version_dir).expect("create registry version dir");
    fs::write(version_dir.join(archive_name), archive).expect("write registry archive");
    fs::write(
        version_dir.join("nexus-package.toml"),
        format!(
            "name = \"{name}\"\nversion = \"{version}\"\narchive = \"{archive_name}\"\nsha256 = \"{checksum}\"\n"
        ),
    )
    .expect("write registry metadata");
}

fn tar_archive(files: &[(&str, &str)]) -> Vec<u8> {
    let mut output = Vec::new();
    for (name, source) in files {
        append_tar_file(&mut output, name, source.as_bytes());
    }
    output.extend_from_slice(&[0u8; 1024]);
    output
}

fn append_tar_file(output: &mut Vec<u8>, name: &str, content: &[u8]) {
    assert!(
        name.len() <= 100,
        "test tar helper only supports short names"
    );
    let mut header = [0u8; 512];
    header[..name.len()].copy_from_slice(name.as_bytes());
    write_octal(&mut header[100..108], 0o644);
    write_octal(&mut header[108..116], 0);
    write_octal(&mut header[116..124], 0);
    write_octal(&mut header[124..136], content.len());
    write_octal(&mut header[136..148], 0);
    header[148..156].fill(b' ');
    header[156] = b'0';
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    let checksum: usize = header.iter().map(|byte| *byte as usize).sum();
    let checksum_text = format!("{:06o}\0 ", checksum);
    header[148..156].copy_from_slice(checksum_text.as_bytes());
    output.extend_from_slice(&header);
    output.extend_from_slice(content);
    let padding = (512 - (content.len() % 512)) % 512;
    output.extend(std::iter::repeat_n(0, padding));
}

fn write_octal(field: &mut [u8], value: usize) {
    let text = format!("{:0width$o}\0", value, width = field.len() - 1);
    field.copy_from_slice(text.as_bytes());
}

fn registry_http_server(root: PathBuf, requests: usize) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind registry http server");
    let url = format!("http://{}", listener.local_addr().expect("local addr"));
    let handle = thread::spawn(move || {
        for _ in 0..requests {
            let (mut stream, _) = listener.accept().expect("accept registry request");
            let mut buffer = [0u8; 2048];
            let size = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..size]);
            let request_path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("/");
            let relative_path = request_path.trim_start_matches('/');
            let file_path = root.join(relative_path);
            match fs::read(&file_path) {
                Ok(body) => {
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .expect("write response headers");
                    stream.write_all(&body).expect("write response body");
                }
                Err(_) => {
                    stream
                        .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                        .expect("write not found");
                }
            }
        }
    });
    (url, handle)
}

#[test]
fn package_install_creates_manifest_lock_and_local_cache() {
    let project = TempProject::new("install");

    assert_success(run_nexus(&project.path, &["install"]));

    let manifest = fs::read_to_string(project.path.join("nexus.toml")).expect("manifest");
    let lockfile = fs::read_to_string(project.path.join("nexus.lock")).expect("lockfile");

    assert!(manifest.contains("[package]"));
    assert!(manifest.contains("entry = \"main.nx\""));
    assert!(manifest.contains("[dependencies]"));
    assert!(lockfile.contains("version = 1"));
    assert!(project.path.join(".nexus").join("packages").is_dir());
}

#[test]
fn package_add_records_dependency_and_update_refreshes_lockfile() {
    let project = TempProject::new("add-update");

    assert_success(run_nexus(&project.path, &["add", "crm_core"]));
    assert_success(run_nexus(&project.path, &["add", "crm_core"]));
    assert_success(run_nexus(&project.path, &["update"]));

    let manifest = fs::read_to_string(project.path.join("nexus.toml")).expect("manifest");
    let lockfile = fs::read_to_string(project.path.join("nexus.lock")).expect("lockfile");
    let marker = fs::read_to_string(
        project
            .path
            .join(".nexus")
            .join("packages")
            .join("crm_core")
            .join("PACKAGE.txt"),
    )
    .expect("package marker");

    assert_eq!(manifest.matches("crm_core = \"local\"").count(), 1);
    assert!(lockfile.contains("[[package]]"));
    assert!(lockfile.contains("name = \"crm_core\""));
    assert!(lockfile.contains("source = \"local\""));
    assert!(marker.contains("name=crm_core"));
}

#[test]
fn package_add_supports_path_dependency_and_lock_metadata() {
    let parent = TempProject::new("path-dependency");

    assert_success(run_nexus(&parent.path, &["new", "crm_core"]));
    assert_success(run_nexus(&parent.path, &["new", "erp_app"]));

    let app = parent.path.join("erp_app");
    assert_success(run_nexus(
        &app,
        &["add", "crm-core", "--path", "../crm_core"],
    ));
    assert_success(run_nexus(&app, &["install"]));

    let manifest = fs::read_to_string(app.join("nexus.toml")).expect("manifest");
    let lockfile = fs::read_to_string(app.join("nexus.lock")).expect("lockfile");
    let marker = fs::read_to_string(
        app.join(".nexus")
            .join("packages")
            .join("crm-core")
            .join("PACKAGE.txt"),
    )
    .expect("package marker");

    assert!(manifest.contains("crm-core = \"path:../crm_core\""));
    assert!(lockfile.contains("kind = \"path\""));
    assert!(lockfile.contains("source = \"path:../crm_core\""));
    assert!(lockfile.contains("version = \"0.1.0\""));
    assert!(lockfile.contains("resolved_path = "));
    assert!(marker.contains("kind=path"));
}

#[test]
fn package_update_prunes_stale_local_cache_entries() {
    let project = TempProject::new("prune-cache");

    assert_success(run_nexus(&project.path, &["add", "crm_core"]));
    assert!(project
        .path
        .join(".nexus")
        .join("packages")
        .join("crm_core")
        .exists());

    fs::write(
        project.path.join("nexus.toml"),
        "[package]\nname = \"prune-cache\"\nversion = \"0.1.0\"\nentry = \"main.nx\"\n\n[dependencies]\n",
    )
    .expect("rewrite manifest");

    assert_success(run_nexus(&project.path, &["update"]));
    assert!(!project
        .path
        .join(".nexus")
        .join("packages")
        .join("crm_core")
        .exists());
}

#[test]
fn package_manifest_validation_rejects_unknown_sections_and_bad_paths() {
    let project = TempProject::new("manifest-validation");

    fs::write(
        project.path.join("nexus.toml"),
        "[package]\nname = \"manifest-validation\"\nversion = \"0.1.0\"\nentry = \"main.nx\"\n\n[unexpected]\nvalue = \"x\"\n",
    )
    .expect("write invalid manifest");

    let err = assert_failure(run_nexus(&project.path, &["install"]));
    assert!(err.contains("Secao [unexpected] desconhecida"));

    fs::write(
        project.path.join("nexus.toml"),
        "[package]\nname = \"manifest-validation\"\nversion = \"0.1.0\"\nentry = \"../main.nx\"\n\n[dependencies]\n",
    )
    .expect("write invalid manifest");

    let err = assert_failure(run_nexus(&project.path, &["install"]));
    assert!(err.contains("Entry invalido"));
}

#[test]
fn package_add_supports_registry_contract_without_download() {
    let project = TempProject::new("registry-contract");

    assert_success(run_nexus(
        &project.path,
        &["add", "crm_core", "--registry", "crm_core@0.1.0"],
    ));

    let manifest = fs::read_to_string(project.path.join("nexus.toml")).expect("manifest");
    let lockfile = fs::read_to_string(project.path.join("nexus.lock")).expect("lockfile");
    let marker = fs::read_to_string(
        project
            .path
            .join(".nexus")
            .join("packages")
            .join("crm_core")
            .join("PACKAGE.txt"),
    )
    .expect("package marker");

    assert!(manifest.contains("crm_core = \"registry:crm_core@0.1.0\""));
    assert!(lockfile.contains("kind = \"registry\""));
    assert!(lockfile.contains("registry_package = \"crm_core\""));
    assert!(marker.contains("kind=registry"));

    let err = assert_failure(run_nexus(
        &project.path,
        &["add", "crm_core", "--registry", "other@0.1.0"],
    ));
    assert!(err.contains("nao corresponde"));
}

#[test]
fn package_install_downloads_registry_dependency_and_compiler_resolves_cache() {
    let workspace = TempProject::new("registry-download");
    let registry = workspace.path.join("registry");
    let app = workspace.path.join("erp_app");
    fs::create_dir_all(&app).expect("create app");
    let checksum = write_registry_package(
        &registry,
        "crm_core",
        "0.1.0",
        &[
            (
                "nexus.toml",
                r#"[package]
name = "crm_core"
version = "0.1.0"
entry = "main.nx"

[dependencies]
"#,
            ),
            (
                "main.nx",
                r#"
export fn customer_label() -> string {
    return "Cliente via registry"
}
"#,
            ),
            (
                "models.nx",
                r#"
export fn account_status() -> string {
    return "Conta remota ativa"
}
"#,
            ),
        ],
    );

    fs::write(
        app.join("nexus.toml"),
        r#"[package]
name = "erp_app"
version = "0.1.0"
entry = "main.nx"

[dependencies]
crm_core = "registry:crm_core@0.1.0"
"#,
    )
    .expect("write app manifest");
    fs::write(
        app.join("main.nx"),
        r#"
import customer_label from "crm_core"
import account_status from "crm_core/models"

print(customer_label())
print(account_status())
"#,
    )
    .expect("write app entry");

    assert_success(run_nexus_with_registry(&app, &["install"], &registry));

    let package_dir = app.join(".nexus").join("packages").join("crm_core");
    let lockfile = fs::read_to_string(app.join("nexus.lock")).expect("lockfile");
    let marker = fs::read_to_string(package_dir.join("PACKAGE.txt")).expect("package marker");

    assert!(package_dir.join("nexus.toml").is_file());
    assert!(package_dir.join("main.nx").is_file());
    assert!(lockfile.contains("kind = \"registry\""));
    assert!(lockfile.contains("resolved_path = "));
    assert!(lockfile.contains(&format!("checksum = \"sha256:{checksum}\"")));
    assert!(marker.contains(&format!("checksum=sha256:{checksum}")));

    assert_success(run_nexus(&app, &["check"]));
    let stdout = assert_success_stdout(run_nexus(&app, &["run"]));
    assert!(stdout.contains("Cliente via registry"), "stdout: {stdout}");
    assert!(stdout.contains("Conta remota ativa"), "stdout: {stdout}");
}

#[test]
fn package_install_supports_plain_http_registry_base() {
    let workspace = TempProject::new("registry-http");
    let registry = workspace.path.join("registry");
    let app = workspace.path.join("erp_app");
    fs::create_dir_all(&app).expect("create app");
    write_registry_package(
        &registry,
        "crm_core",
        "0.1.0",
        &[
            (
                "nexus.toml",
                r#"[package]
name = "crm_core"
version = "0.1.0"
entry = "main.nx"

[dependencies]
"#,
            ),
            (
                "main.nx",
                r#"
export fn customer_label() -> string {
    return "Cliente via http registry"
}
"#,
            ),
        ],
    );
    fs::write(
        app.join("nexus.toml"),
        r#"[package]
name = "erp_app"
version = "0.1.0"
entry = "main.nx"

[dependencies]
crm_core = "registry:crm_core@0.1.0"
"#,
    )
    .expect("write app manifest");

    let (registry_url, server) = registry_http_server(registry, 3);
    assert_success(run_nexus_with_registry_url(
        &app,
        &["install"],
        &registry_url,
    ));
    server.join().expect("registry http server");

    assert!(app
        .join(".nexus")
        .join("packages")
        .join("crm_core")
        .join("main.nx")
        .is_file());
}

#[test]
fn package_install_rejects_registry_checksum_mismatch() {
    let workspace = TempProject::new("registry-checksum");
    let registry = workspace.path.join("registry");
    let app = workspace.path.join("app");
    fs::create_dir_all(&app).expect("create app");
    write_registry_package(
        &registry,
        "crm_core",
        "0.1.0",
        &[(
            "nexus.toml",
            r#"[package]
name = "crm_core"
version = "0.1.0"
entry = "main.nx"

[dependencies]
"#,
        )],
    );
    fs::write(
        registry
            .join("crm_core")
            .join("0.1.0")
            .join("nexus-package.toml"),
        "name = \"crm_core\"\nversion = \"0.1.0\"\narchive = \"crm_core-0.1.0.tar\"\nsha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n",
    )
    .expect("rewrite metadata");
    fs::write(
        app.join("nexus.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nentry = \"main.nx\"\n\n[dependencies]\ncrm_core = \"registry:crm_core@0.1.0\"\n",
    )
    .expect("write app manifest");

    let err = assert_failure(run_nexus_with_registry(&app, &["install"], &registry));
    assert!(err.contains("Checksum invalido"), "stderr: {err}");
}

#[test]
fn package_install_rejects_registry_archive_path_traversal() {
    let workspace = TempProject::new("registry-traversal");
    let registry = workspace.path.join("registry");
    let app = workspace.path.join("app");
    fs::create_dir_all(&app).expect("create app");
    let archive = tar_archive(&[("../evil.nx", "print(\"bad\")")]);
    let checksum = sha256_hex(&archive);
    write_registry_package_with_archive(
        &registry,
        "crm_core",
        "0.1.0",
        "crm_core-0.1.0.tar",
        &archive,
        &checksum,
    );
    fs::write(
        app.join("nexus.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nentry = \"main.nx\"\n\n[dependencies]\ncrm_core = \"registry:crm_core@0.1.0\"\n",
    )
    .expect("write app manifest");

    let err = assert_failure(run_nexus_with_registry(&app, &["install"], &registry));
    assert!(err.contains("caminho inseguro"), "stderr: {err}");
    assert!(!workspace.path.join("evil.nx").exists());
}

#[test]
fn package_install_rejects_invalid_or_missing_registry_metadata() {
    let workspace = TempProject::new("registry-metadata");
    let registry = workspace.path.join("registry");
    let app = workspace.path.join("app");
    fs::create_dir_all(registry.join("crm_core").join("0.1.0")).expect("create registry");
    fs::create_dir_all(&app).expect("create app");
    fs::write(
        registry
            .join("crm_core")
            .join("0.1.0")
            .join("nexus-package.toml"),
        "name = \"crm_core\"\nversion = \"0.1.0\"\n",
    )
    .expect("write invalid metadata");
    fs::write(
        app.join("nexus.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nentry = \"main.nx\"\n\n[dependencies]\ncrm_core = \"registry:crm_core@0.1.0\"\n",
    )
    .expect("write app manifest");

    let err = assert_failure(run_nexus_with_registry(&app, &["install"], &registry));
    assert!(err.contains("precisa de archive"), "stderr: {err}");

    fs::remove_file(
        registry
            .join("crm_core")
            .join("0.1.0")
            .join("nexus-package.toml"),
    )
    .expect("remove metadata");
    let err = assert_failure(run_nexus_with_registry(&app, &["install"], &registry));
    assert!(
        err.contains("Falha ao ler registry resource"),
        "stderr: {err}"
    );
}

#[test]
fn new_project_includes_package_manifest_and_lockfile() {
    let parent = TempProject::new("new-parent");

    assert_success(run_nexus(&parent.path, &["new", "erp_app"]));

    let root = parent.path.join("erp_app");
    let manifest = fs::read_to_string(root.join("nexus.toml")).expect("manifest");
    let lockfile = fs::read_to_string(root.join("nexus.lock")).expect("lockfile");

    assert!(manifest.contains("name = \"erp-app\""));
    assert!(manifest.contains("version = \"0.1.0\""));
    assert!(manifest.contains("entry = \"main.nx\""));
    assert!(lockfile.contains("version = 1"));
    assert!(root.join("tests").join("smoke.nx").is_file());
    assert!(root.join("tests").join("smoke.out").is_file());
}
