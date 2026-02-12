mod app;
mod bar;
mod config;
mod modules;
mod settings;
mod theme;
mod widgets;

use gtk::prelude::*;
use std::sync::OnceLock;
use tokio::runtime::Runtime;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// Get or initialize the shared Tokio runtime
pub fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        Runtime::new().expect("Failed to create Tokio runtime")
    })
}

/// Spawn a future on the shared Tokio runtime
pub fn spawn<F>(f: F) -> tokio::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    runtime().spawn(f)
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ferritebar=info".parse().unwrap()),
        )
        .init();

    // Initialize the runtime before GTK
    let _ = runtime();

    // Check for CLI subcommands
    let args: Vec<String> = std::env::args().collect();
    let open_settings = args.get(1).map(|s| s.as_str()) == Some("settings");

    let application = gtk::Application::builder()
        .application_id("com.ferritebar.bar")
        .build();

    if open_settings {
        application.connect_activate(|app| {
            settings::open(app);
        });
    } else {
        application.connect_activate(app::activate);
    }

    application.run_with_args::<String>(&[]);
}
