use crate::ast::{Expr, HttpMethod, Type};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelStaticOperation {
    All,
    Page,
    Create,
    Find,
    Where,
    WherePage,
    WhereNot,
    WhereNotPage,
    WhereNotIn,
    WhereNotInPage,
    WhereNotInOptional,
    WhereNotInOptionalPage,
    WhereOptional,
    WhereOptionalPage,
    WhereIn,
    WhereInPage,
    WhereInOptional,
    WhereInOptionalPage,
    WhereCompare,
    WhereComparePage,
    WhereText,
    WhereTextPage,
    WhereBetween,
    WhereBetweenPage,
    WhereAll,
    WhereAllPage,
    WhereAny,
    WhereAnyPage,
    Update,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteMethodRequirement {
    Get,
    Post,
    Put,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelOperationArgumentShape {
    All,
    Page,
    Create,
    Lookup,
    Where,
    WherePage,
    AdvancedWhere,
    AdvancedWherePage,
    CompositeWhere,
    CompositeWherePage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelOperationReturnKind {
    Model,
    List,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelOperationCheckerValidation {
    All,
    Page,
    Create,
    Lookup,
    Where,
    WherePage,
    WhereNot,
    WhereNotPage,
    WhereOptional,
    WhereOptionalPage,
    WhereIn,
    WhereInPage,
    WhereNotIn,
    WhereNotInPage,
    WhereNotInOptional,
    WhereNotInOptionalPage,
    WhereInOptional,
    WhereInOptionalPage,
    WhereCompare,
    WhereComparePage,
    WhereText,
    WhereTextPage,
    WhereBetween,
    WhereBetweenPage,
    WhereAll,
    WhereAllPage,
    WhereAny,
    WhereAnyPage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelOperationStorageCategory {
    ListRecords,
    PageRecords,
    CreateRecord,
    FindRecord,
    EqualityFilter { negated: bool },
    InclusionFilter { negated: bool },
    OptionalEqualityFilter,
    OptionalInclusionFilter,
    OptionalExclusionFilter,
    ComparisonFilter,
    TextFilter,
    RangeFilter,
    CompositeFilter { any: bool },
    UpdateRecord,
    DeleteRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelOperationOpenApiFeature {
    CompositeFilters,
    OrFilters,
    ExclusionFilters,
    OptionalFilters,
    InFilters,
    ComparisonFilters,
    TextFilters,
    RangeFilters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelOperationOpenApiFlags(u32);

impl ModelOperationOpenApiFlags {
    pub const NONE: Self = Self(0);
    pub const REQUEST_BODY: Self = Self(1 << 0);
    pub const CREATED_STATUS: Self = Self(1 << 1);
    pub const NOT_FOUND_RESPONSE: Self = Self(1 << 2);
    pub const CONFLICT_RESPONSE: Self = Self(1 << 3);
    pub const PAGINATION: Self = Self(1 << 4);
    pub const TOTAL_COUNT: Self = Self(1 << 5);
    pub const ORDERING: Self = Self(1 << 6);
    pub const COMPOSITE_FILTERS: Self = Self(1 << 7);
    pub const OR_FILTERS: Self = Self(1 << 8);
    pub const EXCLUSION_FILTERS: Self = Self(1 << 9);
    pub const OPTIONAL_FILTERS: Self = Self(1 << 10);
    pub const IN_FILTERS: Self = Self(1 << 11);
    pub const COMPARISON_FILTERS: Self = Self(1 << 12);
    pub const TEXT_FILTERS: Self = Self(1 << 13);
    pub const RANGE_FILTERS: Self = Self(1 << 14);

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ModelStaticOperationDescriptor {
    pub operation: ModelStaticOperation,
    pub method_name: &'static str,
    pub argument_shape: ModelOperationArgumentShape,
    pub route_method: Option<RouteMethodRequirement>,
    pub return_kind: ModelOperationReturnKind,
    pub checker_validation: ModelOperationCheckerValidation,
    pub storage_category: ModelOperationStorageCategory,
    pub openapi_flags: ModelOperationOpenApiFlags,
}

#[derive(Debug, Clone, Copy)]
pub struct CheckedModelOperationArgs<'a> {
    pub raw: &'a [Expr],
    pub kind: CheckedModelOperationArgsKind<'a>,
    pub ordering: Option<CheckedModelOrderingArgs<'a>>,
    pub pagination: Option<CheckedModelPaginationArgs<'a>>,
    pub page_response: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum CheckedModelOperationArgsKind<'a> {
    Empty,
    RequestBody,
    List,
    Lookup {
        field: &'a Expr,
        value: &'a Expr,
    },
    Filter {
        field: &'a Expr,
        value: &'a Expr,
    },
    AdvancedFilter {
        field: &'a Expr,
        operator: &'a Expr,
        value: &'a Expr,
    },
    RangeFilter {
        field: &'a Expr,
        min: &'a Expr,
        max: &'a Expr,
    },
    CompositeFilter {
        filters: &'a [Expr],
    },
}

#[derive(Debug, Clone, Copy)]
pub struct CheckedModelOrderingArgs<'a> {
    pub field: &'a Expr,
    pub direction: &'a Expr,
}

#[derive(Debug, Clone, Copy)]
pub struct CheckedModelPaginationArgs<'a> {
    pub limit: &'a Expr,
    pub offset: &'a Expr,
}

impl<'a> CheckedModelOperationArgs<'a> {
    pub fn has_ordering(self) -> bool {
        self.ordering.is_some()
    }

    pub fn has_pagination(self) -> bool {
        self.pagination.is_some()
    }

    pub fn has_page_response(self) -> bool {
        self.page_response
    }

    pub fn lookup(self) -> Option<(&'a Expr, &'a Expr)> {
        match self.kind {
            CheckedModelOperationArgsKind::Lookup { field, value }
            | CheckedModelOperationArgsKind::Filter { field, value } => Some((field, value)),
            _ => None,
        }
    }

    pub fn advanced_filter(self) -> Option<(&'a Expr, &'a Expr, &'a Expr)> {
        match self.kind {
            CheckedModelOperationArgsKind::AdvancedFilter {
                field,
                operator,
                value,
            } => Some((field, operator, value)),
            _ => None,
        }
    }

    pub fn range_filter(self) -> Option<(&'a Expr, &'a Expr, &'a Expr)> {
        match self.kind {
            CheckedModelOperationArgsKind::RangeFilter { field, min, max } => {
                Some((field, min, max))
            }
            _ => None,
        }
    }

    pub fn composite_filter_args(self) -> Option<&'a [Expr]> {
        match self.kind {
            CheckedModelOperationArgsKind::CompositeFilter { filters } => Some(filters),
            _ => None,
        }
    }
}

impl RouteMethodRequirement {
    pub fn name(self) -> &'static str {
        match self {
            RouteMethodRequirement::Get => "GET",
            RouteMethodRequirement::Post => "POST",
            RouteMethodRequirement::Put => "PUT",
            RouteMethodRequirement::Delete => "DELETE",
        }
    }

    pub fn matches(self, method: &HttpMethod) -> bool {
        matches!(
            (self, method),
            (RouteMethodRequirement::Get, HttpMethod::Get)
                | (RouteMethodRequirement::Post, HttpMethod::Post)
                | (RouteMethodRequirement::Put, HttpMethod::Put)
                | (RouteMethodRequirement::Delete, HttpMethod::Delete)
        )
    }
}

impl ModelOperationOpenApiFeature {
    const fn flag(self) -> ModelOperationOpenApiFlags {
        match self {
            ModelOperationOpenApiFeature::CompositeFilters => {
                ModelOperationOpenApiFlags::COMPOSITE_FILTERS
            }
            ModelOperationOpenApiFeature::OrFilters => ModelOperationOpenApiFlags::OR_FILTERS,
            ModelOperationOpenApiFeature::ExclusionFilters => {
                ModelOperationOpenApiFlags::EXCLUSION_FILTERS
            }
            ModelOperationOpenApiFeature::OptionalFilters => {
                ModelOperationOpenApiFlags::OPTIONAL_FILTERS
            }
            ModelOperationOpenApiFeature::InFilters => ModelOperationOpenApiFlags::IN_FILTERS,
            ModelOperationOpenApiFeature::ComparisonFilters => {
                ModelOperationOpenApiFlags::COMPARISON_FILTERS
            }
            ModelOperationOpenApiFeature::TextFilters => ModelOperationOpenApiFlags::TEXT_FILTERS,
            ModelOperationOpenApiFeature::RangeFilters => ModelOperationOpenApiFlags::RANGE_FILTERS,
        }
    }
}

impl ModelStaticOperation {
    pub const ALL: [Self; 30] = [
        Self::All,
        Self::Page,
        Self::Create,
        Self::Find,
        Self::Where,
        Self::WherePage,
        Self::WhereNot,
        Self::WhereNotPage,
        Self::WhereNotIn,
        Self::WhereNotInPage,
        Self::WhereNotInOptional,
        Self::WhereNotInOptionalPage,
        Self::WhereOptional,
        Self::WhereOptionalPage,
        Self::WhereIn,
        Self::WhereInPage,
        Self::WhereInOptional,
        Self::WhereInOptionalPage,
        Self::WhereCompare,
        Self::WhereComparePage,
        Self::WhereText,
        Self::WhereTextPage,
        Self::WhereBetween,
        Self::WhereBetweenPage,
        Self::WhereAll,
        Self::WhereAllPage,
        Self::WhereAny,
        Self::WhereAnyPage,
        Self::Update,
        Self::Delete,
    ];

    pub fn from_method(method: &str) -> Option<Self> {
        MODEL_STATIC_OPERATION_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.method_name == method)
            .map(|descriptor| descriptor.operation)
    }

    pub fn descriptor(self) -> &'static ModelStaticOperationDescriptor {
        MODEL_STATIC_OPERATION_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.operation == self)
            .expect("ModelStaticOperation descriptor missing")
    }

    pub fn method_name(self) -> &'static str {
        self.descriptor().method_name
    }

    pub fn checker_validation(self) -> ModelOperationCheckerValidation {
        self.descriptor().checker_validation
    }

    pub fn storage_category(self) -> ModelOperationStorageCategory {
        self.descriptor().storage_category
    }

    pub fn return_type(self, model: &str) -> Type {
        match self.descriptor().return_kind {
            ModelOperationReturnKind::Model => Type::Model(model.to_string()),
            ModelOperationReturnKind::List => Type::Array(Box::new(Type::Model(model.to_string()))),
        }
    }

    pub fn checked_args<'a>(self, args: &'a [Expr]) -> Option<CheckedModelOperationArgs<'a>> {
        let (kind, ordering, pagination) = match self.descriptor().argument_shape {
            ModelOperationArgumentShape::All => {
                let (ordering, pagination) = list_suffix(args, 0, false)?;
                (CheckedModelOperationArgsKind::List, ordering, pagination)
            }
            ModelOperationArgumentShape::Page => {
                let (ordering, pagination) = list_suffix(args, 0, true)?;
                (CheckedModelOperationArgsKind::List, ordering, pagination)
            }
            ModelOperationArgumentShape::Create => {
                if !args.is_empty() {
                    return None;
                }
                (CheckedModelOperationArgsKind::RequestBody, None, None)
            }
            ModelOperationArgumentShape::Lookup => {
                let [field, value] = args else {
                    return None;
                };
                (
                    CheckedModelOperationArgsKind::Lookup { field, value },
                    None,
                    None,
                )
            }
            ModelOperationArgumentShape::Where => {
                let [field, value, ..] = args else {
                    return None;
                };
                let (ordering, pagination) = list_suffix(args, 2, false)?;
                (
                    CheckedModelOperationArgsKind::Filter { field, value },
                    ordering,
                    pagination,
                )
            }
            ModelOperationArgumentShape::WherePage => {
                let [field, value, ..] = args else {
                    return None;
                };
                let (ordering, pagination) = list_suffix(args, 2, true)?;
                (
                    CheckedModelOperationArgsKind::Filter { field, value },
                    ordering,
                    pagination,
                )
            }
            ModelOperationArgumentShape::AdvancedWhere => {
                let [field, second, third, ..] = args else {
                    return None;
                };
                let (ordering, pagination) = list_suffix(args, 3, false)?;
                (
                    advanced_filter_kind(self, field, second, third),
                    ordering,
                    pagination,
                )
            }
            ModelOperationArgumentShape::AdvancedWherePage => {
                let [field, second, third, ..] = args else {
                    return None;
                };
                let (ordering, pagination) = list_suffix(args, 3, true)?;
                (
                    advanced_filter_kind(self, field, second, third),
                    ordering,
                    pagination,
                )
            }
            ModelOperationArgumentShape::CompositeWhere => {
                let filter_arg_count = where_all_filter_arg_count(args)?;
                let (ordering, pagination) = list_suffix(args, filter_arg_count, false)?;
                (
                    CheckedModelOperationArgsKind::CompositeFilter {
                        filters: &args[..filter_arg_count],
                    },
                    ordering,
                    pagination,
                )
            }
            ModelOperationArgumentShape::CompositeWherePage => {
                let filter_arg_count = where_all_page_filter_arg_count(args)?;
                let (ordering, pagination) = list_suffix(args, filter_arg_count, true)?;
                (
                    CheckedModelOperationArgsKind::CompositeFilter {
                        filters: &args[..filter_arg_count],
                    },
                    ordering,
                    pagination,
                )
            }
        };

        Some(CheckedModelOperationArgs {
            raw: args,
            kind,
            ordering,
            pagination,
            page_response: self.is_page_response_operation(),
        })
    }

    pub fn required_route_method(self, arg_count: usize) -> Option<RouteMethodRequirement> {
        if matches!(self, ModelStaticOperation::All) && arg_count == 0 {
            None
        } else {
            self.descriptor().route_method
        }
    }

    pub fn route_method_error(self, model: &str, arg_count: usize) -> Option<String> {
        let required = self.required_route_method(arg_count)?;
        let call = if matches!(self, ModelStaticOperation::All) {
            "all(limit, offset)".to_string()
        } else {
            format!("{}()", self.method_name())
        };
        Some(format!(
            "{}::{} so pode ser usado em route {}",
            model,
            call,
            required.name()
        ))
    }

    pub fn args_supported(self, args: &[Expr]) -> bool {
        self.checked_args(args).is_some()
    }

    pub fn uses_request_body(self) -> bool {
        self.descriptor()
            .openapi_flags
            .contains(ModelOperationOpenApiFlags::REQUEST_BODY)
    }

    pub fn is_create(self) -> bool {
        matches!(self, ModelStaticOperation::Create)
    }

    pub fn has_not_found_response(self) -> bool {
        self.descriptor()
            .openapi_flags
            .contains(ModelOperationOpenApiFlags::NOT_FOUND_RESPONSE)
    }

    pub fn may_conflict_on_unique_fields(self) -> bool {
        self.descriptor()
            .openapi_flags
            .contains(ModelOperationOpenApiFlags::CONFLICT_RESPONSE)
    }

    pub fn has_page_response(self, args: &[Expr]) -> bool {
        self.checked_args(args)
            .is_some_and(CheckedModelOperationArgs::has_page_response)
    }

    pub fn has_pagination(self, args: &[Expr]) -> bool {
        self.checked_args(args)
            .is_some_and(CheckedModelOperationArgs::has_pagination)
    }

    pub fn has_ordering(self, args: &[Expr]) -> bool {
        self.checked_args(args)
            .is_some_and(CheckedModelOperationArgs::has_ordering)
    }

    pub fn openapi_flags(self, args: &[Expr]) -> ModelOperationOpenApiFlags {
        let Some(args) = self.checked_args(args) else {
            return self.descriptor().openapi_flags;
        };
        self.openapi_flags_for_checked_args(args)
    }

    pub fn openapi_flags_for_checked_args(
        self,
        args: CheckedModelOperationArgs<'_>,
    ) -> ModelOperationOpenApiFlags {
        let mut flags = self.descriptor().openapi_flags;
        if args.has_pagination() {
            flags = flags.with(ModelOperationOpenApiFlags::PAGINATION);
        }
        if args.has_page_response() {
            flags = flags.with(ModelOperationOpenApiFlags::TOTAL_COUNT);
        }
        if args.has_ordering() {
            flags = flags.with(ModelOperationOpenApiFlags::ORDERING);
        }
        flags
    }

    pub fn has_openapi_feature(self, feature: ModelOperationOpenApiFeature, args: &[Expr]) -> bool {
        self.openapi_flags(args).contains(feature.flag())
    }

    pub fn has_openapi_feature_for_checked_args(
        self,
        feature: ModelOperationOpenApiFeature,
        args: CheckedModelOperationArgs<'_>,
    ) -> bool {
        self.openapi_flags_for_checked_args(args)
            .contains(feature.flag())
    }

    fn is_page_response_operation(self) -> bool {
        matches!(
            self,
            ModelStaticOperation::Page
                | ModelStaticOperation::WherePage
                | ModelStaticOperation::WhereNotPage
                | ModelStaticOperation::WhereNotInPage
                | ModelStaticOperation::WhereNotInOptionalPage
                | ModelStaticOperation::WhereOptionalPage
                | ModelStaticOperation::WhereInPage
                | ModelStaticOperation::WhereInOptionalPage
                | ModelStaticOperation::WhereComparePage
                | ModelStaticOperation::WhereTextPage
                | ModelStaticOperation::WhereBetweenPage
                | ModelStaticOperation::WhereAllPage
                | ModelStaticOperation::WhereAnyPage
        )
    }
}

