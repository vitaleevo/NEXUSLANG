use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const MANIFEST_FILE: &str = "nexus.toml";
pub const LOCK_FILE: &str = "nexus.lock";

#[derive(Debug, Clone)]
struct NexusManifest {
    name: String,
    version: String,
    entry: String,
    dependencies: BTreeMap<String, DependencySource>,
}

#[derive(Debug, Clone)]
pub enum DependencyRequest<'a> {
    Local,
    Path(&'a str),
    Registry(&'a str),
}

#[derive(Debug, Clone)]
enum DependencySource {
    Local,
    Path(String),
    Registry(RegistryDependency),
}

#[derive(Debug, Clone)]
struct RegistryDependency {
    package: String,
    version: String,
}

#[derive(Debug)]
pub struct PackageReport {
    pub root: PathBuf,
    pub manifest_created: bool,
    pub lock_written: bool,
    pub dependency_added: Option<bool>,
    pub dependency_name: Option<String>,
    pub dependency_source: Option<String>,
    pub dependency_count: usize,
}

pub fn install_current_dir() -> Result<PackageReport, String> {
    let root = project_root_or_current()?;
    let (manifest, manifest_created) = load_or_create_manifest(&root)?;
    write_lockfile(&root, &manifest)?;
    sync_local_packages(&root, &manifest)?;

    Ok(PackageReport {
        root,
        manifest_created,
        lock_written: true,
        dependency_added: None,
        dependency_name: None,
        dependency_source: None,
        dependency_count: manifest.dependencies.len(),
    })
}

pub fn add_dependency_current_dir(
    package_name: &str,
    request: DependencyRequest<'_>,
) -> Result<PackageReport, String> {
    validate_package_name(package_name)?;

    let root = project_root_or_current()?;
    let (mut manifest, manifest_created) = load_or_create_manifest(&root)?;
    let dependency_source = source_from_request(package_name, request)?;
    validate_dependency_source(&root, package_name, &dependency_source)?;
    let dependency_source_text = dependency_source.manifest_value();
    let dependency_added = manifest
        .dependencies
        .insert(package_name.to_string(), dependency_source)
        .map(|previous| previous.manifest_value() != dependency_source_text)
        .unwrap_or(true);

    write_manifest(&root, &manifest)?;
    write_lockfile(&root, &manifest)?;
    sync_local_packages(&root, &manifest)?;

    Ok(PackageReport {
        root,
        manifest_created,
        lock_written: true,
        dependency_added: Some(dependency_added),
        dependency_name: Some(package_name.to_string()),
        dependency_source: Some(dependency_source_text),
        dependency_count: manifest.dependencies.len(),
    })
}

pub fn update_current_dir() -> Result<PackageReport, String> {
    let root = project_root_or_current()?;
    let (manifest, manifest_created) = load_or_create_manifest(&root)?;
    write_lockfile(&root, &manifest)?;
    sync_local_packages(&root, &manifest)?;

    Ok(PackageReport {
        root,
        manifest_created,
        lock_written: true,
        dependency_added: None,
        dependency_name: None,
        dependency_source: None,
        dependency_count: manifest.dependencies.len(),
    })
}

pub fn write_new_project_package_files(root: &Path, project_name: &str) -> Result<(), String> {
    let manifest = NexusManifest {
        name: normalize_project_name(project_name),
        version: "0.1.0".to_string(),
        entry: "main.nx".to_string(),
        dependencies: BTreeMap::new(),
    };

    write_manifest(root, &manifest)?;
    write_lockfile(root, &manifest)?;
    Ok(())
}

fn project_root_or_current() -> Result<PathBuf, String> {
    let current = env::current_dir().map_err(|e| e.to_string())?;
    Ok(find_manifest_root(&current).unwrap_or(current))
}

fn find_manifest_root(start: &Path) -> Option<PathBuf> {
    for ancestor in start.ancestors() {
        if ancestor.join(MANIFEST_FILE).is_file() {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

fn load_or_create_manifest(root: &Path) -> Result<(NexusManifest, bool), String> {
    let manifest_path = root.join(MANIFEST_FILE);
    if manifest_path.exists() {
        let source = fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
        let manifest = parse_manifest(&source)?;
        validate_manifest(root, &manifest)?;
        return Ok((manifest, false));
    }

    let project_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("nexus-project");
    let manifest = NexusManifest {
        name: normalize_project_name(project_name),
        version: "0.1.0".to_string(),
        entry: "main.nx".to_string(),
        dependencies: BTreeMap::new(),
    };
    write_manifest(root, &manifest)?;
    Ok((manifest, true))
}

fn parse_manifest(source: &str) -> Result<NexusManifest, String> {
    let mut section = "";
    let mut name = None;
    let mut version = None;
    let mut entry = Some("main.nx".to_string());
    let mut dependencies = BTreeMap::new();
    let mut package_keys = BTreeSet::new();
    let mut dependency_keys = BTreeSet::new();

    for (index, raw_line) in source.lines().enumerate() {
        let line_number = index + 1;
        let line_without_comment = raw_line.split('#').next().unwrap_or("");
        let line = line_without_comment.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            section = line.trim_matches(&['[', ']'][..]).trim();
            match section {
                "package" | "dependencies" => {}
                _ => {
                    return Err(format!(
                        "Secao [{}] desconhecida em {} na linha {}",
                        section, MANIFEST_FILE, line_number
                    ));
                }
            }
            continue;
        }

        if section.is_empty() {
            return Err(format!(
                "Linha {} em {} esta fora de uma secao",
                line_number, MANIFEST_FILE
            ));
        }

        let (key, raw_value) = line
            .split_once('=')
            .ok_or_else(|| format!("Linha {} inválida em {}", line_number, MANIFEST_FILE))?;
        let key = key.trim();
        let value = parse_string_value(raw_value.trim(), line_number)?;

        match section {
            "package" => match key {
                "name" => {
                    ensure_unique_key(&mut package_keys, key, line_number)?;
                    validate_package_name(&value)?;
                    name = Some(value);
                }
                "version" => {
                    ensure_unique_key(&mut package_keys, key, line_number)?;
                    validate_version(&value)?;
                    version = Some(value);
                }
                "entry" => {
                    ensure_unique_key(&mut package_keys, key, line_number)?;
                    validate_entry(&value)?;
                    entry = Some(value);
                }
                _ => {
                    return Err(format!(
                        "Chave [package].{} desconhecida em {} na linha {}",
                        key, MANIFEST_FILE, line_number
                    ));
                }
            },
            "dependencies" => {
                ensure_unique_key(&mut dependency_keys, key, line_number)?;
                validate_package_name(key)?;
                let source = DependencySource::parse(&value)?;
                dependencies.insert(key.to_string(), source);
            }
            _ => {}
        }
    }

    Ok(NexusManifest {
        name: name.ok_or_else(|| "nexus.toml precisa de [package].name".to_string())?,
        version: version.ok_or_else(|| "nexus.toml precisa de [package].version".to_string())?,
        entry: entry.unwrap_or_else(|| "main.nx".to_string()),
        dependencies,
    })
}

fn validate_manifest(root: &Path, manifest: &NexusManifest) -> Result<(), String> {
    validate_package_name(&manifest.name)?;
    validate_version(&manifest.version)?;
    validate_entry(&manifest.entry)?;

    for (name, source) in &manifest.dependencies {
        validate_package_name(name)?;
        validate_dependency_source(root, name, source)?;
    }

    Ok(())
}

fn parse_string_value(raw: &str, line_number: usize) -> Result<String, String> {
    if !raw.starts_with('"') || !raw.ends_with('"') || raw.len() < 2 {
        return Err(format!(
            "Linha {} inválida em {}: use valores entre aspas",
            line_number, MANIFEST_FILE
        ));
    }

    let inner = &raw[1..raw.len() - 1];
    let mut value = String::new();
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('"') => value.push('"'),
                Some('\\') => value.push('\\'),
                Some(other) => {
                    value.push('\\');
                    value.push(other);
                }
                None => value.push('\\'),
            }
        } else {
            value.push(ch);
        }
    }
    Ok(value)
}

