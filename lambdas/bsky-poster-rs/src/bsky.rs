use anyhow::Result;
use atrium_api::{
    app::bsky::{
        embed::images::{ImageData, MainData},
        feed::post::{RecordData, RecordEmbedRefs},
    },
    types::string::Datetime,
};
use aws_sdk_secretsmanager::Client as SecretsManagerClient;
use bsky_sdk::{BskyAgent, rich_text::RichText};
use lambda_runtime::tracing;
use reqwest::Client as HttpClient;
use serde::Deserialize;

use crate::{
    ClientHandler,
    selector::{Card, ImageUri},
};

#[derive(Deserialize)]
struct BSkyCredentials {
    #[serde(rename = "BSKY_USER")]
    username: String,

    #[serde(rename = "BSKY_PASSWORD")]
    password: String,
}

pub async fn post(clients: &ClientHandler, card: Card) -> Result<()> {
    let text = RichText::new_with_detect_facets(card.text()).await?;
    let BSkyCredentials { username, password } =
        load_bsky_credentials(&clients.secrets_manager).await?;

    let agent = BskyAgent::builder().build().await?;
    agent.login(&username, &password).await?;
    tracing::info!("logged into bsky successfully");
    let img_embed = create_image_embed(&agent, &clients.http, &card).await?;
    post_to_bluesky(agent, img_embed, text).await?;
    Ok(())
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
