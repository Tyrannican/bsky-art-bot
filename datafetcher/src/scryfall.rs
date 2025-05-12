use anyhow::Result;
use lambda_runtime::tracing;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const URL: &str = "https://api.scryfall.com/bulk-data";

#[derive(Deserialize, Debug)]
struct BulkData {
    data: Vec<BulkEntry>,
}

#[derive(Deserialize, Debug)]
struct BulkEntry {
    #[serde(rename = "download_uri")]
    url: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Card {
    name: String,
    #[serde(rename = "image_uris")]
    images: Option<CardImageData>,
    set_name: String,
    flavor_text: Option<String>,
    artist: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
struct CardImageData {
    art_crop: Option<String>,
}

impl Card {
    pub fn is_invalid(&self) -> bool {
        match self.set_name.as_str() {
            "Unglued" | "Unhinged" | "Unsanctioned" | "Unfinity" | "Unstable" => return true,
            _ => {}
        }

        if self.set_name == "Unknown Event" {
            return true;
        }

        if self.flavor_text.is_none() || self.artist.is_none() {
            return true;
        }

        if let Some(images) = &self.images {
            if images.art_crop.is_none() {
                return true;
            }
        }

        false
    }
}

async fn download_data<T: serde::de::DeserializeOwned>(url: &str) -> Result<T> {
    let client = Client::new();
    let data = client
        .get(url)
        .header("accept", "application/json")
        .header("user-agent", "reqwest")
        .send()
        .await?
        .json::<T>()
        .await?;

    Ok(data)
}

pub async fn download() -> Result<Vec<Card>> {
    let bulk = download_data::<BulkData>(URL).await?;
    tracing::info!("downloaded bulk card data");
    let cards = download_data::<Vec<Card>>(&bulk.data[0].url).await?;
    let cards_len = cards.len();
    let filtered_cards: Vec<Card> = cards.into_iter().filter(|c| !c.is_invalid()).collect();
    tracing::info!(
        "downloaded cards :: {} oracle cards :: {} unique cards",
        cards_len,
        filtered_cards.len()
    );

    Ok(filtered_cards)
}