pub const MODEL_STATIC_OPERATION_DESCRIPTORS: &[ModelStaticOperationDescriptor] = &[
    descriptor(
        ModelStaticOperation::All,
        "all",
        ModelOperationArgumentShape::All,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::All,
        ModelOperationStorageCategory::ListRecords,
        ModelOperationOpenApiFlags::NONE,
    ),
    descriptor(
        ModelStaticOperation::Page,
        "page",
        ModelOperationArgumentShape::Page,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::Page,
        ModelOperationStorageCategory::PageRecords,
        ModelOperationOpenApiFlags::NONE,
    ),
    descriptor(
        ModelStaticOperation::Create,
        "create",
        ModelOperationArgumentShape::Create,
        Some(RouteMethodRequirement::Post),
        ModelOperationReturnKind::Model,
        ModelOperationCheckerValidation::Create,
        ModelOperationStorageCategory::CreateRecord,
        ModelOperationOpenApiFlags::REQUEST_BODY
            .with(ModelOperationOpenApiFlags::CREATED_STATUS)
            .with(ModelOperationOpenApiFlags::CONFLICT_RESPONSE),
    ),
    descriptor(
        ModelStaticOperation::Find,
        "find",
        ModelOperationArgumentShape::Lookup,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::Model,
        ModelOperationCheckerValidation::Lookup,
        ModelOperationStorageCategory::FindRecord,
        ModelOperationOpenApiFlags::NOT_FOUND_RESPONSE,
    ),
    descriptor(
        ModelStaticOperation::Where,
        "where",
        ModelOperationArgumentShape::Where,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::Where,
        ModelOperationStorageCategory::EqualityFilter { negated: false },
        ModelOperationOpenApiFlags::NONE,
    ),
    descriptor(
        ModelStaticOperation::WherePage,
        "where_page",
        ModelOperationArgumentShape::WherePage,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WherePage,
        ModelOperationStorageCategory::EqualityFilter { negated: false },
        ModelOperationOpenApiFlags::NONE,
    ),
    descriptor(
        ModelStaticOperation::WhereNot,
        "where_not",
        ModelOperationArgumentShape::Where,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereNot,
        ModelOperationStorageCategory::EqualityFilter { negated: true },
        ModelOperationOpenApiFlags::EXCLUSION_FILTERS,
    ),
    descriptor(
        ModelStaticOperation::WhereNotPage,
        "where_not_page",
        ModelOperationArgumentShape::WherePage,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereNotPage,
        ModelOperationStorageCategory::EqualityFilter { negated: true },
        ModelOperationOpenApiFlags::EXCLUSION_FILTERS,
    ),
    descriptor(
        ModelStaticOperation::WhereNotIn,
        "where_not_in",
        ModelOperationArgumentShape::Where,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereNotIn,
        ModelOperationStorageCategory::InclusionFilter { negated: true },
        ModelOperationOpenApiFlags::EXCLUSION_FILTERS.with(ModelOperationOpenApiFlags::IN_FILTERS),
    ),
    descriptor(
        ModelStaticOperation::WhereNotInPage,
        "where_not_in_page",
        ModelOperationArgumentShape::WherePage,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereNotInPage,
        ModelOperationStorageCategory::InclusionFilter { negated: true },
        ModelOperationOpenApiFlags::EXCLUSION_FILTERS.with(ModelOperationOpenApiFlags::IN_FILTERS),
    ),
    descriptor(
        ModelStaticOperation::WhereNotInOptional,
        "where_not_in_optional",
        ModelOperationArgumentShape::Where,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereNotInOptional,
        ModelOperationStorageCategory::OptionalExclusionFilter,
        ModelOperationOpenApiFlags::EXCLUSION_FILTERS
            .with(ModelOperationOpenApiFlags::OPTIONAL_FILTERS)
            .with(ModelOperationOpenApiFlags::IN_FILTERS),
    ),
    descriptor(
        ModelStaticOperation::WhereNotInOptionalPage,
        "where_not_in_optional_page",
        ModelOperationArgumentShape::WherePage,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereNotInOptionalPage,
        ModelOperationStorageCategory::OptionalExclusionFilter,
        ModelOperationOpenApiFlags::EXCLUSION_FILTERS
            .with(ModelOperationOpenApiFlags::OPTIONAL_FILTERS)
            .with(ModelOperationOpenApiFlags::IN_FILTERS),
    ),
    descriptor(
        ModelStaticOperation::WhereOptional,
        "where_optional",
        ModelOperationArgumentShape::Where,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereOptional,
        ModelOperationStorageCategory::OptionalEqualityFilter,
        ModelOperationOpenApiFlags::OPTIONAL_FILTERS,
    ),
    descriptor(
        ModelStaticOperation::WhereOptionalPage,
        "where_optional_page",
        ModelOperationArgumentShape::WherePage,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereOptionalPage,
        ModelOperationStorageCategory::OptionalEqualityFilter,
        ModelOperationOpenApiFlags::OPTIONAL_FILTERS,
    ),
    descriptor(
        ModelStaticOperation::WhereIn,
        "where_in",
        ModelOperationArgumentShape::Where,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereIn,
        ModelOperationStorageCategory::InclusionFilter { negated: false },
        ModelOperationOpenApiFlags::IN_FILTERS,
    ),
    descriptor(
        ModelStaticOperation::WhereInPage,
        "where_in_page",
        ModelOperationArgumentShape::WherePage,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereInPage,
        ModelOperationStorageCategory::InclusionFilter { negated: false },
        ModelOperationOpenApiFlags::IN_FILTERS,
    ),
    descriptor(
        ModelStaticOperation::WhereInOptional,
        "where_in_optional",
        ModelOperationArgumentShape::Where,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereInOptional,
        ModelOperationStorageCategory::OptionalInclusionFilter,
        ModelOperationOpenApiFlags::OPTIONAL_FILTERS.with(ModelOperationOpenApiFlags::IN_FILTERS),
    ),
    descriptor(
        ModelStaticOperation::WhereInOptionalPage,
        "where_in_optional_page",
        ModelOperationArgumentShape::WherePage,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereInOptionalPage,
        ModelOperationStorageCategory::OptionalInclusionFilter,
        ModelOperationOpenApiFlags::OPTIONAL_FILTERS.with(ModelOperationOpenApiFlags::IN_FILTERS),
    ),
    descriptor(
        ModelStaticOperation::WhereCompare,
        "where_compare",
        ModelOperationArgumentShape::AdvancedWhere,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereCompare,
        ModelOperationStorageCategory::ComparisonFilter,
        ModelOperationOpenApiFlags::COMPARISON_FILTERS,
    ),
    descriptor(
        ModelStaticOperation::WhereComparePage,
        "where_compare_page",
        ModelOperationArgumentShape::AdvancedWherePage,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereComparePage,
        ModelOperationStorageCategory::ComparisonFilter,
        ModelOperationOpenApiFlags::COMPARISON_FILTERS,
    ),
    descriptor(
        ModelStaticOperation::WhereText,
        "where_text",
        ModelOperationArgumentShape::AdvancedWhere,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereText,
        ModelOperationStorageCategory::TextFilter,
        ModelOperationOpenApiFlags::TEXT_FILTERS,
    ),
    descriptor(
        ModelStaticOperation::WhereTextPage,
        "where_text_page",
        ModelOperationArgumentShape::AdvancedWherePage,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereTextPage,
        ModelOperationStorageCategory::TextFilter,
        ModelOperationOpenApiFlags::TEXT_FILTERS,
    ),
    descriptor(
        ModelStaticOperation::WhereBetween,
        "where_between",
        ModelOperationArgumentShape::AdvancedWhere,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereBetween,
        ModelOperationStorageCategory::RangeFilter,
        ModelOperationOpenApiFlags::RANGE_FILTERS,
    ),
    descriptor(
        ModelStaticOperation::WhereBetweenPage,
        "where_between_page",
        ModelOperationArgumentShape::AdvancedWherePage,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereBetweenPage,
        ModelOperationStorageCategory::RangeFilter,
        ModelOperationOpenApiFlags::RANGE_FILTERS,
    ),
    descriptor(
        ModelStaticOperation::WhereAll,
        "where_all",
        ModelOperationArgumentShape::CompositeWhere,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereAll,
        ModelOperationStorageCategory::CompositeFilter { any: false },
        ModelOperationOpenApiFlags::COMPOSITE_FILTERS,
    ),
    descriptor(
        ModelStaticOperation::WhereAllPage,
        "where_all_page",
        ModelOperationArgumentShape::CompositeWherePage,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereAllPage,
        ModelOperationStorageCategory::CompositeFilter { any: false },
        ModelOperationOpenApiFlags::COMPOSITE_FILTERS,
    ),
    descriptor(
        ModelStaticOperation::WhereAny,
        "where_any",
        ModelOperationArgumentShape::CompositeWhere,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereAny,
        ModelOperationStorageCategory::CompositeFilter { any: true },
        ModelOperationOpenApiFlags::OR_FILTERS,
    ),
    descriptor(
        ModelStaticOperation::WhereAnyPage,
        "where_any_page",
        ModelOperationArgumentShape::CompositeWherePage,
        Some(RouteMethodRequirement::Get),
        ModelOperationReturnKind::List,
        ModelOperationCheckerValidation::WhereAnyPage,
        ModelOperationStorageCategory::CompositeFilter { any: true },
        ModelOperationOpenApiFlags::OR_FILTERS,
    ),
    descriptor(
        ModelStaticOperation::Update,
        "update",
        ModelOperationArgumentShape::Lookup,
        Some(RouteMethodRequirement::Put),
        ModelOperationReturnKind::Model,
        ModelOperationCheckerValidation::Lookup,
        ModelOperationStorageCategory::UpdateRecord,
        ModelOperationOpenApiFlags::REQUEST_BODY
            .with(ModelOperationOpenApiFlags::NOT_FOUND_RESPONSE)
            .with(ModelOperationOpenApiFlags::CONFLICT_RESPONSE),
    ),
    descriptor(
        ModelStaticOperation::Delete,
        "delete",
        ModelOperationArgumentShape::Lookup,
        Some(RouteMethodRequirement::Delete),
        ModelOperationReturnKind::Model,
        ModelOperationCheckerValidation::Lookup,
        ModelOperationStorageCategory::DeleteRecord,
        ModelOperationOpenApiFlags::NOT_FOUND_RESPONSE,
    ),
];

