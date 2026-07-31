use anyhow::Result;
use aws_sdk_secretsmanager::Client as SecretsManagerClient;

use jacquard::{
    api::app_bsky::{
        embed::images::{Image, Images},
        feed::post::{Post, PostEmbed},
    },
    client::{
        Agent, AgentSessionExt, AtpSession, MemorySessionStore,
        credential_session::CredentialSession,
    },
    identity::JacquardResolver,
    richtext::RichText,
    session::SessionKey,
    types::{
        blob::{Blob, MimeType},
        string::Datetime,
    },
};
use lambda_runtime::tracing;
use reqwest::Client as HttpClient;
use serde::Deserialize;
use smol_str::SmolStr;
use std::sync::Arc;

use crate::{ClientHandler, selector::DisplayCard};

type SessionType =
    CredentialSession<MemorySessionStore<SessionKey, AtpSession>, JacquardResolver<HttpClient>>;

#[derive(Deserialize)]
struct BSkyCredentials {
    #[serde(rename = "BSKY_USER")]
    username: String,

    #[serde(rename = "BSKY_PASSWORD")]
    password: String,
}

pub async fn post(clients: Arc<ClientHandler>, card: DisplayCard) -> Result<()> {
    let agent = initialise_agent(&clients.secrets_manager).await?;
    tracing::info!("logged into bsky successfully");
    let img = upload_image(&clients.http, &agent, &card).await?;
    let post = create_post(&agent, img, &card).await?;
    let output = agent.create_record(post, None).await?;
    tracing::info!("posted to bsky: {}", output.uri);

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

async fn initialise_agent(client: &SecretsManagerClient) -> Result<Agent<SessionType>> {
    let BSkyCredentials { username, password } = load_bsky_credentials(client).await?;

    let store = Arc::new(MemorySessionStore::default());
    let resolver = Arc::new(JacquardResolver::new(
        reqwest::Client::new(),
        Default::default(),
    ));
    let session = CredentialSession::new(store, resolver);
    if let Err(e) = session
        .login(&username, &password, None, None, None, None)
        .await
    {
        anyhow::bail!("error logging in: {e:?}");
    };

    Ok(Agent::from(session))
}

async fn upload_image(
    client: &HttpClient,
    agent: &Agent<SessionType>,
    card: &DisplayCard,
) -> Result<Blob> {
    let url = card.art_crop.clone();
    let image = client.get(url).send().await?.bytes().await?;
    let mime_type = MimeType::new("image/jpeg");
    Ok(agent.upload_blob(image, mime_type).await?)
}

async fn create_post(agent: &Agent<SessionType>, img: Blob, card: &DisplayCard) -> Result<Post> {
    let post_text = RichText::parse(card.text()).build_async(agent).await?;
    let image = Image {
        alt: SmolStr::from(card.alt_text()),
        image: img.into(),
        aspect_ratio: None,
        extra_data: Default::default(),
    };

    let embed = PostEmbed::Images(Box::new(Images {
        images: vec![image],
        extra_data: Default::default(),
    }));

    Ok(Post {
        text: post_text.text,
        created_at: Datetime::now(),
        embed: Some(embed),
        entities: None,
        facets: post_text.facets,
        labels: None,
        langs: None,
        reply: None,
        tags: None,
        extra_data: Default::default(),
    })
}
