#!/bin/bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
browser="${1:-$repo_root/target/release/arch-browser}"
runtime="${2:-$repo_root/target/release/archetype-runtime}"
browser_entitlements="$repo_root/config/macos/browser.entitlements"
runtime_entitlements="$repo_root/config/macos/runtime.entitlements"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "runtime entitlement verification requires macOS" >&2
  exit 2
fi
for artifact in "$browser" "$runtime"; do
  if [[ ! -x "$artifact" ]]; then
    echo "signed artifact input is not executable: $artifact" >&2
    exit 2
  fi
done

work_dir="$(mktemp -d -t archetype-signed-runtime)"
trap 'rm -rf "$work_dir"' EXIT
signed_browser="$work_dir/arch-browser"
signed_runtime="$work_dir/archetype-runtime"
cp "$browser" "$signed_browser"
cp "$runtime" "$signed_runtime"

/usr/bin/codesign --force --sign - --entitlements "$browser_entitlements" "$signed_browser"
/usr/bin/codesign --force --sign - --entitlements "$runtime_entitlements" "$signed_runtime"
/usr/bin/codesign --verify --strict "$signed_browser"
/usr/bin/codesign --verify --strict "$signed_runtime"

/usr/bin/codesign --display --entitlements :- "$signed_browser" > "$work_dir/browser.plist" 2>/dev/null
/usr/bin/codesign --display --entitlements :- "$signed_runtime" > "$work_dir/runtime.plist" 2>/dev/null

assert_true() {
  local plist="$1"
  local key="$2"
  local value
  value="$(/usr/libexec/PlistBuddy -c "Print :$key" "$plist")"
  if [[ "$value" != "true" ]]; then
    echo "expected true entitlement $key in $plist" >&2
    exit 1
  fi
}

assert_absent() {
  local plist="$1"
  local key="$2"
  if /usr/libexec/PlistBuddy -c "Print :$key" "$plist" >/dev/null 2>&1; then
    echo "forbidden entitlement $key in $plist" >&2
    exit 1
  fi
}

assert_only_keys() {
  local plist="$1"
  local expected="$2"
  local actual
  actual="$(/usr/bin/plutil -p "$plist" | sed -n 's/^  "\([^"]*\)" =>.*/\1/p' | sort)"
  if [[ "$actual" != "$expected" ]]; then
    echo "unexpected entitlement set in $plist" >&2
    printf 'expected:\n%s\nactual:\n%s\n' "$expected" "$actual" >&2
    exit 1
  fi
}

assert_true "$work_dir/browser.plist" "com.apple.security.app-sandbox"
assert_true "$work_dir/browser.plist" "com.apple.security.files.user-selected.read-only"
assert_true "$work_dir/browser.plist" "com.apple.security.network.client"
assert_true "$work_dir/runtime.plist" "com.apple.security.app-sandbox"
assert_true "$work_dir/runtime.plist" "com.apple.security.inherit"
assert_only_keys "$work_dir/browser.plist" "$(printf '%s\n' \
  "com.apple.security.app-sandbox" \
  "com.apple.security.files.user-selected.read-only" \
  "com.apple.security.network.client" | sort)"
assert_only_keys "$work_dir/runtime.plist" "$(printf '%s\n' \
  "com.apple.security.app-sandbox" \
  "com.apple.security.inherit" | sort)"

for entitlement in \
  "com.apple.security.files.downloads.read-only" \
  "com.apple.security.files.downloads.read-write" \
  "com.apple.security.files.user-selected.read-only" \
  "com.apple.security.files.user-selected.read-write" \
  "com.apple.security.network.client" \
  "com.apple.security.network.server"; do
  assert_absent "$work_dir/runtime.plist" "$entitlement"
done
