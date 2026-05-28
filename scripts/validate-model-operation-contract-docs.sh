#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DOC="$ROOT_DIR/MODEL_OPERATIONS.md"
SRC="$ROOT_DIR/nexuslang-src/src/model_ops.rs"
CHECKER_SRC="$ROOT_DIR/nexuslang-src/src/checker/model_ops.rs"
README="$ROOT_DIR/README.md"
ROADMAP="$ROOT_DIR/nexuslang-src/ROADMAP.md"

require_file() {
    local file="$1"
    if [ ! -f "$file" ]; then
        echo "Required file is missing: $file" >&2
        exit 1
    fi
}

require_text() {
    local file="$1"
    local needle="$2"
    if ! grep -Fq -- "$needle" "$file"; then
        echo "Missing required model operation contract anchor in $file: $needle" >&2
        exit 1
    fi
}

require_file "$DOC"
require_file "$SRC"
require_file "$CHECKER_SRC"
require_file "$README"
require_file "$ROADMAP"

require_text "$DOC" "MODEL_STATIC_OPERATION_DESCRIPTORS"
require_text "$DOC" "ModelStaticOperationDescriptor"
require_text "$DOC" "ModelOperationArgumentShape"
require_text "$DOC" "ModelOperationCheckerValidation"
require_text "$DOC" "ModelOperationStorageCategory"
require_text "$DOC" "ModelOperationOpenApiFlags"
require_text "$DOC" "ModelOperationOpenApiFeature"
require_text "$DOC" "CheckedModelOperationArgs"
require_text "$DOC" "checker/model_ops.rs"
require_text "$DOC" "route_hir.rs"
require_text "$DOC" "Adding A Model Operation"

require_text "$SRC" "pub enum ModelStaticOperation"
require_text "$SRC" "pub const ALL: [Self; 30]"
require_text "$SRC" "pub struct ModelStaticOperationDescriptor"
require_text "$SRC" "pub const MODEL_STATIC_OPERATION_DESCRIPTORS"
require_text "$SRC" "pub enum ModelOperationArgumentShape"
require_text "$SRC" "pub enum ModelOperationCheckerValidation"
require_text "$SRC" "pub enum ModelOperationStorageCategory"
require_text "$SRC" "pub enum ModelOperationOpenApiFeature"
require_text "$SRC" "pub struct ModelOperationOpenApiFlags"
require_text "$SRC" "pub struct CheckedModelOperationArgs"
require_text "$SRC" "pub enum CheckedModelOperationArgsKind"

require_text "$CHECKER_SRC" "pub(super) fn check_model_static_operation"
require_text "$CHECKER_SRC" "ModelLookupValidation"
require_text "$CHECKER_SRC" "ModelAdvancedFilterValidation"

require_text "$README" "MODEL_OPERATIONS.md"
require_text "$ROADMAP" "MODEL_OPERATIONS.md"

echo "Model operation contract documentation validation passed."
