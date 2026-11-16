use anyhow::Context;
use aws_config::BehaviorVersion;
use aws_lambda_events::sqs::{SqsEvent, SqsMessage};
use aws_sdk_s3::{Client as S3Client, error::ProvideErrorMetadata, primitives::ByteStream};
use lambda_runtime::{Error, LambdaEvent, run, service_fn, tracing};
use scryone::objects::Card;
use std::sync::Arc;

async fn create_s3_client() -> Arc<S3Client> {
    let cfg = aws_config::load_defaults(BehaviorVersion::latest()).await;
    Arc::new(S3Client::new(&cfg))
}

async fn process_record(record: SqsMessage, bucket: &str, s3: Arc<S3Client>) -> Result<(), Error> {
    let Some(body) = record.body else {
        return Ok(());
    };

    let card: Card = serde_json::from_str(&body).context("deserialising card body")?;
    let key = format!("{}.json", card.id);

    match s3
        .put_object()
        .bucket(bucket)
        .key(&key)
        .if_none_match("*")
        .body(ByteStream::from(body.into_bytes()))
        .send()
        .await
    {
        Ok(_) => tracing::info!(key, "inserted into s3 bucket"),
        Err(e) if e.as_service_error().and_then(|e| e.code()) == Some("PreconditionFailed") => {
            tracing::debug!(key, "card exists, skipping");
        }
        Err(e) => {
            tracing::error!(key, "failed to write to s3");
            return Err(e.into());
        }
    }

    Ok(())
}

async fn handler(event: LambdaEvent<SqsEvent>, s3: Arc<S3Client>) -> Result<(), Error> {
    let bucket = std::env::var("BUCKET")?;
    let futs: Vec<_> = event
        .payload
        .records
        .into_iter()
        .map(|record| {
            let s3 = Arc::clone(&s3);
            process_record(record, &bucket, s3)
        })
        .collect();

    futures::future::join_all(futs).await;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let s3 = create_s3_client().await;
    tracing::init_default_subscriber();

    run(service_fn(move |event| {
        let s3 = Arc::clone(&s3);
        async move { handler(event, s3).await }
    }))
    .await
}
