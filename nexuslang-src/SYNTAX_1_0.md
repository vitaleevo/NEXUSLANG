# NexusLang 1.0 Syntax Baseline

Status: Phase 7.62 OpenAPI 1.0 readiness. This document records the syntax and
HTTP/OpenAPI contract that should stay stable while the project moves toward
the 1.0 release target.

## Program Shape

NexusLang programs are newline-friendly scripts made of top-level declarations
and statements. Semicolons are tokenized for future compatibility, but the 1.0
baseline uses statement forms, braces, and comma-separated lists instead of
semicolon-separated syntax.

Supported top-level declarations:

- `fn name(param: type, other: type) -> type { ... }`
- `model Name { field: type unique index min value max value = default }`
- `workflow Name { step name { ... } }`
- `route METHOD /path/:param { ... }`
- `route METHOD /path ?(query: type) { ... }`
- `route METHOD /path/static-segment { ... }`
- `invoice { key: value item "desc" qty 1 price 100 kz }`

Top-level executable statements remain supported for ERP scripts and examples.

## Stable Lists

Comma-separated lists must use commas between items:

- function parameters: `fn soma(a: int, b: int) -> int`
- function call arguments: `soma(1, 2)`
- static call arguments: `Model::method(a, b)` when such methods exist
- arrays: `[1, 2, 3]`

The parser rejects accidental forms such as `soma(1 2)`, `[1 2]`, and
`fn soma(a: int b: int)`.
Trailing commas are not part of the baseline yet.

## Model Instances

Models can be materialized as structured values with object literals:

```nexus
model Customer {
    name: string
    email: string?
    status: string = "active"
    balance: money
    active: bool = true
}

let customer: Customer = Customer { name: "Ana", balance: 1000 kz }
let label: string = customer.name
let email: string? = customer.email
let status: string = customer.status
```

Object literal fields are comma-separated and must match the declared model
fields. The checker rejects unknown fields, duplicated fields, missing required
fields, and values whose type does not match the model field type.

Model fields may declare simple defaults with `=`:

```nexus
model InvoiceDraft {
    status: string = "draft"
    paid: bool = false
    currency: string = "AOA"
    discount: money? = nil
}
```

When an object literal omits a field with a default, the runtime fills that
field with the declared default. Defaults in this baseline are static values:
literals, `nil`, or array literals. Calls, identifiers, field access and other
dynamic expressions are rejected for model field defaults in this phase.

Model fields may declare simple `unique`, `index`, `min`, and `max`
constraints:

```nexus
model Customer {
    email: string unique
    nif: string unique index
    status: string index = "active"
    birthday: date? index
    name: string min 2 max 80
    balance: money min 1000 kz max 500000 kz
    joined_at: date? min "2026-01-01" max "2026-12-31"
}
```

`unique` is supported for `string`, `int`, `float`, `bool`, `money`, `date` and
optional forms of those types. In this baseline, `unique` is enforced by
`Model::create()` and `Model::update()` against JSON storage. If another stored
record already has the same value for a unique field, the HTTP response is
`409`. The first implementation treats `null` as a value for uniqueness.

`index` is supported for the same scalar field types and optional scalar field
types as `unique`. In this baseline, `index` is declarative metadata for
semantic contracts, generated formatting/playground docs, and OpenAPI
schemas. JSON storage does not build physical indexes yet.

`min` and `max` are supported for `string`, `int`, `float`, `money`, `date`
and optional forms of those types. For `string`, the values are integer length
bounds. For `int`/`float`, the values are numeric bounds. For `money`, the
values are money literals and `min`/`max` must use the same currency. For
`date`, the values are ISO-style string literals compared lexicographically.
Static defaults are checked by the semantic checker, and `Model::create()` and
`Model::update()` reject invalid request bodies with `400`. Optional
`nil`/`null` values skip `min`/`max` validation.

Types can be marked optional with `?`:

```nexus
let phone: string? = nil
let discount: money? = 1000 kz
```

`nil` is only assignable to optional types. Optional model fields may be omitted
from object literals; omitted optional fields are represented as `nil` at
runtime and `null` in HTTP JSON responses.

Fields can be read with postfix access syntax:

```nexus
print(customer.name)
print(customer.balance)
```

Field access is type checked against the declared model. The checker rejects
field access on non-model values and rejects fields that the model does not
declare.

Routes can return model instances as JSON:

