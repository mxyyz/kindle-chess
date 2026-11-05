//! Pillow (the Kindle's UI framework) display handoff + eips helpers.
//!
//! While pillow is *disabled* (`disableEnablePillow disable`), X11 clients own
//! the screen and no Kindle chrome (status bar) is drawn. While it is
//! *enabled*, pillow/AWM composite the status bar over X11 windows whose title
//! carries `PC:T`. Toggling is done from the shell tools; off-device (dev
//! harness) they do not exist and every call is a logged no-op.

use std::process::{Command, Stdio};

use log::{info, warn};

fn lipc_available() -> bool {
    Command::new("lipc-set-prop")
        .arg("-h")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

/// Disable pillow so X11 owns the screen (fullscreen mode).
/// Returns true when the command ran on-device.
pub fn disable_pillow() -> bool {
    set_pillow_disabled(true)
}

/// Re-enable pillow so the Kindle status bar (PC:T chrome) is drawn over X11.
pub fn enable_pillow() -> bool {
    set_pillow_disabled(false)
}

fn set_pillow_disabled(disabled: bool) -> bool {
    if !lipc_available() {
        info!("pillow toggle: lipc tools not available (off-device?) — no-op");
        return false;
    }
    let value = if disabled { "disable" } else { "enable" };
    match Command::new("lipc-set-prop")
        .args(["com.lab126.pillow", "disableEnablePillow", value])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(s) if s.success() => {
            info!("pillow disableEnablePillow={}", value);
            true
        }
        Ok(s) => {
            warn!("lipc-set-prop pillow failed with status {}", s);
            false
        }
        Err(e) => {
            warn!("lipc-set-prop pillow failed: {}", e);
            false
        }
    }
}

/// `eips -c` — clear the e-ink framebuffer (used when handing the display back
/// to our fullscreen window). No-op off-device.
pub fn eips_clear() {
    if Command::new("eips")
        .args(["-c"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_err()
    {
        info!("eips not available (off-device?) — clear skipped");
    }
}
