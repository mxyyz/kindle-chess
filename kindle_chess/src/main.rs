use log::error;

use log::LevelFilter;
use log::info;
use log4rs::Handle;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;

use kindli_chess::models::app::App;
use kindli_chess::version;

#[tokio::main]
async fn main() {
    match init_log() {
        Ok(_handle) => {
            println!("Logger initialized successfully.");
            info!("kindle-hello {}", version::full());
            info!("Creating App instance..");

            match App::new() {
                Ok(app) => {
                    info!("App instance created. Starting...");
                    app.run()
                }
                Err(e) => {
                    error!("Failed to initialize App: {}", e);
                }
            };

            info!("App instancing finished");
        }
        Err(e) => {
            eprintln!("Failed to initialize logger: {}", e);
            std::process::exit(1);
        }
    }
}
// ]}
// info!("Creating App instance..");
// let app = match App::new() {
//     Ok(app) => {
//         info!("Starting App instance...");
//         app.run()
//     }
//     Err(e) => {
//         error!("Failed to initialize App: {}", e);
//     }
// };

// let app = App::new();
// app.unwrap().run();
// }

fn init_log() -> Result<Handle, Box<dyn std::error::Error>> {
    // LOGGING
    // 1. Appender für die Datei definieren
    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {m}{n}")))
        .build(concat!(env!("LOG_FILE_DIR"), "app.log"))?;

    // 2. Logging-Konfiguration erstellen
    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(Root::builder().appender("logfile").build(LevelFilter::Info))?;

    // 3. Logger initialisieren
    let handle = log4rs::init_config(config)?;

    // 4. Loslegen mit dem Logging
    info!("Anwendung gestartet. Log-Nachrichten werden in 'app.log' geschrieben.");

    Ok(handle)
}