```nexus
route GET /customers/:name {
    return Customer { name: name, balance: 1000 kz }
}

route GET /customers/:name/label {
    return Customer { name: name, balance: 1000 kz }.name
}

route GET /customers/:name/email {
    return Customer { name: name, balance: 1000 kz }.email
}

route GET /customers/:name/status {
    return Customer { name: name, balance: 1000 kz }.status
}
```

Routes can declare typed query params after the path with `?(...)`:

```nexus
route GET /customers ?(limit: int, offset: int) {
    return Customer::all(limit, offset)
}

route GET /customers/page ?(limit: int = 20, offset: int = 0) {
    return Customer::all(limit, offset)
}

route GET /customers/search ?(status: string) {
    return Customer::where("status", status)
}

route GET /customers/maybe ?(status: string?) {
    return status
}

route GET /customers/balance ?(min_balance: money) {
    return Customer::where_compare("balance", ">=", min_balance)
}

route GET /customers/tags ?(tags: [string]) {
    return tags
}
```

Path params such as `:id` are still available as `string`. Query params are
available in the route body with their declared type. The supported query param
types are `string`, `int`, `float`, `bool`, `money`, `date`, and optional forms
of those types such as `string?` and `money?`. Query params also support simple
arrays of those scalar types, such as `[string]`, `[int]`, `[money]`, and
optional array params such as `[string]?`. Arrays use comma-separated query
values, for example `/customers/tags?tags=vip,active`; an empty provided value
such as `tags=` is parsed as `[]`. Nested arrays and arrays of optional items
are not supported in this baseline. Query params without `?` or `=` are
required. Missing required query params return `400`; invalid provided values
also return `400`. `money` query params use `amount:currency`, for example
`/customers/balance?min_balance=1000:kz`; the runtime also accepts the
literal-like encoded form `1000+kz`. When a query param has a static default,
for example `limit: int = 20`, `min_balance: money = 1000 kz`, or
`tags: [string] = ["vip"]`, the runtime uses that value when the param is
absent. When an optional query param without a default is absent, the route
variable is `nil`.

Routes can also create model records from a JSON request body:

```nexus
route POST /customers {
    return Customer::create()
}
```

`Model::create()` is only valid in `POST` routes in this baseline. The HTTP
runtime expects a JSON object, validates fields against the declared model,
rejects unknown fields and missing required fields, fills static defaults and
omitted optional fields, persists the record in JSON storage, and returns the
created object with status `201`.

Routes can read one model record from JSON storage with a typed field lookup:

```nexus
route GET /customers/:name {
    return Customer::find("name", name)
}
```

`Model::find("field", value)` is only valid in `GET` routes in this baseline.
The field name must be a string literal that exists on the model, and the value
must be assignable to that field type. The HTTP runtime returns the matched
record as JSON, filling static defaults and omitted optional fields. If no
record matches, the response is `404`.

Routes can filter model records from JSON storage with a typed field lookup:

```nexus
route GET /customers/status/:status {
    return Customer::where("status", status)
}
```

`Model::where("field", value)` is only valid in `GET` routes in this baseline.
The field name must be a string literal that exists on the model, and the value
must be assignable to that field type. The HTTP runtime returns an array of all
matching records as JSON, normalizing each record against the model and filling
static defaults and omitted optional fields in the response. If no records
match, the response is `[]` with status `200`.

Routes can exclude model records with a typed field lookup:

```nexus
route GET /customers/not-status ?(status: string) {
    return Customer::where_not("status", status)
}
```

`Model::where_not("field", value)` is only valid in `GET` routes in this
baseline. The field name must be a string literal that exists on the model, and
the value must be assignable to that field type. The HTTP runtime returns
records whose stored field is different from the provided value. Records where
the selected field is absent in storage are ignored, matching the conservative
behavior of `Model::where(...)`.

Routes can apply a typed filter only when an optional query param is present:

```nexus
route GET /customers/search ?(status: string?) {
    return Customer::where_optional("status", status)
}
```

`Model::where_optional("field", value?)` is only valid in `GET` routes in this
baseline. The value argument must have an optional type such as `string?`, and
its inner type must be assignable to the selected model field. When the value is
present, the HTTP runtime behaves like `Model::where()`. When the value is
`nil`, the filter is skipped and all normalized records are returned.

Routes can filter model records by membership in a typed array:

```nexus
route GET /customers/statuses ?(statuses: [string]) {
    return Customer::where_in("status", statuses)
}
```

