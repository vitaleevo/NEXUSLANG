use crate::ast::{AuthConfig, Type};
use crate::hir::{self, HirRefId, HirSymbolId, HirSymbolKind};

use super::resolver::ResolvedProgram;
use super::{CheckResult, Checker};

impl Checker {
    pub(super) fn collect_auth_declaration(
        &mut self,
        config: &AuthConfig,
        resolved: &ResolvedProgram<'_>,
    ) -> CheckResult<()> {
        if self.auths.contains_key(&config.name) {
            return Err(self.error(
                config.span,
                format!("Auth '{}' declarado mais de uma vez", config.name),
            ));
        }

        self.auths.insert(config.name.clone(), config.clone());
        self.symbols.set_top_level(
            HirSymbolKind::Auth,
            &config.name,
            resolved.top_level_symbol(HirSymbolKind::Auth, &config.name),
        );

        Ok(())
    }

    pub(super) fn check_auth_declaration(
        &self,
        config: &AuthConfig,
        hir_decl: Option<&hir::HirDecl<'_>>,
    ) -> CheckResult<()> {
        self.check_auth_config(config, hir_decl)
    }

    pub(super) fn check_auth_config(
        &self,
        config: &AuthConfig,
        hir_decl: Option<&hir::HirDecl<'_>>,
    ) -> CheckResult<()> {
        let fields = if self.model_symbol(&config.model).is_some() {
            self.models.get(&config.model)
        } else {
            None
        }
        .ok_or_else(|| {
            self.error(
                config.span,
                format!(
                    "Auth '{}' referencia model '{}' inexistente",
                    config.name, config.model
                ),
            )
        })?;

        let Some(identity) = fields.iter().find(|field| field.name == config.identity) else {
            return Err(self.error(
                config.span,
                format!(
                    "Auth '{}' identity '{}.{}' nao existe",
                    config.name, config.model, config.identity
                ),
            ));
        };
        if !matches!(identity.ty, Type::String) || !identity.unique {
            return Err(self.error(
                identity.span,
                format!(
                    "Auth '{}' identity '{}.{}' deve ser string unique",
                    config.name, config.model, config.identity
                ),
            ));
        }

        if let Some(role) = &config.role {
            let Some(role_field) = fields.iter().find(|field| field.name == *role) else {
                return Err(self.error(
                    config.span,
                    format!(
                        "Auth '{}' role '{}.{}' nao existe",
                        config.name, config.model, role
                    ),
                ));
            };
            if !matches!(role_field.ty, Type::String) {
                return Err(self.error(
                    role_field.span,
                    format!(
                        "Auth '{}' role '{}.{}' deve ser string",
                        config.name, config.model, role
                    ),
                ));
            }
        }

        if config.password_min < 15 {
            return Err(self.error(
                config.span,
                format!("Auth '{}' password_min deve ser pelo menos 15", config.name),
            ));
        }
        if config.session_ttl_minutes == 0 || config.idle_ttl_minutes == 0 {
            return Err(self.error(
                config.span,
                format!("Auth '{}' TTLs devem ser maiores que zero", config.name),
            ));
        }
        if config.idle_ttl_minutes > config.session_ttl_minutes {
            return Err(self.error(
                config.span,
                format!(
                    "Auth '{}' idle_ttl_minutes nao pode exceder session_ttl_minutes",
                    config.name
                ),
            ));
        }

        if let Some(refs) = hir_auth_refs(hir_decl) {
            if let Some(symbol) = self.model_symbol(&config.model) {
                self.link_and_consume_auth_reference(refs.model, symbol);
            }
            if let Some(symbol) = self.model_field_symbol(&config.model, &config.identity) {
                self.link_and_consume_auth_reference(refs.identity, symbol);
            }
            if let (Some(role), Some(role_ref)) = (&config.role, refs.role) {
                if let Some(symbol) = self.model_field_symbol(&config.model, role) {
                    self.link_and_consume_auth_reference(role_ref, symbol);
                }
            }
        }

        Ok(())
    }

    fn link_and_consume_auth_reference(&self, reference: HirRefId, symbol: HirSymbolId) {
        self.link_hir_reference_symbol(reference, symbol);
        let _ = self.typed_hir_reference_symbol(reference);
    }
}

#[derive(Debug, Clone, Copy)]
struct HirAuthRefs {
    model: HirRefId,
    identity: HirRefId,
    role: Option<HirRefId>,
}

fn hir_auth_refs(hir_decl: Option<&hir::HirDecl<'_>>) -> Option<HirAuthRefs> {
    let hir_decl = hir_decl?;
    let hir::HirDeclBody::Auth {
        model,
        identity,
        role,
    } = &hir_decl.body
    else {
        return None;
    };

    Some(HirAuthRefs {
        model: *model,
        identity: *identity,
        role: *role,
    })
}
