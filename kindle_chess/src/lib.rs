//! kindli_chess library crate.
//!
//! The module tree lives here (not in `main.rs`) so integration tests under
//! `tests/` can link against it. `main.rs` is a thin binary over this crate.

pub mod api {
    pub mod board;
    pub mod client;
    pub mod github;
    pub mod litchee_adapter;
    pub mod oauth;
    pub mod traits;
    pub mod update;
}
pub mod app;
pub mod i18n;
pub mod local;
pub mod models;
pub mod system;
pub mod ui;
pub mod utils;
pub mod version;
