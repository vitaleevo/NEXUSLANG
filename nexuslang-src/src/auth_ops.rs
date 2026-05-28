use crate::ast::{Expr, HttpMethod};

pub const AUTH_STATIC_TYPE_NAME: &str = "Auth";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthStaticOperation {
    Register,
    Login,
    Logout,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthOperationArgumentShape {
    AuthConfig,
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthOperationReturnKind {
    AuthSuccess,
    CurrentUser,
    Bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthOperationRequestBodyKind {
    Register,
    Login,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthRouteMethodRequirement {
    Get,
    Post,
}

#[derive(Debug, Clone, Copy)]
pub struct AuthStaticOperationDescriptor {
    pub operation: AuthStaticOperation,
    pub method_name: &'static str,
    pub argument_shape: AuthOperationArgumentShape,
    pub route_method: AuthRouteMethodRequirement,
    pub return_kind: AuthOperationReturnKind,
    pub success_status: u16,
    pub request_body_kind: Option<AuthOperationRequestBodyKind>,
    pub has_bad_request_response: bool,
    pub has_rate_limit_response: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct CheckedAuthOperationArgs<'a> {
    pub raw: &'a [Expr],
    pub kind: CheckedAuthOperationArgsKind<'a>,
}

#[derive(Debug, Clone, Copy)]
pub enum CheckedAuthOperationArgsKind<'a> {
    AuthConfig { name: &'a str, expr: &'a Expr },
    Empty,
}

impl<'a> CheckedAuthOperationArgs<'a> {
    pub fn auth_config_name(self) -> Option<&'a str> {
        match self.kind {
            CheckedAuthOperationArgsKind::AuthConfig { name, .. } => Some(name),
            CheckedAuthOperationArgsKind::Empty => None,
        }
    }

    pub fn auth_config_expr(self) -> Option<&'a Expr> {
        match self.kind {
            CheckedAuthOperationArgsKind::AuthConfig { expr, .. } => Some(expr),
            CheckedAuthOperationArgsKind::Empty => None,
        }
    }
}

impl AuthRouteMethodRequirement {
    pub fn name(self) -> &'static str {
        match self {
            AuthRouteMethodRequirement::Get => "GET",
            AuthRouteMethodRequirement::Post => "POST",
        }
    }

    pub fn matches(self, method: &HttpMethod) -> bool {
        matches!(
            (self, method),
            (AuthRouteMethodRequirement::Get, HttpMethod::Get)
                | (AuthRouteMethodRequirement::Post, HttpMethod::Post)
        )
    }
}

impl AuthStaticOperation {
    pub const ALL: [Self; 4] = [Self::Register, Self::Login, Self::Logout, Self::User];

    pub fn from_method(method: &str) -> Option<Self> {
        AUTH_STATIC_OPERATION_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.method_name == method)
            .map(|descriptor| descriptor.operation)
    }

    pub fn descriptor(self) -> &'static AuthStaticOperationDescriptor {
        AUTH_STATIC_OPERATION_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.operation == self)
            .expect("AuthStaticOperation descriptor missing")
    }

    pub fn method_name(self) -> &'static str {
        self.descriptor().method_name
    }

    pub fn call_name(self) -> String {
        format!("{}::{}()", AUTH_STATIC_TYPE_NAME, self.method_name())
    }

    pub fn argument_shape(self) -> AuthOperationArgumentShape {
        self.descriptor().argument_shape
    }

    pub fn return_kind(self) -> AuthOperationReturnKind {
        self.descriptor().return_kind
    }

    pub fn required_route_method(self) -> AuthRouteMethodRequirement {
        self.descriptor().route_method
    }

    pub fn route_method_error(self) -> String {
        format!(
            "{}::{}() so pode ser usado em route {}",
            AUTH_STATIC_TYPE_NAME,
            self.method_name(),
            self.required_route_method().name()
        )
    }

    pub fn success_status(self) -> u16 {
        self.descriptor().success_status
    }

    pub fn success_status_name(self) -> &'static str {
        match self.success_status() {
            201 => "201",
            _ => "200",
        }
    }

    pub fn is_create_like(self) -> bool {
        self.success_status() == 201
    }

    pub fn has_rate_limit_response(self) -> bool {
        self.descriptor().has_rate_limit_response
    }

    pub fn request_body_kind(self) -> Option<AuthOperationRequestBodyKind> {
        self.descriptor().request_body_kind
    }

    pub fn uses_request_body(self) -> bool {
        self.request_body_kind().is_some()
    }

    pub fn has_bad_request_response(self) -> bool {
        self.descriptor().has_bad_request_response
    }

    pub fn checked_args<'a>(self, args: &'a [Expr]) -> Option<CheckedAuthOperationArgs<'a>> {
        let kind = match self.argument_shape() {
            AuthOperationArgumentShape::AuthConfig => {
                let [Expr::Ident { name, .. }] = args else {
                    return None;
                };
                CheckedAuthOperationArgsKind::AuthConfig {
                    name: name.as_str(),
                    expr: &args[0],
                }
            }
            AuthOperationArgumentShape::Empty => {
                if !args.is_empty() {
                    return None;
                }
                CheckedAuthOperationArgsKind::Empty
            }
        };

        Some(CheckedAuthOperationArgs { raw: args, kind })
    }

    pub fn argument_error(self, args: &[Expr]) -> String {
        match self.argument_shape() {
            AuthOperationArgumentShape::AuthConfig => {
                if args.len() != 1 {
                    format!(
                        "{}::{}() recebe exatamente 1 auth",
                        AUTH_STATIC_TYPE_NAME,
                        self.method_name()
                    )
                } else {
                    format!(
                        "{}::{}() espera nome de auth",
                        AUTH_STATIC_TYPE_NAME,
                        self.method_name()
                    )
                }
            }
            AuthOperationArgumentShape::Empty => {
                format!(
                    "{}::{}() nao recebe argumentos",
                    AUTH_STATIC_TYPE_NAME,
                    self.method_name()
                )
            }
        }
    }
}

