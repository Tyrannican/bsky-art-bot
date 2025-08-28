use std::collections::HashMap;

use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_secretsmanager::Client as SecretsManagerClient;
use aws_types::SdkConfig;
use bsky_sdk::BskyAgent;
use lambda_runtime::{Error, LambdaEvent, run, service_fn, tracing};
use serde::Deserialize;

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
    image_uris: HashMap<String, String>,
    set_name: String,
    flavor_test: String,
    artist: String,
}

fn parse_json_str<T: serde::de::DeserializeOwned>(input: &str) -> Result<T> {
    Ok(serde_json::from_str(input)?)
}

fn parse_json_slice<T: serde::de::DeserializeOwned>(input: &[u8]) -> Result<T> {
    Ok(serde_json::from_slice(input)?)
}

async fn load_bsky_credentials(config: &SdkConfig) -> Result<BSkyCredentials> {
    let client = SecretsManagerClient::new(&config);

    let resp = client
        .get_secret_value()
        .secret_id("bsky-artbot-credentials")
        .send()
        .await?;

    let Some(secret) = resp.secret_string() else {
        tracing::error!("no credentials found in secrets manager");
        std::process::exit(1);
    };

    Ok(parse_json_str(secret)?)
}

async fn download_card_data(config: &SdkConfig) -> Result<Vec<Card>> {
    let client = S3Client::new(&config);
    let bucket = std::env::var("BUCKET")?;
    let key = std::env::var("BUCKET_KEY")?;

    let card_data = client.get_object().bucket(bucket).key(key).send().await?;
    let Some(stream) = card_data.body().bytes() else {
        tracing::error!("no bytes in object bytestream");
        std::process::exit(1);
    };

    Ok(parse_json_slice(stream)?)
}

async fn handler(_event: LambdaEvent<serde_json::Value>) -> Result<(), Error> {
    let config = aws_config::load_defaults(BehaviorVersion::v2025_08_07()).await;
    let cards = download_card_data(&config).await?;
    let BSkyCredentials { username, password } = load_bsky_credentials(&config).await?;
    let agent = BskyAgent::builder().build().await?;
    agent.login(&username, &password).await?;
    tracing::info!("running lambda");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();
    run(service_fn(handler)).await?;

    Ok(())
}