`Model::where_in("field", values)` is only valid in `GET` routes in this
baseline. The field name must be a string literal that exists on the model, and
`values` must be an array whose item type is assignable to that field type. It
works naturally with array query params such as `?(statuses: [string])`. The
HTTP runtime returns records whose stored field equals any item in the array.
An empty array matches no records and returns `[]`.

Routes can exclude model records by membership in a typed array:

```nexus
route GET /customers/not-statuses ?(statuses: [string]) {
    return Customer::where_not_in("status", statuses)
}
```

`Model::where_not_in("field", values)` is only valid in `GET` routes in this
baseline. The field name must be a string literal that exists on the model, and
`values` must be an array whose item type is assignable to that field type. It
works naturally with array query params such as `?(statuses: [string])`. The
HTTP runtime returns records whose stored field is equal to none of the array
items. An empty array matches every record where the selected field is present.
Records where the selected field is absent in storage are ignored.

Routes can also skip a set-exclusion filter when an optional array query param
is absent:

```nexus
route GET /customers/not-statuses ?(statuses: [string]?) {
    return Customer::where_not_in_optional("status", statuses)
}
```

`Model::where_not_in_optional("field", values?)` is only valid in `GET` routes
in this baseline. The `values?` argument must be an optional array whose item
type is assignable to the selected model field. When it is `nil`, the filter is
skipped and all normalized records are returned. When it is an array, the HTTP
runtime behaves like `Model::where_not_in()`. A present empty array matches
every record where the selected field is present.

Routes can also skip an inclusion filter when an optional array query param is
absent:

```nexus
route GET /customers/statuses ?(statuses: [string]?) {
    return Customer::where_in_optional("status", statuses)
}
```

`Model::where_in_optional("field", values?)` is only valid in `GET` routes in
this baseline. The `values?` argument must be an optional array whose item type
is assignable to the selected model field. When it is `nil`, the filter is
skipped and all normalized records are returned. When it is an array, the HTTP
runtime behaves like `Model::where_in()`. A present empty array still matches
no records.

Routes can filter model records with a typed comparison:

```nexus
route GET /customers/search ?(min_balance: float) {
    return Customer::where_compare("balance", ">=", min_balance)
}

route GET /invoices/due ?(after: date) {
    return Invoice::where_compare("due", ">", after)
}
```

`Model::where_compare("field", "op", value)` is only valid in `GET` routes in
this baseline. The field name and operator must be string literals. Supported
operators are `"=="`, `"!="`, `">"`, `">="`, `"<"` and `"<="`. Equality
comparisons support scalar model fields: `string`, `int`, `float`, `bool`,
`money` and `date`, including optional forms of those fields. Ordering
comparisons support `string`, `int`, `float`, `money` and `date`, including
optional fields when the comparison value is not `nil`. Date comparisons use
the stored ISO-like date string order.

Routes can filter model records with an inclusive typed range:

```nexus
route GET /customers/range ?(min_balance: float, max_balance: float) {
    return Customer::where_between("balance", min_balance, max_balance)
}

route GET /invoices/range ?(start: date, end: date) {
    return Invoice::where_between("due", start, end)
}
```

`Model::where_between("field", min, max)` is only valid in `GET` routes in
this baseline. The field name must be a string literal. The selected model
field must support ordering: `string`, `int`, `float`, `money` or `date`,
including optional forms of those fields. The `min` and `max` values must be
concrete values assignable to the field type; optional bounds and `nil` are
rejected by the checker. The HTTP runtime applies an inclusive range:
`field >= min && field <= max`. If the stored field is `nil`, the record does
not match. Date ranges use the stored ISO-like date string order.

Routes can filter model records by multiple typed field lookups:

```nexus
route GET /customers/search ?(status: string, tenant: string) {
    return Customer::where_all("status", status, "tenant", tenant)
}
```

`Model::where_all("field", value, "other", other)` is only valid in `GET`
routes in this baseline and requires at least two `field`/`value` pairs. Each
field name must be a string literal that exists on the model, and each value
must be assignable to its field type. The HTTP runtime returns records matching
all filters, normalized with defaults and omitted optional fields.

Routes can filter model records by any of multiple typed field lookups:

```nexus
route GET /customers/search ?(status: string, tenant: string) {
    return Customer::where_any("status", status, "tenant", tenant)
}
```

`Model::where_any("field", value, "other", other)` is only valid in `GET`
routes in this baseline and requires at least two `field`/`value` pairs. Each
field name must be a string literal that exists on the model, and each value
must be assignable to its field type. The HTTP runtime returns records matching
at least one filter, normalized with defaults and omitted optional fields. A
record matching more than one filter appears once.

