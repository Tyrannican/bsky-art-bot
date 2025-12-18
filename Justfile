# Run build by default
default: build

# Make dist/ folder and build lambdas
build: _dist build-lambdas

# Build all lambda functions
build-lambdas:
    #!/usr/bin/env bash
    set -euxo pipefail
    cd lambdas
    mkdir -p build
    CARGO_TARGET_DIR=./build cargo lambda build --workspace --release --arm64
    mv build/lambda/datafetcher/bootstrap .
    zip ../dist/scryfall-datafetcher.zip bootstrap
    rm bootstrap
    mv build/lambda/bsky-poster-rs/bootstrap .
    zip ../dist/bsky-poster.zip bootstrap
    rm -r build bootstrap

# Deploy the infrastructure to AWS
deploy:
    cd infra && npm run cdk deploy
    rm -rf dist/

_dist:
    mkdir -p dist
