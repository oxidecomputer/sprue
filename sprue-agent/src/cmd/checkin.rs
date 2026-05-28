use chrono::Utc;
use sprue_sdk::{Client, types::TypedUuidForServerRegistrationId};

pub struct CheckinRequest<'a> {
    pub client: &'a Client,
    pub registration_id: TypedUuidForServerRegistrationId,
}

pub async fn checkin(request: CheckinRequest<'_>) -> anyhow::Result<()> {
    let CheckinRequest {
        client,
        registration_id,
    } = request;
    client
        .checkin_server()
        .server(registration_id)
        .body_map(|body| body.checked_in_at(Utc::now()))
        .send()
        .await?;
    Ok(())
}
