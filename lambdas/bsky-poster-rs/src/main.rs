use std::sync::OnceLock;

use anyhow::Result;
use atrium_api::{
    app::bsky::{
        embed::images::{ImageData, MainData},
        feed::post::{RecordData, RecordEmbedRefs},
    },
    types::string::Datetime,
};
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client as DynamoClient;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_secretsmanager::Client as SecretsManagerClient;
use aws_types::SdkConfig;
use bsky_sdk::{BskyAgent, rich_text::RichText};
use lambda_runtime::{Error, LambdaEvent, run, service_fn, tracing};
use reqwest::Client as HttpClient;
use serde::Deserialize;

struct ClientHandler {
    s3: S3Client,
    dynamo: DynamoClient,
    secrets_manager: SecretsManagerClient,
    http: HttpClient,
}

impl ClientHandler {
    pub fn new(config: &SdkConfig) -> Self {
        Self {
            s3: S3Client::new(config),
            dynamo: DynamoClient::new(config),
            secrets_manager: SecretsManagerClient::new(config),
            http: HttpClient::new(),
        }
    }
}

static CLIENTS: OnceLock<ClientHandler> = OnceLock::new();

#[derive(Deserialize)]
struct BSkyCredentials {
    #[serde(rename = "BSKY_USER")]
    username: String,

    #[serde(rename = "BSKY_PASSWORD")]
    password: String,
}

#[derive(Deserialize)]
struct Card {
    name: String,
    image_uris: ImageUri,
    set_name: String,
    flavor_text: String,
    artist: String,
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

#[derive(Deserialize)]
enum ImageUri {
    #[serde(rename = "art_crop")]
    ArtCrop(String),
}

async fn load_bsky_credentials(client: &SecretsManagerClient) -> Result<BSkyCredentials> {
    let resp = client
        .get_secret_value()
        .secret_id("bsky-artbot-credentials")
        .send()
        .await?;

    let Some(secret) = resp.secret_string() else {
        tracing::error!("no credentials found in secrets manager");
        std::process::exit(1);
    };

    Ok(serde_json::from_str(secret)?)
}

async fn download_card_data(client: &S3Client) -> Result<Vec<Card>> {
    let bucket = std::env::var("BUCKET")?;
    let key = std::env::var("BUCKET_KEY")?;

    let card_data = client.get_object().bucket(bucket).key(key).send().await?;
    let stream = card_data.body.collect().await?.into_bytes();

    Ok(serde_json::from_slice(&stream)?)
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

async fn retrieve_card<'a>(cards: &'a [Card], client: &DynamoClient) -> Result<&'a Card> {
    let db_name = std::env::var("DB_NAME")?;
    let total_cards = cards.len();
    let mut idx: usize = rand::random_range(0..total_cards);
    let mut card = &cards[idx];

    while posted_before(&db_name, card, client).await? {
        idx = rand::random_range(0..total_cards);
        card = &cards[idx];
    }

    Ok(card)
}

async fn create_image_embed(
    agent: &BskyAgent,
    client: &HttpClient,
    card: &Card,
) -> Result<ImageData> {
    let ImageUri::ArtCrop(url) = &card.image_uris;
    let image = client.get(url).send().await?.bytes().await?;
    let upload_response = agent
        .api
        .com
        .atproto
        .repo
        .upload_blob(image.to_vec())
        .await?;

    let embed = ImageData {
        image: upload_response.blob.clone(),
        alt: card.alt_text(),
        aspect_ratio: None,
    };

    Ok(embed)
}

async fn post_to_bluesky(agent: BskyAgent, image: ImageData, post_text: RichText) -> Result<()> {
    let image_embed =
        atrium_api::types::Union::Refs(RecordEmbedRefs::AppBskyEmbedImagesMain(Box::new(
            MainData {
                images: vec![image.into()],
            }
            .into(),
        )));

    let post = RecordData {
        created_at: Datetime::now(),
        text: post_text.text,
        facets: post_text.facets,
        embed: Some(image_embed),
        entities: None,
        labels: None,
        langs: None,
        reply: None,
        tags: None,
    };

    let result = agent.create_record(post).await?;
    tracing::info!("posted to bsky: {}", result.uri);
    Ok(())
}

async fn select_appropriate_card<'a>(
    cards: &'a [Card],
    client: &DynamoClient,
) -> Result<(&'a Card, String)> {
    let mut card = retrieve_card(cards, client).await?;
    let mut text = card.text();

    while text.len() > 300 {
        card = retrieve_card(cards, client).await?;
        text = card.text();
    }

    Ok((card, text))
}

async fn handler(_event: LambdaEvent<serde_json::Value>) -> Result<(), Error> {
    let config = aws_config::load_defaults(BehaviorVersion::v2025_08_07()).await;
    let clients = CLIENTS.get_or_init(|| ClientHandler::new(&config));
    let cards = download_card_data(&clients.s3).await?;
    let (card, text) = select_appropriate_card(&cards, &clients.dynamo).await?;
    let text = RichText::new_with_detect_facets(text).await?;

    let BSkyCredentials { username, password } =
        load_bsky_credentials(&clients.secrets_manager).await?;
    let agent = BskyAgent::builder().build().await?;
    agent.login(&username, &password).await?;
    let img_embed = create_image_embed(&agent, &clients.http, card).await?;
    post_to_bluesky(agent, img_embed, text).await?;

    tracing::info!("running lambda");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();
    run(service_fn(handler)).await?;

    Ok(())
}
