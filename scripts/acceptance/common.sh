# Shared assertions and host-path helpers for the acceptance scenarios.

assert_json() {
  local file="$1"
  local expression="$2"
  "$PYTHON_BIN" - "$file" "$expression" <<'PY'
import json
import sys

path, expression = sys.argv[1:]
with open(path, encoding="utf-8") as handle:
    value = json.load(handle)
if not eval(expression, {"__builtins__": {"len": len, "isinstance": isinstance, "dict": dict}}, {"v": value}):
    raise SystemExit(f"assertion failed for {path}: {expression}\n{value}")
PY
}

file_sha256() {
  "$PYTHON_BIN" - "$1" <<'PY'
import hashlib
import sys

digest = hashlib.sha256()
with open(sys.argv[1], "rb") as handle:
    for chunk in iter(lambda: handle.read(1024 * 1024), b""):
        digest.update(chunk)
print(digest.hexdigest())
PY
}

native_path() {
  if command -v cygpath >/dev/null; then
    local converted
    if converted="$(cygpath -m "$1" 2>/dev/null)"; then
      printf '%s' "$converted"
    else
      local parent="${1%/*}"
      local leaf="${1##*/}"
      printf '%s/%s' "$(cygpath -m "$parent")" "$leaf"
    fi
  else
    printf '%s' "$1"
  fi
}
