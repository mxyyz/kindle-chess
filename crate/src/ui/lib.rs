pub mod app;
pub mod events;
pub mod renderer;
pub mod widgets;

pub use app::ChessApp;
pub use events::{AppEvent, TouchEvent, TouchKind};
pub use renderer::Renderer;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_imports() {
        // This should compile if modules are set up correctly
        let _ = std::mem::size_of::<widgets::BoardWidget>();
        let _ = std::mem::size_of::<widgets::SidebarWidget>();
    }
}
