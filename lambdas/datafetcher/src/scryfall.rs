use anyhow::{Context, Result};
use lambda_runtime::tracing;

use scryone::{
    api::{
        request::{BulkDataFromIdRequest, BulkDataId},
        ScryfallClient,
    },
    objects::{BulkDataType, Card},
};

fn is_invalid(card: &Card) -> bool {
    card.content_warning.is_some()
        || card.set_type == "funny"
        || card.set_name == "Unknown Event"
        || card.flavor_text.is_none()
        || card.artist.is_none()
        || card.image_uris.is_none()
}

pub async fn download() -> Result<Vec<Card>> {
    let client = ScryfallClient::new();
    let req = BulkDataFromIdRequest::builder()
        .data_type(BulkDataId::Type(BulkDataType::OracleCards))
        .build()?;

    let result = client.get(req).await.context("retrieving bulk metadata")?;
    tracing::info!("download bulk metadata");

    let cards: Vec<Card> = client
        .call(result.download_uri)
        .await
        .context("downloading bulk card data")?;

    tracing::info!("downloaded card data");
    let cards_len = cards.len();
    let filtered_cards: Vec<Card> = cards.into_iter().filter(|c| !is_invalid(&c)).collect();
    tracing::info!(
        "downloaded cards :: {} oracle cards :: {} unique cards",
        cards_len,
        filtered_cards.len()
    );

    Ok(filtered_cards)
}
