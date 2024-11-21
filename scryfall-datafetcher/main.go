package main

import (
	"context"
	"encoding/json"
	"log"
	aws "scryfall-datafetcher/aws"
	sf "scryfall-datafetcher/scryfall"

	"github.com/aws/aws-lambda-go/lambda"
	"github.com/aws/aws-sdk-go-v2/service/s3"
)

var (
	client *s3.Client
)

func init() {
	client = aws.CreateS3Client()
}

func HandleRequest(ctx context.Context, event json.RawMessage) error {
	cards := sf.Download()
	log.Println("downloaded card data")
	err := aws.UploadCardInfo(client, cards)
	log.Println("upload card info to s3")
	return err
}

func main() {
	lambda.Start(HandleRequest)
}
