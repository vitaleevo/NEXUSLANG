use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

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
    Command::new(env!("CARGO_BIN_EXE_nexus"))
}

fn run_nexus(current_dir: &Path, args: &[&str]) -> Output {
    nexus()
        .current_dir(current_dir)
        .args(args)
        .output()
        .expect("run nexus")
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

fn assert_failure(output: Output) -> String {
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stderr).to_string()
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
