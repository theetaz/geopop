-- GeoPop schema: WorldPop population grid + GeoNames places + Natural Earth countries
CREATE EXTENSION IF NOT EXISTS postgis;
-- pg_trgm powers fuzzy / prefix / substring matching on place names for /cities/search
CREATE EXTENSION IF NOT EXISTS pg_trgm;
-- unaccent lets us match "Sao Paulo" against "São Paulo" transparently
CREATE EXTENSION IF NOT EXISTS unaccent;

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
    type        TEXT,
    sovereign   BOOLEAN NOT NULL DEFAULT true,
    pop_est     BIGINT,
    geom        GEOMETRY(MultiPolygon, 4326) NOT NULL
);

CREATE INDEX idx_countries_geom      ON countries USING GiST (geom);
CREATE INDEX idx_countries_iso_a2    ON countries (iso_a2);
CREATE INDEX idx_countries_iso_a3    ON countries (iso_a3);
CREATE INDEX idx_countries_continent ON countries (LOWER(continent));
CREATE INDEX idx_countries_region_un ON countries (LOWER(region_un));

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

-- ── City search indexes ──
-- Trigram GIN index powers fuzzy search (% operator, similarity(), ILIKE '%foo%').
CREATE INDEX idx_geonames_name_trgm
    ON geonames USING GIN (name gin_trgm_ops);

-- Prefix index for the fast-path "starts-with" branch of autocomplete.
CREATE INDEX idx_geonames_name_lower
    ON geonames (LOWER(name) text_pattern_ops);

-- Country scoping for /cities/search?country=XX
CREATE INDEX idx_geonames_country_code
    ON geonames (country_code);

-- Lets "rank by population within a country" sort use the index.
CREATE INDEX idx_geonames_country_pop
    ON geonames (country_code, population DESC);

-- Feature code filter speeds up the "cities only" subset (PPL*, excluding hamlets/farms).
CREATE INDEX idx_geonames_feature_code
    ON geonames (feature_code);
