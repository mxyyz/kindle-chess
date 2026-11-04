#!/bin/sh

# ─── appreg.db registration ────────────────────────────────────────────────────
# Registers kindle-chess in /var/local/appreg.db so the app is launchable from
# the Kindle home screen (interface "application"), like a native booklet.
# Idempotent and safe off-device: exits 0 when appreg.db or sqlite3 is absent.
# Reference: https://kindlemodding.org/kindle-hacking/appreg.html
# (properties modeled on com.notmarek.shell_integration.launcher and
#  com.lab126.booklet.home)

# APPREG/CMD overridable via env so the script is testable off-device.
APPREG=${APPREG:-/var/local/appreg.db}
HANDLER=org.kindlemodding.kindlechess
CMD=${CMD:-/mnt/us/hellokindle/chess_app.sh}

if [ ! -f "$APPREG" ]; then
    echo "register_appreg: $APPREG not found (off-device?) — skipping"
    exit 0
fi
if ! command -v sqlite3 >/dev/null 2>&1; then
    echo "register_appreg: sqlite3 not found — skipping"
    exit 0
fi

# Idempotency: already registered?
if [ "$(sqlite3 "$APPREG" "SELECT COUNT(*) FROM handlerIds WHERE handlerId='$HANDLER';")" -ge 1 ]; then
    echo "register_appreg: $HANDLER already registered — no-op"
    exit 0
fi

BACKUP="$APPREG.bak.$(date +%s)"
if ! cp "$APPREG" "$BACKUP"; then
    echo "register_appreg: ERROR backup failed — aborting without changes"
    exit 1
fi

rollback() {
    cp "$BACKUP" "$APPREG"
    echo "register_appreg: ERROR rolled back from $BACKUP"
    exit 1
}

# default-chrome-style N matches the Phase-1 fullscreen window title (PC:N);
# Phase 2 switches both to T.
if ! sqlite3 "$APPREG" <<SQL
BEGIN;
INSERT OR IGNORE INTO handlerIds(handlerId) VALUES('$HANDLER');
INSERT OR IGNORE INTO properties(handlerId,name,value) VALUES
  ('$HANDLER','command','$CMD'),
  ('$HANDLER','lipcId','$HANDLER'),
  ('$HANDLER','supportedOrientation','U'),
  ('$HANDLER','default-chrome-style','N'),
  ('$HANDLER','unloadPolicy','unloadOnPause');
INSERT OR IGNORE INTO associations(handlerId,interface,contentId,defaultAssoc)
  VALUES('$HANDLER','application','none',0);
COMMIT;
SQL
then
    rollback
fi

if [ "$(sqlite3 "$APPREG" "PRAGMA integrity_check;")" != "ok" ]; then
    rollback
fi

echo "register_appreg: registered $HANDLER (command=$CMD)"
exit 0
