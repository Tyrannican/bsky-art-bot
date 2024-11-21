package aws

import (
	"bytes"
	"context"
	"encoding/json"
	"log"
	sf "scryfall-datafetcher/scryfall"

	"github.com/aws/aws-sdk-go-v2/aws"
	"github.com/aws/aws-sdk-go-v2/config"
	"github.com/aws/aws-sdk-go-v2/service/s3"
)

const BUCKET = "muspelheim"
const OBJECT = "scryfall-oracle-cards.json"

func CreateS3Client() *s3.Client {
	cfg, err := config.LoadDefaultConfig(context.TODO())
	if err != nil {
		log.Fatalf("unable to load SDK config: %v\n", err)
	}

	return s3.NewFromConfig(cfg)
}

func UploadCardInfo(client *s3.Client, cards []sf.Card) error {
	raw, err := json.Marshal(cards)
	if err != nil {
		log.Fatalf("error marshalling card data: %v\n", err)
	}

	_, err = client.PutObject(context.TODO(), &s3.PutObjectInput{
		Bucket: aws.String(BUCKET),
		Key:    aws.String(OBJECT),
		Body:   bytes.NewReader(raw),
	})

	if err != nil {
		log.Printf("error occurred uploading to S3 bucket\n")
		return err
	}

	return nil
}
