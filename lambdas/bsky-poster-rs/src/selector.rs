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
use uuid::Uuid;

// Number of times to check if cards have been posted
// If above this number, post regardless
const CHECK_ITERATIONS: usize = 5;

#[derive(Clone, Deserialize)]
pub struct DisplayCard {
    id: Uuid,
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
            id: card.id,
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
    let cards_ids = download_card_data(&clients.s3).await?;
    tracing::info!("successfully retrieved card id dataset");
    let card = select_appropriate_card(&cards_ids, clients).await?;
    tracing::info!("selected card - {card}");

    Ok(card)
}

async fn select_appropriate_card(
    cards: &[Uuid],
    clients: Arc<ClientHandler>,
) -> Result<DisplayCard> {
    let mut card = retrieve_card(cards, Arc::clone(&clients)).await?;
    let mut text = card.text();

    while text.len() > 300 {
        card = retrieve_card(cards, Arc::clone(&clients)).await?;
        text = card.text();
    }

    Ok(card)
}

async fn download_card_data(client: &S3Client) -> Result<Vec<Uuid>> {
    let bucket = std::env::var("BUCKET")?;
    let key = std::env::var("BUCKET_KEY")?;

    let card_data = client.get_object().bucket(bucket).key(key).send().await?;
    let stream = card_data.body.collect().await?.into_bytes();

    Ok(serde_json::from_slice(&stream)?)
}

async fn retrieve_card(card_ids: &[Uuid], client: Arc<ClientHandler>) -> Result<DisplayCard> {
    let db_name = std::env::var("DB_NAME")?;
    let total_cards = card_ids.len();
    let mut idx: usize = rand::random_range(0..total_cards);
    let mut card = fetch_card_from_s3(card_ids[idx], &client.s3).await?;

    for _ in 0..CHECK_ITERATIONS {
        if posted_before(&db_name, &card, &client.dynamo).await? {
            idx = rand::random_range(0..total_cards);
            card = fetch_card_from_s3(card_ids[idx], &client.s3).await?;
        } else {
            break;
        }
    }

    Ok(card)
}

async fn fetch_card_from_s3(id: Uuid, client: &S3Client) -> Result<DisplayCard> {
    let bucket = std::env::var("BUCKET")?;
    let card_data = client
        .get_object()
        .bucket(bucket)
        .key(format!("scryfall/{id}.json"))
        .send()
        .await?;

    let stream = card_data.body.collect().await?.into_bytes();
    let card: Card = serde_json::from_slice(&stream)?;

    Ok(DisplayCard::from_scryfall_card(&card))
}

async fn posted_before(db_name: &str, card: &DisplayCard, client: &DynamoClient) -> Result<bool> {
    match client
        .put_item()
        .table_name(db_name)
        .item("id", AttributeValue::S(card.id.into()))
        .condition_expression("attribute_not_exists(id)")
        .send()
        .await
    {
        Ok(_) => Ok(false),
        Err(e) => {
            let err = e.into_service_error();
            if err.is_conditional_check_failed_exception() {
                Ok(true)
            } else {
                Err(err.into())
            }
        }
    }
}
