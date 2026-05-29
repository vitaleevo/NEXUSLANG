use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

#[cfg(not(target_arch = "wasm32"))]
use std::io::{Read, Write};
#[cfg(not(target_arch = "wasm32"))]
use std::net::{TcpStream, ToSocketAddrs};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

pub const MANIFEST_FILE: &str = "nexus.toml";
pub const LOCK_FILE: &str = "nexus.lock";
pub const REGISTRY_ENV: &str = "NEXUS_REGISTRY_URL";
const REGISTRY_METADATA_FILE: &str = "nexus-package.toml";
#[cfg(not(target_arch = "wasm32"))]
const REGISTRY_HTTP_CONNECT_TIMEOUT_SECS: u64 = 5;
#[cfg(not(target_arch = "wasm32"))]
const REGISTRY_HTTP_IO_TIMEOUT_SECS: u64 = 15;

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

#[derive(Debug, Clone)]
pub struct ProjectManifest {
    pub root: PathBuf,
    pub name: String,
    pub version: String,
    pub entry: String,
    pub dependencies: BTreeMap<String, ProjectDependencySource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectDependencySource {
    Local,
    Path(PathBuf),
    Registry {
        package: String,
        version: String,
        cache_path: PathBuf,
    },
}

impl ProjectManifest {
    pub fn entry_path(&self) -> PathBuf {
        self.root.join(&self.entry)
    }

