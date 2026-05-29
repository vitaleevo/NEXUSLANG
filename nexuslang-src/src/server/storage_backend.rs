use std::fmt;
use std::path::{Path, PathBuf};

use crate::ast::*;

use super::json::JsonStorage;
use super::sqlite::SqliteStorage;
use super::storage::*;

pub const NEXUS_DATA_DIR_ENV: &str = "NEXUS_DATA_DIR";

pub enum Storage {
    Json(JsonStorage),
    Sqlite(SqliteStorage),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageDriver {
    Json,
    Sqlite,
}

impl StorageDriver {
    pub const DEFAULT: StorageDriver = StorageDriver::Json;

    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "json" => Some(StorageDriver::Json),
            "sqlite" | "sqlite3" => Some(StorageDriver::Sqlite),
            _ => None,
        }
    }

    pub fn parse(name: &str) -> Result<Self, String> {
        Self::from_name(name).ok_or_else(|| {
            format!(
                "Storage driver '{}' nao suportado. Drivers disponiveis: {}",
                name,
                Self::available_names()
            )
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            StorageDriver::Json => "json",
            StorageDriver::Sqlite => "sqlite",
        }
    }

    pub fn available_names() -> &'static str {
        "json, sqlite"
    }

    pub fn target_path(self, data_dir: &Path) -> PathBuf {
        match self {
            StorageDriver::Json => data_dir.to_path_buf(),
            StorageDriver::Sqlite => data_dir.join("nexus.db"),
        }
    }
}

impl fmt::Display for StorageDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageMigrationPlan {
    pub driver: StorageDriver,
    pub target_path: PathBuf,
    pub actions: Vec<StorageMigrationAction>,
    pub blockers: Vec<StorageMigrationBlocker>,
}

