use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_sdk_sqs::{Client, types::SendMessageBatchRequestEntry};
use futures::StreamExt;
use std::sync::Arc;

use scryone::objects::Card;

const DEFAULT_BATCH_SIZE: usize = 10;

async fn load_client() -> Arc<Client> {
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    Arc::new(Client::new(&config))
}

pub async fn send_to_processor(cards: Vec<Card>) -> Result<()> {
    let client = load_client().await;
    let url = std::env::var("PROCESSOR_QUEUE_URL")?;

    let mut requests = Vec::new();
    for batch in cards.chunks(DEFAULT_BATCH_SIZE) {
        let entries = batch
            .iter()
            .enumerate()
            .map(|(idx, card)| -> Result<_> {
                Ok(SendMessageBatchRequestEntry::builder()
                    .id(idx.to_string())
                    .message_body(serde_json::to_string(card)?)
                    .build()?)
            })
            .collect::<Result<Vec<_>>>()?;

        requests.push(
            client
                .send_message_batch()
                .queue_url(&url)
                .set_entries(Some(entries))
                .send(),
        );
    }

    futures::stream::iter(requests)
        .buffer_unordered(DEFAULT_BATCH_SIZE)
        .collect::<Vec<_>>()
        .await;

    Ok(())
}
