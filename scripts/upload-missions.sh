#!/usr/bin/env bash
# Upload the base missions to the local backend.
#   ./scripts/upload-missions.sh <token>

set -euo pipefail

TOKEN="$1"

# Percent-encode a single path segment; mission names contain spaces.
urlencode() {
  local s="$1" out="" i c
  for ((i = 0; i < ${#s}; i++)); do
    c="${s:i:1}"
    case "$c" in
      [a-zA-Z0-9.~_-]) out+="$c" ;;
      *) out+="$(printf '%%%02X' "'$c")" ;;
    esac
  done
  printf '%s' "$out"
}

for file in "$(dirname "$0")"/../gcs/missions/*.json; do
  name="$(basename "$file" .json)"
  status="$(curl -sS -o /dev/null -w '%{http_code}' \
    -X POST "http://127.0.0.1:8000/missions/$(urlencode "$name")" \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $TOKEN" \
    --data-binary "@$file")"

  printf '%-16s %s\n' "$status" "$name"
done
