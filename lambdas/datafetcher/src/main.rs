use anyhow::Context;
use lambda_runtime::{run, service_fn, tracing, Error, LambdaEvent};

mod s3;
mod scryfall;

async fn handler(_event: LambdaEvent<serde_json::Value>) -> Result<(), Error> {
    let cards = scryfall::download()
        .await
        .context("downloading card data")?;

    s3::upload_cards(cards)
        .await
        .context("uploading card data to aws")?;

    tracing::info!("uploaded cards to S3");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();
    run(service_fn(handler)).await?;

    Ok(())
}