    pub fn dependency(&self, name: &str) -> Option<&ProjectDependencySource> {
        self.dependencies.get(name)
    }
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
    sync_local_packages(&root, &manifest)?;
    write_lockfile(&root, &manifest)?;

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
    sync_local_packages(&root, &manifest)?;
    write_lockfile(&root, &manifest)?;

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
    sync_local_packages(&root, &manifest)?;
    write_lockfile(&root, &manifest)?;

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

pub fn find_manifest_root_from(start: &Path) -> Option<PathBuf> {
    let search_start = if start.is_file() {
        start.parent().unwrap_or(start)
    } else {
        start
    };
    find_manifest_root(search_start)
}

pub fn load_project_manifest(root: &Path) -> Result<ProjectManifest, String> {
    let manifest_path = root.join(MANIFEST_FILE);
    let source = fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
    let manifest = parse_manifest(&source)?;
    validate_manifest(root, &manifest)?;
    Ok(ProjectManifest::from_private(root, manifest))
}

pub fn load_nearest_project_manifest(start: &Path) -> Result<Option<ProjectManifest>, String> {
    let Some(root) = find_manifest_root_from(start) else {
        return Ok(None);
    };
    load_project_manifest(&root).map(Some)
}

pub fn project_entry_current_dir() -> Result<PathBuf, String> {
    let current = env::current_dir().map_err(|e| e.to_string())?;
    project_entry_from(&current)
}

pub fn project_entry_from(start: &Path) -> Result<PathBuf, String> {
    let root = find_manifest_root_from(start).ok_or_else(|| {
        format!(
            "{} nao encontrado a partir de {}",
            MANIFEST_FILE,
            start.display()
        )
    })?;
    let manifest = load_project_manifest(&root)?;
    Ok(manifest.entry_path())
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
        if let Some(checksum) = metadata.checksum {
            output.push_str(&format!("checksum = \"{}\"\n", escape_string(&checksum)));
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
        if let DependencySource::Registry(registry) = source {
            if sync_registry_package(&packages_dir, name, registry)? {
                continue;
            }
        }

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

fn sync_registry_package(
    packages_dir: &Path,
    package_name: &str,
    registry: &RegistryDependency,
) -> Result<bool, String> {
    let Some(base) = registry_base_from_env()? else {
        return Ok(false);
    };

    let metadata = load_registry_metadata(&base, registry)?;
    let archive = read_registry_resource(&base.archive_resource(registry, &metadata.archive)?)?;
    if let Some(expected) = &metadata.sha256 {
        let actual = sha256_hex(&archive);
        if &actual != expected {
            return Err(format!(
                "Checksum invalido para registry package '{}': esperado {}, obtido {}",
                package_name, expected, actual
            ));
        }
    }

    let package_dir = packages_dir.join(package_name);
    if package_dir.exists() {
        validate_cache_child(packages_dir, &package_dir)?;
        fs::remove_dir_all(&package_dir).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&package_dir).map_err(|e| e.to_string())?;
    extract_tar_archive(&archive, &package_dir)?;
    validate_cached_registry_package(&package_dir, package_name, registry)?;

    let checksum_line = metadata
        .sha256
        .as_ref()
        .map(|checksum| format!("checksum=sha256:{}\n", checksum))
        .unwrap_or_default();
    let marker = format!(
        "name={}\nkind=registry\nsource=registry:{}@{}\nversion={}\nregistry_url={}\n{}managed_by=nexus\n",
        package_name,
        registry.package,
        registry.version,
        registry.version,
        base.display(),
        checksum_line
    );
    fs::write(package_dir.join("PACKAGE.txt"), marker).map_err(|e| e.to_string())?;
    Ok(true)
}

fn validate_cached_registry_package(
    package_dir: &Path,
    package_name: &str,
    registry: &RegistryDependency,
) -> Result<(), String> {
    let manifest_path = package_dir.join(MANIFEST_FILE);
    if !manifest_path.is_file() {
        return Err(format!(
            "Registry package '{}' precisa conter {} na raiz do archive",
            package_name, MANIFEST_FILE
        ));
    }

    let source = fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
    let manifest = parse_manifest(&source)?;
    if manifest.name != package_name {
        return Err(format!(
            "Registry package '{}' contem pacote '{}'",
            package_name, manifest.name
        ));
    }
    if manifest.version != registry.version {
        return Err(format!(
            "Registry package '{}' esta na versao '{}', esperado '{}'",
            package_name, manifest.version, registry.version
        ));
    }

    Ok(())
}

fn registry_lock_checksum(registry: &RegistryDependency) -> Result<Option<String>, String> {
    let Some(base) = registry_base_from_env()? else {
        return Ok(None);
    };
    let metadata = load_registry_metadata(&base, registry)?;
    Ok(metadata
        .sha256
        .map(|checksum| format!("sha256:{}", checksum)))
}

fn registry_resolved_path(root: &Path, package_name: &str) -> Option<String> {
    registry_base_from_env().ok().flatten().map(|_| {
        root.join(".nexus")
            .join("packages")
            .join(package_name)
            .display()
            .to_string()
    })
}

#[derive(Debug, Clone)]
enum RegistryBase {
    Local(PathBuf),
    Http(String),
}

#[derive(Debug, Clone)]
enum RegistryResource {
    Local(PathBuf),
    Http(String),
}

#[derive(Debug)]
struct RegistryPackageMetadata {
    name: String,
    version: String,
    archive: String,
    sha256: Option<String>,
}

fn registry_base_from_env() -> Result<Option<RegistryBase>, String> {
    match env::var(REGISTRY_ENV) {
        Ok(value) => {
            let value = value.trim();
            if value.is_empty() {
                Ok(None)
            } else {
                RegistryBase::parse(value).map(Some)
            }
        }
        Err(env::VarError::NotPresent) => Ok(None),
        Err(e) => Err(format!("{} invalido: {}", REGISTRY_ENV, e)),
    }
}

impl RegistryBase {
    fn parse(value: &str) -> Result<Self, String> {
        if let Some(path) = value.strip_prefix("file://") {
            if path.trim().is_empty() {
                return Err(format!("{} file:// precisa de caminho", REGISTRY_ENV));
            }
            Ok(Self::Local(PathBuf::from(path)))
        } else if value.starts_with("http://") {
            Ok(Self::Http(value.trim_end_matches('/').to_string()))
        } else if value.starts_with("https://") {
            Err(format!(
                "{} ainda nao suporta HTTPS neste MVP; use file:// ou http://",
                REGISTRY_ENV
            ))
        } else {
            Ok(Self::Local(PathBuf::from(value)))
        }
    }

    fn metadata_resource(&self, registry: &RegistryDependency) -> RegistryResource {
        match self {
            Self::Local(root) => RegistryResource::Local(
                root.join(&registry.package)
                    .join(&registry.version)
                    .join(REGISTRY_METADATA_FILE),
            ),
            Self::Http(base) => RegistryResource::Http(format!(
                "{}/{}/{}/{}",
                base, registry.package, registry.version, REGISTRY_METADATA_FILE
            )),
        }
    }

    fn archive_resource(
        &self,
        registry: &RegistryDependency,
        archive: &str,
    ) -> Result<RegistryResource, String> {
        validate_registry_archive_path(archive)?;
        Ok(match self {
            Self::Local(root) => RegistryResource::Local(
                root.join(&registry.package)
                    .join(&registry.version)
                    .join(archive),
            ),
            Self::Http(base) => RegistryResource::Http(format!(
                "{}/{}/{}/{}",
                base, registry.package, registry.version, archive
            )),
        })
    }

    fn display(&self) -> String {
        match self {
            Self::Local(path) => path.display().to_string(),
            Self::Http(url) => url.clone(),
        }
    }
}

fn load_registry_metadata(
    base: &RegistryBase,
    registry: &RegistryDependency,
) -> Result<RegistryPackageMetadata, String> {
    let source = read_registry_resource(&base.metadata_resource(registry))?;
    let source = String::from_utf8(source).map_err(|e| {
        format!(
            "{} de registry package '{}@{}' nao e UTF-8 valido: {}",
            REGISTRY_METADATA_FILE, registry.package, registry.version, e
        )
    })?;
    parse_registry_metadata(&source, registry)
}

fn parse_registry_metadata(
    source: &str,
    registry: &RegistryDependency,
) -> Result<RegistryPackageMetadata, String> {
    let mut keys = BTreeSet::new();
    let mut name = None;
    let mut version = None;
    let mut archive = None;
    let mut sha256 = None;

    for (index, raw_line) in source.lines().enumerate() {
        let line_number = index + 1;
        let line_without_comment = raw_line.split('#').next().unwrap_or("");
        let line = line_without_comment.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            return Err(format!(
                "{} nao aceita secoes; linha {} invalida",
                REGISTRY_METADATA_FILE, line_number
            ));
        }

        let (key, raw_value) = line.split_once('=').ok_or_else(|| {
            format!(
                "Linha {} invalida em {}",
                line_number, REGISTRY_METADATA_FILE
            )
        })?;
        let key = key.trim();
        ensure_unique_key(&mut keys, key, line_number)?;
        let value = parse_registry_string_value(raw_value.trim(), line_number)?;

        match key {
            "name" => {
                validate_package_name(&value)?;
                name = Some(value);
            }
            "version" => {
                validate_version(&value)?;
                version = Some(value);
            }
            "archive" => {
                validate_registry_archive_path(&value)?;
                archive = Some(value);
            }
            "sha256" => {
                validate_sha256(&value)?;
                sha256 = Some(value.to_ascii_lowercase());
            }
            _ => {
                return Err(format!(
                    "Chave '{}' desconhecida em {} na linha {}",
                    key, REGISTRY_METADATA_FILE, line_number
                ));
            }
        }
    }

    let metadata = RegistryPackageMetadata {
        name: name.ok_or_else(|| format!("{} precisa de name", REGISTRY_METADATA_FILE))?,
        version: version.ok_or_else(|| format!("{} precisa de version", REGISTRY_METADATA_FILE))?,
        archive: archive.ok_or_else(|| format!("{} precisa de archive", REGISTRY_METADATA_FILE))?,
        sha256,
    };

    if metadata.name != registry.package {
        return Err(format!(
            "Registry metadata declara pacote '{}', esperado '{}'",
            metadata.name, registry.package
        ));
    }
    if metadata.version != registry.version {
        return Err(format!(
            "Registry metadata declara versao '{}', esperado '{}'",
            metadata.version, registry.version
        ));
    }

    Ok(metadata)
}

fn parse_registry_string_value(raw: &str, line_number: usize) -> Result<String, String> {
    if !raw.starts_with('"') || !raw.ends_with('"') || raw.len() < 2 {
        return Err(format!(
            "Linha {} invalida em {}: use valores entre aspas",
            line_number, REGISTRY_METADATA_FILE
        ));
    }
    Ok(raw[1..raw.len() - 1]
        .replace("\\\"", "\"")
        .replace("\\\\", "\\"))
}

fn validate_registry_archive_path(path: &str) -> Result<(), String> {
    let archive_path = Path::new(path);
    let valid = !path.trim().is_empty()
        && !archive_path.is_absolute()
        && !archive_path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        });

