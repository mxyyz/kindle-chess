#!/bin/sh

# ─── Chess App Launcher ───────────────────────────────────────────────────────
# Deploys from /mnt/us/kindlichess/
# Binary:  bin/kindle-hello
# Logs:    log/app.log (written by the binary itself via log4rs)
# Secrets: secrets/token.json

APP_DIR="${KINDLECHESS_APP_DIR:-/mnt/us/kindlichess}"
BINARY="$APP_DIR/bin/kindle-hello"
SCRIPT_LOG="$APP_DIR/log/launch.log"
PID_FILE="/tmp/chess_app.pid"

# Ensure log dir exists
mkdir -p "$APP_DIR/log"
mkdir -p "$APP_DIR/secrets"

echo "=== chess_app.sh started ===" > "$SCRIPT_LOG"
echo "Time: $(date)"               >> "$SCRIPT_LOG"
echo "Source: ${1:-unknown} (scriptlet / kual menu)" >> "$SCRIPT_LOG"

# ─── Sanity checks ────────────────────────────────────────────────────────────
if [ ! -f "$BINARY" ]; then
    echo "ERROR: Binary not found at $BINARY" >> "$SCRIPT_LOG"
    eips -c
    eips "ERROR: chess-app binary \"kindle-hello\" not found"
    sleep 5
    exit 1
fi

if [ ! -x "$BINARY" ]; then
    echo "WARNING: Binary not executable — fixing..." >> "$SCRIPT_LOG"
    chmod +x "$BINARY"
fi

# ─── Kill any stale instance ──────────────────────────────────────────────────

if [ -f "$PID_FILE" ]; then
    OLD_PID=$(cat "$PID_FILE")
    if kill -0 "$OLD_PID" 2>/dev/null; then
        echo "Killing stale instance (PID $OLD_PID)" >> "$SCRIPT_LOG"
        kill "$OLD_PID"
        sleep 1
    fi
    rm -f "$PID_FILE"
fi

# ─── Update trampoline v2 ─────────────────────────────────────────────────────
# Runs at launch while no instance is running — the safe point to install.
#
# Bundle flow (e05+): the in-app updater stages a verified update.tar.gz at
# $BINARY.bundle.new. Extract to .staging, verify MANIFEST, install binary +
# assets atomically, self-update this script if it changed, then re-exec.
#
# Legacy flow (0.4.0 clients): $BINARY.new staged by the old updater — just mv.
#
# Any failure removes the partial files and continues with the old install.

STAGING="$APP_DIR/.staging"
BUNDLE_NEW="$BINARY.bundle.new"

if [ -f "$BUNDLE_NEW" ]; then
    echo "Bundle update staged at $BUNDLE_NEW — installing" >> "$SCRIPT_LOG"
    rm -rf "$STAGING"
    mkdir -p "$STAGING"
    if tar -xzf "$BUNDLE_NEW" -C "$STAGING" 2>>"$SCRIPT_LOG" \
       && (cd "$STAGING" && sha256sum -c MANIFEST.sha256 >>"$SCRIPT_LOG" 2>&1); then
        # Install binary atomically (tmp + mv so a failed copy keeps the old one)
        if [ -f "$STAGING/bin/kindle-hello" ]; then
            if cp "$STAGING/bin/kindle-hello" "$BINARY.tmp" 2>>"$SCRIPT_LOG" \
               && mv "$BINARY.tmp" "$BINARY"; then
                chmod +x "$BINARY"
                echo "Binary updated" >> "$SCRIPT_LOG"
            else
                rm -f "$BINARY.tmp"
                echo "WARNING: binary install failed — keeping old binary" >> "$SCRIPT_LOG"
            fi
        fi
        # Install assets
        if [ -d "$STAGING/assets" ]; then
            cp -f "$STAGING/assets/"* "$APP_DIR/assets/" 2>>"$SCRIPT_LOG" \
                && echo "Assets updated" >> "$SCRIPT_LOG"
        fi
        # Self-update this script if the bundle ships a different one
        if [ -f "$STAGING/chess_app.sh" ] && ! cmp -s "$STAGING/chess_app.sh" "$APP_DIR/chess_app.sh"; then
            cp "$STAGING/chess_app.sh" "$APP_DIR/chess_app.sh.new" \
                && mv "$APP_DIR/chess_app.sh.new" "$APP_DIR/chess_app.sh" \
                && chmod +x "$APP_DIR/chess_app.sh"
            rm -rf "$STAGING" "$BUNDLE_NEW"
            echo "Script self-updated — re-exec" >> "$SCRIPT_LOG"
            exec "$APP_DIR/chess_app.sh" "$@"
        fi
        rm -rf "$STAGING" "$BUNDLE_NEW"
        echo "Bundle update installed" >> "$SCRIPT_LOG"
    else
        echo "WARNING: bundle verify/install failed — keeping old install" >> "$SCRIPT_LOG"
        rm -rf "$STAGING" "$BUNDLE_NEW"
    fi
