//! Device wake detection via the Kindle's powerd lipc service.
//!
//! On-device we listen with `lipc-wait-event -m com.lab126.powerd "*"`; the
//! resume/wake event is `outOfScreenSaver` (power button press while asleep).
//! Off-device (dev harness) the binary does not exist: we log once and the
//! watcher thread exits — the app then simply never receives DeviceWake.
//!
//! Reference: kindlemodding.org/kindle-apps-and-services (lipc CLI), powerd
//! event names confirmed against on-device `lipc-wait-event` output.

use std::io::{BufRead, BufReader};
use std::process::{ChildStdout, Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use log::{info, warn};

use crate::ui::events::AppEvent;

/// Minimum gap between two emitted wake events — powerd can emit a burst
/// (e.g. `outOfScreenSaver` plus related events) around one physical wake.
pub const WAKE_DEBOUNCE: Duration = Duration::from_secs(2);

/// Pure debounce decision — unit-tested without spawning processes.
pub fn should_emit_wake(last: Option<Instant>, now: Instant) -> bool {
    match last {
        Some(last) => now.duration_since(last) >= WAKE_DEBOUNCE,
        None => true,
    }
}

/// Spawn the powerd watcher thread. Returns `None` off-device (no lipc tools).
pub fn spawn_powerd_watcher(event_tx: Sender<AppEvent>) -> Option<std::thread::JoinHandle<()>> {
    if Command::new("lipc-wait-event")
        .arg("-h")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_err()
    {
        info!(
            "powerd watcher: lipc-wait-event not available (off-device?) — wake detection disabled"
        );
        return None;
    }

    Some(std::thread::spawn(move || {
        let mut child = match Command::new("lipc-wait-event")
            .args(["-m", "com.lab126.powerd", "*"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                warn!("powerd watcher: failed to spawn lipc-wait-event: {}", e);
                return;
            }
        };

        let stdout: ChildStdout = match child.stdout.take() {
            Some(s) => s,
            None => return,
        };

        info!("powerd watcher: listening for com.lab126.powerd events");
        let mut last_wake: Option<Instant> = None;
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            // Lines look like: `outOfScreenSaver 1` / `goingToScreenSaver 2`
            if line.starts_with("outOfScreenSaver") {
                let now = Instant::now();
                if should_emit_wake(last_wake, now) {
                    info!("powerd watcher: device wake (outOfScreenSaver)");
                    last_wake = Some(now);
                    if event_tx.send(AppEvent::DeviceWake).is_err() {
                        break; // Main thread dropped the receiver — app is quitting
                    }
                }
            }
        }

        let _ = child.kill();
        info!("powerd watcher: listener ended");
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_wake_always_emits() {
        assert!(should_emit_wake(None, Instant::now()));
    }

    #[test]
    fn wake_within_debounce_is_suppressed() {
        let t0 = Instant::now();
        assert!(should_emit_wake(Some(t0), t0 + Duration::from_millis(500)) == false);
    }

    #[test]
    fn wake_after_debounce_emits() {
        let t0 = Instant::now();
        assert!(should_emit_wake(Some(t0), t0 + WAKE_DEBOUNCE));
        assert!(should_emit_wake(
            Some(t0),
            t0 + WAKE_DEBOUNCE + Duration::from_secs(5)
        ));
    }
}
