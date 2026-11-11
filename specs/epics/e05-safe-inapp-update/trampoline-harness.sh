#!/bin/sh
# trampoline-harness.sh — host-side test for the chess_app.sh update trampoline v2.
# Builds a fake APP_DIR + tiny update bundle, runs the trampoline block with
# --trampoline-test, and asserts install success + failure-path safety.
#
# Usage: sh specs/epics/e05-safe-inapp-update/trampoline-harness.sh
# Exit 0 = all assertions pass.

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
TRAMPOLINE="$REPO_ROOT/kindlichess/chess_app.sh"

FAILURES=0
assert() { # assert <condition-cmd...>
    if "$@" >/dev/null 2>&1; then
        echo "  PASS: $*"
    else
        echo "  FAIL: $*"
        FAILURES=$((FAILURES + 1))
    fi
}

# ─── Setup fake APP_DIR ───────────────────────────────────────────────────────
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
APP="$WORK/app"
mkdir -p "$APP/bin" "$APP/assets" "$APP/log" "$APP/secrets" "$APP/config"

# Old install
echo "old-binary" > "$APP/bin/kindle-hello"
echo "old-asset"  > "$APP/assets/old.png"
cp "$TRAMPOLINE" "$APP/chess_app.sh"
chmod +x "$APP/chess_app.sh"

# ─── Build a tiny update bundle ───────────────────────────────────────────────
BUNDLE_SRC="$WORK/bundle"
mkdir -p "$BUNDLE_SRC/bin" "$BUNDLE_SRC/assets"
echo "new-binary" > "$BUNDLE_SRC/bin/kindle-hello"
echo "new-asset"  > "$BUNDLE_SRC/assets/new.png"
cp "$TRAMPOLINE" "$BUNDLE_SRC/chess_app.sh"
(cd "$BUNDLE_SRC" && find . -type f ! -name MANIFEST.sha256 | sort | xargs sha256sum > MANIFEST.sha256)
tar -czf "$APP/bin/kindle-hello.bundle.new" -C "$BUNDLE_SRC" .

# ─── Test 1: successful bundle install ────────────────────────────────────────
echo "Test 1: successful bundle install"
KINDLECHESS_APP_DIR="$APP" sh "$TRAMPOLINE" --trampoline-test

assert grep -q "new-binary" "$APP/bin/kindle-hello"
assert grep -q "new-asset" "$APP/assets/new.png"
assert test ! -f "$APP/bin/kindle-hello.bundle.new"
assert test ! -d "$APP/.staging"
assert grep -q "old-asset" "$APP/assets/old.png"  # old assets not deleted

# ─── Test 2: MANIFEST mismatch keeps old install ──────────────────────────────
echo "Test 2: MANIFEST mismatch keeps old install"
echo "corrupt-binary" > "$APP/bin/kindle-hello"   # reset to known state
echo "corrupt-asset"  > "$APP/assets/new.png"
# Build a bundle whose MANIFEST does not match contents
BAD_SRC="$WORK/bad"
mkdir -p "$BAD_SRC/bin"
echo "tampered" > "$BAD_SRC/bin/kindle-hello"
(cd "$BAD_SRC" && echo "0000000000000000000000000000000000000000000000000000000000000000  ./bin/kindle-hello" > MANIFEST.sha256)
tar -czf "$APP/bin/kindle-hello.bundle.new" -C "$BAD_SRC" .

KINDLECHESS_APP_DIR="$APP" sh "$TRAMPOLINE" --trampoline-test

assert grep -q "corrupt-binary" "$APP/bin/kindle-hello"  # old binary kept
assert grep -q "corrupt-asset" "$APP/assets/new.png"       # old asset kept
assert test ! -f "$APP/bin/kindle-hello.bundle.new"       # failed bundle cleaned up
assert test ! -d "$APP/.staging"

# ─── Test 3: legacy BINARY.new path still works ──────────────────────────────
echo "Test 3: legacy binary-only .new path"
echo "legacy-new" > "$APP/bin/kindle-hello.new"
KINDLECHESS_APP_DIR="$APP" sh "$TRAMPOLINE" --trampoline-test
assert grep -q "legacy-new" "$APP/bin/kindle-hello"
assert test ! -f "$APP/bin/kindle-hello.new"

# ─── Result ───────────────────────────────────────────────────────────────────
echo ""
if [ "$FAILURES" -eq 0 ]; then
    echo "TRAMPOLINE_HARNESS: ALL PASS"
    exit 0
else
    echo "TRAMPOLINE_HARNESS: $FAILURES FAILURE(S)"
    exit 1
fi