List routes can also apply simple pagination with `limit` and `offset`:

```nexus
route GET /customers/page {
    return Customer::all(20, 0)
}

route GET /customers/status/:status/page {
    return Customer::where("status", status, 20, 0)
}

route GET /customers/search ?(min_balance: float, limit: int, offset: int) {
    return Customer::where_compare("balance", ">=", min_balance, limit, offset)
}

route GET /customers/range ?(min_balance: float, max_balance: float, limit: int, offset: int) {
    return Customer::where_between("balance", min_balance, max_balance, limit, offset)
}
```

Paginated `Model::all(limit, offset)` and
`Model::where("field", value, limit, offset)` are only valid in `GET` routes.
`Model::where_optional(...)`, `Model::where_in(...)`,
`Model::where_not_in(...)`, `Model::where_not_in_optional(...)`,
`Model::where_in_optional(...)`, `Model::where_compare(...)` and
`Model::where_between(...)` support the same trailing `limit, offset` pair.
Both pagination arguments must be `int`; `limit` must be greater than zero and
`offset` cannot be negative. Paginated list responses are arrays of normalized
model records. If the slice has no records, the response is `[]` with status
`200`.

When an HTTP client also needs the total number of matching records before the
slice, use the explicit paged forms:

```nexus
route GET /customers/page ?(limit: int, offset: int) {
    return Customer::page("name", "asc", limit, offset)
}

route GET /customers/status ?(status: string, limit: int, offset: int) {
    return Customer::where_page("status", status, "name", "asc", limit, offset)
}

route GET /customers/not-status ?(status: string, limit: int, offset: int) {
    return Customer::where_not_page("status", status, "name", "asc", limit, offset)
}
```

`Model::page(limit, offset)` and `Model::page("field", "asc"|"desc", limit,
offset)` list all records and return an envelope:
`{ "total": n, "items": [...] }`. `Model::where_page("field", value, limit,
offset)` and `Model::where_page("field", value, "order_field", "asc"|"desc",
limit, offset)` apply one typed equality filter first.
`Model::where_not_page("field", value, limit, offset)` and its ordered variant
apply one typed exclusion filter first. `Model::where_not_in_page("field",
values, limit, offset)` and its ordered variant apply a typed set-exclusion
filter first. `Model::where_not_in_optional_page("field", values?, limit,
offset)` does the same when the optional array is present, and skips the filter
when it is `nil`. The `total` value is computed after filtering and before
ordering/pagination. Existing list forms such as `Model::all(...)`,
`Model::where(...)`, `Model::where_not(...)`, `Model::where_not_in(...)` and
`Model::where_not_in_optional(...)` keep returning arrays.

Advanced filters also have explicit total-count page forms:

```nexus
route GET /customers/statuses ?(statuses: [string], limit: int, offset: int) {
    return Customer::where_in_page("status", statuses, "name", "asc", limit, offset)
}

route GET /customers/not-statuses ?(statuses: [string], limit: int, offset: int) {
    return Customer::where_not_in_page("status", statuses, "name", "asc", limit, offset)
}

route GET /customers/not-statuses/optional ?(statuses: [string]?, limit: int, offset: int) {
    return Customer::where_not_in_optional_page("status", statuses, "name", "asc", limit, offset)
}

route GET /customers/statuses/optional ?(statuses: [string]?, limit: int, offset: int) {
    return Customer::where_in_optional_page("status", statuses, "name", "asc", limit, offset)
}

route GET /customers/search ?(min_balance: float, limit: int, offset: int) {
    return Customer::where_compare_page("balance", ">=", min_balance, "name", "asc", limit, offset)
}

route GET /customers/range ?(min_balance: float, max_balance: float, limit: int, offset: int) {
    return Customer::where_between_page("balance", min_balance, max_balance, "name", "asc", limit, offset)
}

route GET /customers/all ?(status: string, tenant: string, limit: int, offset: int) {
    return Customer::where_all_page("status", status, "tenant", tenant, "name", "asc", limit, offset)
}

route GET /customers/any ?(status: string, tenant: string, limit: int, offset: int) {
    return Customer::where_any_page("status", status, "tenant", tenant, "name", "asc", limit, offset)
}
```

