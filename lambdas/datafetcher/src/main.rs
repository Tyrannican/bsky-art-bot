use anyhow::Context;
use lambda_runtime::{Error, LambdaEvent, run, service_fn, tracing};

mod scryfall;
mod sqs;

async fn handler(_event: LambdaEvent<serde_json::Value>) -> Result<(), Error> {
    let cards = scryfall::download()
        .await
        .context("downloading card data")?;

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
