use std::fs;
use std::path::{Path, PathBuf};

use crate::ast::*;

use super::storage::*;
use super::storage_backend::Storage;

pub struct JsonStorage {
    data_dir: PathBuf,
}

impl JsonStorage {
    pub fn new(data_dir: &Path) -> Self {
        JsonStorage {
            data_dir: data_dir.to_path_buf(),
        }
    }

    fn model_file(&self, model: &str) -> PathBuf {
        self.data_dir.join(format!("{}.json", model.to_lowercase()))
    }

    #[allow(dead_code)]
    pub(crate) fn auth_file(&self) -> PathBuf {
        self.data_dir.join(".nexus-auth.json")
    }

    pub(crate) fn read_auth_store_json(&self) -> Result<Option<String>, String> {
        let path = self.auth_file();
        if !path.exists() {
            return Ok(None);
        }
        fs::read_to_string(path)
            .map(Some)
            .map_err(|e| e.to_string())
    }

    pub(crate) fn write_auth_store_json(&self, source: &str) -> Result<(), String> {
        let path = self.auth_file();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, source).map_err(|e| e.to_string())
    }

    fn read_model_json(&self, model: &str) -> Result<String, String> {
        let path = self.model_file(model);
        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::write(&path, "[]\n").map_err(|e| e.to_string())?;
        }
        fs::read_to_string(path).map_err(|e| e.to_string())
    }

    fn read_model_records(&self, model: &str) -> Result<Vec<JsonValue>, String> {
        let storage = parse_json(&self.read_model_json(model)?)
            .map_err(|message| format!("Storage JSON de '{}' invalido: {}", model, message))?;
        let JsonValue::Array(records) = storage else {
            return Err(format!("Storage JSON de '{}' deve ser array", model));
        };
        Ok(records)
    }

    fn append_model_record(&self, model: &str, item_json: &str) -> Result<(), String> {
        let path = self.model_file(model);
        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::write(&path, "[]\n").map_err(|e| e.to_string())?;
        }
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let trimmed = content.trim();
        let new_content = if trimmed.is_empty() || trimmed == "[]" {
            format!("[{}]", item_json)
        } else if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let inner = &trimmed[1..trimmed.len() - 1];
            if inner.trim().is_empty() {
                format!("[{}]", item_json)
            } else {
                format!("[{},{}]", inner, item_json)
            }
        } else {
            return Err(format!("Storage JSON de '{}' deve ser array", model));
        };

        fs::write(path, new_content).map_err(|e| e.to_string())
    }

    fn write_all_records(&self, model: &str, records: &[String]) -> Result<(), String> {
        let path = self.model_file(model);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, format!("[{}]", records.join(","))).map_err(|e| e.to_string())
    }

    pub fn ensure_storage(&self, program: &Program) -> Result<(), String> {
        fs::create_dir_all(&self.data_dir).map_err(|e| e.to_string())?;
        for decl in &program.decls {
            if let Decl::Model { name, .. } = decl {
                let path = self.model_file(name);
                if !path.exists() {
                    fs::write(&path, "[]\n").map_err(|e| e.to_string())?;
                }
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
            let records = self.read_model_records(model)?;
            ensure_unique_constraints(storage, program, model, fields, &record, &records, None)?;
        }
        self.append_model_record(model, &server_value_json(record.clone()))?;
        Ok(record)
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
        let field = fields
            .iter()
            .find(|candidate| candidate.name == field_name)
            .ok_or_else(|| format!("Campo '{}.{}' nao existe", model, field_name))?;
        let records = self.read_model_records(model)?;

        for record in records {
            let JsonValue::Object(record_fields) = record else {
                return Err(format!("Storage JSON de '{}' deve conter objetos", model));
            };
            let Some((_, stored_value)) = record_fields
                .iter()
                .find(|(candidate, _)| candidate == field_name)
            else {
                continue;
            };
            let stored =
                json_value_to_server_value(storage, program, &field.ty, stored_value.clone())
                    .map_err(|message| {
                        format!(
                            "Storage JSON de '{}': campo '{}': {}",
                            model, field_name, message
                        )
                    })?;
            if server_values_equal(&stored, expected) {
                let context = format!("Storage JSON de '{}'", model);
                return model_record_from_json(storage, program, fields, record_fields, &context);
            }
        }

        Err(format!(
            "Nao encontrado: {} com {} = {}",
            model,
            field_name,
            expected.display()
        ))
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
        let field = fields
            .iter()
            .find(|candidate| candidate.name == field_name)
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
        let updated_json = server_value_json(updated.clone());

        let records = self.read_model_records(model)?;
        let mut out = Vec::new();
        let mut matched = false;
        let mut matched_index = None;

        for (index, record) in records.iter().enumerate() {
            let JsonValue::Object(record_fields) = record else {
                return Err(format!("Storage JSON de '{}' deve conter objetos", model));
            };
            let Some((_, stored_value)) = record_fields
                .iter()
                .find(|(candidate, _)| candidate == field_name)
            else {
                out.push(json_value_json(record));
                continue;
            };
            let stored =
                json_value_to_server_value(storage, program, &field.ty, stored_value.clone())
                    .map_err(|message| {
                        format!(
                            "Storage JSON de '{}': campo '{}': {}",
                            model, field_name, message
                        )
                    })?;
            if !matched && server_values_equal(&stored, expected) {
                out.push(updated_json.clone());
                matched = true;
                matched_index = Some(index);
            } else {
                out.push(json_value_json(record));
            }
        }

        if !matched {
            return Err(format!(
                "Nao encontrado: {} com {} = {}",
                model,
                field_name,
                expected.display()
            ));
        }

        ensure_unique_constraints(
            storage,
            program,
            model,
            fields,
            &updated,
            &records,
            matched_index,
        )?;
        self.write_all_records(model, &out)?;
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
        let field = fields
            .iter()
            .find(|candidate| candidate.name == field_name)
            .ok_or_else(|| format!("Campo '{}.{}' nao existe", model, field_name))?;
        let records = self.read_model_records(model)?;
        let mut out = Vec::new();
        let mut deleted = None;

        for record in records {
            let JsonValue::Object(record_fields) = record else {
                return Err(format!("Storage JSON de '{}' deve conter objetos", model));
            };
            let Some((_, stored_value)) = record_fields
                .iter()
                .find(|(candidate, _)| candidate == field_name)
            else {
                out.push(json_value_json(&JsonValue::Object(record_fields)));
                continue;
            };
            let stored =
                json_value_to_server_value(storage, program, &field.ty, stored_value.clone())
                    .map_err(|message| {
                        format!(
                            "Storage JSON de '{}': campo '{}': {}",
                            model, field_name, message
                        )
                    })?;
            if deleted.is_none() && server_values_equal(&stored, expected) {
                let context = format!("Storage JSON de '{}'", model);
                deleted = Some(model_record_from_json(
                    storage,
                    program,
                    fields,
                    record_fields,
                    &context,
                )?);
            } else {
                out.push(json_value_json(&JsonValue::Object(record_fields)));
            }
        }

        let Some(deleted) = deleted else {
            return Err(format!(
                "Nao encontrado: {} com {} = {}",
                model,
                field_name,
                expected.display()
            ));
        };

        self.write_all_records(model, &out)?;
        Ok(deleted)
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

    fn model_records_from_storage(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
    ) -> Result<Vec<ServerValue>, String> {
        let fields = model_fields(program, model)
            .ok_or_else(|| format!("Model '{}' nao encontrado", model))?;
        let records = self.read_model_records(model)?;
        let mut out = Vec::new();

        for record in records {
            let JsonValue::Object(record_fields) = record else {
                return Err(format!("Storage JSON de '{}' deve conter objetos", model));
            };
            let context = format!("Storage JSON de '{}'", model);
            out.push(model_record_from_json(
                storage,
                program,
                fields,
                record_fields,
                &context,
            )?);
        }

        Ok(out)
    }

    pub fn read_model_raw_json(&self, model: &str) -> Result<String, String> {
        let path = self.model_file(model);
        if !path.exists() {
            return Ok("[]".to_string());
        }
        fs::read_to_string(path).map_err(|e| e.to_string())
    }

    pub fn replace_dataset_json(
        &self,
        model_records: &[(String, Vec<String>)],
        auth_json: Option<&str>,
        replace_auth: bool,
    ) -> Result<(), String> {
        fs::create_dir_all(&self.data_dir).map_err(|e| e.to_string())?;
        for (model, records) in model_records {
            self.write_all_records(model, records)?;
        }

        if replace_auth {
            match auth_json {
                Some(source) => self.write_auth_store_json(source)?,
                None => {
                    let path = self.auth_file();
                    if path.exists() {
                        fs::remove_file(path).map_err(|e| e.to_string())?;
                    }
                }
            }
        }

        Ok(())
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
        let matches =
            self.filter_model_record_values(storage, program, model, field_name, expected)?;
        let matches = apply_ordering(matches, ordering.as_ref())?;
        Ok(ServerValue::Array(apply_pagination(matches, pagination)))
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
        let matches =
            self.filter_model_record_values(storage, program, model, field_name, expected)?;
        paginated_list_response(matches, ordering, pagination)
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
        let matches =
            self.filter_model_record_values_not(storage, program, model, field_name, expected)?;
        let matches = apply_ordering(matches, ordering.as_ref())?;
        Ok(ServerValue::Array(apply_pagination(matches, pagination)))
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
        let matches =
            self.filter_model_record_values_in(storage, program, model, field_name, values)?;
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
        let matches =
            self.filter_model_record_values_not_in(storage, program, model, field_name, values)?;
        let matches = apply_ordering(matches, ordering.as_ref())?;
        Ok(ServerValue::Array(apply_pagination(matches, pagination)))
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
        let fields = model_fields(program, model)
            .ok_or_else(|| format!("Model '{}' nao encontrado", model))?;
        let records = self.read_model_records(model)?;
        let mut matches = Vec::new();

        for record in records {
            let JsonValue::Object(record_fields) = record else {
                return Err(format!("Storage JSON de '{}' deve conter objetos", model));
            };
            let mut matched = true;
            for filter in filters {
                if !record_matches_filter(storage, program, model, fields, &record_fields, filter)?
                {
                    matched = false;
                    break;
                }
            }
            if matched {
                let context = format!("Storage JSON de '{}'", model);
                matches.push(model_record_from_json(
                    storage,
                    program,
                    fields,
                    record_fields,
                    &context,
                )?);
            }
        }

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
        let fields = model_fields(program, model)
            .ok_or_else(|| format!("Model '{}' nao encontrado", model))?;
        let records = self.read_model_records(model)?;
        let mut matches = Vec::new();

        for record in records {
            let JsonValue::Object(record_fields) = record else {
                return Err(format!("Storage JSON de '{}' deve conter objetos", model));
            };
            let mut matched = false;
            for filter in filters {
                if record_matches_filter(storage, program, model, fields, &record_fields, filter)? {
                    matched = true;
                    break;
                }
            }
            if matched {
                let context = format!("Storage JSON de '{}'", model);
                matches.push(model_record_from_json(
                    storage,
                    program,
                    fields,
                    record_fields,
                    &context,
                )?);
            }
        }

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
        let field = fields
            .iter()
            .find(|candidate| candidate.name == field_name)
            .ok_or_else(|| format!("Campo '{}.{}' nao existe", model, field_name))?;
        let records = self.read_model_records(model)?;
        let mut matches = Vec::new();

        for record in records {
            let JsonValue::Object(record_fields) = record else {
                return Err(format!("Storage JSON de '{}' deve conter objetos", model));
            };
            let Some((_, stored_value)) = record_fields
                .iter()
                .find(|(candidate, _)| candidate == field_name)
            else {
                continue;
            };
            let stored =
                json_value_to_server_value(storage, program, &field.ty, stored_value.clone())
                    .map_err(|message| {
                        format!(
                            "Storage JSON de '{}': campo '{}': {}",
                            model, field_name, message
                        )
                    })?;
            if server_values_compare(&stored, operator, expected) {
                let context = format!("Storage JSON de '{}'", model);
                matches.push(model_record_from_json(
                    storage,
                    program,
                    fields,
                    record_fields,
                    &context,
                )?);
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
        let field = fields
            .iter()
            .find(|candidate| candidate.name == field_name)
            .ok_or_else(|| format!("Campo '{}.{}' nao existe", model, field_name))?;
        let records = self.read_model_records(model)?;
        let mut matches = Vec::new();

        for record in records {
            let JsonValue::Object(record_fields) = record else {
                return Err(format!("Storage JSON de '{}' deve conter objetos", model));
            };
            let Some((_, stored_value)) = record_fields
                .iter()
                .find(|(candidate, _)| candidate == field_name)
            else {
                continue;
            };
            let stored =
                json_value_to_server_value(storage, program, &field.ty, stored_value.clone())
                    .map_err(|message| {
                        format!(
                            "Storage JSON de '{}': campo '{}': {}",
                            model, field_name, message
                        )
                    })?;
            if server_values_text_match(&stored, operator, pattern) {
                let context = format!("Storage JSON de '{}'", model);
                matches.push(model_record_from_json(
                    storage,
                    program,
                    fields,
                    record_fields,
                    &context,
                )?);
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
        let field = fields
            .iter()
            .find(|candidate| candidate.name == field_name)
            .ok_or_else(|| format!("Campo '{}.{}' nao existe", model, field_name))?;
        let records = self.read_model_records(model)?;
        let mut matches = Vec::new();

        for record in records {
            let JsonValue::Object(record_fields) = record else {
                return Err(format!("Storage JSON de '{}' deve conter objetos", model));
            };
            let Some((_, stored_value)) = record_fields
                .iter()
                .find(|(candidate, _)| candidate == field_name)
            else {
                continue;
            };
            let stored =
                json_value_to_server_value(storage, program, &field.ty, stored_value.clone())
                    .map_err(|message| {
                        format!(
                            "Storage JSON de '{}': campo '{}': {}",
                            model, field_name, message
                        )
                    })?;
            if server_values_between(&stored, min, max) {
                let context = format!("Storage JSON de '{}'", model);
                matches.push(model_record_from_json(
                    storage,
                    program,
                    fields,
                    record_fields,
                    &context,
                )?);
            }
        }

        let matches = apply_ordering(matches, ordering.as_ref())?;
        Ok(ServerValue::Array(apply_pagination(matches, pagination)))
    }

    fn filter_model_record_values(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        field_name: &str,
        expected: &ServerValue,
    ) -> Result<Vec<ServerValue>, String> {
        let fields = model_fields(program, model)
            .ok_or_else(|| format!("Model '{}' nao encontrado", model))?;
        let field = fields
            .iter()
            .find(|candidate| candidate.name == field_name)
            .ok_or_else(|| format!("Campo '{}.{}' nao existe", model, field_name))?;
        let records = self.read_model_records(model)?;
        let mut matches = Vec::new();

        for record in records {
            let JsonValue::Object(record_fields) = record else {
                return Err(format!("Storage JSON de '{}' deve conter objetos", model));
            };
            let Some((_, stored_value)) = record_fields
                .iter()
                .find(|(candidate, _)| candidate == field_name)
            else {
                continue;
            };
            let stored =
                json_value_to_server_value(storage, program, &field.ty, stored_value.clone())
                    .map_err(|message| {
                        format!(
                            "Storage JSON de '{}': campo '{}': {}",
                            model, field_name, message
                        )
                    })?;
            if server_values_equal(&stored, expected) {
                let context = format!("Storage JSON de '{}'", model);
                matches.push(model_record_from_json(
                    storage,
                    program,
                    fields,
                    record_fields,
                    &context,
                )?);
            }
        }

        Ok(matches)
    }

    fn filter_model_record_values_not(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        field_name: &str,
        expected: &ServerValue,
    ) -> Result<Vec<ServerValue>, String> {
        let fields = model_fields(program, model)
            .ok_or_else(|| format!("Model '{}' nao encontrado", model))?;
        let field = fields
            .iter()
            .find(|candidate| candidate.name == field_name)
            .ok_or_else(|| format!("Campo '{}.{}' nao existe", model, field_name))?;
        let records = self.read_model_records(model)?;
        let mut matches = Vec::new();

        for record in records {
            let JsonValue::Object(record_fields) = record else {
                return Err(format!("Storage JSON de '{}' deve conter objetos", model));
            };
            let Some((_, stored_value)) = record_fields
                .iter()
                .find(|(candidate, _)| candidate == field_name)
            else {
                continue;
            };
            let stored =
                json_value_to_server_value(storage, program, &field.ty, stored_value.clone())
                    .map_err(|message| {
                        format!(
                            "Storage JSON de '{}': campo '{}': {}",
                            model, field_name, message
                        )
                    })?;
            if !server_values_equal(&stored, expected) {
                let context = format!("Storage JSON de '{}'", model);
                matches.push(model_record_from_json(
                    storage,
                    program,
                    fields,
                    record_fields,
                    &context,
                )?);
            }
        }

        Ok(matches)
    }

    fn filter_model_record_values_in(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        field_name: &str,
        values: &[ServerValue],
    ) -> Result<Vec<ServerValue>, String> {
        if values.is_empty() {
            return Ok(Vec::new());
        }

        let fields = model_fields(program, model)
            .ok_or_else(|| format!("Model '{}' nao encontrado", model))?;
        let field = fields
            .iter()
            .find(|candidate| candidate.name == field_name)
            .ok_or_else(|| format!("Campo '{}.{}' nao existe", model, field_name))?;
        let records = self.read_model_records(model)?;
        let mut matches = Vec::new();

        for record in records {
            let JsonValue::Object(record_fields) = record else {
                return Err(format!("Storage JSON de '{}' deve conter objetos", model));
            };
            let Some((_, stored_value)) = record_fields
                .iter()
                .find(|(candidate, _)| candidate == field_name)
            else {
                continue;
            };
            let stored =
                json_value_to_server_value(storage, program, &field.ty, stored_value.clone())
                    .map_err(|message| {
                        format!(
                            "Storage JSON de '{}': campo '{}': {}",
                            model, field_name, message
                        )
                    })?;
            if values
                .iter()
                .any(|value| server_values_equal(&stored, value))
            {
                let context = format!("Storage JSON de '{}'", model);
                matches.push(model_record_from_json(
                    storage,
                    program,
                    fields,
                    record_fields,
                    &context,
                )?);
            }
        }

        Ok(matches)
    }

    fn filter_model_record_values_not_in(
        &self,
        storage: &Storage,
        program: &Program,
        model: &str,
        field_name: &str,
        values: &[ServerValue],
    ) -> Result<Vec<ServerValue>, String> {
        let fields = model_fields(program, model)
            .ok_or_else(|| format!("Model '{}' nao encontrado", model))?;
        let field = fields
            .iter()
            .find(|candidate| candidate.name == field_name)
            .ok_or_else(|| format!("Campo '{}.{}' nao existe", model, field_name))?;
        let records = self.read_model_records(model)?;
        let mut matches = Vec::new();

        for record in records {
            let JsonValue::Object(record_fields) = record else {
                return Err(format!("Storage JSON de '{}' deve conter objetos", model));
            };
            let Some((_, stored_value)) = record_fields
                .iter()
                .find(|(candidate, _)| candidate == field_name)
            else {
                continue;
            };
            let stored =
                json_value_to_server_value(storage, program, &field.ty, stored_value.clone())
                    .map_err(|message| {
                        format!(
                            "Storage JSON de '{}': campo '{}': {}",
                            model, field_name, message
                        )
                    })?;
            if !values
                .iter()
                .any(|value| server_values_equal(&stored, value))
            {
                let context = format!("Storage JSON de '{}'", model);
                matches.push(model_record_from_json(
                    storage,
                    program,
                    fields,
                    record_fields,
                    &context,
                )?);
            }
        }

        Ok(matches)
    }
}