    if valid {
        Ok(())
    } else {
        Err(format!(
            "Archive de registry invalido: '{}'. Use caminho relativo dentro do registry.",
            path
        ))
    }
}

fn validate_sha256(value: &str) -> Result<(), String> {
    let valid = value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        Err(format!(
            "SHA-256 invalido em {}: '{}'",
            REGISTRY_METADATA_FILE, value
        ))
    }
}

fn read_registry_resource(resource: &RegistryResource) -> Result<Vec<u8>, String> {
    match resource {
        RegistryResource::Local(path) => fs::read(path)
            .map_err(|e| format!("Falha ao ler registry resource {}: {}", path.display(), e)),
        RegistryResource::Http(url) => fetch_http_url(url),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch_http_url(url: &str) -> Result<Vec<u8>, String> {
    let without_scheme = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("Registry URL HTTP invalida: {}", url))?;
    let (authority, path) = without_scheme
        .split_once('/')
        .map(|(authority, path)| (authority, format!("/{}", path)))
        .unwrap_or((without_scheme, "/".to_string()));
    if authority.is_empty() {
        return Err(format!("Registry URL HTTP sem host: {}", url));
    }

    let address = if authority.contains(':') {
        authority.to_string()
    } else {
        format!("{}:80", authority)
    };
    let resolved_addresses = address
        .to_socket_addrs()
        .map_err(|e| format!("Falha ao resolver host de registry {}: {}", url, e))?
        .collect::<Vec<_>>();
    if resolved_addresses.is_empty() {
        return Err(format!(
            "Falha ao resolver host de registry {}: sem endereco",
            url
        ));
    }
    let connect_timeout = Duration::from_secs(REGISTRY_HTTP_CONNECT_TIMEOUT_SECS);
    let io_timeout = Some(Duration::from_secs(REGISTRY_HTTP_IO_TIMEOUT_SECS));
    let mut last_error = None;
    let mut stream = None;
    for socket_addr in resolved_addresses {
        match TcpStream::connect_timeout(&socket_addr, connect_timeout) {
            Ok(candidate) => {
                stream = Some(candidate);
                break;
            }
            Err(error) => last_error = Some(error),
        }
    }
    let mut stream = stream.ok_or_else(|| {
        let error = last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "sem endereco disponivel".to_string());
        format!(
            "Falha ao conectar {} em ate {}s: {}",
            url, REGISTRY_HTTP_CONNECT_TIMEOUT_SECS, error
        )
    })?;
    stream
        .set_read_timeout(io_timeout)
        .map_err(|e| format!("Falha ao configurar timeout de leitura para {}: {}", url, e))?;
    stream
        .set_write_timeout(io_timeout)
        .map_err(|e| format!("Falha ao configurar timeout de escrita para {}: {}", url, e))?;
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: nexuslang-package-manager\r\n\r\n",
        path, authority
    );
    stream.write_all(request.as_bytes()).map_err(|e| {
        format!(
            "Falha ao requisitar {} em ate {}s: {}",
            url, REGISTRY_HTTP_IO_TIMEOUT_SECS, e
        )
    })?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response).map_err(|e| {
        format!(
            "Falha ao ler resposta de {} em ate {}s: {}",
            url, REGISTRY_HTTP_IO_TIMEOUT_SECS, e
        )
    })?;
    let header_end = find_subsequence(&response, b"\r\n\r\n")
        .ok_or_else(|| format!("Resposta HTTP invalida de {}", url))?;
    let headers = String::from_utf8_lossy(&response[..header_end]);
    let status_line = headers.lines().next().unwrap_or("");
    if !status_line.contains(" 200 ") {
        return Err(format!(
            "Registry resource {} retornou status HTTP '{}'",
            url, status_line
        ));
    }
    if headers.lines().any(|line| {
        line.to_ascii_lowercase()
            .starts_with("transfer-encoding: chunked")
    }) {
        return Err(format!(
            "Registry resource {} usa chunked encoding, ainda nao suportado neste MVP",
            url
        ));
    }

    Ok(response[header_end + 4..].to_vec())
}

