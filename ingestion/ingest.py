#!/usr/bin/env python3
"""Ingest WorldPop GeoTIFF into PostgreSQL.

Reads the raster row-by-row, maps each pixel to a canonical 30 arc-second
cell_id (matching the Rust API and SQL function), and streams to PostgreSQL
via COPY for maximum throughput.
"""

import os, sys, time, io
import numpy as np
import rasterio
import psycopg

NCOLS = 43200   # 360° × 120
NROWS = 21600   # 180° × 120
BATCH_SIZE = 500_000


def connect(db_url: str, retries: int = 30) -> psycopg.Connection:
    for attempt in range(retries):
        try:
            return psycopg.connect(db_url, connect_timeout=5)
        except psycopg.OperationalError:
            if attempt == retries - 1:
                raise
            print(f"  DB not ready (attempt {attempt + 1}/{retries}), retrying...")
            time.sleep(2)


def get_db_url() -> str:
    if url := os.environ.get("DATABASE_URL"):
        return url
    u = os.environ.get("POSTGRES_USER", "geopop")
    p = os.environ.get("POSTGRES_PASSWORD", "geopop")
    h = os.environ.get("POSTGRES_HOST", "localhost")
    port = os.environ.get("POSTGRES_PORT", "5432")
    db = os.environ.get("POSTGRES_DB", "geopop")
    return f"postgresql://{u}:{p}@{h}:{port}/{db}"


def find_tif() -> str:
    data_dir = os.path.join(os.path.dirname(__file__), "..", "data")
    if os.path.isdir(data_dir):
        for f in os.listdir(data_dir):
            if f.endswith(".tif") and "global_pop_" in f and "1km" in f:
                return os.path.join(data_dir, f)
    print("ERROR: No WorldPop .tif found in data/. Run: make download-worldpop")
    sys.exit(1)


def ingest(tif_path: str, db_url: str) -> None:
    print(f"Opening raster: {tif_path}")

    with rasterio.open(tif_path) as src:
        nodata = src.nodata
        t = src.transform
        print(f"Raster: {src.width}x{src.height}, CRS={src.crs}, NoData={nodata}")

        row_lats = t.f + (np.arange(src.height) + 0.5) * t.e
        canonical_rows = np.floor((90.0 - row_lats) * 120.0).astype(np.int64)

        col_lons = t.c + (np.arange(src.width) + 0.5) * t.a
        canonical_cols = np.floor((col_lons + 180.0) * 120.0).astype(np.int64)

        conn = connect(db_url)
        conn.autocommit = False

        with conn.cursor() as cur:
            cur.execute("TRUNCATE population")
        conn.commit()
        print("Truncated population table.")

        total = skipped_oob = skipped_dup = 0
        start = time.time()
        buf = io.StringIO()
        buf_count = 0
        seen = set()

        for row_idx in range(src.height):
            crow = canonical_rows[row_idx]
            if crow < 0 or crow >= NROWS:
                skipped_oob += 1
                continue

            data = src.read(1, window=rasterio.windows.Window(0, row_idx, src.width, 1)).flatten()
            mask = (data > 0) & np.isfinite(data)
            if nodata is not None:
                mask &= data != nodata

            valid_cols = np.where(mask)[0]
            if len(valid_cols) == 0:
                continue

            for ccol, pop_val in zip(canonical_cols[valid_cols], data[valid_cols]):
                if ccol < 0 or ccol >= NCOLS:
                    skipped_oob += 1
                    continue

                cell_id = int(crow) * NCOLS + int(ccol)
                if cell_id in seen:
                    skipped_dup += 1
                    continue
                seen.add(cell_id)

                buf.write(f"{cell_id}\t{pop_val:.6g}\n")
                buf_count += 1

                if buf_count >= BATCH_SIZE:
                    _flush(conn, buf, buf_count)
                    total += buf_count
                    buf, buf_count = io.StringIO(), 0

            if (row_idx + 1) % 1000 == 0:
                elapsed = time.time() - start
                pct = (row_idx + 1) / src.height * 100
                rate = total / elapsed if elapsed > 0 else 0
                print(f"  Row {row_idx+1}/{src.height} ({pct:.1f}%) — {total:,} rows — {rate:,.0f}/s")

        if buf_count > 0:
            _flush(conn, buf, buf_count)
            total += buf_count

        elapsed = time.time() - start
        print(f"\nDone: {total:,} rows in {elapsed:.1f}s ({total/elapsed:,.0f}/s)")
        print(f"Skipped: {skipped_oob:,} out-of-bounds, {skipped_dup:,} duplicates")

        print("Running VACUUM ANALYZE...")
        conn.autocommit = True
        with conn.cursor() as cur:
            cur.execute("VACUUM ANALYZE population")
        conn.close()
        print("Complete.")


def _flush(conn, buf: io.StringIO, count: int) -> None:
    buf.seek(0)
    with conn.cursor() as cur:
        with cur.copy("COPY population (cell_id, pop) FROM STDIN") as copy:
            copy.write(buf.read())
    conn.commit()


if __name__ == "__main__":
    tif = find_tif()
    url = get_db_url()
    print(f"Database: {url.split('@')[1] if '@' in url else url}")
    ingest(tif, url)
