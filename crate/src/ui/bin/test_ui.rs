use kindle_x11_test::ChessApp;
use log::error;

fn main() {
    // Initialize logging
    env_logger::builder()
        .target(env_logger::Target::Stdout)
        .filter_level(log::LevelFilter::Debug)
        .format_timestamp(None)
        .init();

    // Run the app
    match ChessApp::new() {
        Ok(app) => {
            if let Err(e) = app.run() {
                error!("Application error: {}", e);
            }
        }
        Err(e) => {
            error!("Failed to initialize: {}", e);
        }
    }
}
