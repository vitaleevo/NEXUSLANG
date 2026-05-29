use std::path::{Path, PathBuf};

use crate::ast::*;

use super::storage::*;
use super::storage_backend::{
    Storage, StorageDriver, StorageMigrationAction, StorageMigrationBlocker, StorageMigrationPlan,
};

pub struct SqliteStorage {
    conn: rusqlite::Connection,
    path: PathBuf,
}

#[derive(Debug)]
struct SqliteColumnInfo {
    name: String,
    column_type: String,
    not_null: bool,
    primary_key: bool,
}

impl SqliteStorage {
    pub fn new(path: &Path) -> Result<Self, String> {
        let conn = rusqlite::Connection::open(path)
            .map_err(|e| format!("Erro ao abrir SQLite '{}': {}", path.display(), e))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000; PRAGMA foreign_keys=ON;",
        )
        .map_err(|e| e.to_string())?;
        Ok(SqliteStorage {
            conn,
            path: path.to_path_buf(),
        })
    }

    pub(crate) fn schema_migration_plan_for_path(
        path: &Path,
        program: &Program,
    ) -> Result<StorageMigrationPlan, String> {
        if path.exists() {
            return Self::new_readonly(path)?.schema_migration_plan(program);
        }

        let mut plan = StorageMigrationPlan::new(StorageDriver::Sqlite, path.to_path_buf());
        if has_auth(program) {
            plan.actions
                .push(StorageMigrationAction::CreateSqliteAuthTable {
                    table: "nexus_auth".to_string(),
                });
        }
        for decl in &program.decls {
            if let Decl::Model { name, fields, .. } = decl {
                Self::push_missing_model_actions(&mut plan, name, fields);
            }
        }
        Ok(plan)
    }

    fn new_readonly(path: &Path) -> Result<Self, String> {
        let conn = rusqlite::Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| {
            format!(
                "Erro ao abrir SQLite '{}' em modo leitura: {}",
                path.display(),
                e
            )
        })?;
        conn.execute_batch("PRAGMA busy_timeout=5000; PRAGMA foreign_keys=ON;")
            .map_err(|e| e.to_string())?;
        Ok(SqliteStorage {
            conn,
            path: path.to_path_buf(),
        })
    }

    fn table_name(model: &str) -> String {
        model.to_lowercase()
    }

    fn index_name(table: &str, field: &str) -> String {
        format!("idx_{}_{}", table, field)
    }

    fn quote_identifier(identifier: &str) -> String {
        format!("\"{}\"", identifier.replace('"', "\"\""))
    }

    fn json_path(field: &str) -> String {
        format!("$.{}", field)
    }

    fn sql_string_literal(value: &str) -> String {
        format!("'{}'", value.replace('\'', "''"))
    }

    fn ensure_table(&self, model: &str) -> Result<(), String> {
        let table = Self::table_name(model);
        self.create_model_table(&table)
            .map_err(|e| format!("Erro ao criar tabela '{}': {}", table, e))
    }

    fn create_model_table(&self, table: &str) -> Result<(), String> {
        self.conn
            .execute(
                &format!(
                    "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY AUTOINCREMENT, data TEXT NOT NULL)",
                    Self::quote_identifier(table)
                ),
                [],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn ensure_auth_table(&self) -> Result<(), String> {
        self.create_auth_table()
            .map_err(|e| format!("Erro ao criar tabela auth SQLite: {}", e))
    }

    fn create_auth_table(&self) -> Result<(), String> {
        self.conn
            .execute(
                "CREATE TABLE IF NOT EXISTS \"nexus_auth\" (key TEXT PRIMARY KEY, data TEXT NOT NULL)",
                [],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn sqlite_object_exists(&self, object_type: &str, name: &str) -> Result<bool, String> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = ?1 AND name = ?2",
                rusqlite::params![object_type, name],
                |row| row.get(0),
            )
            .map_err(|e| format!("Erro ao introspectar SQLite '{}': {}", name, e))?;
        Ok(count > 0)
    }

    fn table_exists(&self, table: &str) -> Result<bool, String> {
        self.sqlite_object_exists("table", table)
    }

    fn index_is_unique(&self, table: &str, index: &str) -> Result<Option<bool>, String> {
        let mut stmt = self
            .conn
            .prepare(&format!(
                "PRAGMA index_list({})",
                Self::quote_identifier(table)
            ))
            .map_err(|e| format!("Erro ao introspectar indices SQLite '{}': {}", table, e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(1)?, row.get::<_, i64>(2)? != 0))
            })
            .map_err(|e| format!("Erro ao introspectar indices SQLite '{}': {}", table, e))?;

        for row in rows {
            let (name, unique) =
                row.map_err(|e| format!("Erro ao introspectar indice SQLite '{}': {}", table, e))?;
            if name == index {
                return Ok(Some(unique));
            }
        }
        Ok(None)
    }

    fn table_columns(&self, table: &str) -> Result<Vec<SqliteColumnInfo>, String> {
        let mut stmt = self
            .conn
            .prepare(&format!(
                "PRAGMA table_info({})",
                Self::quote_identifier(table)
            ))
            .map_err(|e| format!("Erro ao introspectar tabela SQLite '{}': {}", table, e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(SqliteColumnInfo {
                    name: row.get(1)?,
                    column_type: row.get(2)?,
                    not_null: row.get::<_, i64>(3)? != 0,
                    primary_key: row.get::<_, i64>(5)? != 0,
                })
            })
            .map_err(|e| format!("Erro ao introspectar tabela SQLite '{}': {}", table, e))?;

        let mut columns = Vec::new();
        for row in rows {
            columns.push(
                row.map_err(|e| format!("Erro ao introspectar coluna SQLite '{}': {}", table, e))?,
            );
        }
        Ok(columns)
    }

    fn expected_model_table_shape(&self, table: &str) -> Result<Option<String>, String> {
        let columns = self.table_columns(table)?;
        let id = columns.iter().find(|column| column.name == "id");
        let data = columns.iter().find(|column| column.name == "data");
        if !matches!(id, Some(column) if column.primary_key) {
            return Ok(Some(
                "esperava coluna 'id' como PRIMARY KEY interna".to_string(),
            ));
        }
        if !matches!(data, Some(column) if column.not_null && column.column_type.eq_ignore_ascii_case("TEXT"))
        {
            return Ok(Some(
                "esperava coluna 'data TEXT NOT NULL' para payload JSON".to_string(),
            ));
        }
        Ok(None)
    }

    fn expected_auth_table_shape(&self) -> Result<Option<String>, String> {
        let columns = self.table_columns("nexus_auth")?;
        let key = columns.iter().find(|column| column.name == "key");
        let data = columns.iter().find(|column| column.name == "data");
        if !matches!(key, Some(column) if column.primary_key) {
            return Ok(Some("esperava coluna 'key' como PRIMARY KEY".to_string()));
        }
        if !matches!(data, Some(column) if column.not_null && column.column_type.eq_ignore_ascii_case("TEXT"))
        {
            return Ok(Some(
                "esperava coluna 'data TEXT NOT NULL' para auth JSON".to_string(),
            ));
        }
        Ok(None)
    }

    fn unique_index_has_duplicate_values(&self, table: &str, field: &str) -> Result<bool, String> {
        let sql = format!(
            "SELECT COUNT(*) FROM (\
             SELECT json_extract(data, ?1) AS value \
             FROM {} \
             WHERE json_extract(data, ?1) IS NOT NULL \
             GROUP BY value \
             HAVING COUNT(*) > 1\
             )",
            Self::quote_identifier(table)
        );
        let duplicate_groups: i64 = self
            .conn
            .query_row(&sql, rusqlite::params![Self::json_path(field)], |row| {
                row.get(0)
            })
            .map_err(|e| {
                format!(
                    "Erro ao verificar duplicados para indice unico '{}.{}': {}",
                    table, field, e
                )
            })?;
        Ok(duplicate_groups > 0)
    }

    fn create_json_index(
        &self,
        table: &str,
        field: &str,
        index: &str,
        unique: bool,
    ) -> Result<(), String> {
        let create_index = if unique {
            "CREATE UNIQUE INDEX"
        } else {
            "CREATE INDEX"
        };
        let sql = format!(
            "{} IF NOT EXISTS {} ON {}(json_extract(data, {}))",
            create_index,
            Self::quote_identifier(index),
            Self::quote_identifier(table),
            Self::sql_string_literal(&Self::json_path(field))
        );
        self.conn.execute(&sql, []).map_err(|e| {
            format!(
                "Erro ao criar indice SQLite '{}' em '{}.{}': {}",
                index, table, field, e
            )
        })?;
        Ok(())
    }

    fn push_missing_model_actions(plan: &mut StorageMigrationPlan, model: &str, fields: &[Field]) {
        let table = Self::table_name(model);
        plan.actions
            .push(StorageMigrationAction::CreateSqliteModelTable {
                model: model.to_string(),
                table: table.clone(),
            });
        for field in fields {
            if field.unique {
                plan.actions
                    .push(StorageMigrationAction::CreateSqliteUniqueIndex {
                        model: model.to_string(),
                        table: table.clone(),
                        field: field.name.clone(),
                        index: Self::index_name(&table, &field.name),
                    });
            } else if field.index {
                plan.actions
                    .push(StorageMigrationAction::CreateSqliteIndex {
                        model: model.to_string(),
                        table: table.clone(),
                        field: field.name.clone(),
                        index: Self::index_name(&table, &field.name),
                    });
            }
        }
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
            .prepare(&format!(
                "SELECT id, data FROM {} ORDER BY id",
                Self::quote_identifier(&table)
            ))
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
                &format!(
                    "INSERT INTO {} (data) VALUES (?1)",
                    Self::quote_identifier(&table)
                ),
                rusqlite::params![data_json],
            )
            .map_err(|e| format!("Erro ao inserir em '{}': {}", model, e))?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update_record(&self, model: &str, id: i64, data_json: &str) -> Result<(), String> {
        let table = Self::table_name(model);
        self.conn
            .execute(
                &format!(
                    "UPDATE {} SET data = ?1 WHERE id = ?2",
                    Self::quote_identifier(&table)
                ),
                rusqlite::params![data_json, id],
            )
            .map_err(|e| format!("Erro ao atualizar '{}': {}", model, e))?;
        Ok(())
    }

    fn delete_record(&self, model: &str, id: i64) -> Result<(), String> {
        let table = Self::table_name(model);
        self.conn
            .execute(
                &format!(
                    "DELETE FROM {} WHERE id = ?1",
                    Self::quote_identifier(&table)
                ),
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
        self.apply_schema_migration_plan(program).map(|_| ())
    }

    pub fn schema_migration_plan(&self, program: &Program) -> Result<StorageMigrationPlan, String> {
        let mut plan = StorageMigrationPlan::new(StorageDriver::Sqlite, self.path.clone());

        if has_auth(program) {
            if self.table_exists("nexus_auth")? {
                if let Some(reason) = self.expected_auth_table_shape()? {
                    plan.blockers
                        .push(StorageMigrationBlocker::new("nexus_auth", reason));
                }
            } else {
                plan.actions
                    .push(StorageMigrationAction::CreateSqliteAuthTable {
                        table: "nexus_auth".to_string(),
                    });
            }
        }

        for decl in &program.decls {
            if let Decl::Model { name, fields, .. } = decl {
                let table = Self::table_name(name);
                let table_exists = self.table_exists(&table)?;
                if table_exists {
                    if let Some(reason) = self.expected_model_table_shape(&table)? {
                        plan.blockers
                            .push(StorageMigrationBlocker::new(table.clone(), reason));
                        continue;
                    }
                } else {
                    Self::push_missing_model_actions(&mut plan, name, fields);
                    continue;
                }

                for field in fields {
                    if field.unique {
                        let index = Self::index_name(&table, &field.name);
                        match self.index_is_unique(&table, &index)? {
                            Some(true) => {}
                            Some(false) => {
                                plan.blockers.push(StorageMigrationBlocker::new(
                                    format!("{}.{}", name, field.name),
                                    format!("indice SQLite '{}' existe, mas nao e unico", index),
                                ));
                            }
                            None => {
                                if self.unique_index_has_duplicate_values(&table, &field.name)? {
                                    plan.blockers.push(StorageMigrationBlocker::new(
                                        format!("{}.{}", name, field.name),
                                        format!(
                                            "nao e seguro criar indice unico '{}' porque existem valores duplicados",
                                            index
                                        ),
                                    ));
                                } else {
                                    plan.actions.push(
                                        StorageMigrationAction::CreateSqliteUniqueIndex {
                                            model: name.clone(),
                                            table: table.clone(),
                                            field: field.name.clone(),
                                            index,
                                        },
                                    );
                                }
                            }
                        }
                    } else if field.index {
                        let index = Self::index_name(&table, &field.name);
                        match self.index_is_unique(&table, &index)? {
                            Some(true) => {
                                plan.blockers.push(StorageMigrationBlocker::new(
                                    format!("{}.{}", name, field.name),
                                    format!(
                                        "indice SQLite '{}' existe como unico, mas o campo nao declara unique",
                                        index
                                    ),
                                ));
                            }
                            Some(false) => {}
                            None => {
                                plan.actions
                                    .push(StorageMigrationAction::CreateSqliteIndex {
                                        model: name.clone(),
                                        table: table.clone(),
                                        field: field.name.clone(),
                                        index,
                                    });
                            }
                        }
                    }
                }
            }
        }

        Ok(plan)
    }

    pub fn apply_schema_migration_plan(
        &self,
        program: &Program,
    ) -> Result<StorageMigrationPlan, String> {
        let plan = self.schema_migration_plan(program)?;
        if plan.has_blockers() {
            return Err(format!(
                "Plano de migracao SQLite bloqueado: {}",
                plan.blocker_summary()
            ));
        }

        for action in &plan.actions {
            self.apply_migration_action(action)?;
        }

        Ok(plan)
    }

    fn apply_migration_action(&self, action: &StorageMigrationAction) -> Result<(), String> {
        match action {
            StorageMigrationAction::CreateSqliteModelTable { table, .. } => {
                self.create_model_table(table)
            }
            StorageMigrationAction::CreateSqliteAuthTable { .. } => self.create_auth_table(),
            StorageMigrationAction::CreateSqliteUniqueIndex {
                table,
                field,
                index,
                ..
            } => self.create_json_index(table, field, index, true),
            StorageMigrationAction::CreateSqliteIndex {
                table,
                field,
                index,
                ..
            } => self.create_json_index(table, field, index, false),
        }
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
                let table = Self::table_name(model);
                let idx_name = Self::index_name(&table, &field.name);
                if self
                    .create_json_index(&table, &field.name, &idx_name, true)
                    .is_err()
                {
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
