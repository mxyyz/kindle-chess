use std::sync::{Arc, RwLock};

use crate::models::ui::{Display, Screen};
use crate::utils::config::AppConfig;

pub struct App {
    pub display: Display,
    pub screen_stack: Vec<Box<dyn Screen>>,
    /// App-wide configuration, loaded once at startup from the
    /// user-editable config file `<bundle>/config/settings.json` (see
    /// `AppConfig::load`). Cloned into the screens that read or update it;
    /// user choices confirmed at runtime live here for the rest of the app's
    /// lifetime.
    pub config: Arc<RwLock<AppConfig>>,
    /// True while the display is locked (DeviceLock seen, no DeviceWake yet) —
    /// screen redraws are suppressed so the custom screensaver stays up.
    pub locked: bool,
}
