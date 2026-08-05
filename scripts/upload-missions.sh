#!/usr/bin/env bash
#   ./scripts/upload-missions.sh [base-url]

set -euo pipefail

BASE_URL="${1:-${MISSION_STORE_URL:-http://127.0.0.1:8000}}"
MISSIONS_DIR="$(dirname "$0")/../drone-commander/missions"

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

failed=0
for file in "$MISSIONS_DIR"/*.json; do
  name="$(basename "$file" .json)"
  status="$(curl -sS -o /dev/null -w '%{http_code}' \
    -X POST "$BASE_URL/missions/$(urlencode "$name")" \
    -H 'Content-Type: application/json' \
    --data-binary "@$file")"

  printf '%-16s %s\n' "$status" "$name"
  [[ "$status" == 2* ]] || failed=1
done

exit "$failed"
