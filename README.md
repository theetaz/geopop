# GeoPop

**High-performance global population & geocoding API** backed by PostGIS, Rust, and open datasets.

Query any coordinate on Earth and get back population estimates, reverse geocoding, country info, and disaster risk exposure analysis — all in under 50ms for typical requests.

---

## Features

- **Population lookup** — 1km resolution grid covering the entire globe (175M+ cells)
- **Population grid** — retrieve all grid cells within a radius with bounds for map rendering
- **Batch queries** — up to 1,000 coordinate lookups in a single request
- **Reverse geocoding** — nearest populated place from 4.8M+ GeoNames entries
- **Exposure analysis** — population within a radius with directional info for each place
- **Disaster impact analysis** — auto-expanding radius search for remote/ocean epicentres
- **Country lookup** — point-in-polygon and ISO code lookup with Natural Earth boundaries
- **Swagger UI** — interactive API docs at `/api/v1/docs/`

## Architecture

![Architecture](docs/images/architecture.png)

## Data Pipeline

![Data Pipeline](docs/images/data-pipeline.png)

## Database Schema

![Database Schema](docs/images/db-schema.png)

## Quick Start

### Prerequisites

- Docker & Docker Compose
- Python 3.10+ (for data ingestion)
- ~5 GB disk space (database + downloaded data)

### 1. Clone and configure

```bash
git clone https://github.com/your-username/geopop.git
cd geopop
cp .env.example .env
```

### 2. Full automated setup

```bash
make setup
```

This downloads all datasets, starts the database, runs ingestion, and starts the API. The WorldPop download (~723 MB) and ingestion (~175M rows) takes about 30–45 minutes depending on your connection and hardware.

### Or step by step

```bash
# Download datasets
make download-all

# Start database
make db-up

# Ingest data (order matters: Natural Earth → WorldPop → GeoNames)
make ingest-all

# Start all services
make up
```

### 3. Verify

```bash
make test
```

The API is available at `http://localhost:8080/api/v1` and Swagger UI at `http://localhost:8080/api/v1/docs/`.

All routes are prefixed with `/api/v1`. The prefix is defined once in `api/src/config.rs` (`API_PREFIX` constant) — change it there to update all routes, Swagger UI path, and OpenAPI spec simultaneously.

## API Endpoints

### `GET /api/v1/population`

Population at a single coordinate (1km grid cell). Optionally provide a `radius` (max 10 km) to get all non-empty grid cells within the circle, with bounds for map rendering.

**Single cell (no radius):**

```bash
curl "localhost:8080/api/v1/population?lat=51.5074&lon=-0.1278"
```

```json
{
  "code": 200,
  "message": "success",
  "payload": {
    "lat": 51.5074,
    "lon": -0.1278,
    "population": 5765.2,
    "resolution_km": 1.0
  }
}
```

**Grid cells (with radius):**

```bash
curl "localhost:8080/api/v1/population?lat=51.5074&lon=-0.1278&radius=2"
```

```json
{
  "code": 200,
  "message": "success",
  "payload": {
    "coordinate": { "lat": 51.5074, "lon": -0.1278 },
    "radius_km": 2.0,
    "total_population": 87432.5,
    "cell_count": 18,
    "cells": [
      {
        "lat": 51.50833,
        "lon": -0.12917,
        "population": 5765.2,
        "bounds": {
          "min_lat": 51.50417,
          "max_lat": 51.5125,
          "min_lon": -0.13333,
          "max_lon": -0.125
        }
      }
    ]
  }
}
```

Each cell includes centre coordinates and geographic `bounds` (min/max lat/lon) for rendering grid rectangles on a map. Only cells with `population > 0` are returned, sorted by population descending.

| Parameter | Type    | Required | Description                              |
| --------- | ------- | -------- | ---------------------------------------- |
| `lat`     | float   | yes      | Latitude (-90 to 90)                     |
| `lon`     | float   | yes      | Longitude (-180 to 180)                  |
| `radius`  | float   | no       | Search radius in km (max 10). When omitted, returns a single cell. |

### `POST /api/v1/population/batch`

Batch lookup for up to 1,000 coordinates.

```bash
curl -X POST "localhost:8080/api/v1/population/batch" \
  -H "Content-Type: application/json" \
  -d '{"points":[{"lat":51.5074,"lon":-0.1278},{"lat":35.6762,"lon":139.6503}]}'
```

### `GET /api/v1/reverse`

Nearest populated place (reverse geocoding).

