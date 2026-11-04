#!/bin/sh

# ─── Chess App Launcher ───────────────────────────────────────────────────────
# Deploys from /mnt/us/hellokindle/
# Binary:  bin/kindle-hello
# Logs:    log/app.log (written by the binary itself via log4rs)
# Secrets: secrets/token.json

APP_DIR="/mnt/us/hellokindle"
BINARY="$APP_DIR/bin/kindle-hello"
SCRIPT_LOG="$APP_DIR/log/launch.log"
PID_FILE="/tmp/chess_app.pid"

# Ensure log dir exists
mkdir -p "$APP_DIR/log"
mkdir -p "$APP_DIR/secrets"

echo "=== chess_app.sh started ===" > "$SCRIPT_LOG"
echo "Time: $(date)"               >> "$SCRIPT_LOG"

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

# ─── Update trampoline ────────────────────────────────────────────────────────
# The in-app updater stages a verified new binary at $BINARY.new (it does not
# replace the running ELF directly because /mnt/us is VFAT). At launch time —
# while no instance is running — we move it into place. If the swap fails,
# leave the existing binary untouched and continue.
if [ -f "$BINARY.new" ]; then
    echo "Update staged at $BINARY.new — installing" >> "$SCRIPT_LOG"
    if mv "$BINARY.new" "$BINARY"; then
        chmod +x "$BINARY"
        echo "Update installed" >> "$SCRIPT_LOG"
    else
        echo "WARNING: failed to install $BINARY.new — keeping current binary" >> "$SCRIPT_LOG"
    fi
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

# AB HIER FRANKENSTEINED

# Sicherheitscheck 3: Backup des aktuellen Displays (falls möglich)
echo "Starting test app..." >> "$SCRIPT_LOG"
eips "Starting test app..."
sleep 3
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
    eips "Error! Check logs: /mnt/us/hellokindle/log/app.log and launch.log"
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














# ─── X11 / display setup ──────────────────────────────────────────────────────

# # The Kindle's Xorg instance is always on :0
# export DISPLAY=:0

# # Hand the screen over from the Kindle UI to X11.
# # eips -c clears the eInk framebuffer so X11 draws on a clean slate.
# eips -c

# # Tell the Kindle framework to stop managing the display so X11 can own it.
# # This is the same lipc call used by KOReader and similar X11 apps on Kindle.
# lipc-set-prop com.lab126.pillow disableEnablePillow disable 2>/dev/null || true

# echo "Display setup done, launching binary..." >> "$SCRIPT_LOG"

# # ─── Launch ───────────────────────────────────────────────────────────────────

# cd "$APP_DIR"
# "$BINARY" >> "$SCRIPT_LOG" 2>&1 &
# echo $! > "$PID_FILE"
# echo "Launched with PID $(cat $PID_FILE)" >> "$SCRIPT_LOG"

# # Wait for the binary to finish
# wait $(cat "$PID_FILE")
# EXIT_CODE=$?
# rm -f "$PID_FILE"

# echo "Binary exited with code $EXIT_CODE" >> "$SCRIPT_LOG"

# # ─── Cleanup ──────────────────────────────────────────────────────────────────

# # Hand display back to the Kindle UI
# lipc-set-prop com.lab126.pillow disableEnablePillow enable 2>/dev/null || true
# eips -c
# eips -f

# echo "=== chess_app.sh done ===" >> "$SCRIPT_LOG"
