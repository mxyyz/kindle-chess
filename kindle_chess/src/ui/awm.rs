//! Awesome Window Manager (AWM) window-title conventions.
//!
//! The Kindle's tweaked Awesome Window Manager reads a window's role from its
//! X11 `WM_NAME` title, formatted as underscore-separated `KEY:VALUE` pairs,
//! e.g. the Kindle store uses `L:A_N:application_PC:TS_ID:com.lab126.store`.
//! GTK's `gtk_window_set_title` is just a `WM_NAME` write — here we do the same
//! directly over x11rb. Reference: kindlemodding.org/kindle-dev/awesome-window-manager
//!
//! Keys used here:
//! - `L` layer: `A` = app layer, `D` = dialog layer
//! - `N` role: `application` / `dialog`
//! - `PC` persistent chrome: `T` top bar, `S` search bar, `B` legacy bottom bar,
//!   `N` no bars (same as omitting PC)
//! - `ID` app identifier (reverse-domain, no underscores)
//! - `O` orientation mask: any combination of `U` `D` `L` `R`

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, PropMode, Window};

/// Reverse-domain identifier for this app, used as the AWM `ID` key and as the
/// appreg.db handlerId.
pub const APP_ID: &str = "org.kindlemodding.kindlechess";

/// Window layer (`L` key). `A`pp is the normal application layer; `D`ialog is
/// for modal dialogs (phase 2+).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    App,
    Dialog,
}

impl Layer {
    fn key(self) -> &'static str {
        match self {
            Layer::App => "A",
            Layer::Dialog => "D",
        }
    }
}

/// Window role (`N` key).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Application,
    Dialog,
}

impl Role {
    fn key(self) -> &'static str {
        match self {
            Role::Application => "application",
            Role::Dialog => "dialog",
        }
    }
}

/// Persistent chrome flags (`PC` key) — which Kindle chrome elements AWM keeps
/// visible around the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PersistentChrome {
    pub top_bar: bool,
    pub search_bar: bool,
    pub bottom_bar: bool,
}

impl PersistentChrome {
    /// `PC:N` — no bars; the app draws its own chrome (Phase 1 fullscreen mode).
    pub const NONE: Self = Self {
        top_bar: false,
        search_bar: false,
        bottom_bar: false,
    };

    /// `PC:T` — Kindle top bar (clock/battery/home, brightness via quick
    /// settings) stays visible (Phase 2 chrome mode).
    pub const TOP_BAR: Self = Self {
        top_bar: true,
        search_bar: false,
        bottom_bar: false,
    };

    fn key(self) -> String {
        if !self.top_bar && !self.search_bar && !self.bottom_bar {
            return "N".to_string();
        }
        let mut s = String::new();
        if self.top_bar {
            s.push('T');
        }
        if self.search_bar {
            s.push('S');
        }
        if self.bottom_bar {
            s.push('B');
        }
        s
    }
}

/// Supported-orientation mask (`O` key), serialized in `UDLR` order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Orientation {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

impl Orientation {
    /// Portrait-only (`O:U`), the Kindle's native orientation.
    pub const UP: Self = Self {
        up: true,
        down: false,
        left: false,
        right: false,
    };

    fn key(self) -> String {
        let mut s = String::new();
        if self.up {
            s.push('U');
        }
        if self.down {
            s.push('D');
        }
        if self.left {
            s.push('L');
        }
        if self.right {
            s.push('R');
        }
        s
    }
}

/// An AWM window title. Build with [`WindowTitle::fullscreen_app`] (Phase 1) or
/// [`WindowTitle::chrome_app`] (Phase 2), then [`WindowTitle::apply`] to set it
/// as the window's `WM_NAME`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowTitle {
    pub layer: Layer,
    pub role: Role,
    pub chrome: PersistentChrome,
    pub id: String,
    pub orientation: Orientation,
}

impl WindowTitle {
    /// Phase 1: fullscreen app window, no persistent chrome.
    /// `L:A_N:application_PC:N_ID:org.kindlemodding.kindlechess_O:U`
    pub fn fullscreen_app() -> Self {
        Self {
            layer: Layer::App,
            role: Role::Application,
            chrome: PersistentChrome::NONE,
            id: APP_ID.to_string(),
            orientation: Orientation::UP,
        }
    }

    /// Phase 2: app window with the Kindle top bar persistent.
    /// `L:A_N:application_PC:T_ID:org.kindlemodding.kindlechess_O:U`
    pub fn chrome_app() -> Self {
        Self {
            chrome: PersistentChrome::TOP_BAR,
            ..Self::fullscreen_app()
        }
    }

    /// Serialize to the underscore-separated `KEY:VALUE` title format AWM parses.
    pub fn to_title_string(&self) -> String {
        format!(
            "L:{}_N:{}_PC:{}_ID:{}_O:{}",
            self.layer.key(),
            self.role.key(),
            self.chrome.key(),
            self.id,
            self.orientation.key()
        )
    }

    /// Set this title as the window's `WM_NAME` property. Harmless on a plain
    /// X server (dev harness): the property is just set, no WM interprets it.
    pub fn apply<C: Connection>(
        &self,
        conn: &C,
        window: Window,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let title = self.to_title_string();
        conn.change_property(
            PropMode::REPLACE,
            window,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            8,
            title.len() as u32,
            title.as_bytes(),
        )?;
        conn.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fullscreen_app_title_format() {
        assert_eq!(
            WindowTitle::fullscreen_app().to_title_string(),
            "L:A_N:application_PC:N_ID:org.kindlemodding.kindlechess_O:U"
        );
    }

    #[test]
    fn chrome_app_title_uses_pc_t() {
        assert_eq!(
            WindowTitle::chrome_app().to_title_string(),
            "L:A_N:application_PC:T_ID:org.kindlemodding.kindlechess_O:U"
        );
    }

    #[test]
    fn chrome_none_serializes_as_n() {
        assert_eq!(PersistentChrome::NONE.key(), "N");
        assert_eq!(PersistentChrome::TOP_BAR.key(), "T");
        let ts = PersistentChrome {
            top_bar: true,
            search_bar: true,
            bottom_bar: false,
        };
        assert_eq!(ts.key(), "TS");
    }

    #[test]
    fn orientation_mask_udlr_order() {
        assert_eq!(Orientation::UP.key(), "U");
        let all = Orientation {
            up: true,
            down: true,
            left: true,
            right: true,
        };
        assert_eq!(all.key(), "UDLR");
    }

    #[test]
    fn app_id_has_no_underscore() {
        // '_' is the title delimiter; an underscore in ID would corrupt parsing.
        assert!(!APP_ID.contains('_'));
    }
}