```bash
curl "localhost:8080/api/v1/reverse?lat=35.6762&lon=139.6503"
```

```json
{
  "code": 200,
  "message": "success",
  "payload": {
    "place_id": 1850147,
    "name": "Tokyo",
    "display_name": "Tokyo, Tokyo, Japan",
    "address": {
      "city": "Tokyo",
      "state": "Tokyo",
      "country": "Japan",
      "country_code": "jp"
    }
  }
}
```

### `GET /api/v1/exposure`

Population exposure within a radius — useful for disaster risk assessment. Each place includes its compass `direction` and `bearing_deg` from the epicentre.

```bash
curl "localhost:8080/api/v1/exposure?lat=20.4657&lon=93.9572&radius=10"
```

```json
{
  "code": 200,
  "message": "success",
  "payload": {
    "coordinate": { "lat": 20.4657, "lon": 93.9572 },
    "radius_km": 10.0,
    "total_population": 1653.2,
    "area_km2": 314.16,
    "density_per_km2": 5.3,
    "cell_population": 5.16,
    "cell_area_km2": 0.81,
    "cell_density_per_km2": 6.4,
    "places": [
      {
        "place_id": 1325189,
        "name": "Hetsaw",
        "display_name": "Hetsaw, Kyaunkpyu District, Rakhine, Myanmar",
        "address": {
          "city": "Hetsaw",
          "district": "Kyaunkpyu District",
          "state": "Rakhine",
          "country": "Myanmar",
          "country_code": "mm"
        },
        "distance_km": 4.69,
        "direction": "SW",
        "bearing_deg": 233.3
      }
    ]
  }
}
```

| Parameter | Type  | Required | Default | Description                     |
| --------- | ----- | -------- | ------- | ------------------------------- |
| `lat`     | float | yes      | —       | Latitude (-90 to 90)            |
| `lon`     | float | yes      | —       | Longitude (-180 to 180)         |
| `radius`  | float | no       | 1       | Search radius in km (max 500)   |

The `direction` field is an 8-point compass value (N, NE, E, SE, S, SW, W, NW) and `bearing_deg` is the precise azimuth (0° = North, 90° = East).

### `GET /api/v1/analyse`

Disaster impact analysis with auto-expanding radius. Takes only a coordinate — no radius needed. The endpoint automatically identifies the country, finds the nearest named place, and expands the search radius in 5 km increments (up to 1000 km) until population is found.

Ideal for disaster events where the epicentre may be in ocean, desert, or uninhabited terrain.

```bash
curl "localhost:8080/api/v1/analyse?lat=5.0&lon=75.0"
```

```json
{
  "code": 200,
  "message": "success",
  "payload": {
    "coordinate": { "lat": 5.0, "lon": 75.0 },
    "country": {
      "iso_a2": "MV", "iso_a3": "MDV", "name": "Maldives",
      "formal_name": "Republic of Maldives",
      "continent": "Seven seas (open ocean)",
      "region": "Asia", "subregion": "Southern Asia"
    },
    "nearest_place": {
      "place_id": 6692738,
      "name": "Meerufenfushi",
      "display_name": "Meerufenfushi, Kaafu Atoll, Maldives",
      "address": { "city": "Meerufenfushi", "state": "Kaafu Atoll", "country": "Maldives", "country_code": "mv" },
      "distance_km": 154.65,
      "direction": "SW",
      "bearing_deg": 246.9
    },
    "population": {
      "search_radius_km": 155.0,
      "total_population": 1797.2,
      "area_km2": 75476.76,
      "density_per_km2": 0.0,
      "epicentre_population": 0.0
    }
  }
}
```

| Field | Description |
| ----- | ----------- |
| `country` | Country the epicentre is in, or nearest country if in ocean |
| `nearest_place` | Closest named city/town/village with distance, compass direction, and bearing |
| `population.search_radius_km` | How far the search expanded to find population (indicates remoteness) |
| `population.epicentre_population` | Population at the exact epicentre cell (0 if ocean/desert) |
| `population.total_population` | Total population within the search radius |

### `GET /api/v1/country`

Country containing a coordinate.

```bash
curl "localhost:8080/api/v1/country?lat=48.8566&lon=2.3522"
```

### `GET /api/v1/country/{iso3}`

Country details by ISO 3166-1 alpha-3 code.

```bash
curl "localhost:8080/api/v1/country/FRA"
```

### `GET /api/v1/countries`

