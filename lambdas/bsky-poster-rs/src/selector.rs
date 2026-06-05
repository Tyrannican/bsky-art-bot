use crate::ClientHandler;
use std::sync::Arc;

use anyhow::Result;
use aws_sdk_dynamodb::Client as DynamoClient;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_s3::Client as S3Client;
use lambda_runtime::tracing;
use scryone::objects::Card;
use serde::Deserialize;
use url::Url;

// Number of times to check if cards have been posted
// If above this number, post regardless
const CHECK_ITERATIONS: usize = 5;

#[derive(Clone, Deserialize)]
pub struct DisplayCard {
    pub name: String,
    pub art_crop: Url,
    pub set_name: String,
    pub flavor_text: String,
    pub artist: String,
}

impl DisplayCard {
    pub fn from_scryfall_card(card: &Card) -> Self {
        let flavor_text = card
            .flavor_text
            .as_ref()
            .expect("flavor text should be present due to filters");

        let artist = card
            .artist
            .as_ref()
            .expect("artist should be present due to filters");

        let image_uris = card
            .image_uris
            .as_ref()
            .expect("image uris should be present due to filters");

        Self {
            name: card.name.clone(),
            set_name: card.set_name.clone(),
            flavor_text: flavor_text.to_string(),
            artist: artist.to_string(),
            art_crop: image_uris.art_crop.clone(),
        }
    }

    pub fn text(&self) -> String {
        format!(
            "{} ({})\nArtist: {}\n\n{}\n\n#magicthegathering #mtg",
            self.name, self.set_name, self.artist, self.flavor_text
        )
    }

    pub fn alt_text(&self) -> String {
        format!(
            "Art for the Magic: the Gathering card '{}' from the set '{}' by the artist '{}'",
            self.name, self.set_name, self.artist
        )
    }
}

impl std::fmt::Display for DisplayCard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}) - '{}'",
            self.name, self.set_name, self.flavor_text
        )
    }
}

pub async fn select_card(clients: Arc<ClientHandler>) -> Result<DisplayCard> {
    let cards = download_card_data(&clients.s3).await?;
    tracing::info!("successfully retrieved card dataset");
    let card = select_appropriate_card(&cards, &clients.dynamo).await?;
    tracing::info!("selected card - {card}");

    Ok(card)
}

async fn select_appropriate_card(cards: &[Card], client: &DynamoClient) -> Result<DisplayCard> {
    let mut card = retrieve_card(cards, client).await?;
    let mut text = card.text();

    while text.len() > 300 {
        card = retrieve_card(cards, client).await?;
        text = card.text();
    }

    Ok(card)
}

async fn download_card_data(client: &S3Client) -> Result<Vec<Card>> {
    let bucket = std::env::var("BUCKET")?;
    let key = std::env::var("BUCKET_KEY")?;

    let card_data = client.get_object().bucket(bucket).key(key).send().await?;
    let stream = card_data.body.collect().await?.into_bytes();

    Ok(serde_json::from_slice(&stream)?)
}

async fn retrieve_card(cards: &[Card], client: &DynamoClient) -> Result<DisplayCard> {
    let db_name = std::env::var("DB_NAME")?;
    let total_cards = cards.len();
    let mut idx: usize = rand::random_range(0..total_cards);
    let mut card = DisplayCard::from_scryfall_card(&cards[idx]);

    for _ in 0..CHECK_ITERATIONS {
        if posted_before(&db_name, &card, client).await? {
            idx = rand::random_range(0..total_cards);
            card = DisplayCard::from_scryfall_card(&cards[idx]);
        } else {
            break;
        }
    }

    Ok(card)
}

async fn posted_before(db_name: &str, card: &DisplayCard, client: &DynamoClient) -> Result<bool> {
    let resp = client
        .get_item()
        .table_name(db_name)
        .key("name", AttributeValue::S(card.name.to_owned()))
        .key("set", AttributeValue::S(card.set_name.to_owned()))
        .send()
        .await?;

    if resp.item().is_some() {
        return Ok(true);
    }

    client
        .put_item()
        .table_name(db_name)
        .item("name", AttributeValue::S(card.name.to_owned()))
        .item("set", AttributeValue::S(card.set_name.to_owned()))
        .send()
        .await?;

    Ok(false)
}
