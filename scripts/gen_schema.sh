#!/usr/bin/env bash
# ACOS schema generation script (MVP placeholder).
#
# In MVP this documents the intended generation flow. Once `protoc` and the
# language-specific plugins are available, this script generates:
#   - Rust bindings   → crates/acos-core/src/generated/
#   - TypeScript types → packages/sdk-typescript/src/generated/
#   - Python types    → packages/sdk-python/acos/generated/
#
# See ADR-0005 (docs/adrs/adr-0005-proto-wire-json-manifests.md).

set -euo pipefail

SCHEMA_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../schemas" && pwd)"

echo "ACOS schema generation"
echo "  schema dir: ${SCHEMA_DIR}"
echo ""
echo "Schemas found:"
find "${SCHEMA_DIR}" -name '*.proto' -exec echo "  - {}" \;
echo ""
echo "[MVP placeholder] No code generated yet. Install protoc + plugins and"
echo "uncomment the generation steps below to produce Rust/TS/Python bindings."

# Example (uncomment when protoc is available):
# protoc \
#   --proto_path="${SCHEMA_DIR}" \
#   --rust_out=crates/acos-core/src/generated \
#   --ts_out=packages/sdk-typescript/src/generated \
#   "${SCHEMA_DIR}"/cir/cir.proto \
#   "${SCHEMA_DIR}"/task/task.proto \
#   "${SCHEMA_DIR}"/primitive/primitive.proto \
#   "${SCHEMA_DIR}"/events/events.proto
