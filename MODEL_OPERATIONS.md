# NexusLang Model Operation Contract

This document describes the internal contract for static model operations such
as `Customer::create()`, `Customer::where(...)`, and `Customer::page(...)`.

The source of truth is `nexuslang-src/src/model_ops.rs`. New model operations
must be added to `ModelStaticOperation` and to the central
`MODEL_STATIC_OPERATION_DESCRIPTORS` table before any checker, router, or
OpenAPI code depends on them.

## Purpose

Static model operations are a compiler/runtime boundary. They are parsed as
regular static calls, then recognized as supported model operations by semantic
checking and route/runtime layers.

The descriptor table keeps this boundary explicit:

- the public method name, for example `where_in_optional_page`;
- the accepted argument shape;
- the route HTTP method required by the operation;
- the route return kind exposed to type checking and OpenAPI;
- the checker validation variant;
- the runtime storage dispatch category;
- the OpenAPI features and response behavior.

This avoids duplicated string chains such as `"create" | "find" | "where"`
across the checker, router, and OpenAPI generator.

`CheckedModelOperationArgs` is the normalized argument view built from this
descriptor. It records the raw argument slice plus resolved lookup/filter
slots, ordering, pagination, and total-count page-envelope behavior so
checker, route HIR, runtime, storage helpers, and OpenAPI do not have to
recalculate argument positions independently.

## Source Of Truth

The central descriptor is:

```rust
pub const MODEL_STATIC_OPERATION_DESCRIPTORS: &[ModelStaticOperationDescriptor]
```

Each `ModelStaticOperationDescriptor` contains:

| Field | Meaning |
| --- | --- |
| `operation` | Stable enum variant used by checked route/runtime code. |
| `method_name` | Public static call name after `Model::`. |
| `argument_shape` | Coarse shape accepted before detailed semantic checks. |
| `route_method` | Required HTTP method for route returns, when applicable. |
| `return_kind` | `Model` or `List` response type. |
| `checker_validation` | Detailed checker rule family. |
| `storage_category` | Runtime storage dispatch class. |
| `openapi_flags` | Request body, status code, error response, and feature markers. |

`ModelStaticOperation::checked_args(...)` returns
`CheckedModelOperationArgs` when the raw AST arguments match the descriptor
shape. Legacy helpers such as `args_supported(...)`, `has_pagination(...)`,
`has_ordering(...)`, and `openapi_flags(...)` are wrappers around that
normalized form.

## Consumers

The descriptor contract is consumed by these layers:

- `nexuslang-src/src/checker/mod.rs` recognizes operations through
  `ModelStaticOperation::from_method` and remains the route-checking
  orchestrator.
- `nexuslang-src/src/checker/model_ops.rs` owns operation-specific semantic
  validation through `ModelOperationCheckerValidation`. After the specific
  checks pass, the checker also requires `CheckedModelOperationArgs` to be
  produced successfully. Model filter validation is grouped by normalized
  argument family: exact lookup, optional lookup, array lookup, optional-array
  lookup, comparison, text, range, and composite filters.
- `nexuslang-src/src/route_hir.rs` converts checked route return expressions
  into `CheckedRouteExpr::ModelOperation` with attached
  `CheckedModelOperationArgs`, so server code does not need to rediscover raw
  AST string calls or argument offsets.
- `nexuslang-src/src/server/router.rs` uses `storage_category()` and
  `CheckedModelOperationArgs` to dispatch supported runtime behavior.
- `nexuslang-src/src/server/storage.rs` evaluates list options and composite
  filters from normalized ordering, pagination, and filter slices.
- `nexuslang-src/src/server/openapi.rs` uses return kind, request-body flags,
  HTTP status behavior, pagination/ordering detection, and feature flags to
  generate OpenAPI for the supported route subset.
- `nexuslang-src/src/server/mod.rs` keeps a route contract matrix so every
  `ModelStaticOperation` has server/OpenAPI coverage expectations.

## Argument Shapes

`ModelOperationArgumentShape` intentionally represents broad families, not the
full type system:

