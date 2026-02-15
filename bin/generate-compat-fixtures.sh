#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
ORACLE_DIR="$ROOT_DIR/tools/oracle-node"

cd "$ORACLE_DIR"
mise x -- npm install
mise x -- npm run generate