fn write_manifest(root: &Path, manifest: &NexusManifest) -> Result<(), String> {
    let mut output = format!(
        "[package]\nname = \"{}\"\nversion = \"{}\"\nentry = \"{}\"\n\n[dependencies]\n",
        escape_string(&manifest.name),
        escape_string(&manifest.version),
        escape_string(&manifest.entry)
    );

    for (name, source) in &manifest.dependencies {
        output.push_str(&format!(
            "{} = \"{}\"\n",
            name,
            escape_string(&source.manifest_value())
        ));
    }

    fs::write(root.join(MANIFEST_FILE), output).map_err(|e| e.to_string())
}

fn write_lockfile(root: &Path, manifest: &NexusManifest) -> Result<(), String> {
    let mut output = "# Generated by NexusLang. Do not edit by hand.\n".to_string();
    output.push_str("version = 1\n\n");

    for (name, source) in &manifest.dependencies {
        let metadata = lock_metadata(root, name, source)?;
        output.push_str("[[package]]\n");
        output.push_str(&format!("name = \"{}\"\n", escape_string(name)));
        output.push_str(&format!("kind = \"{}\"\n", metadata.kind));
        output.push_str(&format!(
            "source = \"{}\"\n",
            escape_string(&source.manifest_value())
        ));
        output.push_str(&format!(
            "version = \"{}\"\n",
            escape_string(&metadata.version)
        ));
        if let Some(resolved_path) = metadata.resolved_path {
            output.push_str(&format!(
                "resolved_path = \"{}\"\n",
                escape_string(&resolved_path)
            ));
        }
        if let Some(registry_package) = metadata.registry_package {
            output.push_str(&format!(
                "registry_package = \"{}\"\n",
                escape_string(&registry_package)
            ));
        }
        output.push('\n');
    }

    fs::write(root.join(LOCK_FILE), output).map_err(|e| e.to_string())
}

