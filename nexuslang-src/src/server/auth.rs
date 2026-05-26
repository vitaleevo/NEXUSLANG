use crate::ast::*;

use super::storage::*;
use super::storage_backend::Storage;

pub(crate) const SESSION_COOKIE: &str = "__Host-nexus_session";

#[derive(Debug, Clone)]
pub(crate) struct AuthenticatedUser {
    pub auth: String,
    pub identity: String,
    pub role: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct AuthRouteResponse {
    pub status: u16,
    pub body: String,
    pub headers: Vec<(String, String)>,
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
    use argon2::{Algorithm, Argon2, Params, Version};
    use rand_core::{OsRng, RngCore};
    use sha2::{Digest, Sha256};

    use super::*;

    #[derive(Debug, Clone)]
    struct AuthUser {
        auth: String,
        identity: String,
        password_hash: String,
        role: Option<String>,
        created_at: u64,
    }

    #[derive(Debug, Clone)]
    struct AuthSession {
        auth: String,
        identity: String,
        session_hash: String,
        role: Option<String>,
        created_at: u64,
        last_seen_at: u64,
        expires_at: u64,
    }

    #[derive(Debug, Clone)]
    struct AuthToken {
        auth: String,
        identity: String,
        token_hash: String,
        role: Option<String>,
        created_at: u64,
        expires_at: u64,
    }

    #[derive(Debug, Clone, Default)]
    struct AuthStore {
        users: Vec<AuthUser>,
        sessions: Vec<AuthSession>,
        tokens: Vec<AuthToken>,
    }

    pub(crate) fn authenticate_request(
        program: &Program,
        storage: &Storage,
        guard: &RouteAuthGuard,
        headers: &[(String, String)],
    ) -> Result<AuthenticatedUser, String> {
        let config = auth_config(program, &guard.auth)
            .ok_or_else(|| format!("Nao autorizado: auth '{}' nao encontrado", guard.auth))?;
        let now = unix_now()?;
        let mut store = read_store(storage)?;
        store.prune_expired(now);

        let user = if let Some(session) = session_token(headers) {
            let session_hash = token_hash(&session);
            let idle_ttl = config.idle_ttl_minutes * 60;
            let found = store.sessions.iter_mut().find(|candidate| {
                candidate.auth == config.name && candidate.session_hash == session_hash
            });
            let Some(session_record) = found else {
                write_store(storage, &store)?;
                return Err("Nao autorizado: sessao invalida".to_string());
            };
            if now.saturating_sub(session_record.last_seen_at) > idle_ttl {
                store.sessions.retain(|candidate| {
                    !(candidate.auth == config.name && candidate.session_hash == session_hash)
                });
                write_store(storage, &store)?;
                return Err("Nao autorizado: sessao expirada".to_string());
            }
            session_record.last_seen_at = now;
            let user = AuthenticatedUser {
                auth: session_record.auth.clone(),
                identity: session_record.identity.clone(),
                role: session_record.role.clone(),
            };
            write_store(storage, &store)?;
            user
        } else if let Some(token) = bearer_token(headers) {
            let token_hash = token_hash(&token);
            let Some(token_record) = store.tokens.iter().find(|candidate| {
                candidate.auth == config.name && candidate.token_hash == token_hash
            }) else {
                write_store(storage, &store)?;
                return Err("Nao autorizado: token invalido".to_string());
            };
            let user = AuthenticatedUser {
                auth: token_record.auth.clone(),
                identity: token_record.identity.clone(),
                role: token_record.role.clone(),
            };
            write_store(storage, &store)?;
            user
        } else {
            write_store(storage, &store)?;
            return Err("Nao autorizado: auth requerido".to_string());
        };

        if let Some(required_role) = &guard.role {
            if user.role.as_deref() != Some(required_role.as_str()) {
                return Err("Proibido: role insuficiente".to_string());
            }
        }

        Ok(user)
    }

    pub(crate) fn eval_auth_return(
        expr: &Expr,
        program: &Program,
        storage: &Storage,
        headers: &[(String, String)],
        request_body: &str,
        current_user: Option<&AuthenticatedUser>,
    ) -> Option<Result<AuthRouteResponse, String>> {
        let Expr::StaticCall {
            ty, method, args, ..
        } = expr
        else {
            return None;
        };
        if ty != "Auth" {
            return None;
        }

        Some(match method.as_str() {
            "register" => register(program, storage, args, request_body),
            "login" => login(program, storage, args, request_body),
            "logout" => logout(storage, headers, current_user),
            "user" => current_user_response(program, storage, current_user),
            _ => Err(format!("Metodo estatico 'Auth::{}' nao existe", method)),
        })
    }

    fn register(
        program: &Program,
        storage: &Storage,
        args: &[Expr],
        request_body: &str,
    ) -> Result<AuthRouteResponse, String> {
        let config = auth_config_arg(program, "register", args)?;
        let mut fields = request_object(request_body, "Auth::register()")?;
        let password = take_string_field(&mut fields, "password")
            .ok_or_else(|| "Requisicao invalida: campo 'password' obrigatorio".to_string())?;
        if password.chars().count() < config.password_min {
            return Err(format!(
                "Requisicao invalida: password deve ter pelo menos {} caracteres",
                config.password_min
            ));
        }

        let identity = string_json_field(&fields, &config.identity)
            .ok_or_else(|| {
                format!(
                    "Requisicao invalida: campo '{}' deve ser string",
                    config.identity
                )
            })?
            .to_string();

        let mut store = read_store(storage)?;
        store.prune_expired(unix_now()?);
        if store
            .users
            .iter()
            .any(|user| user.auth == config.name && user.identity == identity)
        {
            write_store(storage, &store)?;
            return Err("Conflito: credencial ja existe".to_string());
        }

        let sanitized_body = json_value_json(&JsonValue::Object(fields));
        let user = storage.create_model_record(program, &config.model, &sanitized_body)?;
        let role = config
            .role
            .as_ref()
            .and_then(|field| server_object_field_string(&user, field));
        store.users.push(AuthUser {
            auth: config.name.clone(),
            identity: identity.clone(),
            password_hash: hash_password(&password)?,
            role: role.clone(),
            created_at: unix_now()?,
        });

        let issued = issue_session_and_token(&mut store, config, &identity, role)?;
        write_store(storage, &store)?;
        Ok(auth_success_response(201, user, issued))
    }

    fn login(
        program: &Program,
        storage: &Storage,
        args: &[Expr],
        request_body: &str,
    ) -> Result<AuthRouteResponse, String> {
        let config = auth_config_arg(program, "login", args)?;
        let fields = request_object(request_body, "Auth::login()")?;
        let identity = string_json_field(&fields, &config.identity)
            .ok_or_else(|| {
                format!(
                    "Requisicao invalida: campo '{}' deve ser string",
                    config.identity
                )
            })?
            .to_string();
        let password = string_json_field(&fields, "password")
            .ok_or_else(|| "Requisicao invalida: campo 'password' deve ser string".to_string())?;

        let mut store = read_store(storage)?;
        store.prune_expired(unix_now()?);
        let Some(user_credential) = store
            .users
            .iter()
            .find(|user| user.auth == config.name && user.identity == identity)
        else {
            write_store(storage, &store)?;
            return Err("Nao autorizado: credenciais invalidas".to_string());
        };
        if !verify_password(password, &user_credential.password_hash)? {
            write_store(storage, &store)?;
            return Err("Nao autorizado: credenciais invalidas".to_string());
        }

        let user = storage.find_model_record(
            program,
            &config.model,
            &config.identity,
            &ServerValue::Str(identity.clone()),
        )?;
        let role = config
            .role
            .as_ref()
            .and_then(|field| server_object_field_string(&user, field))
            .or_else(|| user_credential.role.clone());
        let issued = issue_session_and_token(&mut store, config, &identity, role)?;
        write_store(storage, &store)?;
        Ok(auth_success_response(200, user, issued))
    }

    fn logout(
        storage: &Storage,
        headers: &[(String, String)],
        current_user: Option<&AuthenticatedUser>,
    ) -> Result<AuthRouteResponse, String> {
        let mut store = read_store(storage)?;
        if let Some(session) = session_token(headers) {
            let session_hash = token_hash(&session);
            store
                .sessions
                .retain(|candidate| candidate.session_hash != session_hash);
        }
        if let Some(token) = bearer_token(headers) {
            let token_hash = token_hash(&token);
            store
                .tokens
                .retain(|candidate| candidate.token_hash != token_hash);
        }
        if let Some(user) = current_user {
            store.sessions.retain(|candidate| {
                !(candidate.auth == user.auth && candidate.identity == user.identity)
            });
        }
        write_store(storage, &store)?;
        Ok(AuthRouteResponse {
            status: 200,
            body: "true".to_string(),
            headers: vec![("Set-Cookie".to_string(), expired_session_cookie())],
        })
    }

    fn current_user_response(
        program: &Program,
        storage: &Storage,
        current_user: Option<&AuthenticatedUser>,
    ) -> Result<AuthRouteResponse, String> {
        let user = current_user.ok_or_else(|| "Nao autorizado: auth requerido".to_string())?;
        let config = auth_config(program, &user.auth)
            .ok_or_else(|| format!("Nao autorizado: auth '{}' nao encontrado", user.auth))?;
        let record = storage.find_model_record(
            program,
            &config.model,
            &config.identity,
            &ServerValue::Str(user.identity.clone()),
        )?;
        Ok(AuthRouteResponse {
            status: 200,
            body: server_value_json(record),
            headers: Vec::new(),
        })
    }

    struct IssuedSecrets {
        session: String,
        token: String,
        expires_in: u64,
    }

    fn issue_session_and_token(
        store: &mut AuthStore,
        config: &AuthConfig,
        identity: &str,
        role: Option<String>,
    ) -> Result<IssuedSecrets, String> {
        let now = unix_now()?;
        let expires_in = config.session_ttl_minutes * 60;
        let expires_at = now + expires_in;
        let session = random_token();
        let token = random_token();
        store.sessions.push(AuthSession {
            auth: config.name.clone(),
            identity: identity.to_string(),
            session_hash: token_hash(&session),
            role: role.clone(),
            created_at: now,
            last_seen_at: now,
            expires_at,
        });
        store.tokens.push(AuthToken {
            auth: config.name.clone(),
            identity: identity.to_string(),
            token_hash: token_hash(&token),
            role,
            created_at: now,
            expires_at,
        });
        Ok(IssuedSecrets {
            session,
            token,
            expires_in,
        })
    }

    fn auth_success_response(
        status: u16,
        user: ServerValue,
        issued: IssuedSecrets,
    ) -> AuthRouteResponse {
        AuthRouteResponse {
            status,
            body: format!(
                r#"{{"user":{},"token":"{}","expires_in":{}}}"#,
                server_value_json(user),
                escape_json(&issued.token),
                issued.expires_in
            ),
            headers: vec![(
                "Set-Cookie".to_string(),
                session_cookie(&issued.session, issued.expires_in),
            )],
        }
    }

    fn auth_config_arg<'a>(
        program: &'a Program,
        method: &str,
        args: &[Expr],
    ) -> Result<&'a AuthConfig, String> {
        if args.len() != 1 {
            return Err(format!("Auth::{}() recebe exatamente 1 auth", method));
        }
        let Expr::Ident { name, .. } = &args[0] else {
            return Err(format!("Auth::{}() espera nome de auth", method));
        };
        auth_config(program, name).ok_or_else(|| format!("Auth '{}' nao declarado", name))
    }

    fn request_object(body: &str, context: &str) -> Result<Vec<(String, JsonValue)>, String> {
        let value = parse_json(body)
            .map_err(|message| format!("Requisicao invalida: JSON invalido: {}", message))?;
        let JsonValue::Object(fields) = value else {
            return Err(format!(
                "Requisicao invalida: {} espera JSON object",
                context
            ));
        };
        Ok(fields)
    }

    fn string_json_field<'a>(fields: &'a [(String, JsonValue)], name: &str) -> Option<&'a str> {
        fields.iter().find_map(|(field, value)| {
            if field == name {
                if let JsonValue::String(value) = value {
                    Some(value.as_str())
                } else {
                    None
                }
            } else {
                None
            }
        })
    }

    fn take_string_field(fields: &mut Vec<(String, JsonValue)>, name: &str) -> Option<String> {
        let pos = fields.iter().position(|(field, _)| field == name)?;
        match fields.remove(pos).1 {
            JsonValue::String(value) => Some(value),
            _ => None,
        }
    }

    fn server_object_field_string(value: &ServerValue, name: &str) -> Option<String> {
        let ServerValue::Object(fields) = value else {
            return None;
        };
        fields.iter().find_map(|(field, value)| {
            if field == name {
                if let ServerValue::Str(value) = value {
                    Some(value.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
    }

    fn hash_password(password: &str) -> Result<String, String> {
        let salt = SaltString::generate(&mut OsRng);
        argon2()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|e| format!("Erro ao gerar hash Argon2id: {}", e))
    }

    fn verify_password(password: &str, hash: &str) -> Result<bool, String> {
        let parsed = PasswordHash::new(hash)
            .map_err(|e| format!("Hash de password invalido no storage auth: {}", e))?;
        Ok(argon2()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    }

    fn argon2() -> Argon2<'static> {
        let params = Params::new(19_456, 2, 1, None).expect("Argon2id params validos");
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
    }

    fn random_token() -> String {
        let mut bytes = [0_u8; 32];
        OsRng.fill_bytes(&mut bytes);
        hex_encode(&bytes)
    }

    fn token_hash(token: &str) -> String {
        let digest = Sha256::digest(token.as_bytes());
        hex_encode(&digest)
    }

    fn hex_encode(bytes: &[u8]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
        out
    }

    fn unix_now() -> Result<u64, String> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .map_err(|e| e.to_string())
    }

    fn session_cookie(token: &str, max_age: u64) -> String {
        format!(
            "{}={}; Path=/; Max-Age={}; HttpOnly; Secure; SameSite=Lax",
            SESSION_COOKIE, token, max_age
        )
    }

    fn expired_session_cookie() -> String {
        format!(
            "{}=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax",
            SESSION_COOKIE
        )
    }

    fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
        headers
            .iter()
            .find(|(header, _)| header.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    fn bearer_token(headers: &[(String, String)]) -> Option<String> {
        let value = header_value(headers, "Authorization")?.trim();
        value
            .strip_prefix("Bearer ")
            .or_else(|| value.strip_prefix("bearer "))
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty())
    }

    fn session_token(headers: &[(String, String)]) -> Option<String> {
        let cookie = header_value(headers, "Cookie")?;
        for part in cookie.split(';') {
            let (name, value) = part.trim().split_once('=')?;
            if name == SESSION_COOKIE && !value.is_empty() {
                return Some(value.to_string());
            }
        }
        None
    }

    impl AuthStore {
        fn prune_expired(&mut self, now: u64) {
            self.sessions.retain(|session| session.expires_at > now);
            self.tokens.retain(|token| token.expires_at > now);
        }
    }

    fn read_store(storage: &Storage) -> Result<AuthStore, String> {
        let path = storage.auth_file()?;
        if !path.exists() {
            return Ok(AuthStore::default());
        }
        let source = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if source.trim().is_empty() {
            return Ok(AuthStore::default());
        }
        let value = parse_json(&source)
            .map_err(|message| format!("Storage auth JSON invalido: {}", message))?;
        auth_store_from_json(value)
    }

    fn write_store(storage: &Storage, store: &AuthStore) -> Result<(), String> {
        let path = storage.auth_file()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, auth_store_json(store)).map_err(|e| e.to_string())
    }

    fn auth_store_from_json(value: JsonValue) -> Result<AuthStore, String> {
        let JsonValue::Object(mut fields) = value else {
            return Err("root deve ser object".to_string());
        };
        Ok(AuthStore {
            users: auth_users_from_json(take_array(&mut fields, "users")?)?,
            sessions: auth_sessions_from_json(take_array(&mut fields, "sessions")?)?,
            tokens: auth_tokens_from_json(take_array(&mut fields, "tokens")?)?,
        })
    }

    fn take_array(
        fields: &mut Vec<(String, JsonValue)>,
        name: &str,
    ) -> Result<Vec<JsonValue>, String> {
        let Some(pos) = fields.iter().position(|(field, _)| field == name) else {
            return Ok(Vec::new());
        };
        let JsonValue::Array(items) = fields.remove(pos).1 else {
            return Err(format!("campo '{}' deve ser array", name));
        };
        Ok(items)
    }

    fn auth_users_from_json(items: Vec<JsonValue>) -> Result<Vec<AuthUser>, String> {
        items
            .into_iter()
            .map(|item| {
                let JsonValue::Object(mut fields) = item else {
                    return Err("user auth deve ser object".to_string());
                };
                Ok(AuthUser {
                    auth: take_required_string(&mut fields, "auth")?,
                    identity: take_required_string(&mut fields, "identity")?,
                    password_hash: take_required_string(&mut fields, "password_hash")?,
                    role: take_optional_string(&mut fields, "role")?,
                    created_at: take_required_u64(&mut fields, "created_at")?,
                })
            })
            .collect()
    }

    fn auth_sessions_from_json(items: Vec<JsonValue>) -> Result<Vec<AuthSession>, String> {
        items
            .into_iter()
            .map(|item| {
                let JsonValue::Object(mut fields) = item else {
                    return Err("session auth deve ser object".to_string());
                };
                Ok(AuthSession {
                    auth: take_required_string(&mut fields, "auth")?,
                    identity: take_required_string(&mut fields, "identity")?,
                    session_hash: take_required_string(&mut fields, "session_hash")?,
                    role: take_optional_string(&mut fields, "role")?,
                    created_at: take_required_u64(&mut fields, "created_at")?,
                    last_seen_at: take_required_u64(&mut fields, "last_seen_at")?,
                    expires_at: take_required_u64(&mut fields, "expires_at")?,
                })
            })
            .collect()
    }

    fn auth_tokens_from_json(items: Vec<JsonValue>) -> Result<Vec<AuthToken>, String> {
        items
            .into_iter()
            .map(|item| {
                let JsonValue::Object(mut fields) = item else {
                    return Err("token auth deve ser object".to_string());
                };
                Ok(AuthToken {
                    auth: take_required_string(&mut fields, "auth")?,
                    identity: take_required_string(&mut fields, "identity")?,
                    token_hash: take_required_string(&mut fields, "token_hash")?,
                    role: take_optional_string(&mut fields, "role")?,
                    created_at: take_required_u64(&mut fields, "created_at")?,
                    expires_at: take_required_u64(&mut fields, "expires_at")?,
                })
            })
            .collect()
    }

    fn take_required_string(
        fields: &mut Vec<(String, JsonValue)>,
        name: &str,
    ) -> Result<String, String> {
        let Some(pos) = fields.iter().position(|(field, _)| field == name) else {
            return Err(format!("campo '{}' ausente", name));
        };
        let JsonValue::String(value) = fields.remove(pos).1 else {
            return Err(format!("campo '{}' deve ser string", name));
        };
        Ok(value)
    }

    fn take_optional_string(
        fields: &mut Vec<(String, JsonValue)>,
        name: &str,
    ) -> Result<Option<String>, String> {
        let Some(pos) = fields.iter().position(|(field, _)| field == name) else {
            return Ok(None);
        };
        match fields.remove(pos).1 {
            JsonValue::String(value) => Ok(Some(value)),
            JsonValue::Null => Ok(None),
            _ => Err(format!("campo '{}' deve ser string ou null", name)),
        }
    }

    fn take_required_u64(fields: &mut Vec<(String, JsonValue)>, name: &str) -> Result<u64, String> {
        let Some(pos) = fields.iter().position(|(field, _)| field == name) else {
            return Err(format!("campo '{}' ausente", name));
        };
        let JsonValue::Number(value) = fields.remove(pos).1 else {
            return Err(format!("campo '{}' deve ser number", name));
        };
        if value < 0.0 || value.fract() != 0.0 {
            return Err(format!("campo '{}' deve ser inteiro positivo", name));
        }
        Ok(value as u64)
    }

    fn auth_store_json(store: &AuthStore) -> String {
        format!(
            r#"{{"users":[{}],"sessions":[{}],"tokens":[{}]}}"#,
            store
                .users
                .iter()
                .map(auth_user_json)
                .collect::<Vec<_>>()
                .join(","),
            store
                .sessions
                .iter()
                .map(auth_session_json)
                .collect::<Vec<_>>()
                .join(","),
            store
                .tokens
                .iter()
                .map(auth_token_json)
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    fn auth_user_json(user: &AuthUser) -> String {
        format!(
            r#"{{"auth":"{}","identity":"{}","password_hash":"{}","role":{},"created_at":{}}}"#,
            escape_json(&user.auth),
            escape_json(&user.identity),
            escape_json(&user.password_hash),
            option_string_json(user.role.as_deref()),
            user.created_at
        )
    }

    fn auth_session_json(session: &AuthSession) -> String {
        format!(
            r#"{{"auth":"{}","identity":"{}","session_hash":"{}","role":{},"created_at":{},"last_seen_at":{},"expires_at":{}}}"#,
            escape_json(&session.auth),
            escape_json(&session.identity),
            escape_json(&session.session_hash),
            option_string_json(session.role.as_deref()),
            session.created_at,
            session.last_seen_at,
            session.expires_at
        )
    }

    fn auth_token_json(token: &AuthToken) -> String {
        format!(
            r#"{{"auth":"{}","identity":"{}","token_hash":"{}","role":{},"created_at":{},"expires_at":{}}}"#,
            escape_json(&token.auth),
            escape_json(&token.identity),
            escape_json(&token.token_hash),
            option_string_json(token.role.as_deref()),
            token.created_at,
            token.expires_at
        )
    }

    fn option_string_json(value: Option<&str>) -> String {
        value
            .map(|value| format!(r#""{}""#, escape_json(value)))
            .unwrap_or_else(|| "null".to_string())
    }
}

#[cfg(target_arch = "wasm32")]
mod native {
    use super::*;

    pub(crate) fn authenticate_request(
        _program: &Program,
        _storage: &Storage,
        _guard: &RouteAuthGuard,
        _headers: &[(String, String)],
    ) -> Result<AuthenticatedUser, String> {
        Err("Auth nativo nao esta disponivel no target WASM".to_string())
    }

    pub(crate) fn eval_auth_return(
        expr: &Expr,
        _program: &Program,
        _storage: &Storage,
        _headers: &[(String, String)],
        _request_body: &str,
        _current_user: Option<&AuthenticatedUser>,
    ) -> Option<Result<AuthRouteResponse, String>> {
        if matches!(expr, Expr::StaticCall { ty, .. } if ty == "Auth") {
            Some(Err(
                "Auth nativo nao esta disponivel no target WASM".to_string()
            ))
        } else {
            None
        }
    }
}

pub(crate) use native::{authenticate_request, eval_auth_return};