const fn descriptor(
    operation: ModelStaticOperation,
    method_name: &'static str,
    argument_shape: ModelOperationArgumentShape,
    route_method: Option<RouteMethodRequirement>,
    return_kind: ModelOperationReturnKind,
    checker_validation: ModelOperationCheckerValidation,
    storage_category: ModelOperationStorageCategory,
    openapi_flags: ModelOperationOpenApiFlags,
) -> ModelStaticOperationDescriptor {
    ModelStaticOperationDescriptor {
        operation,
        method_name,
        argument_shape,
        route_method,
        return_kind,
        checker_validation,
        storage_category,
        openapi_flags,
    }
}

fn list_suffix<'a>(
    args: &'a [Expr],
    base_arg_count: usize,
    pagination_required: bool,
) -> Option<(
    Option<CheckedModelOrderingArgs<'a>>,
    Option<CheckedModelPaginationArgs<'a>>,
)> {
    if args.len() < base_arg_count {
        return None;
    }

    let suffix = &args[base_arg_count..];
    match suffix {
        [] if !pagination_required => Some((None, None)),
        [limit, offset] if pagination_required || !starts_ordering_args(suffix) => {
            Some((None, Some(CheckedModelPaginationArgs { limit, offset })))
        }
        [field, direction] => Some((Some(CheckedModelOrderingArgs { field, direction }), None)),
        [field, direction, limit, offset] => Some((
            Some(CheckedModelOrderingArgs { field, direction }),
            Some(CheckedModelPaginationArgs { limit, offset }),
        )),
        _ => None,
    }
}

