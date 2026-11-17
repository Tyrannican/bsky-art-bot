use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_sdk_s3::{Client, primitives::ByteStream};
use uuid::Uuid;

pub async fn send_ids(ids: Vec<Uuid>) -> Result<()> {
    let bucket = std::env::var("BUCKET")?;
    let cfg = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let client = Client::new(&cfg);
    let body = serde_json::to_string(&ids)?;

    client
        .put_object()
        .bucket(bucket)
        .key("scryfall/oracle-card-ids.json")
        .body(ByteStream::from(body.into_bytes()))
        .send()
        .await?;

    Ok(())
}
