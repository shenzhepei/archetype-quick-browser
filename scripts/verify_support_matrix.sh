#!/bin/bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
matrix="${1:-$repo_root/docs/html-css-support.json}"

jq -e '
  .schema_version == 1 and
  (.release | test("^[0-9]+\\.[0-9]+\\.[0-9]+$")) and
  (.features | length > 0) and
  ([.features[].id] | length == (unique | length)) and
  all(.features[];
    (.status == "supported" or .status == "partial" or .status == "unsupported") and
    (.evidence | type == "array") and
    (if .status == "unsupported" then true else (.evidence | length > 0) end)
  )
' "$matrix" >/dev/null
