#!/usr/bin/env bash
set -euo pipefail

DATA_DIR="$(cd "$(dirname "$0")/.." && pwd)/data/naturalearth"
mkdir -p "$DATA_DIR"

URL="https://naciscdn.org/naturalearth/10m/cultural/ne_10m_admin_0_countries.zip"
SHP="ne_10m_admin_0_countries.shp"

if [ -f "$DATA_DIR/$SHP" ]; then
    echo "Already exists: $DATA_DIR/$SHP"
    exit 0
fi

echo "Downloading Natural Earth 10m countries (~5 MB)..."
curl -L --progress-bar --retry 3 --retry-delay 5 -o "$DATA_DIR/ne_10m_admin_0_countries.zip" "$URL"

echo "Extracting..."
unzip -o "$DATA_DIR/ne_10m_admin_0_countries.zip" -d "$DATA_DIR"
echo "Natural Earth data ready in $DATA_DIR"