pub const AUTH_STATIC_OPERATION_DESCRIPTORS: &[AuthStaticOperationDescriptor] = &[
    descriptor(
        AuthStaticOperation::Register,
        "register",
        AuthOperationArgumentShape::AuthConfig,
        AuthRouteMethodRequirement::Post,
        AuthOperationReturnKind::AuthSuccess,
        201,
        Some(AuthOperationRequestBodyKind::Register),
        true,
        true,
    ),
    descriptor(
        AuthStaticOperation::Login,
        "login",
        AuthOperationArgumentShape::AuthConfig,
        AuthRouteMethodRequirement::Post,
        AuthOperationReturnKind::AuthSuccess,
        200,
        Some(AuthOperationRequestBodyKind::Login),
        true,
        true,
    ),
    descriptor(
        AuthStaticOperation::Logout,
        "logout",
        AuthOperationArgumentShape::Empty,
        AuthRouteMethodRequirement::Post,
        AuthOperationReturnKind::Bool,
        200,
        None,
        false,
        false,
    ),
    descriptor(
        AuthStaticOperation::User,
        "user",
        AuthOperationArgumentShape::Empty,
        AuthRouteMethodRequirement::Get,
        AuthOperationReturnKind::CurrentUser,
        200,
        None,
        false,
        false,
    ),
];

const fn descriptor(
    operation: AuthStaticOperation,
    method_name: &'static str,
    argument_shape: AuthOperationArgumentShape,
    route_method: AuthRouteMethodRequirement,
    return_kind: AuthOperationReturnKind,
    success_status: u16,
    request_body_kind: Option<AuthOperationRequestBodyKind>,
    has_bad_request_response: bool,
    has_rate_limit_response: bool,
) -> AuthStaticOperationDescriptor {
    AuthStaticOperationDescriptor {
        operation,
        method_name,
        argument_shape,
        route_method,
        return_kind,
        success_status,
        request_body_kind,
        has_bad_request_response,
        has_rate_limit_response,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::ast::Span;

    fn ident(name: &str) -> Expr {
        Expr::Ident {
            name: name.to_string(),
            span: Span::unknown(),
        }
    }

    #[test]
    fn descriptors_cover_all_auth_static_operations() {
        let mut names = HashSet::new();
        let mut operations = HashSet::new();

        for descriptor in AUTH_STATIC_OPERATION_DESCRIPTORS {
            assert!(names.insert(descriptor.method_name));
            assert!(operations.insert(descriptor.operation));
            assert_eq!(
                Some(descriptor.operation),
                AuthStaticOperation::from_method(descriptor.method_name)
            );
            assert_eq!(descriptor.method_name, descriptor.operation.method_name());
        }

        assert_eq!(
            AUTH_STATIC_OPERATION_DESCRIPTORS.len(),
            AuthStaticOperation::ALL.len()
        );
        assert_eq!(names.len(), AuthStaticOperation::ALL.len());
        assert_eq!(operations.len(), AuthStaticOperation::ALL.len());
    }

    #[test]
    fn descriptor_exposes_route_status_and_rate_limit_contract() {
        let register = AuthStaticOperation::Register.descriptor();
        assert_eq!(
            register.argument_shape,
            AuthOperationArgumentShape::AuthConfig
        );
        assert_eq!(register.route_method, AuthRouteMethodRequirement::Post);
        assert_eq!(register.return_kind, AuthOperationReturnKind::AuthSuccess);
        assert_eq!(register.success_status, 201);
        assert_eq!(
            register.request_body_kind,
            Some(AuthOperationRequestBodyKind::Register)
        );
        assert!(register.has_bad_request_response);
        assert!(register.has_rate_limit_response);

        let user = AuthStaticOperation::User.descriptor();
        assert_eq!(user.argument_shape, AuthOperationArgumentShape::Empty);
        assert_eq!(user.route_method, AuthRouteMethodRequirement::Get);
        assert_eq!(user.return_kind, AuthOperationReturnKind::CurrentUser);
        assert!(user.request_body_kind.is_none());
        assert!(!user.has_bad_request_response);
        assert!(!user.has_rate_limit_response);
    }

    #[test]
    fn checked_args_normalize_auth_config_and_empty_shapes() {
        let config_args = vec![ident("UserAuth")];
        let checked = AuthStaticOperation::Login
            .checked_args(&config_args)
            .expect("login args should normalize");
        assert_eq!(checked.auth_config_name(), Some("UserAuth"));

        let empty_args = Vec::new();
        let checked = AuthStaticOperation::Logout
            .checked_args(&empty_args)
            .expect("logout args should normalize");
        assert!(checked.auth_config_name().is_none());

        assert!(AuthStaticOperation::Register
            .checked_args(&empty_args)
            .is_none());
        assert!(AuthStaticOperation::User
            .checked_args(&config_args)
            .is_none());
    }
}