| Shape | Accepted form |
| --- | --- |
| `All` | `all()`, `all(limit, offset)`, `all(order_by, direction)`, or `all(order_by, direction, limit, offset)`. |
| `Page` | Explicit paged list with `limit, offset`, optionally ordered. |
| `Create` | Empty static call; body comes from the HTTP request payload. |
| `Lookup` | Lookup/update/delete identity pair. |
| `Where` | Field/value filter, optionally ordered and/or paginated. |
| `WherePage` | Field/value filter with total-count page semantics. |
| `AdvancedWhere` | Field/operator/value filter, optionally ordered and/or paginated. |
| `AdvancedWherePage` | Advanced filter with total-count page semantics. |
| `CompositeWhere` | One or more field/value filter pairs, optionally ordered and/or paginated. |
| `CompositeWherePage` | Composite filter with total-count page semantics. |

Detailed type validation remains in the checker. The shape layer exists to keep
route/runtime/OpenAPI branching predictable.

## Checked Argument HIR

`CheckedModelOperationArgs` contains:

| Field | Meaning |
| --- | --- |
| `raw` | Original AST arguments for diagnostics and recursive route validation. |
| `kind` | Normalized operation family: request body, list, lookup, simple filter, advanced filter, range filter, or composite filter. |
| `ordering` | Optional ordering field and direction expressions. |
| `pagination` | Optional limit and offset expressions. |
| `page_response` | Whether the operation returns a total-count envelope. |

The normalized kind is intentionally still expression-based. It does not
evaluate route parameters or request data; runtime code evaluates the referenced
expressions later against the current HTTP request.

The checker submodule uses this HIR to avoid repeating suffix parsing and
field/value validation across every `where*` method. Operation-specific
wrappers still own the public diagnostic text for each method, while shared
family validators own the semantic rules.

## Route Methods And Return Kinds

The route method contract is descriptor-owned:

| Operation group | HTTP method | Return kind |
| --- | --- | --- |
| `all`, `page`, `where*`, `where_*` filters | `GET` | list of model instances |
| `create` | `POST` | single model instance |
| `find` | `GET` | single model instance |
| `update` | `PUT` | single model instance |
| `delete` | `DELETE` | single model instance |

`Model::all()` without pagination remains special-cased as a route return that
does not require a method-specific diagnostic beyond the route's own method
validation.

## OpenAPI Flags

`ModelOperationOpenApiFlags` describes behavior that the OpenAPI generator must
emit:

- `REQUEST_BODY` for body-backed operations such as create/update;
- `CREATED_STATUS` for `201` create responses;
- `NOT_FOUND_RESPONSE` for lookup/update/delete misses;
- `CONFLICT_RESPONSE` for unique-field conflict risks;
- `PAGINATION`, `TOTAL_COUNT`, and `ORDERING` when arguments imply them;
- filter extension flags for composite, OR, exclusion, optional, in-list,
  comparison, text, and range filters.

`ModelOperationOpenApiFeature` maps feature-level OpenAPI checks back to these
flags instead of repeating operation names.

## Storage Categories

`ModelOperationStorageCategory` is the router-facing dispatch contract. It
distinguishes list, page, create, find, update, delete, equality filters,
negated filters, inclusion filters, optional filters, comparison filters, text
filters, range filters, and composite/all-vs-any filters.

Runtime code may branch on storage categories. It should not branch on public
method-name strings once `ModelStaticOperation` has been resolved.

## Adding A Model Operation

When adding a model operation:

1. Add the enum variant to `ModelStaticOperation::ALL`.
2. Add exactly one entry to `MODEL_STATIC_OPERATION_DESCRIPTORS`.
3. Pick an existing `ModelOperationArgumentShape` or introduce a new shape with
   tests.
4. Ensure `ModelStaticOperation::checked_args(...)` can normalize the new
   operation into `CheckedModelOperationArgs`.
5. Add or reuse a `ModelOperationCheckerValidation` rule.
6. Add or reuse a `ModelOperationStorageCategory`.
7. Add OpenAPI flags for request body, response codes, pagination, ordering,
   total-count responses, and feature extensions.
8. Extend checker tests, server route matrix tests, smoke tests, and OpenAPI
   contract tests.
9. Update this document if the public contract or operation family changes.

## Quality Gate

The repository quality gate runs
`scripts/validate-model-operation-contract-docs.sh` to verify that this
documentation and the Rust source still expose the central contract anchors.

The Rust unit tests in `model_ops.rs` verify that descriptors cover every
`ModelStaticOperation`, that public method names are unique, and that key
route/OpenAPI/storage metadata stays reachable through the descriptor API.
They also verify that `CheckedModelOperationArgs` normalizes suffixes,
advanced filters, and composite filters used by runtime and OpenAPI.
