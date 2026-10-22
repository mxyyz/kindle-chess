#!/usr/bin/env bash
# run-dev.sh — Run kindle-chess on your dev machine, emulating the Kindle Paperwhite display.
#
# Kindle display specs:  1072 x 1448 px, 16-bit grayscale, 300 DPI
# We emulate this with a Xvfb virtual framebuffer and view it via:
#   1. Xephyr  (preferred — crisp nested X window)
#   2. x11vnc  (AUR — connect any VNC client)
#   3. feh screenshot loop  (zero-dep fallback — refreshes every second)
#
# Requirements (Arch):
#   sudo pacman -S xorg-server-xvfb          # always needed
#   sudo pacman -S xorg-server-xephyr        # option 1 (recommended)
#   yay  -S x11vnc                           # option 2 (AUR)
#   sudo pacman -S feh xorg-xwd              # option 3 (fallback)
#
# Usage:
#   ./run-dev.sh              # auto-detect viewer, build debug, launch
#   ./run-dev.sh --release    # build release profile
#   ./run-dev.sh --no-build   # skip cargo build
#   ./run-dev.sh --viewer xephyr|x11vnc|feh|none
#   ./run-dev.sh --display :42   # use a custom virtual display number

set -euo pipefail

# ── Config ────────────────────────────────────────────────────────────────────
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATE_DIR="$REPO_ROOT/kindle_chess"
HELLOKINDLE_DIR="$REPO_ROOT/kindle_KUAL/hellokindle"

# Kindle Paperwhite 4 resolution
KINDLE_W=1072
KINDLE_H=1448
KINDLE_DPI=300

# Defaults (overridable via flags)
VDISPLAY=":99"
BUILD=true
PROFILE="debug"
CARGO_FLAGS=()
VIEWER_ARG="auto"

# ── Argument parsing ──────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --release)
            PROFILE="release"
            CARGO_FLAGS+=("--release")
            shift ;;
        --no-build)
            BUILD=false
            shift ;;
        --viewer)
            VIEWER_ARG="$2"
            shift 2 ;;
        --display)
            VDISPLAY="$2"
            shift 2 ;;
        --help|-h)
            sed -n 's/^# \?//p' "$0" | head -20
            exit 0 ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1 ;;
    esac
done

BINARY="$CRATE_DIR/target/$PROFILE/kindle-hello"
DISPLAY_NUM="${VDISPLAY#:}"
WIN_TITLE="Kindle Chess — ${KINDLE_W}×${KINDLE_H} @ ${KINDLE_DPI}dpi"

XVFB_PID=""
VIEWER_PID=""

# ── Helpers ───────────────────────────────────────────────────────────────────
need() {
    local cmd="$1" install="$2"
    if ! command -v "$cmd" &>/dev/null; then
        echo "ERROR: '$cmd' not found."
        echo "  Install with: $install"
        exit 1
    fi
}

has() { command -v "$1" &>/dev/null; }