The page variants are `Model::where_not_page(...)`,
`Model::where_optional_page(...)`,
`Model::where_in_page(...)`, `Model::where_not_in_page(...)`,
`Model::where_not_in_optional_page(...)`, `Model::where_in_optional_page(...)`,
`Model::where_compare_page(...)`, `Model::where_text_page(...)`,
`Model::where_between_page(...)`, `Model::where_all_page(...)`, and
`Model::where_any_page(...)`. They use the same typed filter rules as their
array-returning counterparts, but require a trailing `limit, offset` pair,
optionally preceded by ordering. They return
`{ "total": n, "items": [...] }`; the non-`_page` forms keep returning arrays.

List routes can also apply simple ordering by model field:

```nexus
route GET /customers/order {
    return Customer::all("name", "asc")
}

route GET /customers/page {
    return Customer::all("name", "asc", 20, 0)
}

route GET /customers/status/:status/order {
    return Customer::where("status", status, "name", "desc")
}

route GET /customers/search ?(status: string?) {
    return Customer::where_optional("status", status, "name", "asc")
}

route GET /customers/statuses ?(statuses: [string]) {
    return Customer::where_in("status", statuses, "name", "asc")
}

route GET /customers/not-statuses ?(statuses: [string]) {
    return Customer::where_not_in("status", statuses, "name", "asc")
}

route GET /customers/not-statuses/optional ?(statuses: [string]?) {
    return Customer::where_not_in_optional("status", statuses, "name", "asc")
}

route GET /customers/statuses/optional ?(statuses: [string]?) {
    return Customer::where_in_optional("status", statuses, "name", "asc")
}

route GET /customers/search ?(min_balance: float) {
    return Customer::where_compare("balance", ">=", min_balance, "name", "asc")
}

route GET /customers/range ?(min_balance: float, max_balance: float) {
    return Customer::where_between("balance", min_balance, max_balance, "name", "asc")
}

route GET /customers/status/:status/page {
    return Customer::where("status", status, "name", "desc", 20, 0)
}

route GET /customers/search ?(status: string, tenant: string, limit: int, offset: int) {
    return Customer::where_all("status", status, "tenant", tenant, "name", "asc", limit, offset)
}

route GET /customers/any ?(status: string, tenant: string, limit: int, offset: int) {
    return Customer::where_any("status", status, "tenant", tenant, "name", "asc", limit, offset)
}
```

Ordered list forms are only valid in `GET` routes. The order field must be a
string literal that exists on the model, and the direction must be `"asc"` or
`"desc"`. Ordering supports scalar model fields and optional scalar fields:
`string`, `int`, `float`, `bool`, `money` and `date`. When ordering and
pagination are both present, the HTTP runtime filters first, orders the
matching records, and then applies `limit`/`offset`. Composite `where_all`
routes support pagination as a trailing `limit, offset` pair, or ordering plus
pagination as a trailing `"field", "asc"|"desc", limit, offset` group.
OR `where_any` routes support the same trailing pagination and ordering plus
pagination group.
Inclusion `where_in` routes support the same trailing pagination, ordering, and
ordering-plus-pagination shapes after the array value.
Set exclusion `where_not_in` routes support those same shapes after the array
value.
Optional set exclusion `where_not_in_optional` routes support those same shapes
after the optional array value.
Optional inclusion `where_in_optional` routes support those same shapes after
the optional array value.
Comparison `where_compare` routes support the same trailing pagination,
ordering, and ordering-plus-pagination shapes after the comparison value.
Range `where_between` routes support the same trailing pagination, ordering,
and ordering-plus-pagination shapes after the `max` value.

Routes can filter model records with simple typed text operators:

```nexus
route GET /customers/search ?(term: string) {
    return Customer::where_text("name", "contains", term)
}

route GET /customers/email_prefix ?(term: string) {
    return Customer::where_text("email", "starts_with", term)
}

route GET /customers/email_domain ?(term: string) {
    return Customer::where_text("email", "ends_with", term)
}

route GET /customers/search_ci ?(term: string) {
    return Customer::where_text("name", "icontains", term)
}
```

`Model::where_text("field", "op", value)` is only valid in `GET` routes in
this baseline. The field name and operator must be string literals. Supported
operators are `"contains"`, `"starts_with"`, `"ends_with"`, `"icontains"`,
`"istarts_with"` and `"iends_with"`. The selected model field must be `string`
or `string?`, and the value must be `string` or `string?`. Operators prefixed
with `i` perform simple case-insensitive matching by lowercasing both sides;
this is not locale-aware collation. If the stored field value or the provided
value is `nil`, the record does not match. Text filters support the same
trailing pagination, ordering, and ordering-plus-pagination shapes after the
text value. `Model::where_text_page(...)` accepts the same operators and
returns `{ "total": n, "items": [...] }` after filtering and before the slice.

