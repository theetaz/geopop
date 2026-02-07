-- GeoPop schema: WorldPop population grid + GeoNames places + Natural Earth countries
CREATE EXTENSION IF NOT EXISTS postgis;

-- ── WorldPop 1km population grid ──
-- Each cell is a 30 arc-second (~1km) grid cell identified by a canonical cell_id.
-- cell_id = row * 43200 + col, where:
--   row = floor((90 - lat) * 120)
--   col = floor((lon + 180) * 120)

CREATE TABLE population (
    cell_id INTEGER PRIMARY KEY,
    pop     REAL    NOT NULL
);

CREATE OR REPLACE FUNCTION get_population(lat DOUBLE PRECISION, lon DOUBLE PRECISION)
RETURNS REAL AS $$
DECLARE
    cid INTEGER;
    result REAL;
BEGIN
    cid := (FLOOR((90.0 - lat) * 120.0))::INTEGER * 43200
         + (FLOOR((lon + 180.0) * 120.0))::INTEGER;
    SELECT pop INTO result FROM population WHERE cell_id = cid;
    RETURN COALESCE(result, 0.0);
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- ── Natural Earth 10m country boundaries ──

CREATE TABLE countries (
    gid         SERIAL PRIMARY KEY,
    iso_a2      CHAR(2),
    iso_a3      CHAR(3),
    name        TEXT NOT NULL,
    formal_name TEXT,
    continent   TEXT NOT NULL,
    region_un   TEXT,
    subregion   TEXT,
    pop_est     BIGINT,
    geom        GEOMETRY(MultiPolygon, 4326) NOT NULL
);

CREATE INDEX idx_countries_geom      ON countries USING GiST (geom);
CREATE INDEX idx_countries_iso_a2    ON countries (iso_a2);
CREATE INDEX idx_countries_iso_a3    ON countries (iso_a3);
CREATE INDEX idx_countries_continent ON countries (LOWER(continent));

-- ── GeoNames reverse geocoding ──

CREATE TABLE admin1_codes (
    code TEXT PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE admin2_codes (
    code TEXT PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE geonames (
    geonameid    INTEGER PRIMARY KEY,
    name         TEXT NOT NULL,
    latitude     DOUBLE PRECISION NOT NULL,
    longitude    DOUBLE PRECISION NOT NULL,
    feature_code TEXT,
    country_code CHAR(2),
    admin1_code  TEXT,
    admin2_code  TEXT,
    population   BIGINT,
    geom         GEOMETRY(Point, 4326) NOT NULL
);

CREATE INDEX idx_geonames_geom ON geonames USING GiST (geom);
CREATE INDEX idx_geonames_geog ON geonames USING GiST ((geom::geography));