fn sync_local_packages(root: &Path, manifest: &NexusManifest) -> Result<(), String> {
    let packages_dir = root.join(".nexus").join("packages");
    fs::create_dir_all(&packages_dir).map_err(|e| e.to_string())?;
    prune_stale_packages(&packages_dir, &manifest.dependencies)?;

    for (name, source) in &manifest.dependencies {
        let package_dir = packages_dir.join(name);
        fs::create_dir_all(&package_dir).map_err(|e| e.to_string())?;
        let metadata = lock_metadata(root, name, source)?;
        let marker = format!(
            "name={}\nkind={}\nsource={}\nversion={}\nmanaged_by=nexus\n",
            name,
            metadata.kind,
            source.manifest_value(),
            metadata.version
        );
        fs::write(package_dir.join("PACKAGE.txt"), marker).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn source_from_request(
    package_name: &str,
    request: DependencyRequest<'_>,
) -> Result<DependencySource, String> {
    match request {
        DependencyRequest::Local => Ok(DependencySource::Local),
        DependencyRequest::Path(path) => {
            if path.trim().is_empty() {
                return Err("Caminho de dependencia local nao pode estar vazio".to_string());
            }
            Ok(DependencySource::Path(normalize_source_path(path)))
        }
        DependencyRequest::Registry(spec) => {
            let registry = RegistryDependency::parse(spec)?;
            if registry.package != package_name {
                return Err(format!(
                    "Registry spec '{}' nao corresponde ao pacote '{}'",
                    spec, package_name
                ));
            }
            Ok(DependencySource::Registry(registry))
        }
    }
}

fn validate_dependency_source(
    root: &Path,
    package_name: &str,
    source: &DependencySource,
) -> Result<(), String> {
    match source {
        DependencySource::Local => Ok(()),
        DependencySource::Path(path) => validate_path_dependency(root, package_name, path),
        DependencySource::Registry(registry) => {
            if registry.package != package_name {
                return Err(format!(
                    "Dependencia '{}' aponta para registry package '{}'",
                    package_name, registry.package
                ));
            }
            validate_version(&registry.version)
        }
    }
}

fn validate_path_dependency(root: &Path, package_name: &str, path: &str) -> Result<(), String> {
    if path.starts_with("registry:") || path.starts_with("path:") {
        return Err(format!("Caminho de dependencia invalido: {}", path));
    }

    let dependency_root = root.join(path);
    if !dependency_root.is_dir() {
        return Err(format!(
            "Dependencia '{}' aponta para caminho inexistente: {}",
            package_name, path
        ));
    }

    let dependency_manifest_path = dependency_root.join(MANIFEST_FILE);
    if !dependency_manifest_path.is_file() {
        return Err(format!(
            "Dependencia '{}' precisa de {} em {}",
            package_name,
            MANIFEST_FILE,
            dependency_root.display()
        ));
    }

    let source = fs::read_to_string(&dependency_manifest_path).map_err(|e| e.to_string())?;
    let dependency_manifest = parse_manifest(&source)?;
    if dependency_manifest.name != package_name {
        return Err(format!(
            "Dependencia '{}' aponta para pacote '{}'",
            package_name, dependency_manifest.name
        ));
    }

    Ok(())
}

fn lock_metadata(
    root: &Path,
    package_name: &str,
    source: &DependencySource,
) -> Result<LockMetadata, String> {
    match source {
        DependencySource::Local => Ok(LockMetadata {
            kind: "local",
            version: "local".to_string(),
            resolved_path: None,
            registry_package: None,
        }),
        DependencySource::Path(path) => {
            validate_path_dependency(root, package_name, path)?;
            let dependency_root = root.join(path);
            let dependency_manifest_path = dependency_root.join(MANIFEST_FILE);
            let source_text =
                fs::read_to_string(&dependency_manifest_path).map_err(|e| e.to_string())?;
            let dependency_manifest = parse_manifest(&source_text)?;
            let resolved_path = dependency_root
                .canonicalize()
                .unwrap_or(dependency_root)
                .display()
                .to_string();
            Ok(LockMetadata {
                kind: "path",
                version: dependency_manifest.version,
                resolved_path: Some(resolved_path),
                registry_package: None,
            })
        }
        DependencySource::Registry(registry) => Ok(LockMetadata {
            kind: "registry",
            version: registry.version.clone(),
            resolved_path: None,
            registry_package: Some(registry.package.clone()),
        }),
    }
}

struct LockMetadata {
    kind: &'static str,
    version: String,
    resolved_path: Option<String>,
    registry_package: Option<String>,
}

fn prune_stale_packages(
    packages_dir: &Path,
    dependencies: &BTreeMap<String, DependencySource>,
) -> Result<(), String> {
    for entry in fs::read_dir(packages_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let file_type = entry.file_type().map_err(|e| e.to_string())?;
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }

        let package_name = entry.file_name().to_string_lossy().to_string();
        if dependencies.contains_key(&package_name) {
            continue;
        }

        validate_cache_child(packages_dir, &entry.path())?;
        fs::remove_dir_all(entry.path()).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn validate_cache_child(packages_dir: &Path, child: &Path) -> Result<(), String> {
    let child_name = child
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Cache local contem caminho invalido".to_string())?;
    validate_package_name(child_name)?;

    if child.parent() != Some(packages_dir) {
        return Err(format!(
            "Recusando limpar cache fora de {}",
            packages_dir.display()
        ));
    }

    Ok(())
}

impl DependencySource {
    fn parse(value: &str) -> Result<Self, String> {
        if value == "local" {
            Ok(Self::Local)
        } else if let Some(path) = value.strip_prefix("path:") {
            if path.trim().is_empty() {
                Err("Dependencia path precisa de caminho".to_string())
            } else {
                Ok(Self::Path(normalize_source_path(path)))
            }
        } else if let Some(spec) = value.strip_prefix("registry:") {
            RegistryDependency::parse(spec).map(Self::Registry)
        } else {
            Err(format!(
                "Origem de dependencia invalida: '{}'. Use 'local', 'path:<dir>' ou 'registry:<pacote>@<versao>'.",
                value
            ))
        }
    }

    fn manifest_value(&self) -> String {
        match self {
            Self::Local => "local".to_string(),
            Self::Path(path) => format!("path:{}", path),
            Self::Registry(registry) => {
                format!("registry:{}@{}", registry.package, registry.version)
            }
        }
    }
}

impl RegistryDependency {
    fn parse(spec: &str) -> Result<Self, String> {
        let (package, version) = spec
            .split_once('@')
            .ok_or_else(|| format!("Registry spec invalido: '{}'. Use <pacote>@<versao>.", spec))?;
        validate_package_name(package)?;
        validate_version(version)?;
        Ok(Self {
            package: package.to_string(),
            version: version.to_string(),
        })
    }
}

fn validate_package_name(name: &str) -> Result<(), String> {
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        && name
            .chars()
            .next()
            .map(|ch| ch.is_ascii_alphanumeric())
            .unwrap_or(false);

    if valid {
        Ok(())
    } else {
        Err(format!(
            "Nome de pacote inválido: '{}'. Use letras, números, '-' ou '_'.",
            name
        ))
    }
}

fn validate_version(version: &str) -> Result<(), String> {
    let valid = !version.is_empty()
        && version
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '+'));

    if valid {
        Ok(())
    } else {
        Err(format!(
            "Versao invalida: '{}'. Use letras, numeros, '.', '-' ou '+'.",
            version
        ))
    }
}

fn validate_entry(entry: &str) -> Result<(), String> {
    let path = Path::new(entry);
    let valid = !entry.trim().is_empty()
        && !path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        && path.extension().and_then(|ext| ext.to_str()) == Some("nx");

    if valid {
        Ok(())
    } else {
        Err(format!(
            "Entry invalido: '{}'. Use um caminho relativo .nx dentro do projeto.",
            entry
        ))
    }
}

fn ensure_unique_key(
    keys: &mut BTreeSet<String>,
    key: &str,
    line_number: usize,
) -> Result<(), String> {
    if keys.insert(key.to_string()) {
        Ok(())
    } else {
        Err(format!(
            "Chave duplicada '{}' em {} na linha {}",
            key, MANIFEST_FILE, line_number
        ))
    }
}

fn normalize_source_path(path: &str) -> String {
    path.trim().replace('\\', "/")
}

fn normalize_project_name(raw: &str) -> String {
    let mut name = String::new();
    let mut last_was_dash = false;

    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            name.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if (ch == '-' || ch == '_' || ch.is_whitespace()) && !last_was_dash {
            name.push('-');
            last_was_dash = true;
        }
    }

    let name = name.trim_matches('-').to_string();
    if name.is_empty() {
        "nexus-project".to_string()
    } else {
        name
    }
}

fn escape_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
