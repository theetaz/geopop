#!/usr/bin/env bash
set -euo pipefail

DATA_DIR="$(cd "$(dirname "$0")/.." && pwd)/data/geonames"
mkdir -p "$DATA_DIR"

BASE_URL="https://download.geonames.org/export/dump"
FILES=("allCountries.zip" "admin1CodesASCII.txt" "admin2Codes.txt" "countryInfo.txt")

for FILE in "${FILES[@]}"; do
    if [ -f "$DATA_DIR/$FILE" ]; then
        echo "Already exists: $DATA_DIR/$FILE"
        continue
    fi
    echo "Downloading $FILE..."
    curl -L --progress-bar --retry 3 --retry-delay 5 -o "$DATA_DIR/$FILE" "$BASE_URL/$FILE"
    echo "Downloaded: $DATA_DIR/$FILE ($(du -h "$DATA_DIR/$FILE" | cut -f1))"
done

echo "GeoNames data ready in $DATA_DIR"
