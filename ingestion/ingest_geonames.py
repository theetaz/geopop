#!/usr/bin/env python3
"""Ingest GeoNames data into PostgreSQL for reverse geocoding.

Loads:
- admin1CodesASCII.txt → admin1_codes
- admin2Codes.txt      → admin2_codes
- allCountries.zip     → geonames (filtered to feature_class='P' populated places)
"""

import os, sys, time, io, zipfile
import psycopg

BATCH_SIZE = 100_000


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


def _load_tsv(conn, path: str, table: str, columns: str) -> int:
    """Load a simple two-column TSV (code, name) into a lookup table."""
    if not os.path.exists(path):
        print(f"  WARNING: {path} not found, skipping")
        return 0

    buf = io.StringIO()
    count = 0
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            if line.startswith("#"):
                continue
            parts = line.strip().split("\t")
            if len(parts) < 2:
                continue
            code, name = parts[0].strip(), parts[1].strip()
            if not code or not name:
                continue
            buf.write(f"{code}\t{name.replace(chr(9), ' ').replace(chr(10), ' ')}\n")
            count += 1

    buf.seek(0)
    with conn.cursor() as cur:
        cur.execute(f"TRUNCATE {table}")
        with cur.copy(f"COPY {table} ({columns}) FROM STDIN") as copy:
            copy.write(buf.read())
    conn.commit()
    print(f"  {table}: {count:,} rows")
    return count


def _load_geonames(conn, zip_path: str) -> int:
    """Stream allCountries.zip, filter to populated places, COPY into geonames."""
    if not os.path.exists(zip_path):
        print(f"ERROR: {zip_path} not found. Run: make download-geonames")
        sys.exit(1)

    with conn.cursor() as cur:
        cur.execute("TRUNCATE geonames")
    conn.commit()

    total = 0
    start = time.time()
    copy_sql = (
        "COPY geonames (geonameid, name, latitude, longitude, "
        "feature_code, country_code, admin1_code, admin2_code, "
        "population, geom) FROM STDIN"
    )

    with zipfile.ZipFile(zip_path) as zf, zf.open("allCountries.txt") as raw:
        buf = io.StringIO()
        buf_count = 0

        for line_bytes in raw:
            parts = line_bytes.decode("utf-8", errors="replace").split("\t")
            if len(parts) < 19 or parts[6].strip() != "P":
                continue

            gid = parts[0].strip()
            lat, lon = parts[4].strip(), parts[5].strip()
            if not gid or not lat or not lon:
                continue

            name = parts[1].strip().replace("\t", " ").replace("\n", " ")
            pop = parts[14].strip() or "0"

            buf.write(
                f"{gid}\t{name}\t{lat}\t{lon}\t"
                f"{parts[7].strip()}\t{parts[8].strip()}\t{parts[10].strip()}\t{parts[11].strip()}\t"
                f"{pop}\tSRID=4326;POINT({lon} {lat})\n"
            )
            buf_count += 1

            if buf_count >= BATCH_SIZE:
                buf.seek(0)
                with conn.cursor() as cur:
                    with cur.copy(copy_sql) as copy:
                        copy.write(buf.read())
                conn.commit()
                total += buf_count
                rate = total / (time.time() - start)
                print(f"    {total:,} rows ({rate:,.0f}/s)")
                buf, buf_count = io.StringIO(), 0

        if buf_count > 0:
            buf.seek(0)
            with conn.cursor() as cur:
                with cur.copy(copy_sql) as copy:
                    copy.write(buf.read())
            conn.commit()
            total += buf_count

    elapsed = time.time() - start
    print(f"  geonames: {total:,} rows in {elapsed:.1f}s")
    return total


def main():
    db_url = get_db_url()
    data_dir = os.path.join(os.path.dirname(__file__), "..", "data", "geonames")

    print(f"Database: {db_url.split('@')[1] if '@' in db_url else db_url}")
    if not os.path.isdir(data_dir):
        print(f"ERROR: {data_dir} not found. Run: make download-geonames")
        sys.exit(1)

    conn = connect(db_url)
    conn.autocommit = False

    print("Loading lookup tables...")
    _load_tsv(conn, os.path.join(data_dir, "admin1CodesASCII.txt"), "admin1_codes", "code, name")
    _load_tsv(conn, os.path.join(data_dir, "admin2Codes.txt"), "admin2_codes", "code, name")

    print("\nLoading populated places...")
    _load_geonames(conn, os.path.join(data_dir, "allCountries.zip"))

    print("\nRunning VACUUM ANALYZE...")
    conn.autocommit = True
    with conn.cursor() as cur:
        for t in ("admin1_codes", "admin2_codes", "geonames"):
            cur.execute(f"VACUUM ANALYZE {t}")
    conn.close()
    print("Complete.")


if __name__ == "__main__":
    main()