fn advanced_filter_kind<'a>(
    operation: ModelStaticOperation,
    field: &'a Expr,
    second: &'a Expr,
    third: &'a Expr,
) -> CheckedModelOperationArgsKind<'a> {
    if matches!(
        operation.storage_category(),
        ModelOperationStorageCategory::RangeFilter
    ) {
        CheckedModelOperationArgsKind::RangeFilter {
            field,
            min: second,
            max: third,
        }
    } else {
        CheckedModelOperationArgsKind::AdvancedFilter {
            field,
            operator: second,
            value: third,
        }
    }
}

pub fn starts_ordering_args(args: &[Expr]) -> bool {
    args.first().is_some_and(expr_is_string_lit)
}

fn expr_is_string_lit(expr: &Expr) -> bool {
    matches!(expr, Expr::StringLit { .. })
}

fn expr_is_order_direction_lit(expr: &Expr) -> bool {
    matches!(expr, Expr::StringLit { value, .. } if value == "asc" || value == "desc")
}

pub fn where_all_filter_arg_count(args: &[Expr]) -> Option<usize> {
    if args.len() < 4 {
        return None;
    }
    let filter_arg_count = if where_all_args_have_ordering(args) {
        args.len() - 4
    } else if where_all_args_have_pagination(args) {
        args.len() - 2
    } else {
        args.len()
    };
    if filter_arg_count >= 4 && filter_arg_count % 2 == 0 {
        Some(filter_arg_count)
    } else {
        None
    }
}

