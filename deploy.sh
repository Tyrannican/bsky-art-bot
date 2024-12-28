#!/bin/bash

build_datafetcher () {
  cd datafetcher
  mkdir build
  CARGO_TARGET_DIR=./build cargo lambda build --release --arm64
  mv build/lambda/datafetcher/bootstrap .
  zip ../dist/scryfall-datafetcher.zip bootstrap
  rm -r build bootstrap
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
