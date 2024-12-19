use std::collections::HashMap;

use lambda_runtime::{run, service_fn, tracing, Error, LambdaEvent};

mod s3;
mod scryfall;

async fn handler(_event: LambdaEvent<HashMap<String, String>>) -> Result<(), Error> {
    let cards = scryfall::download().await?;
    s3::upload_cards(cards).await?;
    tracing::info!("uploaded cards to S3");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();
    run(service_fn(handler)).await?;

    Ok(())
}
