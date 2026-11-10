//! Display-lock-aware powerd watcher via the Kindle's lipc service.
//!
//! On-device we listen with `lipc-wait-event -m com.lab126.powerd "*"`. The two
//! events we care about:
//! - `goingToScreenSaver`  → display locked / asleep  → AppEvent::DeviceLock
//! - `outOfScreenSaver`    → display unlocked / woken   → AppEvent::DeviceWake
//!
//! The lock event is what a custom screensaver (future task) will draw on; the
//! wake event triggers a full redraw so the e-ink panel comes back clean.
//! Off-device (dev harness) the binary does not exist: we log once and the
//! watcher thread exits — the app then simply never receives these events.
//!
//! Reference: kindlemodding.org/kindle-apps-and-services (lipc CLI); powerd
//! event names confirmed against on-device `lipc-wait-event` output.

use std::io::{BufRead, BufReader};
use std::process::{ChildStdout, Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use log::{info, warn};

use crate::ui::events::AppEvent;

/// Minimum gap between two emitted events of the same kind — powerd can emit a
/// burst of duplicate events around one physical lock/unlock. Leading-edge:
/// the first event passes immediately (no added latency), repeats within the
/// window are swallowed. 500 ms is enough to coalesce bursts without a
/// perceptible delay between button press and screen update.
pub const EVENT_DEBOUNCE: Duration = Duration::from_millis(500);

/// A classified powerd event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerEvent {
    /// Display locked / going to screensaver (`goingToScreenSaver`).
    Lock,
    /// Display unlocked / woken (`outOfScreenSaver`).
    Wake,
}

/// Map a raw `lipc-wait-event` output line to a power event.
/// Lines look like: `outOfScreenSaver 1` / `goingToScreenSaver 2`.
pub fn classify_powerd_event(line: &str) -> Option<PowerEvent> {
    if line.starts_with("goingToScreenSaver") {
        Some(PowerEvent::Lock)
    } else if line.starts_with("outOfScreenSaver") {
        Some(PowerEvent::Wake)
    } else {
        None
    }
}

/// Pure debounce decision — unit-tested without spawning processes.
pub fn should_emit(last: Option<Instant>, now: Instant) -> bool {
    match last {
        Some(last) => now.duration_since(last) >= EVENT_DEBOUNCE,
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
            "powerd watcher: lipc-wait-event not available (off-device?) — lock/wake detection disabled"
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
        let mut last_lock: Option<Instant> = None;
        let mut last_wake: Option<Instant> = None;
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            let Some(event) = classify_powerd_event(&line) else {
                continue;
            };
            let now = Instant::now();
            let app_event = match event {
                PowerEvent::Lock => {
                    if !should_emit(last_lock, now) {
                        continue;
                    }
                    last_lock = Some(now);
                    AppEvent::DeviceLock
                }
                PowerEvent::Wake => {
                    if !should_emit(last_wake, now) {
                        continue;
                    }
                    last_wake = Some(now);
                    AppEvent::DeviceWake
                }
            };
            info!("powerd watcher: {:?}", event);
            if event_tx.send(app_event).is_err() {
                break; // Main thread dropped the receiver — app is quitting
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
    fn classifies_lock_and_wake() {
        assert_eq!(
            classify_powerd_event("goingToScreenSaver 2"),
            Some(PowerEvent::Lock)
        );
        assert_eq!(
            classify_powerd_event("outOfScreenSaver 1"),
            Some(PowerEvent::Wake)
        );
        assert_eq!(classify_powerd_event("battLevelChanged 21"), None);
        assert_eq!(classify_powerd_event("charging"), None);
    }

    #[test]
    fn first_event_always_emits() {
        assert!(should_emit(None, Instant::now()));
    }

    #[test]
    fn event_within_debounce_is_suppressed() {
        let t0 = Instant::now();
        assert!(!should_emit(Some(t0), t0 + Duration::from_millis(200)));
    }

    #[test]
    fn event_after_debounce_emits() {
        let t0 = Instant::now();
        assert!(should_emit(Some(t0), t0 + EVENT_DEBOUNCE));
        assert!(should_emit(
            Some(t0),
            t0 + EVENT_DEBOUNCE + Duration::from_secs(5)
        ));
    }
}
