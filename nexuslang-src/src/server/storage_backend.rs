use std::path::Path;

use crate::ast::*;

use super::json::JsonStorage;
use super::sqlite::SqliteStorage;
use super::storage::*;

pub enum Storage {
    Json(JsonStorage),
    Sqlite(SqliteStorage),
}

impl Storage {
    pub fn new_json(data_dir: &Path) -> Self {
        Storage::Json(JsonStorage::new(data_dir))
    }

    pub fn new_sqlite(path: &Path) -> Result<Self, String> {
        Ok(Storage::Sqlite(SqliteStorage::new(path)?))
    }

    pub fn ensure_storage(&self, program: &Program) -> Result<(), String> {
        match self {
            Storage::Json(s) => s.ensure_storage(program),
            Storage::Sqlite(s) => s.ensure_storage(program),
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
}

pub fn default_data_dir(file_path: &str) -> std::path::PathBuf {
    std::path::Path::new(file_path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(".nexus-data")
}