elif [ -f "$BINARY.new" ]; then
    # Legacy binary-only trampoline (0.4.0 clients)
    echo "Update staged at $BINARY.new — installing" >> "$SCRIPT_LOG"
    if mv "$BINARY.new" "$BINARY"; then
        chmod +x "$BINARY"
        echo "Update installed" >> "$SCRIPT_LOG"
    else
        echo "WARNING: failed to install $BINARY.new — keeping current binary" >> "$SCRIPT_LOG"
    fi
fi

# Test hook: harness runs the script with --trampoline-test to exercise only
# the trampoline block above, then exit before touching the display/binary.
if [ "${1:-}" = "--trampoline-test" ]; then
    exit 0
fi


# ─── OAuth callback firewall ─────────────────────────────────────────────────
# The QR auth flow needs the phone to reach this device on TCP 8080. Stock
# firmware restores its iptables rules on every boot, silently dropping the
# phone's callback (browser timeout, no error in app.log). Re-open the port
# at every launch so the rule survives reboots.
if iptables -C INPUT -p tcp --dport 8080 -j ACCEPT 2>/dev/null; then
    echo "Firewall: callback port 8080 already open" >> "$SCRIPT_LOG"
else
    if iptables -I INPUT -p tcp --dport 8080 -j ACCEPT 2>> "$SCRIPT_LOG"; then
        echo "Firewall: opened callback port 8080 (INPUT)" >> "$SCRIPT_LOG"
    else
        echo "WARNING: could not open port 8080 — QR auth callback may time out" >> "$SCRIPT_LOG"
    fi
fi

eips -c

# Hand the framebuffer over to X11 so the app's window is actually visible
# (otherwise pillow keeps compositing the Kindle UI on top of X11).
export DISPLAY=:0
lipc-set-prop com.lab126.pillow disableEnablePillow disable 2>/dev/null || true

# WICHTIG: Timeout - falls das Programm hängt, automatisch beenden nach 3min
# timeout 180
$BINARY >> "$SCRIPT_LOG" 2>&1

EXIT_CODE=$?

echo "Test ended with code: $EXIT_CODE" >> "$SCRIPT_LOG"

# Falls etwas schiefgegangen ist
if [ $EXIT_CODE -ne 0 ] && [ $EXIT_CODE -ne 124 ]; then
    echo "ERROR in running test app!" >> "$SCRIPT_LOG"
    eips -c
    eips "Error! Check logs: /mnt/us/kindlichess/log/app.log and launch.log"
    sleep 3
elif [ $EXIT_CODE -eq 124 ]; then
    echo "Timeout - App has been shutdown automatically" >> "$SCRIPT_LOG"
    eips -c
    eips "Test successfull (Safety-Timeout)"
    sleep 2
else
    echo "Test completed successfully!" >> "$SCRIPT_LOG"
fi

# Hand display back to the Kindle UI
lipc-set-prop com.lab126.pillow disableEnablePillow enable 2>/dev/null || true

# Sicherstellung: Display ist sauber
eips -c
eips -f

echo "=== End of  test script ===" >> "$SCRIPT_LOG"

