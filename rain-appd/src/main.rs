use zbus::connection;

use rain_appd::{server::daemon::Daemon, state::state::DaemonState};
use rain_utils::log::telemetry::init_telemetry;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = init_telemetry();

    let mut state = DaemonState::new();
    state.find_entries();
    let daemon = Daemon::new(state);

    let _connection = connection::Builder::session()?
        .name("org.rain.Appd")?
        .serve_at("/org/rain/Appd", daemon)?
        .build()
        .await?;

    loop {
        std::future::pending::<()>().await;
    }
}
