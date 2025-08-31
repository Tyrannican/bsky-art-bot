import * as cdk from 'aws-cdk-lib';
import { Construct } from 'constructs';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as iam from 'aws-cdk-lib/aws-iam';
import * as event from 'aws-cdk-lib/aws-events';
import * as dynamo from 'aws-cdk-lib/aws-dynamodb';
import { LambdaFunction } from 'aws-cdk-lib/aws-events-targets';
import path = require('path');

export class InfraStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);
    this.createDynamoDb();
    this.createDataFetcherLambda();
    this.createPosterLambda();
    this.createRustPosterLambda();
  }

  createDynamoDb() {
    new dynamo.TableV2(this, 'ScryfallDuplicateChecker', {
      tableName: 'scryfall-duplicate-checker',
      partitionKey: {
        name: 'name',
        type: dynamo.AttributeType.STRING
      },
      sortKey: {
        name: 'set',
        type: dynamo.AttributeType.STRING
      },
      billing: dynamo.Billing.onDemand(),
      removalPolicy: cdk.RemovalPolicy.DESTROY
    });
  }

  createDataFetcherLambda() {
    const fetcherRole = new iam.Role(this, 'ScryfallDataFetcherRole', {
      roleName: 'scryfall-data-fetcher-role',
      assumedBy: new iam.ServicePrincipal('lambda.amazonaws.com')
    });

    fetcherRole.addToPolicy(new iam.PolicyStatement({
      resources: ['*'],
      actions: [
        's3:PutObject',
        'logs:CreateLogGroup',
        'logs:CreateLogStream',
        'logs:PutLogEvents',
        'cloudwatch:*',
      ],
      effect: iam.Effect.ALLOW
    }));

    const fetcherScheduleRule = new event.Rule(this, 'ScryfallDataFetcherScheduleRule', {
      schedule: event.Schedule.cron({
        minute: '0',
        hour: '3',
        weekDay: '1',
        month: '*',
        year: '*'
      }),
      ruleName: 'scryfall-data-fetcher-schedule-rule',
      description: 'Cron job to run the Scryfall data fetcher each week'
    });

    const fn = new lambda.Function(this, 'ScryfallDataFetcherLambda', {
      runtime: lambda.Runtime.PROVIDED_AL2023,
      architecture: lambda.Architecture.ARM_64,
      handler: 'bootstrap',
      code: lambda.Code.fromAsset(path.join(__dirname, '../../dist/scryfall-datafetcher.zip')),
      memorySize: 1024,
      role: fetcherRole,
      functionName: 'scryfall-data-fetcher-fn',
      timeout: cdk.Duration.seconds(30)
    });

    fetcherScheduleRule.addTarget(new LambdaFunction(fn, {
      retryAttempts: 5
    }));
  }

  createRustPosterLambda() {
    const posterRole = new iam.Role(this, 'BskyRustPosterRole', {
      roleName: 'bsky-poster-rust-role',
      assumedBy: new iam.ServicePrincipal('lambda.amazonaws.com')
    });

    posterRole.addToPolicy(new iam.PolicyStatement({
      resources: ['*'],
      actions: [
        'logs:CreateLogGroup',
        'logs:CreateLogStream',
        'logs:PutLogEvents',
        'secretsmanager:GetSecretValue',
        's3:GetObject',
        'cloudwatch:*',
        'dynamodb:GetItem',
        'dynamodb:PutItem'
      ],
      effect: iam.Effect.ALLOW
    }));

    new lambda.Function(this, 'BskyPosterRustLambda', {
      runtime: lambda.Runtime.PROVIDED_AL2023,
      architecture: lambda.Architecture.ARM_64,
      handler: 'bootstrap',
      code: lambda.Code.fromAsset(path.join(__dirname, '../../dist/bsky-poster-rs.zip')),
      role: posterRole,
      environment: {
        BUCKET: 'muspelheim',
        BUCKET_KEY: 'scryfall-oracle-cards.json',
        DB_NAME: 'scryfall-duplicate-checker',
      },
      memorySize: 256,
      functionName: 'bsky-poster-rust-fn',
      timeout: cdk.Duration.seconds(30)
    });
  }

  createPosterLambda() {
    const posterRole = new iam.Role(this, 'BskyPosterRole', {
      roleName: 'bsky-poster-role',
      assumedBy: new iam.ServicePrincipal('lambda.amazonaws.com')
    });

    posterRole.addToPolicy(new iam.PolicyStatement({
      resources: ['*'],
      actions: [
        'logs:CreateLogGroup',
        'logs:CreateLogStream',
        'logs:PutLogEvents',
        'secretsmanager:GetSecretValue',
        's3:GetObject',
        'cloudwatch:*',
        'dynamodb:GetItem',
        'dynamodb:PutItem'
      ],
      effect: iam.Effect.ALLOW
    }));

    const posterScheduleRule = new event.Rule(this, 'BskyPosterScheduleRule', {
      schedule: event.Schedule.cron({
        minute: '0',
        hour: '0/1',
        day: '*',
        month: '*',
        year: '*'
      }),
      ruleName: 'bsky-poster-schedule-rule',
      description: 'Cron job to run the Bluesky poster lambda'
    });

    const fn = new lambda.Function(this, 'BskyPosterLambda', {
      runtime: lambda.Runtime.NODEJS_20_X,
      handler: 'index.handler',
      code: lambda.Code.fromAsset(path.join(__dirname, '../../dist/bsky-poster.zip')),
      role: posterRole,
      environment: {
        BUCKET: 'muspelheim',
        BUCKET_KEY: 'scryfall-oracle-cards.json',
        DB_NAME: 'scryfall-duplicate-checker',
      },
      memorySize: 256,
      functionName: 'bsky-poster-fn',
      timeout: cdk.Duration.seconds(30)
    });

    posterScheduleRule.addTarget(new LambdaFunction(fn, {
      retryAttempts: 5
    }));
  }
}