#[cfg(target_arch = "wasm32")]
fn fetch_http_url(url: &str) -> Result<Vec<u8>, String> {
    Err(format!(
        "Registry HTTP nao esta disponivel no target wasm32: {}",
        url
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{:02x}", byte)).collect()
}

fn extract_tar_archive(archive: &[u8], package_dir: &Path) -> Result<(), String> {
    let mut offset = 0usize;
    while offset + 512 <= archive.len() {
        let header = &archive[offset..offset + 512];
        offset += 512;
        if header.iter().all(|byte| *byte == 0) {
            return Ok(());
        }

        let path = tar_entry_path(header)?;
        let size = parse_tar_octal(&header[124..136])?;
        let typeflag = header[156];
        let data_end = offset
            .checked_add(size)
            .ok_or_else(|| "Archive tar contem tamanho invalido".to_string())?;
        if data_end > archive.len() {
            return Err("Archive tar terminou antes do fim de uma entrada".to_string());
        }

        let Some(relative_path) = safe_tar_entry_path(&path)? else {
            offset = next_tar_offset(data_end);
            continue;
        };
        let target = package_dir.join(&relative_path);
        match typeflag {
            0 | b'0' => {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                fs::write(&target, &archive[offset..data_end]).map_err(|e| e.to_string())?;
            }
            b'5' => {
                fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            }
            b'1' | b'2' => {
                return Err(format!("Archive tar contem link nao suportado: {}", path));
            }
            other => {
                return Err(format!(
                    "Archive tar contem tipo de entrada nao suportado '{}' em {}",
                    other as char, path
                ));
            }
        }

        offset = next_tar_offset(data_end);
    }

    Err("Archive tar sem bloco final vazio".to_string())
}

fn next_tar_offset(data_end: usize) -> usize {
    let remainder = data_end % 512;
    if remainder == 0 {
        data_end
    } else {
        data_end + (512 - remainder)
    }
}

fn tar_entry_path(header: &[u8]) -> Result<String, String> {
    let name = tar_field_string(&header[0..100])?;
    let prefix = tar_field_string(&header[345..500])?;
    if prefix.is_empty() {
        Ok(name)
    } else if name.is_empty() {
        Ok(prefix)
    } else {
        Ok(format!("{}/{}", prefix, name))
    }
}

fn tar_field_string(field: &[u8]) -> Result<String, String> {
    let end = field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len());
    std::str::from_utf8(&field[..end])
        .map(|value| value.trim().to_string())
        .map_err(|e| format!("Archive tar contem nome UTF-8 invalido: {}", e))
}

fn parse_tar_octal(field: &[u8]) -> Result<usize, String> {
    let end = field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len());
    let text = std::str::from_utf8(&field[..end])
        .map_err(|e| format!("Archive tar contem tamanho invalido: {}", e))?
        .trim();
    if text.is_empty() {
        return Ok(0);
    }
    usize::from_str_radix(text, 8)
        .map_err(|_| format!("Archive tar contem tamanho octal invalido: {}", text))
}