Routes can update one model record in JSON storage with the same typed lookup
shape and a JSON request body:

```nexus
route PUT /customers/:name {
    return Customer::update("name", name)
}
```

`Model::update("field", value)` is only valid in `PUT` routes in this
baseline. The lookup field follows the same rules as `Model::find()`. The HTTP
runtime expects a JSON object, validates it against the model, rejects unknown
fields and missing required fields, fills static defaults and omitted optional
fields, replaces the first matching stored record, and returns the updated
object. If no record matches, the response is `404`.

Routes can delete one model record from JSON storage with a typed lookup:

```nexus
route DELETE /customers/:name {
    return Customer::delete("name", name)
}
```

`Model::delete("field", value)` is only valid in `DELETE` routes in this
baseline. The lookup field follows the same rules as `Model::find()`. The HTTP
runtime removes the first matching stored record and returns the deleted record
as JSON, filling static defaults and omitted optional fields in the response. If
no record matches, the response is `404` and the storage file is left unchanged.

`nexus serve` exposes `/openapi.json`. The generated OpenAPI document derives
response schemas from route returns:

- model instances use `$ref` entries under `components.schemas`;
- model names `NexusError` or starting with `NexusPage_`/`NexusList_` are
  reserved for internal OpenAPI components and are rejected by the checker;
- every route operation includes a stable `operationId` generated from the
  HTTP method and path; route params such as `:id` become `by_id`, so
  `GET /employees/:id` becomes `get_employees_by_id`; if two routes normalize
  to the same id, later ids receive a numeric suffix such as `_2`;
- every route operation includes a stable OpenAPI `tags` entry derived from
  the first static path segment, and the document includes a deduplicated
  top-level `tags` list in route declaration order; routes with no static path
  segment use the fallback tag `routes`;
- route operations with the same OpenAPI path are grouped under one Path Item,
  so `GET /customers/:name` and `PUT /customers/:name` share a single
  `/customers/{name}` entry with `get` and `put` methods;
- duplicate route declarations with the same HTTP method and path are rejected
  by the checker; different methods may still share the same path;
- error responses use the reusable `components.schemas.NexusError` schema with
  `{ "error": string }`;
- success responses `200`/`201` for models, `NexusList_<Model>` arrays and
  `NexusPage_<Model>` page envelopes use reusable `components.responses`
  entries;
- the OpenAPI 1.0 contract has a compact golden regression test covering
  paths, `operationId`, tags, reusable params, request bodies, schemas, success
  responses and error responses together;
- the generated OpenAPI document is validated as parseable JSON in a compact
  QA test;
- the generated OpenAPI document is validated for minimum root structure
  (`openapi`, `info`, `tags`, `paths`, `components`) and component buckets
  (`schemas`, `parameters`, `requestBodies`, `responses`);
- generated OpenAPI `paths` are validated so each Path Item has valid HTTP
  operations, and each operation includes `summary`, `operationId`, `tags`,
  `parameters` and `responses`;
- generated OpenAPI `$ref` values are validated so internal references under
  `#/components/...` point to existing `schemas`, `parameters`,
  `requestBodies` or `responses` entries;
- generated OpenAPI operations are validated so `operationId` values are
  globally unique and every operation tag is declared in the top-level `tags`
  list;
- generated reusable OpenAPI components are validated for minimum structure:
  schemas must be non-empty objects, parameters must include `name`, `in`,
  `required` and `schema`, request bodies must expose
  `content.application/json.schema`, and responses must include a
  `description` plus JSON schema content when they carry a body;
- generated model schemas are validated semantically against NexusLang model
  declarations, including `type`, `properties`, `required`, optional
  `nullable` fields, static `default` values, `min`/`max` markers, and
  `unique`/`index` extension flags;
- generated OpenAPI operations are validated against the real route contract:
  request bodies appear only on routes backed by `Model::create()` or
  `Model::update()` and point to the matching `components.requestBodies`
  entry; responses `200`, `201`, `400`, `404` and `409` match the route
  semantics; success response schemas match the inferred route return type;
- route path params and typed route query params are emitted through reusable
  `components.parameters` entries and referenced from operations with `$ref`;
- optional/defaulted query params are emitted with `required: false`, and
  defaulted query params include the OpenAPI schema `default`;
