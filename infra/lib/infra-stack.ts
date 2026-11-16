import * as cdk from 'aws-cdk-lib';
import { Construct } from 'constructs';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as iam from 'aws-cdk-lib/aws-iam';
import * as event from 'aws-cdk-lib/aws-events';
import * as dynamo from 'aws-cdk-lib/aws-dynamodb';
import * as sqs from 'aws-cdk-lib/aws-sqs';
import * as path from 'path';
import { LambdaFunction } from 'aws-cdk-lib/aws-events-targets';
import { SqsEventSource } from 'aws-cdk-lib/aws-lambda-event-sources';

export class InfraStack extends cdk.Stack {
  private processorQueue: sqs.IQueue;

  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);
    this.createDynamoDb();
    this.processorQueue = this.createProcessorQueue();
    this.createDataFetcherLambda();
    this.createPosterLambda();
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

  createProcessorQueue() {
    const dlq = new sqs.Queue(this, 'ScryfallDataFetcherProcessorDeadLetterQueue', {
      queueName: 'scryfall-data-fetcher-processor-dlq',
      retentionPeriod: cdk.Duration.days(14),
    });

    return new sqs.Queue(this, 'ScryfallDataFetcherProcessorQueue', {
      queueName: 'scryfall-data-fetcher-processor-queue',
      visibilityTimeout: cdk.Duration.minutes(1),
      deadLetterQueue: {
        queue: dlq,
        maxReceiveCount: 5
      },
    });
  }

  createDataFetcherProcessorLambda() {
    const processorRole = new iam.Role(this, 'ScryfallDataFetcherProcessorRole', {
      roleName: 'scryfall-data-fetcher-processor-role',
      assumedBy: new iam.ServicePrincipal('lambda.amazonaws.com'),
    });

    processorRole.addToPolicy(new iam.PolicyStatement({
      resources: ['*'],
      actions: [
        's3:PutObject',
        's3:GetObject',
        'logs:CreateLogGroup',
        'logs:CreateLogStream',
        'logs:PutLogEvents',
        'cloudwatch:*',
      ],
      effect: iam.Effect.ALLOW
    }));

    const fn = new lambda.Function(this, 'ScryfallDataFetcherLambda', {
      runtime: lambda.Runtime.PROVIDED_AL2023,
      architecture: lambda.Architecture.ARM_64,
      handler: 'bootstrap',
      code: lambda.Code.fromAsset(path.join(__dirname, '../../dist/scryfall-datafetcher-processor.zip')),
      memorySize: 256,
      reservedConcurrentExecutions: 100,
      role: processorRole,
      environment: {
        BUCKET: 'muspelheim',
      },
      functionName: 'scryfall-data-fetcher-processor-fn',
      timeout: cdk.Duration.seconds(30)
    });

    fn.addEventSource(new SqsEventSource(this.processorQueue, {
      batchSize: 10,
      reportBatchItemFailures: true,
    }));
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

    fetcherRole.addToPolicy(new iam.PolicyStatement({
      effect: iam.Effect.ALLOW,
      actions: ['sqs:SendMessage'],
      resources: [this.processorQueue.queueArn]
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
      runtime: lambda.Runtime.PROVIDED_AL2023,
      architecture: lambda.Architecture.ARM_64,
      handler: 'bootstrap',
      code: lambda.Code.fromAsset(path.join(__dirname, '../../dist/bsky-poster.zip')),
      role: posterRole,
      environment: {
        BUCKET: 'muspelheim',
        BUCKET_KEY: 'scryfall-oracle-cards.json',
        DB_NAME: 'scryfall-duplicate-checker',
      },
      memorySize: 1024,
      functionName: 'bsky-poster-fn',
      timeout: cdk.Duration.seconds(30)
    });

    posterScheduleRule.addTarget(new LambdaFunction(fn, {
      retryAttempts: 5
    }));
  }
}
