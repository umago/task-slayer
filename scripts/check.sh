#!/usr/bin/env bash
set -e

echo "Checking formatting..."
make fmt-check

echo "Running clippy..."
make lint

echo "Running tests..."
make test