- routes declaring typed query params document a `400` response for missing
  required params or invalid provided values;
- `money` query params are emitted as strings with `format: nexus-money`;
- array query params are emitted as OpenAPI arrays with `style: form` and
  `explode: false`;
- `Model::all()` returns an array of model refs, exposed through reusable
  `NexusList_<Model>` schemas;
- `Model::create()` uses a reusable `components.requestBodies` entry, returns
  a model ref, and documents a `400` response for invalid request bodies;
- `Model::find()` returns a model ref and documents a `404` response;
- `Model::where()`, `Model::where_not()`, `Model::where_in()`,
  `Model::where_not_in()`, `Model::where_not_in_optional()`,
  `Model::where_all()` and `Model::where_any()` return an array of model refs,
  exposed through reusable `NexusList_<Model>` schemas;
- `Model::page()`, `Model::where_page()`, and advanced `*_page` filters return
  an object with `total` and `items`, exposed through reusable
  `NexusPage_<Model>` schemas;
- routes using `Model::where_optional()`/`Model::where_optional_page()`/
  `Model::where_not_in_optional()`/
  `Model::where_not_in_optional_page()`/
  `Model::where_in_optional()`/`Model::where_in_optional_page()` include
  `x-nexus-optional-filters: true`;
- routes using `Model::where_in()`/`Model::where_in_page()`/
  `Model::where_not_in()`/`Model::where_not_in_page()`/
  `Model::where_not_in_optional()`/
  `Model::where_not_in_optional_page()`/
  `Model::where_in_optional()`/`Model::where_in_optional_page()` include
  `x-nexus-in-filters: true`;
- routes using `Model::where_compare()`/`Model::where_compare_page()` include
  `x-nexus-comparison-filters: true`;
- routes using `Model::where_text()`/`Model::where_text_page()` include
  `x-nexus-text-filters: true`;
- routes using `Model::where_between()`/`Model::where_between_page()` include
  `x-nexus-range-filters: true`;
- routes using `Model::where_all()`/`Model::where_all_page()` include
  `x-nexus-composite-filters: true`;
- routes using `Model::where_any()`/`Model::where_any_page()` include
  `x-nexus-or-filters: true`;
- routes using `Model::where_not()`/`Model::where_not_page()`/
  `Model::where_not_in()`/`Model::where_not_in_page()`/
  `Model::where_not_in_optional()`/
  `Model::where_not_in_optional_page()` include
  `x-nexus-exclusion-filters: true`;
- paginated `Model::all()`/`Model::where()`/`Model::where_optional()`/
  `Model::where_not()`/`Model::where_in()`/`Model::where_not_in()`/
  `Model::where_not_in_optional()`/`Model::where_in_optional()`/
  `Model::where_compare()`/`Model::where_text()`/
  `Model::where_between()`/`Model::where_all()`/`Model::where_any()`/
  `Model::page()`/`Model::where_page()` and advanced `*_page` filter routes include
  `x-nexus-pagination: true`;
- total-count `Model::page()`/`Model::where_page()` and advanced `*_page`
  filter routes include `x-nexus-total-count: true`;
- ordered `Model::all()`/`Model::where()`/`Model::where_not()`/
  `Model::where_optional()`/`Model::where_in()`/`Model::where_not_in()`/
  `Model::where_not_in_optional()`/`Model::where_in_optional()`/
  `Model::where_compare()`/`Model::where_text()`/
  `Model::where_between()`/`Model::where_all()`/`Model::where_any()`/
  `Model::page()`/`Model::where_page()` and advanced `*_page` filter routes include
  `x-nexus-ordering: true`;
- `Model::update()` uses a reusable `components.requestBodies` entry, returns
  a model ref, and documents `400` invalid-body and `404` not-found responses;
- `Model::delete()` returns a model ref and documents a `404` response;
- routes using `Model::create()` or `Model::update()` on models with unique
  fields document a `409` response;
- field access uses the declared model field type;
- optional fields use `nullable: true`;
- fields with static defaults include their JSON `default`;
- fields with `unique` include `x-nexus-unique: true`;
- fields with `index` include `x-nexus-index: true`;
- string fields with `min`/`max` include `minLength`/`maxLength`;
- numeric fields with `min`/`max` include `minimum`/`maximum`;
- `money` and `date` fields with `min`/`max` include `x-nexus-min`/`x-nexus-max`;
- model/body `money` values are represented as objects with `amount` and
  `currency`; query param `money` values use `amount:currency`.

## OpenAPI 1.0 Readiness

