use anyhow::{Context, Result};
use lambda_runtime::tracing;

use flate2::read::GzDecoder;
use scryone::{
    api::{
        request::{BulkDataFromIdRequest, BulkDataId},
        ScryfallClient,
    },
    objects::{BulkDataType, Card},
};
use std::io::Read;

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

    let bulk_response = client
        .call_raw(result.jsonl_download_uri)
        .await
        .context("downloading gzipped bulk data")?;

    tracing::info!("downloaded gzipped card data");
    let mut raw_cards = Vec::new();
    let mut decoder = GzDecoder::new(&bulk_response[..]);
    decoder
        .read_to_end(&mut raw_cards)
        .context("decoding gzipped data")?;

    let cards: Vec<Card> = raw_cards
        .split(|b| *b == b'\n')
        .filter_map(|arr| {
            if arr.is_empty() {
                return None;
            }

            Some(serde_json::from_slice(arr).expect("card should be valid"))
        })
        .collect();

    tracing::info!("decompressed card data");
    let cards_len = cards.len();
    let filtered_cards: Vec<Card> = cards.into_iter().filter(|c| !is_invalid(c)).collect();
    tracing::info!(
        "downloaded cards :: {} oracle cards :: {} unique cards",
        cards_len,
        filtered_cards.len()
    );

    Ok(filtered_cards)
}
