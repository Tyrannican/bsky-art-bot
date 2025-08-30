use anyhow::Result;
use aws_sdk_s3::{primitives::ByteStream, Client};

use crate::scryfall::Card;

async fn load_client() -> Client {
    let config = aws_config::load_from_env().await;
    Client::new(&config)
}

pub async fn upload_cards(cards: Vec<Card>) -> Result<()> {
    let client = load_client().await;
    let pretty = serde_json::to_string_pretty(&cards)?;
    let body = ByteStream::from(pretty.into_bytes());
    client
        .put_object()
        .bucket("muspelheim")
        .key("scryfall-oracle-cards.json")
        .body(body)
        .send()
        .await?;

    Ok(())
}