OpenAPI 1.0 is considered ready for an internal release candidate with known
risks. The generated contract is stable for the supported HTTP subset:

- routes expose deterministic paths, grouped Path Items, stable `operationId`
  values and stable resource tags;
- model schemas, list schemas, page schemas, request bodies, success responses
  and error responses are reusable components;
- path/query parameters are reusable components, including optional/defaulted
  params, `money` params and simple array params;
- create/update/find/delete/list routes document the expected success and error
  responses for the supported model-backed runtime;
- Nexus-specific capabilities are explicit through `x-nexus-*` extensions.

Release-readiness QA currently covers:

- a compact golden contract snapshot for representative paths, params,
  request bodies, schemas, success responses, error responses, tags and
  `operationId` values;
- JSON parseability of the generated document;
- minimum root, component-bucket, Path Item, operation and reusable component
  structure;
- semantic consistency of representative model schemas against NexusLang model
  declarations;
- internal `$ref` resolution against `components`;
- global uniqueness of `operationId` and consistency between operation tags and
  top-level `tags`;
- consistency between operations, reusable request/response components and the
  route return contract;
- the aggregate coherence suite
  `openapi_1_0_contract_coherence_suite_runs_core_validations`, which runs the
  core OpenAPI QA checks as one readiness gate.

Remaining risks before an external 1.0 release:

- the OpenAPI document is structurally tested by NexusLang tests, but is not
  yet checked by an independent OpenAPI 3.0 validator in CI;
- no generated-client smoke test has confirmed compatibility with common SDK
  generators;
- `x-nexus-*` extension semantics are documented here, but external tooling may
  ignore them;
- JSON storage remains the first runtime backend; `index` is declarative only
  and there is no SQLite/transactional release gate yet.

Release checklist for OpenAPI 1.0:

- run `cargo fmt`;
- run the focused OpenAPI tests, especially `cargo test openapi_generated -- --nocapture`
  and `cargo test openapi_1_0_contract_snapshot_covers_reusable_components -- --nocapture`;
- run `cargo check` and full `cargo test`;
- validate a served `/openapi.json` with an external OpenAPI 3.0 validator;
- run one generated-client or SDK smoke test against the representative
  contract;
- publish release notes that call out the supported HTTP subset and the
  remaining JSON-storage/index limitations.

## ERP Declarations

Routes must start with `/` after the HTTP method:

```nexus
route GET /employees/:id {
    return "employee " + id
}
```

Static route segments may use hyphens:

```nexus
route GET /customers/search-not-in {
    return "ok"
}
```

When a static route and a parameterized route both match the same request path,
the HTTP runtime chooses the more specific static route. For example,
`/customers/search-not-in` takes precedence over `/customers/:name`.

Workflows contain only `step` declarations inside the workflow block. A step may
be a named marker or contain an executable body:

```nexus
workflow Payroll {
    step collect_timesheets
    step close_month {
        print("Payroll closed")
    }
}
```

Invoices support named fields and structured items:

```nexus
invoice {
    customer: "Cliente"
    currency: "AOA"
    tax: 14
    item "Setup ERP" qty 1 price 250000 kz
}
```

## OpenAPI 1.0 External Validation

The OpenAPI 1.0 contract can be validated externally:

1. **Via Python validator**: `scripts/validate-openapi.py` verifies estrutura
   OpenAPI 3.0, `$ref` resolution, e consistência mínima de operations.
2. **Via script completo**: `scripts/validate-openapi.sh` compila, serve,
   recolhe `/openapi.json`, valida com Python + smoke tests.
3. **Via smoke test**: `scripts/smoke-test.sh` corre CRUD, filtros e erros
   contra o servidor real.

O ficheiro `examples/openapi_qa.nx` serve como representante do contrato
OpenAPI 1.0 para validação externa e CI.

### Comandos de Validação

```bash
# Validação completa
bash scripts/validate-openapi.sh

# Apenas smoke tests
bash scripts/smoke-test.sh

# Apenas validação estrutural (com servidor a correr)
curl http://127.0.0.1:5050/openapi.json | python3 scripts/validate-openapi.py
```

## Compatibility Notes

Before Phase 7.1, the parser accepted some ambiguous input by accident:

- parameters or arguments without commas;
- array items without commas;
- `route` declarations without a leading `/`;
- unknown identifiers inside `workflow` blocks.

Those forms are now syntax errors with parser diagnostics. The formatter output
already matched the stricter 1.0 baseline.
