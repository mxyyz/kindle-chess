//! On-device system integration (lipc services). Everything here degrades to a
//! no-op off-device so the dev harness (Xephyr) is unaffected.

pub mod pillow;
pub mod powerd;