impl StorageMigrationPlan {
    pub fn new(driver: StorageDriver, target_path: PathBuf) -> Self {
        Self {
            driver,
            target_path,
            actions: Vec::new(),
            blockers: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty() && self.blockers.is_empty()
    }

    pub fn has_blockers(&self) -> bool {
        !self.blockers.is_empty()
    }

    pub fn blocker_summary(&self) -> String {
        if self.blockers.is_empty() {
            return "sem bloqueios".to_string();
        }
        self.blockers
            .iter()
            .map(|blocker| format!("{}: {}", blocker.resource, blocker.reason))
            .collect::<Vec<_>>()
            .join("; ")
    }

    pub fn render_text(&self, applied: bool) -> String {
        let mut out = String::new();
        out.push_str(&format!("Storage migration plan ({})\n", self.driver));
        out.push_str(&format!("Target: {}\n", self.target_path.display()));
        out.push_str(&format!(
            "Mode: {}\n",
            if applied { "applied" } else { "dry-run" }
        ));
        out.push_str(&format!(
            "Status: {} action(s), {} blocker(s)\n",
            self.actions.len(),
            self.blockers.len()
        ));
        if self.actions.is_empty() {
            out.push_str("Actions: none\n");
        } else {
            out.push_str("Actions:\n");
            for action in &self.actions {
                out.push_str(&format!("  - {}\n", action.summary()));
            }
        }
        if self.blockers.is_empty() {
            out.push_str("Blockers: none\n");
        } else {
            out.push_str("Blockers:\n");
            for blocker in &self.blockers {
                out.push_str(&format!("  - {}: {}\n", blocker.resource, blocker.reason));
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageMigrationAction {
    CreateSqliteMigrationLedger {
        table: String,
    },
    CreateSqliteModelTable {
        model: String,
        table: String,
    },
    CreateSqliteAuthTable {
        table: String,
    },
    CreateSqliteUniqueIndex {
        model: String,
        table: String,
        field: String,
        index: String,
    },
    CreateSqliteIndex {
        model: String,
        table: String,
        field: String,
        index: String,
    },
}

impl StorageMigrationAction {
    pub fn summary(&self) -> String {
        match self {
            StorageMigrationAction::CreateSqliteMigrationLedger { table } => {
                format!("create SQLite migration ledger table '{}'", table)
            }
            StorageMigrationAction::CreateSqliteModelTable { model, table } => {
                format!("create SQLite model table '{}' for {}", table, model)
            }
            StorageMigrationAction::CreateSqliteAuthTable { table } => {
                format!("create SQLite auth table '{}'", table)
            }
            StorageMigrationAction::CreateSqliteUniqueIndex {
                model,
                field,
                index,
                ..
            } => format!(
                "create SQLite unique index '{}' for {}.{}",
                index, model, field
            ),
            StorageMigrationAction::CreateSqliteIndex {
                model,
                field,
                index,
                ..
            } => format!("create SQLite index '{}' for {}.{}", index, model, field),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageMigrationBlocker {
    pub resource: String,
    pub reason: String,
}

impl StorageMigrationBlocker {
    pub fn new(resource: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            resource: resource.into(),
            reason: reason.into(),
        }
    }
}

impl Storage {
    pub fn new_json(data_dir: &Path) -> Self {
        Storage::Json(JsonStorage::new(data_dir))
    }

    pub fn new_sqlite(path: &Path) -> Result<Self, String> {
        Ok(Storage::Sqlite(SqliteStorage::new(path)?))
    }

    pub fn new_driver(driver: StorageDriver, data_dir: &Path) -> Result<Self, String> {
        match driver {
            StorageDriver::Json => Ok(Self::new_json(data_dir)),
            StorageDriver::Sqlite => {
                std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
                Self::new_sqlite(&driver.target_path(data_dir))
            }
        }
    }

    pub fn driver(&self) -> StorageDriver {
        match self {
            Storage::Json(_) => StorageDriver::Json,
            Storage::Sqlite(_) => StorageDriver::Sqlite,
        }
    }

    pub fn ensure_storage(&self, program: &Program) -> Result<(), String> {
        match self {
            Storage::Json(s) => s.ensure_storage(program),
            Storage::Sqlite(s) => s.ensure_storage(program),
        }
    }

    pub fn schema_migration_plan(&self, program: &Program) -> Result<StorageMigrationPlan, String> {
        match self {
            Storage::Json(_) => Ok(StorageMigrationPlan::new(
                StorageDriver::Json,
                PathBuf::from("<json-storage>"),
            )),
            Storage::Sqlite(s) => s.schema_migration_plan(program),
        }
    }

    pub fn apply_schema_migration_plan(
        &self,
        program: &Program,
    ) -> Result<StorageMigrationPlan, String> {
        match self {
            Storage::Json(_) => Ok(StorageMigrationPlan::new(
                StorageDriver::Json,
                PathBuf::from("<json-storage>"),
            )),
            Storage::Sqlite(s) => s.apply_schema_migration_plan(program),
        }
    }

    pub fn schema_migration_plan_for_driver(
        driver: StorageDriver,
        data_dir: &Path,
        program: &Program,
    ) -> Result<StorageMigrationPlan, String> {
        match driver {
            StorageDriver::Json => Ok(StorageMigrationPlan::new(
                StorageDriver::Json,
                driver.target_path(data_dir),
            )),
            StorageDriver::Sqlite => {
                let target_path = driver.target_path(data_dir);
                SqliteStorage::schema_migration_plan_for_path(&target_path, program)
            }
        }
    }

    pub fn create_model_record(
        &self,
        program: &Program,
        model: &str,
        request_body: &str,
    ) -> Result<ServerValue, String> {
        match self {
            Storage::Json(s) => s.create_model_record(self, program, model, request_body),
            Storage::Sqlite(s) => s.create_model_record(self, program, model, request_body),
        }
    }

    pub fn find_model_record(
        &self,
        program: &Program,
        model: &str,
        field_name: &str,
        expected: &ServerValue,
    ) -> Result<ServerValue, String> {
        match self {
            Storage::Json(s) => s.find_model_record(self, program, model, field_name, expected),
            Storage::Sqlite(s) => s.find_model_record(self, program, model, field_name, expected),
        }
    }

    pub fn update_model_record(
        &self,
        program: &Program,
        model: &str,
        field_name: &str,
        expected: &ServerValue,
        request_body: &str,
    ) -> Result<ServerValue, String> {
        match self {
            Storage::Json(s) => {
                s.update_model_record(self, program, model, field_name, expected, request_body)
            }
            Storage::Sqlite(s) => {
                s.update_model_record(self, program, model, field_name, expected, request_body)
            }
        }
    }

    pub fn delete_model_record(
        &self,
        program: &Program,
        model: &str,
        field_name: &str,
        expected: &ServerValue,
    ) -> Result<ServerValue, String> {
        match self {
            Storage::Json(s) => s.delete_model_record(self, program, model, field_name, expected),
            Storage::Sqlite(s) => s.delete_model_record(self, program, model, field_name, expected),
        }
    }

    pub fn list_model_records(
        &self,
        program: &Program,
        model: &str,
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        match self {
            Storage::Json(s) => s.list_model_records(self, program, model, ordering, pagination),
            Storage::Sqlite(s) => s.list_model_records(self, program, model, ordering, pagination),
        }
    }

    pub fn list_model_records_page(
        &self,
        program: &Program,
        model: &str,
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        match self {
            Storage::Json(s) => {
                s.list_model_records_page(self, program, model, ordering, pagination)
            }
            Storage::Sqlite(s) => {
                s.list_model_records_page(self, program, model, ordering, pagination)
            }
        }
    }

    pub fn filter_model_records(
        &self,
        program: &Program,
        model: &str,
        field_name: &str,
        expected: &ServerValue,
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        match self {
            Storage::Json(s) => s.filter_model_records(
                self, program, model, field_name, expected, ordering, pagination,
            ),
            Storage::Sqlite(s) => s.filter_model_records(
                self, program, model, field_name, expected, ordering, pagination,
            ),
        }
    }

    pub fn filter_model_records_page(
        &self,
        program: &Program,
        model: &str,
        field_name: &str,
        expected: &ServerValue,
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        match self {
            Storage::Json(s) => s.filter_model_records_page(
                self, program, model, field_name, expected, ordering, pagination,
            ),
            Storage::Sqlite(s) => s.filter_model_records_page(
                self, program, model, field_name, expected, ordering, pagination,
            ),
        }
    }

    pub fn filter_model_records_not(
        &self,
        program: &Program,
        model: &str,
        field_name: &str,
        expected: &ServerValue,
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        match self {
            Storage::Json(s) => s.filter_model_records_not(
                self, program, model, field_name, expected, ordering, pagination,
            ),
            Storage::Sqlite(s) => s.filter_model_records_not(
                self, program, model, field_name, expected, ordering, pagination,
            ),
        }
    }

    pub fn filter_model_records_by_in(
        &self,
        program: &Program,
        model: &str,
        field_name: &str,
        values: &[ServerValue],
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        match self {
            Storage::Json(s) => s.filter_model_records_by_in(
                self, program, model, field_name, values, ordering, pagination,
            ),
            Storage::Sqlite(s) => s.filter_model_records_by_in(
                self, program, model, field_name, values, ordering, pagination,
            ),
        }
    }

    pub fn filter_model_records_by_not_in(
        &self,
        program: &Program,
        model: &str,
        field_name: &str,
        values: &[ServerValue],
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        match self {
            Storage::Json(s) => s.filter_model_records_by_not_in(
                self, program, model, field_name, values, ordering, pagination,
            ),
            Storage::Sqlite(s) => s.filter_model_records_by_not_in(
                self, program, model, field_name, values, ordering, pagination,
            ),
        }
    }

    pub fn filter_model_records_by_filters(
        &self,
        program: &Program,
        model: &str,
        filters: &[ModelFilter],
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        match self {
            Storage::Json(s) => s.filter_model_records_by_filters(
                self, program, model, filters, ordering, pagination,
            ),
            Storage::Sqlite(s) => s.filter_model_records_by_filters(
                self, program, model, filters, ordering, pagination,
            ),
        }
    }

    pub fn filter_model_records_by_any_filters(
        &self,
        program: &Program,
        model: &str,
        filters: &[ModelFilter],
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        match self {
            Storage::Json(s) => s.filter_model_records_by_any_filters(
                self, program, model, filters, ordering, pagination,
            ),
            Storage::Sqlite(s) => s.filter_model_records_by_any_filters(
                self, program, model, filters, ordering, pagination,
            ),
        }
    }

    pub fn filter_model_records_by_comparison(
        &self,
        program: &Program,
        model: &str,
        field_name: &str,
        operator: CompareOperator,
        expected: &ServerValue,
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        match self {
            Storage::Json(s) => s.filter_model_records_by_comparison(
                self, program, model, field_name, operator, expected, ordering, pagination,
            ),
            Storage::Sqlite(s) => s.filter_model_records_by_comparison(
                self, program, model, field_name, operator, expected, ordering, pagination,
            ),
        }
    }

    pub fn filter_model_records_by_text(
        &self,
        program: &Program,
        model: &str,
        field_name: &str,
        operator: TextOperator,
        pattern: &ServerValue,
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        match self {
            Storage::Json(s) => s.filter_model_records_by_text(
                self, program, model, field_name, operator, pattern, ordering, pagination,
            ),
            Storage::Sqlite(s) => s.filter_model_records_by_text(
                self, program, model, field_name, operator, pattern, ordering, pagination,
            ),
        }
    }

    pub fn filter_model_records_by_range(
        &self,
        program: &Program,
        model: &str,
        field_name: &str,
        min: &ServerValue,
        max: &ServerValue,
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        match self {
            Storage::Json(s) => s.filter_model_records_by_range(
                self, program, model, field_name, min, max, ordering, pagination,
            ),
            Storage::Sqlite(s) => s.filter_model_records_by_range(
                self, program, model, field_name, min, max, ordering, pagination,
            ),
        }
    }

    pub fn read_model_raw_json(&self, model: &str) -> Result<String, String> {
        match self {
            Storage::Json(s) => s.read_model_raw_json(model),
            Storage::Sqlite(s) => s.read_model_raw_json(model),
        }
    }

    pub fn paginated_array_response(
        &self,
        value: ServerValue,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        match self {
            Storage::Json(s) => s.paginated_array_response(value, pagination),
            Storage::Sqlite(s) => s.paginated_array_response(value, pagination),
        }
    }

    pub(crate) fn read_auth_store_json(&self) -> Result<Option<String>, String> {
        match self {
            Storage::Json(s) => s.read_auth_store_json(),
            Storage::Sqlite(s) => s.read_auth_store_json(),
        }
    }

    pub(crate) fn write_auth_store_json(&self, source: &str) -> Result<(), String> {
        match self {
            Storage::Json(s) => s.write_auth_store_json(source),
            Storage::Sqlite(s) => s.write_auth_store_json(source),
        }
    }
}

pub fn default_data_dir(file_path: &str) -> std::path::PathBuf {
    if let Some(path) = crate::runtime_env::var_os(NEXUS_DATA_DIR_ENV) {
        if !path.is_empty() {
            return std::path::PathBuf::from(path);
        }
    }

    std::path::Path::new(file_path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(".nexus-data")
}
