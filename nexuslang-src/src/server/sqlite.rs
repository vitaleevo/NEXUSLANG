use std::path::Path;

use crate::ast::*;

use super::storage::*;
use super::storage_backend::Storage;

pub struct SqliteStorage {
    conn: rusqlite::Connection,
}

impl SqliteStorage {
    pub fn new(path: &Path) -> Result<Self, String> {
        let conn = rusqlite::Connection::open(path)
            .map_err(|e| format!("Erro ao abrir SQLite '{}': {}", path.display(), e))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000; PRAGMA foreign_keys=ON;",
        )
        .map_err(|e| e.to_string())?;
        Ok(SqliteStorage { conn })
    }

    fn table_name(model: &str) -> String {
        model.to_lowercase()
    }

    fn ensure_table(&self, model: &str) -> Result<(), String> {
        let table = Self::table_name(model);
        self.conn.execute(
            &format!("CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY AUTOINCREMENT, data TEXT NOT NULL)", table),
            [],
        ).map_err(|e| format!("Erro ao criar tabela '{}': {}", table, e))?;
        Ok(())
    }

    fn ensure_auth_table(&self) -> Result<(), String> {
        self.conn
            .execute(
                "CREATE TABLE IF NOT EXISTS nexus_auth (key TEXT PRIMARY KEY, data TEXT NOT NULL)",
                [],
            )
            .map_err(|e| format!("Erro ao criar tabela auth SQLite: {}", e))?;
        Ok(())
    }

    pub(crate) fn read_auth_store_json(&self) -> Result<Option<String>, String> {
        self.ensure_auth_table()?;
        let mut stmt = self
            .conn
            .prepare("SELECT data FROM nexus_auth WHERE key = 'auth_store'")
            .map_err(|e| format!("Erro ao ler auth SQLite: {}", e))?;
        let mut rows = stmt
            .query([])
            .map_err(|e| format!("Erro ao ler auth SQLite: {}", e))?;
        match rows
            .next()
            .map_err(|e| format!("Erro ao ler auth SQLite: {}", e))?
        {
            Some(row) => row
                .get::<_, String>(0)
                .map(Some)
                .map_err(|e| format!("Erro ao ler auth SQLite: {}", e)),
            None => Ok(None),
        }
    }

    pub(crate) fn write_auth_store_json(&self, source: &str) -> Result<(), String> {
        self.ensure_auth_table()?;
        self.conn
            .execute(
                "INSERT INTO nexus_auth (key, data) VALUES ('auth_store', ?1) \
                 ON CONFLICT(key) DO UPDATE SET data = excluded.data",
                rusqlite::params![source],
            )
            .map_err(|e| format!("Erro ao gravar auth SQLite: {}", e))?;
        Ok(())
    }

    fn read_all_records_json(&self, model: &str) -> Result<Vec<(i64, String)>, String> {
        let table = Self::table_name(model);
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT id, data FROM {} ORDER BY id", table))
            .map_err(|e| format!("Erro ao ler '{}': {}", model, e))?;
        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let data: String = row.get(1)?;
                Ok((id, data))
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    fn read_all_records_as_server_values(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
    ) -> Result<Vec<(i64, ServerValue)>, String> {
        let fields = model_fields(program, model)
            .ok_or_else(|| format!("Model '{}' nao encontrado", model))?;
        let records = self.read_all_records_json(model)?;
        let mut out = Vec::new();
        for (id, data) in records {
            let parsed = parse_json(&data).map_err(|message| {
                format!("Storage SQLite de '{}' invalido: {}", model, message)
            })?;
            let JsonValue::Object(record_fields) = parsed else {
                return Err(format!("Storage SQLite de '{}' deve conter objetos", model));
            };
            let context = format!("Storage SQLite de '{}'", model);
            let sv = model_record_from_json(storage, program, fields, record_fields, &context)?;
            out.push((id, sv));
        }
        Ok(out)
    }

    fn insert_record(&self, model: &str, data_json: &str) -> Result<i64, String> {
        let table = Self::table_name(model);
        self.conn
            .execute(
                &format!("INSERT INTO {} (data) VALUES (?1)", table),
                rusqlite::params![data_json],
            )
            .map_err(|e| format!("Erro ao inserir em '{}': {}", model, e))?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update_record(&self, model: &str, id: i64, data_json: &str) -> Result<(), String> {
        let table = Self::table_name(model);
        self.conn
            .execute(
                &format!("UPDATE {} SET data = ?1 WHERE id = ?2", table),
                rusqlite::params![data_json, id],
            )
            .map_err(|e| format!("Erro ao atualizar '{}': {}", model, e))?;
        Ok(())
    }

    fn delete_record(&self, model: &str, id: i64) -> Result<(), String> {
        let table = Self::table_name(model);
        self.conn
            .execute(
                &format!("DELETE FROM {} WHERE id = ?1", table),
                rusqlite::params![id],
            )
            .map_err(|e| format!("Erro ao deletar de '{}': {}", model, e))?;
        Ok(())
    }

    fn find_record_id_and_parse(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        field_name: &str,
        expected: &ServerValue,
    ) -> Result<Option<(i64, ServerValue)>, String> {
        let fields = model_fields(program, model)
            .ok_or_else(|| format!("Model '{}' nao encontrado", model))?;
        let _field = fields
            .iter()
            .find(|c| c.name == field_name)
            .ok_or_else(|| format!("Campo '{}.{}' nao existe", model, field_name))?;
        let records = self.read_all_records_as_server_values(storage, program, model)?;
        for (id, record) in records {
            let ServerValue::Object(ref fields_list) = record else {
                continue;
            };
            if let Some((_, stored)) = fields_list.iter().find(|(n, _)| n == field_name) {
                if server_values_equal(stored, expected) {
                    return Ok(Some((id, record)));
                }
            }
        }
        Ok(None)
    }

    pub fn ensure_storage(&self, program: &Program) -> Result<(), String> {
        for decl in &program.decls {
            if let Decl::Model { name, .. } = decl {
                self.ensure_table(name)?;
            }
        }
        Ok(())
    }

    pub fn create_model_record(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        request_body: &str,
    ) -> Result<ServerValue, String> {
        self.ensure_table(model)?;
        let fields = model_fields(program, model)
            .ok_or_else(|| format!("Model '{}' nao encontrado", model))?;
        let value = parse_json(request_body).map_err(|message| {
            format!(
                "Requisicao invalida para '{}::create()': {}",
                model, message
            )
        })?;
        let JsonValue::Object(input) = value else {
            return Err(format!(
                "Requisicao invalida para '{}::create()': corpo deve ser objeto JSON",
                model
            ));
        };
        let context = format!("Requisicao invalida para '{}::create()'", model);
        let record = model_record_from_json(storage, program, fields, input, &context)?;
        ensure_min_max_constraints(storage, program, model, fields, &record, &context)?;
        if has_unique_fields(fields) {
            let records = self.read_all_records_json(model)?;
            let json_records: Vec<JsonValue> = records
                .iter()
                .map(|(_, data)| parse_json(data).unwrap_or(JsonValue::Null))
                .collect();
            ensure_unique_constraints(
                storage,
                program,
                model,
                fields,
                &record,
                &json_records,
                None,
            )?;
        }
        let record_json = server_value_json(record.clone());
        self.insert_record(model, &record_json)?;
        self.update_unique_violation_on_insert(storage, program, model, fields, &record)?;
        Ok(record)
    }

    fn update_unique_violation_on_insert(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        fields: &[Field],
        record: &ServerValue,
    ) -> Result<(), String> {
        for field in fields.iter().filter(|f| f.unique) {
            if server_object_field(record, &field.name).is_some() {
                let idx_name = format!("idx_{}_{}", Self::table_name(model), field.name);
                let sql = format!(
                    "CREATE UNIQUE INDEX IF NOT EXISTS {} ON {}(json_extract(data, '$.{}'))",
                    idx_name,
                    Self::table_name(model),
                    field.name
                );
                if self.conn.execute(&sql, []).is_err() {
                    let records = self.read_all_records_json(model)?;
                    let json_records: Vec<JsonValue> = records
                        .iter()
                        .map(|(_, data)| parse_json(data).unwrap_or(JsonValue::Null))
                        .collect();
                    ensure_unique_constraints(
                        storage,
                        program,
                        model,
                        fields,
                        record,
                        &json_records,
                        None,
                    )?;
                }
            }
        }
        Ok(())
    }

    pub fn find_model_record(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        field_name: &str,
        expected: &ServerValue,
    ) -> Result<ServerValue, String> {
        let fields = model_fields(program, model)
            .ok_or_else(|| format!("Model '{}' nao encontrado", model))?;
        let _field = fields
            .iter()
            .find(|c| c.name == field_name)
            .ok_or_else(|| format!("Campo '{}.{}' nao existe", model, field_name))?;
        let result =
            self.find_record_id_and_parse(storage, program, model, field_name, expected)?;
        match result {
            Some((_, record)) => Ok(record),
            None => Err(format!(
                "Nao encontrado: {} com {} = {}",
                model,
                field_name,
                expected.display()
            )),
        }
    }

    pub fn update_model_record(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        field_name: &str,
        expected: &ServerValue,
        request_body: &str,
    ) -> Result<ServerValue, String> {
        let fields = model_fields(program, model)
            .ok_or_else(|| format!("Model '{}' nao encontrado", model))?;
        let _field = fields
            .iter()
            .find(|c| c.name == field_name)
            .ok_or_else(|| format!("Campo '{}.{}' nao existe", model, field_name))?;
        let value = parse_json(request_body).map_err(|message| {
            format!(
                "Requisicao invalida para '{}::update()': {}",
                model, message
            )
        })?;
        let JsonValue::Object(input) = value else {
            return Err(format!(
                "Requisicao invalida para '{}::update()': corpo deve ser objeto JSON",
                model
            ));
        };
        let context = format!("Requisicao invalida para '{}::update()'", model);
        let updated = model_record_from_json(storage, program, fields, input, &context)?;
        ensure_min_max_constraints(storage, program, model, fields, &updated, &context)?;
        let existing =
            self.find_record_id_and_parse(storage, program, model, field_name, expected)?;
        let (id, _old_record) = existing.ok_or_else(|| {
            format!(
                "Nao encontrado: {} com {} = {}",
                model,
                field_name,
                expected.display()
            )
        })?;
        let records = self.read_all_records_json(model)?;
        let json_records: Vec<JsonValue> = records
            .iter()
            .filter(|(record_id, _)| *record_id != id)
            .map(|(_, data)| parse_json(data).unwrap_or(JsonValue::Null))
            .collect();
        ensure_unique_constraints(
            storage,
            program,
            model,
            fields,
            &updated,
            &json_records,
            None,
        )?;
        let updated_json = server_value_json(updated.clone());
        self.update_record(model, id, &updated_json)?;
        Ok(updated)
    }

    pub fn delete_model_record(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        field_name: &str,
        expected: &ServerValue,
    ) -> Result<ServerValue, String> {
        let fields = model_fields(program, model)
            .ok_or_else(|| format!("Model '{}' nao encontrado", model))?;
        let _field = fields
            .iter()
            .find(|c| c.name == field_name)
            .ok_or_else(|| format!("Campo '{}.{}' nao existe", model, field_name))?;
        let existing =
            self.find_record_id_and_parse(storage, program, model, field_name, expected)?;
        let (id, record) = existing.ok_or_else(|| {
            format!(
                "Nao encontrado: {} com {} = {}",
                model,
                field_name,
                expected.display()
            )
        })?;
        self.delete_record(model, id)?;
        Ok(record)
    }

    fn model_records_from_storage(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
    ) -> Result<Vec<ServerValue>, String> {
        let records = self.read_all_records_as_server_values(storage, program, model)?;
        Ok(records.into_iter().map(|(_, sv)| sv).collect())
    }

    pub fn list_model_records(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        let out = self.model_records_from_storage(storage, program, model)?;
        let out = apply_ordering(out, ordering.as_ref())?;
        Ok(ServerValue::Array(apply_pagination(out, pagination)))
    }

    pub fn list_model_records_page(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        let out = self.model_records_from_storage(storage, program, model)?;
        paginated_list_response(out, ordering, pagination)
    }

    pub fn read_model_raw_json(&self, model: &str) -> Result<String, String> {
        let records = self.read_all_records_json(model)?;
        let items: Vec<String> = records.into_iter().map(|(_, data)| data).collect();
        Ok(format!("[{}]", items.join(",")))
    }

    pub fn paginated_array_response(
        &self,
        value: ServerValue,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        let ServerValue::Array(items) = value else {
            return Err("Resposta paginada esperava array de registros".to_string());
        };
        let total = items.len();
        let items = apply_pagination(items, pagination);
        Ok(ServerValue::Object(vec![
            ("total".to_string(), ServerValue::Number(total as f64)),
            ("items".to_string(), ServerValue::Array(items)),
        ]))
    }

    fn filter_equal(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        field_name: &str,
        expected: &ServerValue,
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
        negate: bool,
    ) -> Result<ServerValue, String> {
        let records = self.read_all_records_as_server_values(storage, program, model)?;
        let fields = model_fields(program, model)
            .ok_or_else(|| format!("Model '{}' nao encontrado", model))?;
        let _field = fields
            .iter()
            .find(|c| c.name == field_name)
            .ok_or_else(|| format!("Campo '{}.{}' nao existe", model, field_name))?;
        let mut matches = Vec::new();
        for (_, record) in records {
            if let Some(stored) = server_object_field(&record, field_name) {
                let eq = server_values_equal(stored, expected);
                if if negate { !eq } else { eq } {
                    matches.push(record);
                }
            }
        }
        let matches = apply_ordering(matches, ordering.as_ref())?;
        Ok(ServerValue::Array(apply_pagination(matches, pagination)))
    }

    pub fn filter_model_records(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        field_name: &str,
        expected: &ServerValue,
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        self.filter_equal(
            storage, program, model, field_name, expected, ordering, pagination, false,
        )
    }

    pub fn filter_model_records_page(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        field_name: &str,
        expected: &ServerValue,
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        let all = self.filter_equal(
            storage,
            program,
            model,
            field_name,
            expected,
            ordering.clone(),
            None,
            false,
        )?;
        let ServerValue::Array(items) = all else {
            return Err("Resposta paginada esperava array".to_string());
        };
        paginated_list_response(items, ordering, pagination)
    }

    pub fn filter_model_records_not(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        field_name: &str,
        expected: &ServerValue,
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        self.filter_equal(
            storage, program, model, field_name, expected, ordering, pagination, true,
        )
    }

    pub fn filter_model_records_by_in(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        field_name: &str,
        values: &[ServerValue],
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        if values.is_empty() {
            return Ok(ServerValue::Array(Vec::new()));
        }
        let records = self.read_all_records_as_server_values(storage, program, model)?;
        let mut matches = Vec::new();
        for (_, record) in records {
            if let Some(stored) = server_object_field(&record, field_name) {
                if values.iter().any(|v| server_values_equal(stored, v)) {
                    matches.push(record);
                }
            }
        }
        let matches = apply_ordering(matches, ordering.as_ref())?;
        Ok(ServerValue::Array(apply_pagination(matches, pagination)))
    }

    pub fn filter_model_records_by_not_in(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        field_name: &str,
        values: &[ServerValue],
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        let records = self.read_all_records_as_server_values(storage, program, model)?;
        let mut matches = Vec::new();
        for (_, record) in records {
            if let Some(stored) = server_object_field(&record, field_name) {
                if !values.iter().any(|v| server_values_equal(stored, v)) {
                    matches.push(record);
                }
            } else {
                matches.push(record);
            }
        }
        let matches = apply_ordering(matches, ordering.as_ref())?;
        Ok(ServerValue::Array(apply_pagination(matches, pagination)))
    }

    fn filter_records_in_memory(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        filters: &[ModelFilter],
        any_match: bool,
    ) -> Result<Vec<ServerValue>, String> {
        let fields = model_fields(program, model)
            .ok_or_else(|| format!("Model '{}' nao encontrado", model))?;
        let records = self.read_all_records_as_server_values(storage, program, model)?;
        let mut matches = Vec::new();
        for (_, record) in records {
            let ServerValue::Object(ref fields_list) = record else {
                continue;
            };
            let matched = if any_match {
                filters
                    .iter()
                    .any(|filter| record_sv_matches_filter(fields, fields_list, filter))
            } else {
                filters
                    .iter()
                    .all(|filter| record_sv_matches_filter(fields, fields_list, filter))
            };
            if matched {
                matches.push(record);
            }
        }
        Ok(matches)
    }

    pub fn filter_model_records_by_filters(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        filters: &[ModelFilter],
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        let matches = self.filter_records_in_memory(storage, program, model, filters, false)?;
        let matches = apply_ordering(matches, ordering.as_ref())?;
        Ok(ServerValue::Array(apply_pagination(matches, pagination)))
    }

    pub fn filter_model_records_by_any_filters(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        filters: &[ModelFilter],
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        let matches = self.filter_records_in_memory(storage, program, model, filters, true)?;
        let matches = apply_ordering(matches, ordering.as_ref())?;
        Ok(ServerValue::Array(apply_pagination(matches, pagination)))
    }

    pub fn filter_model_records_by_comparison(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        field_name: &str,
        operator: CompareOperator,
        expected: &ServerValue,
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        let fields = model_fields(program, model)
            .ok_or_else(|| format!("Model '{}' nao encontrado", model))?;
        let _field = fields
            .iter()
            .find(|c| c.name == field_name)
            .ok_or_else(|| format!("Campo '{}.{}' nao existe", model, field_name))?;
        let records = self.read_all_records_as_server_values(storage, program, model)?;
        let mut matches = Vec::new();
        for (_, record) in records {
            if let Some(stored) = server_object_field(&record, field_name) {
                if server_values_compare(stored, operator, expected) {
                    matches.push(record);
                }
            }
        }
        let matches = apply_ordering(matches, ordering.as_ref())?;
        Ok(ServerValue::Array(apply_pagination(matches, pagination)))
    }

    pub fn filter_model_records_by_text(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        field_name: &str,
        operator: TextOperator,
        pattern: &ServerValue,
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        let fields = model_fields(program, model)
            .ok_or_else(|| format!("Model '{}' nao encontrado", model))?;
        let _field = fields
            .iter()
            .find(|c| c.name == field_name)
            .ok_or_else(|| format!("Campo '{}.{}' nao existe", model, field_name))?;
        let records = self.read_all_records_as_server_values(storage, program, model)?;
        let mut matches = Vec::new();
        for (_, record) in records {
            if let Some(stored) = server_object_field(&record, field_name) {
                if server_values_text_match(stored, operator, pattern) {
                    matches.push(record);
                }
            }
        }
        let matches = apply_ordering(matches, ordering.as_ref())?;
        Ok(ServerValue::Array(apply_pagination(matches, pagination)))
    }

    pub fn filter_model_records_by_range(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        field_name: &str,
        min: &ServerValue,
        max: &ServerValue,
        ordering: Option<ListOrdering>,
        pagination: Option<Pagination>,
    ) -> Result<ServerValue, String> {
        let fields = model_fields(program, model)
            .ok_or_else(|| format!("Model '{}' nao encontrado", model))?;
        let _field = fields
            .iter()
            .find(|c| c.name == field_name)
            .ok_or_else(|| format!("Campo '{}.{}' nao existe", model, field_name))?;
        let records = self.read_all_records_as_server_values(storage, program, model)?;
        let mut matches = Vec::new();
        for (_, record) in records {
            if let Some(stored) = server_object_field(&record, field_name) {
                if server_values_between(stored, min, max) {
                    matches.push(record);
                }
            }
        }
        let matches = apply_ordering(matches, ordering.as_ref())?;
        Ok(ServerValue::Array(apply_pagination(matches, pagination)))
    }
}

fn record_sv_matches_filter(
    fields: &[Field],
    record_fields: &[(String, ServerValue)],
    filter: &ModelFilter,
) -> bool {
    let _field = match fields.iter().find(|c| c.name == filter.field) {
        Some(f) => f,
        None => return false,
    };
    let Some((_, stored)) = record_fields.iter().find(|(n, _)| n == &filter.field) else {
        return false;
    };
    server_values_equal(stored, &filter.expected)
}
