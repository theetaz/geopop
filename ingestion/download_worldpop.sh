#!/usr/bin/env bash
set -euo pipefail

DATA_DIR="$(cd "$(dirname "$0")/.." && pwd)/data"
mkdir -p "$DATA_DIR"

# WorldPop unconstrained + UN-adjusted 1km (~723 MB)
# Distributes population across all land, not just detected settlement footprints.
URL="https://data.worldpop.org/GIS/Population/Global_2015_2030/R2024B/2025/0_Mosaicked/v1/1km_ua/unconstrained/global_pop_2025_UC_1km_R2024B_UA_v1.tif"
FILENAME="global_pop_2025_UC_1km_R2024B_UA_v1.tif"

if [ -f "$DATA_DIR/$FILENAME" ]; then
    echo "Already exists: $DATA_DIR/$FILENAME"
    exit 0
fi

echo "Downloading WorldPop R2024B 2025 unconstrained UN-adjusted 1km (~723 MB)..."

MAX_RETRIES=3
for i in $(seq 1 $MAX_RETRIES); do
    if curl -L --progress-bar --retry 3 --retry-delay 10 \
        -o "$DATA_DIR/$FILENAME.part" "$URL"; then
        mv "$DATA_DIR/$FILENAME.part" "$DATA_DIR/$FILENAME"
        echo "Downloaded: $DATA_DIR/$FILENAME ($(du -h "$DATA_DIR/$FILENAME" | cut -f1))"
        exit 0
    fi
    echo "Attempt $i/$MAX_RETRIES failed."
    rm -f "$DATA_DIR/$FILENAME.part"
done

echo "ERROR: Download failed after $MAX_RETRIES attempts."
exit 1
