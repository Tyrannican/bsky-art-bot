use std::sync::Arc;

use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client as DynamoClient;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_secretsmanager::Client as SecretsManagerClient;
use aws_types::SdkConfig;
use lambda_runtime::{Error, LambdaEvent, run, service_fn, tracing};
use reqwest::Client as HttpClient;

mod bsky;
mod selector;

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

async fn handler(
    _event: LambdaEvent<serde_json::Value>,
    clients: Arc<ClientHandler>,
) -> Result<(), Error> {
    let card = selector::select_card(Arc::clone(&clients)).await?;
    bsky::post(clients, card).await?;
    tracing::info!("successfully sent post");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let clients = Arc::new(ClientHandler::new(&config));

    tracing::init_default_subscriber();
    run(service_fn(move |event| {
        let clients = Arc::clone(&clients);
        async move { handler(event, clients).await }
    }))
    .await
}