pub fn where_all_page_filter_arg_count(args: &[Expr]) -> Option<usize> {
    if args.len() < 6 || !where_all_args_have_pagination(args) {
        return None;
    }
    let filter_arg_count = if where_all_args_have_ordering(args) {
        args.len() - 4
    } else {
        args.len() - 2
    };
    if filter_arg_count >= 4 && filter_arg_count % 2 == 0 {
        Some(filter_arg_count)
    } else {
        None
    }
}

pub fn where_all_args_have_pagination(args: &[Expr]) -> bool {
    args.len() >= 6 && !expr_is_string_lit(&args[args.len() - 2])
}

pub fn where_all_args_have_ordering(args: &[Expr]) -> bool {
    args.len() >= 8
        && expr_is_string_lit(&args[args.len() - 4])
        && expr_is_order_direction_lit(&args[args.len() - 3])
        && !expr_is_string_lit(&args[args.len() - 2])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Span;
    use std::collections::HashSet;

    fn int(value: i64) -> Expr {
        Expr::Integer {
            value,
            span: Span::unknown(),
        }
    }

    fn string(value: &str) -> Expr {
        Expr::StringLit {
            value: value.to_string(),
            span: Span::unknown(),
        }
    }

    #[test]
    fn descriptors_cover_all_model_static_operations() {
        let mut names = HashSet::new();
        let mut operations = HashSet::new();
        for descriptor in MODEL_STATIC_OPERATION_DESCRIPTORS {
            assert!(names.insert(descriptor.method_name));
            assert!(operations.insert(descriptor.operation));
            assert_eq!(
                Some(descriptor.operation),
                ModelStaticOperation::from_method(descriptor.method_name)
            );
            assert_eq!(descriptor.method_name, descriptor.operation.method_name());
        }
        assert_eq!(MODEL_STATIC_OPERATION_DESCRIPTORS.len(), 30);
        assert_eq!(names.len(), 30);
        assert_eq!(operations.len(), 30);
    }

    #[test]
    fn descriptor_exposes_route_return_openapi_and_storage_metadata() {
        let create = ModelStaticOperation::Create.descriptor();
        assert_eq!(create.argument_shape, ModelOperationArgumentShape::Create);
        assert_eq!(create.route_method, Some(RouteMethodRequirement::Post));
        assert_eq!(create.return_kind, ModelOperationReturnKind::Model);
        assert_eq!(
            create.storage_category,
            ModelOperationStorageCategory::CreateRecord
        );
        assert!(create
            .openapi_flags
            .contains(ModelOperationOpenApiFlags::REQUEST_BODY));
        assert!(create
            .openapi_flags
            .contains(ModelOperationOpenApiFlags::CREATED_STATUS));

        let where_not_in_optional = ModelStaticOperation::WhereNotInOptional.descriptor();
        assert_eq!(
            where_not_in_optional.storage_category,
            ModelOperationStorageCategory::OptionalExclusionFilter
        );
        assert!(where_not_in_optional
            .openapi_flags
            .contains(ModelOperationOpenApiFlags::EXCLUSION_FILTERS));
        assert!(where_not_in_optional
            .openapi_flags
            .contains(ModelOperationOpenApiFlags::OPTIONAL_FILTERS));
        assert!(where_not_in_optional
            .openapi_flags
            .contains(ModelOperationOpenApiFlags::IN_FILTERS));
    }

    #[test]
    fn argument_shape_resolves_dynamic_openapi_flags() {
        let all_without_args: Vec<Expr> = vec![];
        let all_with_ordering = vec![string("name"), string("asc")];
        let all_with_pagination = vec![int(10), int(0)];
        let where_page = vec![
            string("status"),
            string("active"),
            string("name"),
            string("asc"),
            int(10),
            int(0),
        ];

        assert_eq!(
            ModelStaticOperation::All.required_route_method(all_without_args.len()),
            None
        );
        assert_eq!(
            ModelStaticOperation::All.required_route_method(all_with_pagination.len()),
            Some(RouteMethodRequirement::Get)
        );
        assert!(ModelStaticOperation::All
            .openapi_flags(&all_with_ordering)
            .contains(ModelOperationOpenApiFlags::ORDERING));
        assert!(ModelStaticOperation::All
            .openapi_flags(&all_with_pagination)
            .contains(ModelOperationOpenApiFlags::PAGINATION));

        let page_flags = ModelStaticOperation::WherePage.openapi_flags(&where_page);
        assert!(page_flags.contains(ModelOperationOpenApiFlags::PAGINATION));
        assert!(page_flags.contains(ModelOperationOpenApiFlags::ORDERING));
        assert!(page_flags.contains(ModelOperationOpenApiFlags::TOTAL_COUNT));
    }

    #[test]
    fn checked_model_operation_args_normalize_suffixes_and_filters() {
        let ordered_page = vec![
            string("status"),
            string("active"),
            string("name"),
            string("asc"),
            int(10),
            int(0),
        ];
        let checked = ModelStaticOperation::WherePage
            .checked_args(&ordered_page)
            .expect("where_page args should normalize");

        assert!(checked.has_ordering());
        assert!(checked.has_pagination());
        assert!(checked.has_page_response());
        let (field, value) = checked.lookup().expect("where_page lookup args");
        assert!(matches!(field, Expr::StringLit { value, .. } if value == "status"));
        assert!(matches!(value, Expr::StringLit { value, .. } if value == "active"));

        let comparison = vec![
            string("score"),
            string("gte"),
            int(50),
            string("name"),
            string("desc"),
        ];
        let checked = ModelStaticOperation::WhereCompare
            .checked_args(&comparison)
            .expect("where_compare args should normalize");
        assert!(checked.has_ordering());
        assert!(!checked.has_pagination());
        let (_, operator, _) = checked
            .advanced_filter()
            .expect("where_compare advanced filter args");
        assert!(matches!(operator, Expr::StringLit { value, .. } if value == "gte"));
    }

    #[test]
    fn checked_model_operation_args_normalize_composite_filters() {
        let composite = vec![
            string("status"),
            string("active"),
            string("tier"),
            string("gold"),
            string("name"),
            string("asc"),
            int(20),
            int(0),
        ];
        let checked = ModelStaticOperation::WhereAll
            .checked_args(&composite)
            .expect("where_all args should normalize");

        assert!(checked.has_ordering());
        assert!(checked.has_pagination());
        assert!(!checked.has_page_response());
        assert_eq!(checked.composite_filter_args().unwrap().len(), 4);

        let page_checked = ModelStaticOperation::WhereAnyPage
            .checked_args(&composite)
            .expect("where_any_page args should normalize");
        assert!(page_checked.has_page_response());
        assert_eq!(page_checked.composite_filter_args().unwrap().len(), 4);
    }
}
