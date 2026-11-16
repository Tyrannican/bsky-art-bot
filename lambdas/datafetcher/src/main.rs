use anyhow::Context;
use lambda_runtime::{Error, LambdaEvent, run, service_fn, tracing};
use uuid::Uuid;

mod s3;
mod scryfall;
mod sqs;

async fn handler(_event: LambdaEvent<serde_json::Value>) -> Result<(), Error> {
    let cards = scryfall::download()
        .await
        .context("downloading card data")?;

    let card_ids = cards.iter().map(|c| c.id).collect::<Vec<Uuid>>();

    s3::send_ids(card_ids)
        .await
        .context("sending id file to s3")?;

    tracing::info!("sent card ids to s3");

    sqs::send_to_processor(cards)
        .await
        .context("sending cards to processor queue")?;

    tracing::info!("sent all cards to processor queue");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();
    run(service_fn(handler)).await?;

    Ok(())
}
