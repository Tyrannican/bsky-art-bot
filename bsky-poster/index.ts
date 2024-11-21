import { AtpAgent, RichText } from '@atproto/api';
import { S3Client, GetObjectCommand } from '@aws-sdk/client-s3';
import { SecretsManagerClient, GetSecretValueCommand } from '@aws-sdk/client-secrets-manager';
import { DynamoDBClient } from '@aws-sdk/client-dynamodb';
import { DynamoDBDocumentClient, GetCommand, PutCommand } from '@aws-sdk/lib-dynamodb';
import axios from 'axios';
import process from 'process';

const agent = new AtpAgent({
  service: 'https://bsky.social'
});

const s3Client = new S3Client({});
const secretClient = new SecretsManagerClient({ region: 'eu-west-1' });
const dynamoDbClient = new DynamoDBClient({ region: 'eu-west-1' });
const dbClient = DynamoDBDocumentClient.from(dynamoDbClient);

type Card = {
  name: string,
  image_uris: {
    art_crop: string
  },
  set_name: string,
  flavor_text: string,
  artist: string
};

type BSkyCredentials = {
  BSKY_USER: string,
  BSKY_PASSWORD: string
};

const loadBlueskyCredentials = async (): Promise<BSkyCredentials> => {
  const secret = await secretClient.send(
    new GetSecretValueCommand({ SecretId: 'bsky-artbot-credentials' })
  );

  return JSON.parse(secret.SecretString!);
};

const downloadCardData = async (): Promise<Card[]> => {
  const cardData = await s3Client.send(
    new GetObjectCommand({ Bucket: process.env.BUCKET, Key: process.env.BUCKET_KEY })
  );

  const raw = await cardData.Body?.transformToString();
  return JSON.parse(raw!);
};

const downloadCardImage = async (card: Card): Promise<Buffer> => {
  const response = await axios.get(card.image_uris.art_crop, { responseType: 'arraybuffer' });
  return Buffer.from(response.data, 'binary');
};

const postToBluesky = async (image: Buffer, postText: RichText, altText: string): Promise<void> => {
  const { BSKY_USER, BSKY_PASSWORD } = await loadBlueskyCredentials();
  await agent.login({ identifier: BSKY_USER, password: BSKY_PASSWORD })
  const response = await agent.uploadBlob(image, { encoding: 'image/jpeg' });
  const imgBlob = response.data.blob;
  await postText.detectFacets(agent);

  await agent.post({
    text: postText.text,
    facets: postText.facets,
    createdAt: new Date().toISOString(),
    embed: {
      $type: 'app.bsky.embed.images',
      images: [
        {
          image: imgBlob,
          alt: altText
        }
      ]
    },
  });
};

// TODO: The longer this goes, the worse this gets
// But it will do for now
const retrieveCard = async (cards: Card[]): Promise<Card> => {
  let count = 0;
  const dbName = process.env.DB_NAME;

  while (true) {
    const card = cards[Math.floor(Math.random() * cards.length)];
    const getCommand = new GetCommand({
      TableName: dbName,
      Key: {
        name: card.name,
        set: card.set_name
      }
    });

    const { Item } = await dbClient.send(getCommand);

    // Five duplicates is enough to repost
    if (Item && count < 5) {
      count += 1;
      continue;
    }

    const putCommand = new PutCommand({
      TableName: dbName,
      Item: {
        name: card.name,
        set: card.set_name
      }
    });
    await dbClient.send(putCommand);

    return card;
  }
};

export const handler = async (event: any, context: object = {}) => {
  const cards = await downloadCardData();

  // Gets a card by checking if it's been posted before
  const card = await retrieveCard(cards);
  const text = new RichText({
    text: `${card.name} (${card.set_name})\nArtist: ${card.artist}\n\n${card.flavor_text}\n\n#magicthegathering #mtg`
  });
  const altText = `Art for the Magic: the Gathering card '${card.name}' from the set '${card.set_name}' by the artist '${card.artist}'`;

  const imgData = await downloadCardImage(card);
  await postToBluesky(imgData, text, altText);
}
