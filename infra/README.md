# BSky Art Bot Infra

Builds out the AWS infra for the bot

## Services

* `S3` to store the raw card information

* `Lambda` to periodically update the card data and post to Bluesky

* `Eventbridge` to orchestrate the Cron jobs to execute the lambdas

* `DynamoDB` to store posted card data to prevent posting duplicates
