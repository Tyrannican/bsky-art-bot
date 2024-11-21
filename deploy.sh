#!/bin/bash

build_datafetcher () {
  cd scryfall-datafetcher
  GOOS=linux GOARCH=arm64 go build -o bootstrap main.go
  zip ../dist/scryfall-datafetcher.zip bootstrap
  rm bootstrap
  cd ..
}

build_poster () {
  cd bsky-poster
  npm run build
  cp package.json dist/
  cd dist/
  npm install --omit=dev
  zip -9 -r ../../dist/bsky-poster.zip *
  cd ../
  rm -rf dist
  cd ../
}

build () {
  mkdir -p dist
  build_datafetcher
  build_poster
}

deploy () {
  cd infra
  npm run cdk deploy 
  cd ..
}

build
deploy