fn safe_tar_entry_path(path: &str) -> Result<Option<PathBuf>, String> {
    let mut normalized = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("Archive tar contem caminho inseguro: {}", path));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        Ok(None)
    } else {
        Ok(Some(normalized))
    }
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
            checksum: None,
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
                checksum: None,
            })
        }
        DependencySource::Registry(registry) => Ok(LockMetadata {
            kind: "registry",
            version: registry.version.clone(),
            resolved_path: registry_resolved_path(root, package_name),
            registry_package: Some(registry.package.clone()),
            checksum: registry_lock_checksum(registry)?,
        }),
    }
}

struct LockMetadata {
    kind: &'static str,
    version: String,
    resolved_path: Option<String>,
    registry_package: Option<String>,
    checksum: Option<String>,
}

impl ProjectManifest {
    fn from_private(root: &Path, manifest: NexusManifest) -> Self {
        let dependencies = manifest
            .dependencies
            .into_iter()
            .map(|(name, source)| {
                let public_source = match source {
                    DependencySource::Local => ProjectDependencySource::Local,
                    DependencySource::Path(path) => {
                        let dependency_root = root.join(path);
                        let resolved_root =
                            dependency_root.canonicalize().unwrap_or(dependency_root);
                        ProjectDependencySource::Path(resolved_root)
                    }
                    DependencySource::Registry(registry) => ProjectDependencySource::Registry {
                        package: registry.package,
                        version: registry.version,
                        cache_path: root.join(".nexus").join("packages").join(&name),
                    },
                };
                (name, public_source)
            })
            .collect();

        Self {
            root: root.to_path_buf(),
            name: manifest.name,
            version: manifest.version,
            entry: manifest.entry,
            dependencies,
        }
    }
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
