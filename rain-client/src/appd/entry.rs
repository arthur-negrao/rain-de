use zbus::Connection;

use crate::dto::AppEntryDTO;

#[zbus::proxy(
    interface = "org.rain.Appd",
    default_service = "org.rain.Appd",
    default_path = "/org/rain/Appd"
)]
pub trait Appd {
    async fn get_all_ids(&self) -> zbus::Result<Vec<String>>;

    async fn get_entry(&self, app_id: &str) -> zbus::Result<AppEntryDTO>;

    async fn get_all_entries(&self) -> zbus::Result<Vec<AppEntryDTO>>;
}

pub async fn connect_to_appd() -> zbus::Result<AppdProxy<'static>> {
    let connection = Connection::session().await?;

    AppdProxy::new(&connection).await
}
