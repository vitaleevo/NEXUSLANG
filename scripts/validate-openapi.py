#!/usr/bin/env python3
"""Validate NexusLang OpenAPI 1.0 document against OpenAPI 3.0 spec.

Usage:
    python3 scripts/validate-openapi.py <openapi.json>

    Or with server running:
    curl http://127.0.0.1:5050/openapi.json | python3 scripts/validate-openapi.py
"""

import json
import sys
from pathlib import Path

def validate_openapi_document(document: dict) -> list[str]:
    errors = []

    if document.get("openapi") != "3.0.0":
        errors.append(f"openapi version should be 3.0.0, got {document.get('openapi')}")

    info = document.get("info", {})
    if not info.get("title"):
        errors.append("info.title is required")
    if not info.get("version"):
        errors.append("info.version is required")

    paths = document.get("paths", {})
    if not paths:
        errors.append("paths must contain at least one path")
    else:
        for path, path_item in paths.items():
            if not path.startswith("/"):
                errors.append(f"path '{path}' must start with /")
            if not isinstance(path_item, dict) or not path_item:
                errors.append(f"path '{path}' must have at least one operation")
            for method in path_item:
                if method not in ("get", "post", "put", "patch", "delete", "options", "head", "trace"):
                    errors.append(f"path '{path}' has invalid method '{method}'")
                else:
                    operation = path_item[method]
                    if not isinstance(operation, dict):
                        continue
                    if "operationId" not in operation:
                        errors.append(f"operation in {method} {path} missing operationId")
                    if "responses" not in operation:
                        errors.append(f"operation in {method} {path} missing responses")
                    elif not operation["responses"]:
                        errors.append(f"operation in {method} {path} has empty responses")

    components = document.get("components", {})
    if not components:
        errors.append("components is required")

    schemas = components.get("schemas", {})
    if not schemas:
        errors.append("components.schemas is required")

    refs_seen: set[str] = set()
    def collect_refs(obj):
        if isinstance(obj, dict):
            if "$ref" in obj:
                refs_seen.add(obj["$ref"])
            for v in obj.values():
                collect_refs(v)
        elif isinstance(obj, list):
            for item in obj:
                collect_refs(item)

    collect_refs(document)

    for ref in refs_seen:
        if not ref.startswith("#/components/"):
            continue
        parts = ref.split("/")
        if len(parts) < 4:
            errors.append(f"invalid $ref format: {ref}")
            continue
        bucket = parts[2]
        name = parts[3]
        bucket_data = components.get(bucket, {})
        if name not in bucket_data:
            errors.append(f"$ref '{ref}' does not resolve to any component in {bucket}")

    return errors


def main():
    if len(sys.argv) > 1:
        path = Path(sys.argv[1])
        raw = path.read_text()
    else:
        raw = sys.stdin.read()

    try:
        document = json.loads(raw)
    except json.JSONDecodeError as e:
        print(f"FALHA: OpenAPI não é JSON válido: {e}")
        sys.exit(1)

    errors = validate_openapi_document(document)

    if errors:
        print(f"❌ OpenAPI 3.0 validation: {len(errors)} error(s)")
        for err in errors:
            print(f"   - {err}")
        sys.exit(1)
    else:
        print("✅ OpenAPI 3.0 document is valid")
        print(f"   Paths: {len(document.get('paths', {}))}")
        schemas = len(document.get("components", {}).get("schemas", {}))
        params = len(document.get("components", {}).get("parameters", {}))
        req_bodies = len(document.get("components", {}).get("requestBodies", {}))
        responses = len(document.get("components", {}).get("responses", {}))
        print(f"   Schemas: {schemas}, Parameters: {params}, RequestBodies: {req_bodies}, Responses: {responses}")


if __name__ == "__main__":
    main()
