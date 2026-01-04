use crate::ClientHandler;

use anyhow::Result;
use aws_sdk_dynamodb::Client as DynamoClient;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_s3::Client as S3Client;
use lambda_runtime::tracing;
use serde::Deserialize;

// Number of times to check if cards have been posted
// If above this number, post regardless
const CHECK_ITERATIONS: usize = 5;

#[derive(Clone, Deserialize)]
pub struct Card {
    pub name: String,
    pub image_uris: ImageUri,
    pub set_name: String,
    pub flavor_text: String,
    pub artist: String,
}

impl Card {
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

impl std::fmt::Display for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}) - '{}'",
            self.name, self.set_name, self.flavor_text
        )
    }
}

#[derive(Clone, Deserialize)]
pub enum ImageUri {
    #[serde(rename = "art_crop")]
    ArtCrop(String),
}

pub async fn select_card(clients: &ClientHandler) -> Result<Card> {
    let cards = download_card_data(&clients.s3).await?;
    tracing::info!("successfully retrieved card dataset");
    let card = select_appropriate_card(&cards, &clients.dynamo).await?;
    tracing::info!("selected card - {card}");

    Ok(card.clone())
}

async fn select_appropriate_card<'a>(cards: &'a [Card], client: &DynamoClient) -> Result<&'a Card> {
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

async fn retrieve_card<'a>(cards: &'a [Card], client: &DynamoClient) -> Result<&'a Card> {
    let db_name = std::env::var("DB_NAME")?;
    let total_cards = cards.len();
    let mut idx: usize = rand::random_range(0..total_cards);
    let mut card = &cards[idx];

    for _ in 0..CHECK_ITERATIONS {
        if posted_before(&db_name, card, client).await? {
            idx = rand::random_range(0..total_cards);
            card = &cards[idx];
        } else {
            break;
        }
    }

    Ok(card)
}

async fn posted_before(db_name: &str, card: &Card, client: &DynamoClient) -> Result<bool> {
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
