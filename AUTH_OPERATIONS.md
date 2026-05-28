# NexusLang Auth Operation Contract

This document describes the internal contract for static auth operations:
`Auth::register(...)`, `Auth::login(...)`, `Auth::logout()`, and
`Auth::user()`.

The source of truth is `nexuslang-src/src/auth_ops.rs`. New auth operations
must be added to `AuthStaticOperation` and to the central
`AUTH_STATIC_OPERATION_DESCRIPTORS` table before checker, route HIR, runtime,
or OpenAPI code depends on them.

## Purpose

Auth operations are a compiler/runtime boundary. They are parsed as regular
static calls and then recognized by semantic checking, route HIR, HTTP runtime,
and OpenAPI generation.

The descriptor table keeps this boundary explicit:

- the public method name after `Auth::`;
- the accepted argument shape;
- the required route HTTP method;
- the response kind;
- the success status;
- whether the operation exposes an OpenAPI request body;
- whether the operation can produce a bad-request response;
- whether the operation can produce a rate-limit response.

This avoids duplicated string chains such as
`"register" | "login" | "logout" | "user"` across checker, route HIR,
runtime auth, and OpenAPI.

## Source Of Truth

The central descriptor is:

```rust
pub const AUTH_STATIC_OPERATION_DESCRIPTORS: &[AuthStaticOperationDescriptor]
```

Each `AuthStaticOperationDescriptor` contains:

| Field | Meaning |
| --- | --- |
| `operation` | Stable enum variant used by checked route/runtime code. |
| `method_name` | Public static call name after `Auth::`. |
| `argument_shape` | `AuthConfig` or `Empty`. |
| `route_method` | Required HTTP method for route returns. |
| `return_kind` | Auth success envelope, current user, or boolean. |
| `success_status` | HTTP success status used by runtime/OpenAPI. |
| `request_body_kind` | OpenAPI request payload kind, when applicable. |
| `has_bad_request_response` | Whether OpenAPI exposes `400`. |
| `has_rate_limit_response` | Whether OpenAPI exposes `429`. |

`AuthStaticOperation::checked_args(...)` returns `CheckedAuthOperationArgs`
when the raw AST arguments match the descriptor shape.

## Consumers

- `nexuslang-src/src/checker/mod.rs` recognizes operations through
  `AuthStaticOperation::from_method`, validates arguments with
  `CheckedAuthOperationArgs`, enforces route HTTP methods, and preserves the
  existing diagnostics.
- `nexuslang-src/src/route_hir.rs` converts route return expressions into
  `CheckedRouteExpr::AuthOperation` with attached `CheckedAuthOperationArgs`.
- `nexuslang-src/src/server/auth.rs` executes checked auth operations without
  reparsing method strings.
- `nexuslang-src/src/server/openapi.rs` uses the descriptor return kind,
  success status, and rate-limit metadata to generate response contracts.

## Current Operations

| Operation | Args | Route method | Return kind | Status | Request body | 400 | Rate limit |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `register` | `AuthConfig` | `POST` | Auth success envelope | `201` | yes | yes | yes |
| `login` | `AuthConfig` | `POST` | Auth success envelope | `200` | yes | yes | yes |
| `logout` | empty | `POST` | boolean | `200` | no | no | no |
| `user` | empty | `GET` | current user | `200` | no | no | no |

## Contract Matrix

`server/mod.rs` contains
`auth_operation_contract_matrix_validates_checker_hir_openapi_and_http`.
The matrix must cover every `AuthStaticOperation::ALL` entry and validate, in
one suite:

- semantic checking of a canonical Auth source;
- route HIR lifting into `CheckedRouteExpr::AuthOperation`;
- normalized `CheckedAuthOperationArgs`;
- OpenAPI request body, success status, auth security, CSRF header, 400, and
  rate-limit responses;
- real HTTP behavior for register, login, user, and logout.
