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

build_lambdas () {
    cd lambdas
    mkdir -p build
    CARGO_TARGET_DIR=./build cargo lambda build --workspace --release --arm64
    mv build/lambda/datafetcher/bootstrap .
    zip ../dist/scryfall-datafetcher.zip bootstrap
    rm bootstrap

    mv build/lambda/bsky-poster-rs/bootstrap .
    zip ../dist/bsky-poster.zip bootstrap
    rm -r build bootstrap
    cd ..
}

build () {
  mkdir -p dist
  build_lambdas
}

deploy () {
  cd infra
  npm run cdk deploy 
  cd ..
  rm -rf dist/
}

build
deploy
