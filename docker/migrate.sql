-- GeoPop idempotent migration script.
--
-- This script brings an existing database up-to-date with the latest schema
-- (extensions, tables, indexes) without dropping or re-ingesting data.
--
-- Safe to run repeatedly. Every statement uses IF NOT EXISTS / CREATE OR REPLACE.
--
-- Usage:
--   psql "$DATABASE_URL" -f docker/migrate.sql
-- or:
--   make migrate
--
-- New VPS deploys should run in this order:
--   1. psql -f docker/init.sql       (creates base schema on an empty DB)
--   2. make ingest-all               (loads Natural Earth, WorldPop, GeoNames)
--   3. psql -f docker/migrate.sql    (ensures indexes/extensions are present)
--
-- Existing deploys just run step 3.

\echo '==> Ensuring required extensions'
CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS unaccent;

\echo '==> Population grid indexes'
-- population.cell_id is the primary key, no extra indexes needed.

\echo '==> Country indexes'
CREATE INDEX IF NOT EXISTS idx_countries_geom      ON countries USING GiST (geom);
CREATE INDEX IF NOT EXISTS idx_countries_iso_a2    ON countries (iso_a2);
CREATE INDEX IF NOT EXISTS idx_countries_iso_a3    ON countries (iso_a3);
CREATE INDEX IF NOT EXISTS idx_countries_continent ON countries (LOWER(continent));
CREATE INDEX IF NOT EXISTS idx_countries_region_un ON countries (LOWER(region_un));

\echo '==> GeoNames spatial indexes'
CREATE INDEX IF NOT EXISTS idx_geonames_geom ON geonames USING GiST (geom);
CREATE INDEX IF NOT EXISTS idx_geonames_geog ON geonames USING GiST ((geom::geography));

\echo '==> GeoNames city-search indexes (this can take a few minutes on 5M rows)'
-- Trigram GIN index powers fuzzy search (% operator, similarity(), ILIKE '%foo%').
CREATE INDEX IF NOT EXISTS idx_geonames_name_trgm
    ON geonames USING GIN (name gin_trgm_ops);

-- Prefix index for the "starts-with" fast path in autocomplete.
CREATE INDEX IF NOT EXISTS idx_geonames_name_lower
    ON geonames (LOWER(name) text_pattern_ops);

-- Country scoping for /cities/search?country=XX
CREATE INDEX IF NOT EXISTS idx_geonames_country_code
    ON geonames (country_code);

-- Helps "rank within country by population" queries use an index sort.
CREATE INDEX IF NOT EXISTS idx_geonames_country_pop
    ON geonames (country_code, population DESC);

-- Feature-code filter (PPLC, PPLA, PPLA2, ...).
CREATE INDEX IF NOT EXISTS idx_geonames_feature_code
    ON geonames (feature_code);

\echo '==> Recreating get_population() function'
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

\echo '==> Updating planner statistics on large tables'
ANALYZE geonames;
ANALYZE countries;
ANALYZE population;

\echo '==> Migration complete'
