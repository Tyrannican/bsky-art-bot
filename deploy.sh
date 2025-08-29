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

build_rust_poster () {
  cd bsky-poster-rs 
  mkdir build
  CARGO_TARGET_DIR=./build cargo lambda build --release --arm64
  mv build/lambda/bsky-poster-rs/bootstrap .
  zip ../dist/bsky-poster-rs.zip bootstrap
  rm -r build bootstrap
  cd ..
}

build_poster () {
  cd bsky-poster
  npm run build
  cp package.json dist/
  cd dist/
  npm install --omit=dev
  npm run minify
  zip -9 -r ../../dist/bsky-poster.zip index.js
  cd ../
  rm -rf dist
  cd ../
}

build () {
  mkdir -p dist
  build_datafetcher
  build_rust_poster
  build_poster
}

deploy () {
  cd infra
  npm run cdk deploy 
  cd ..
  rm -rf dist/
}

build
# deploy
