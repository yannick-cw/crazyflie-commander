#!/usr/bin/env bash
# Create the TUI API token and print it to stdout.

set -euo pipefail

curl -sS -X POST "http://127.0.0.1:8000/admin/tokens" \
  -H 'Content-Type: application/json' \
  -d '{"label": "TUI"}' |
  jq -er '.token'