List countries by continent. Valid values: `asia`, `europe`, `africa`, `oceania`, `americas`, `north-america`, `south-america`.

```bash
curl "localhost:8080/api/v1/countries?continent=europe"
```

### `GET /api/v1/health`

Service health check.

```bash
curl "localhost:8080/api/v1/health"
```

## Performance

| Endpoint              | Typical Latency | Strategy                                     |
| --------------------- | --------------- | -------------------------------------------- |
| `/population`         | ~2ms            | B-tree lookup on `cell_id`                   |
| `/population?radius=` | ~10ms           | `generate_series` grid scan + filter         |
| `/reverse`            | ~5ms            | GiST index nearest-neighbor                  |
| `/exposure` (10km)    | ~20ms           | `generate_series` grid scan + GiST geography |
| `/exposure` (50km)    | ~100ms          | Same strategy, more cells                    |
| `/analyse` (on land)  | ~10ms           | KNN + single grid check                      |
| `/analyse` (ocean)    | ~50–3000ms      | Auto-expanding radius until population found |
| `/country`            | ~10ms           | `ST_Contains` with GiST index                |

Key optimizations:

- **Integer cell_id** — population lookups are B-tree `O(log n)` on 175M rows, not spatial queries
- **Geography GiST index** — `ST_DWithin` on GeoNames uses a dedicated `(geom::geography)` index
- **JIT disabled** — PostgreSQL JIT compilation adds ~700ms overhead on first query; disabled for consistent sub-50ms responses
- **Connection pooling** — `deadpool-postgres` with `RecyclingMethod::Fast`
- **Compiler optimizations** — release build with `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`

## Data Sources

| Dataset       | Source                                                                                 | Size    | Records       |
| ------------- | -------------------------------------------------------------------------------------- | ------- | ------------- |
| WorldPop      | [worldpop.org](https://www.worldpop.org/) — R2024B 2025 unconstrained UN-adjusted 1km  | ~723 MB | 175M cells    |
| GeoNames      | [geonames.org](https://www.geonames.org/) — allCountries, filtered to populated places | ~380 MB | 4.8M places   |
| Natural Earth | [naturalearthdata.com](https://www.naturalearthdata.com/) — 10m Admin 0 countries      | ~5 MB   | 258 countries |

## Project Structure

```
geopop/
├── api/                    # Rust API server
│   ├── src/
│   │   ├── main.rs         # Server setup, connection pool
│   │   ├── config.rs       # Environment configuration & API_PREFIX
│   │   ├── errors.rs       # Error types and response mapping
│   │   ├── grid.rs         # Cell ID computation (30 arc-second grid)
│   │   ├── response.rs     # Unified API response wrapper
│   │   ├── validation.rs   # Input validation helpers
│   │   ├── models/         # Request/response data structures
│   │   ├── repositories/   # Database query layer
│   │   └── routes/         # Endpoint handlers
│   ├── Cargo.toml
│   └── Dockerfile
├── docker/                 # Database container
│   ├── Dockerfile.db
│   ├── init.sql            # Schema, indexes, functions
│   └── postgresql.conf     # Tuned for population workload
├── ingestion/              # Data download & ingestion scripts
│   ├── download_worldpop.sh
│   ├── download_geonames.sh
│   ├── download_naturalearth.sh
│   ├── ingest.py           # WorldPop GeoTIFF → population table
│   ├── ingest_geonames.py  # GeoNames → geonames + admin tables
│   ├── ingest_naturalearth.py  # Shapefile → countries table
│   └── requirements.txt
├── docker-compose.yml
├── Makefile
└── .env.example
```

## Configuration

All configuration is via environment variables (see `.env.example`):

| Variable            | Default  | Description                                        |
| ------------------- | -------- | -------------------------------------------------- |
| `POSTGRES_USER`     | `geopop` | Database username                                  |
| `POSTGRES_PASSWORD` | `geopop` | Database password                                  |
| `POSTGRES_DB`       | `geopop` | Database name                                      |
| `DB_PORT`           | `5432`   | Host port for PostgreSQL                           |
| `API_PORT`          | `8080`   | Host port for the API                              |
| `POOL_SIZE`         | `16`     | Connection pool size                               |
| `DATABASE_URL`      | —        | Full connection string (overrides individual vars) |

## Development

```bash
# Build API locally (requires Rust)
make api-build

# Run smoke tests
make test

# Benchmark (requires 'hey')
make bench

# View logs
make logs

# Full cleanup
make clean
```

## License

MIT