cleanup() {
    echo ""
    echo "→ Shutting down..."
    [[ -n "$VIEWER_PID" ]] && kill "$VIEWER_PID" 2>/dev/null || true
    [[ -n "$XVFB_PID"  ]] && kill "$XVFB_PID"  2>/dev/null || true
    # Clean up the Xvfb socket so it can be restarted cleanly next time
    rm -f "/tmp/.X${DISPLAY_NUM}-lock" "/tmp/.X11-unix/X${DISPLAY_NUM}" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

# ── Dependency checks ─────────────────────────────────────────────────────────
need Xvfb  "sudo pacman -S xorg-server-xvfb"
need cargo "rustup — https://rustup.rs"

# ── Auto-detect viewer ────────────────────────────────────────────────────────
if [[ "$VIEWER_ARG" == "auto" ]]; then
    if   has Xephyr; then VIEWER="xephyr"
    elif has x11vnc; then VIEWER="x11vnc"
    elif has feh && has xwd; then VIEWER="feh"
    else
        VIEWER="none"
        echo "WARNING: No viewer found. The app will run headlessly on $VDISPLAY."
        echo "  Install one of:"
        echo "    sudo pacman -S xorg-server-xephyr   ← recommended"
        echo "    sudo pacman -S feh xorg-xwd          ← lightweight fallback"
        echo "    yay -S x11vnc                        ← VNC-based (AUR)"
        echo ""
    fi
else
    VIEWER="$VIEWER_ARG"
fi

# ── Runtime directory setup ───────────────────────────────────────────────────
echo "→ Ensuring runtime directories..."
mkdir -p "$HELLOKINDLE_DIR/log"
mkdir -p "$HELLOKINDLE_DIR/secrets"
mkdir -p "$HELLOKINDLE_DIR/assets"

# ── Build ─────────────────────────────────────────────────────────────────────
if $BUILD; then
    echo "→ Building ($PROFILE)..."
    (cd "$CRATE_DIR" && cargo build "${CARGO_FLAGS[@]}")
    echo "→ Build OK."
fi

if [[ ! -x "$BINARY" ]]; then
    echo "ERROR: Binary not found: $BINARY"
    echo "  Run without --no-build, or: cd kindle_chess && cargo build"
    exit 1
fi

# ── Start display server ──────────────────────────────────────────────────────
# Xephyr is itself a nested X server — it doesn't need Xvfb.
# Xvfb is only needed for x11vnc, feh (screenshot loop), and headless mode.

start_xvfb() {
    # Clean up any stale lock from a previous crash
    if [[ -e "/tmp/.X${DISPLAY_NUM}-lock" ]]; then
        echo "→ Removing stale Xvfb lock on $VDISPLAY..."
        rm -f "/tmp/.X${DISPLAY_NUM}-lock" "/tmp/.X11-unix/X${DISPLAY_NUM}" 2>/dev/null || true
    fi

    echo "→ Starting Xvfb on $VDISPLAY (${KINDLE_W}x${KINDLE_H}x16, ${KINDLE_DPI}dpi)..."
    Xvfb "$VDISPLAY" \
        -screen 0 "${KINDLE_W}x${KINDLE_H}x16" \
        -dpi "$KINDLE_DPI" \
        -ac \
        &>/tmp/xvfb-kindle.log &
    XVFB_PID=$!

    # Wait for the socket to appear (up to 3 s)
    for i in $(seq 1 30); do
        [[ -S "/tmp/.X11-unix/X${DISPLAY_NUM}" ]] && break
        sleep 0.1
    done

    if ! kill -0 "$XVFB_PID" 2>/dev/null; then
        echo "ERROR: Xvfb failed to start. See /tmp/xvfb-kindle.log"
        exit 1
    fi
    echo "   Xvfb PID: $XVFB_PID"
}

# ── Start viewer ──────────────────────────────────────────────────────────────
case "$VIEWER" in

    xephyr)
        need Xephyr "sudo pacman -S xorg-server-xephyr"
        need Xvfb   "sudo pacman -S xorg-server-xvfb"

        # Xephyr is its own display server — it serves $VDISPLAY itself and
        # renders into a window on the real desktop ($DISPLAY).
        # Clean up any stale lock first.
        if [[ -e "/tmp/.X${DISPLAY_NUM}-lock" ]]; then
            echo "→ Removing stale lock on $VDISPLAY..."
            rm -f "/tmp/.X${DISPLAY_NUM}-lock" "/tmp/.X11-unix/X${DISPLAY_NUM}" 2>/dev/null || true
        fi

        echo "→ Opening Xephyr window (serving $VDISPLAY, ${KINDLE_W}x${KINDLE_H}, ${KINDLE_DPI}dpi)..."
        # -glamor is optional (hardware-accelerated compositing); drop it if unsupported
        XEPHYR_EXTRA=()
        Xephyr -help 2>&1 | grep -q -- '-glamor' && XEPHYR_EXTRA+=(-glamor)

        Xephyr \
            -screen "${KINDLE_W}x${KINDLE_H}" \
            -dpi "$KINDLE_DPI" \
            -title "$WIN_TITLE" \
            -resizeable \
            -grayscale \
            -retro \
            "${XEPHYR_EXTRA[@]}" \
            "$VDISPLAY" \
            &>/tmp/xephyr-kindle.log &
        VIEWER_PID=$!

        # Wait for the Xephyr socket to appear (up to 3 s)
        for i in $(seq 1 30); do
            [[ -S "/tmp/.X11-unix/X${DISPLAY_NUM}" ]] && break
            sleep 0.1
        done

        if ! kill -0 "$VIEWER_PID" 2>/dev/null; then
            echo "WARNING: Xephyr exited unexpectedly. See /tmp/xephyr-kindle.log"
            cat /tmp/xephyr-kindle.log >&2
            echo "  Falling back to Xvfb + headless mode."
            VIEWER="none"
            VIEWER_PID=""
            start_xvfb
        else
            echo "   Xephyr PID: $VIEWER_PID"
        fi
        ;;

    x11vnc)
        need x11vnc "yay -S x11vnc"
        start_xvfb
        VNC_PORT=5999
        echo "→ Starting x11vnc on localhost:$VNC_PORT..."
        x11vnc \
            -display "$VDISPLAY" \
            -localhost \
            -nopw \
            -port "$VNC_PORT" \
            -forever \
            -bg \
            -quiet \
            -o /tmp/x11vnc-kindle.log
        # x11vnc forks itself (-bg), so we track by port rather than PID
        VIEWER_PID=""
        echo ""
        echo "  ┌─────────────────────────────────────────────────┐"
        echo "  │  VNC server ready on localhost:$VNC_PORT          │"
        echo "  │  Connect with:  vncviewer localhost:$VNC_PORT     │"
        echo "  │  or:            xdg-open vnc://localhost:$VNC_PORT │"
        echo "  └─────────────────────────────────────────────────┘"
        echo ""
        ;;

    feh)
        need feh  "sudo pacman -S feh"
        need xwd  "sudo pacman -S xorg-xwd"
        start_xvfb
        SCREENSHOT="/tmp/kindle-screen.png"
        REFRESH_INTERVAL=1   # seconds between refreshes

        echo "→ Starting feh screenshot loop (refresh every ${REFRESH_INTERVAL}s)..."
        echo "  Close the feh window to stop."

        # Take an initial screenshot so feh has something to open
        DISPLAY="$VDISPLAY" xwd -root -silent | \
            convert xwd:- "$SCREENSHOT" 2>/dev/null || true

        # Open feh in the background; it will be reloaded by the loop below
        feh \
            --title "$WIN_TITLE" \
            --zoom fill \
            "$SCREENSHOT" \
            &
        VIEWER_PID=$!
        sleep 0.4

        # Background loop: capture → convert → reload feh
        (
            while kill -0 "$VIEWER_PID" 2>/dev/null; do
                DISPLAY="$VDISPLAY" xwd -root -silent 2>/dev/null \
                    | convert xwd:- "$SCREENSHOT" 2>/dev/null || true
                # Signal feh to reload current image
                kill -USR1 "$VIEWER_PID" 2>/dev/null || true
                sleep "$REFRESH_INTERVAL"
            done
        ) &
        echo "   feh PID: $VIEWER_PID"
        ;;

    none)
        start_xvfb
        echo "→ Running headlessly on $VDISPLAY (no viewer)."
        ;;

    *)
        echo "ERROR: Unknown viewer '$VIEWER'. Choose: xephyr, x11vnc, feh, none"
        exit 1
        ;;
esac

# ── Launch the app ────────────────────────────────────────────────────────────
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Binary  : $BINARY"
echo "  Profile : $PROFILE"
echo "  Display : $VDISPLAY  (${KINDLE_W}x${KINDLE_H}, ${KINDLE_DPI}dpi)"
echo "  Root    : $HELLOKINDLE_DIR"
echo "  Log     : $HELLOKINDLE_DIR/log/app.log"
echo "  Viewer  : $VIEWER"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

DISPLAY="$VDISPLAY" \
RUST_BACKTRACE=1 \
    "$BINARY"

echo ""
echo "→ App exited (exit code $?)."
