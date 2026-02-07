#!/usr/bin/env python3
"""Ingest Natural Earth 10m country boundaries into PostgreSQL.

Loads ISO codes, names, continent, region, population estimates,
and MultiPolygon geometries into the countries table.
"""

import os, sys
import fiona
import psycopg
from shapely.geometry import shape, MultiPolygon


def get_db_url() -> str:
    if url := os.environ.get("DATABASE_URL"):
        return url
    u = os.environ.get("POSTGRES_USER", "geopop")
    p = os.environ.get("POSTGRES_PASSWORD", "geopop")
    h = os.environ.get("POSTGRES_HOST", "localhost")
    port = os.environ.get("POSTGRES_PORT", "5432")
    db = os.environ.get("POSTGRES_DB", "geopop")
    return f"postgresql://{u}:{p}@{h}:{port}/{db}"


def find_shapefile() -> str:
    shp = os.path.join(os.path.dirname(__file__), "..", "data", "naturalearth", "ne_10m_admin_0_countries.shp")
    if os.path.exists(shp):
        return shp
    print("ERROR: Shapefile not found. Run: make download-naturalearth")
    sys.exit(1)


def ingest(shp_path: str, db_url: str) -> None:
    print(f"Opening shapefile: {shp_path}")
    conn = psycopg.connect(db_url)
    conn.autocommit = False

    with conn.cursor() as cur:
        cur.execute("TRUNCATE countries RESTART IDENTITY CASCADE")
    conn.commit()

    count = skipped = 0
    insert_sql = """
        INSERT INTO countries (iso_a2, iso_a3, name, formal_name,
            continent, region_un, subregion, pop_est, geom)
        VALUES (%s, %s, %s, %s, %s, %s, %s, %s, ST_GeomFromEWKT(%s))
    """

    with fiona.open(shp_path) as src:
        print(f"Features: {len(src)}, CRS: {src.crs}")
        for feature in src:
            p = feature["properties"]
            name, continent = p.get("NAME", ""), p.get("CONTINENT", "")
            if not name or not continent:
                skipped += 1
                continue

            iso_a2 = p.get("ISO_A2_EH", "")
            iso_a3 = p.get("ISO_A3_EH", "")
            iso_a2 = None if iso_a2 in ("-99", "-1", "") else iso_a2
            iso_a3 = None if iso_a3 in ("-99", "-1", "") else iso_a3

            pop_est = p.get("POP_EST")
            try:
                pop_est = int(pop_est) if pop_est is not None else None
            except (ValueError, TypeError):
                pop_est = None

            geom = shape(feature["geometry"])
            if geom.geom_type == "Polygon":
                geom = MultiPolygon([geom])
            elif geom.geom_type != "MultiPolygon":
                skipped += 1
                continue

            with conn.cursor() as cur:
                cur.execute(insert_sql, (
                    iso_a2, iso_a3, name, p.get("FORMAL_EN") or None,
                    continent, p.get("REGION_UN") or None, p.get("SUBREGION") or None,
                    pop_est, f"SRID=4326;{geom.wkt}",
                ))
            count += 1
            if count % 50 == 0:
                conn.commit()

    conn.commit()
    print(f"Loaded {count} countries ({skipped} skipped).")

    conn.autocommit = True
    with conn.cursor() as cur:
        cur.execute("VACUUM ANALYZE countries")
    conn.close()
    print("Complete.")


if __name__ == "__main__":
    shp = find_shapefile()
    url = get_db_url()
    print(f"Database: {url.split('@')[1] if '@' in url else url}")
    ingest(shp, url)
