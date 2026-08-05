pub fn init_telemetry() -> tracing_appender::non_blocking::WorkerGuard {
    let (writer, guard) = tracing_appender::non_blocking(std::io::stdout());

    let level_string = std::env::var("RUST_LOG").unwrap_or("debug".to_string());
    let log_level = match level_string.as_str() {
        "debug" => tracing::Level::DEBUG,
        "info" => tracing::Level::INFO,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_writer(writer)
        .init();

    guard
}
