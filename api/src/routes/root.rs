use actix_web::{web, HttpResponse, Result as ActixResult};
use deadpool_postgres::Pool;

use crate::config::API_PREFIX;
use crate::models::{DatabaseStatsPayload, RootPayload, TableSizePayload};
use crate::repositories::StatsRepository;
use crate::response::ApiResponse;

/// Root endpoint: health status, Swagger docs link, and database statistics.
#[utoipa::path(
    get,
    path = "/",
    tag = "System",
    summary = "Root / landing",
    description = "Returns health status, link to Swagger docs, and database statistics (counts, table sizes).",
    responses(
        (status = 200, description = "Service info with optional database stats", body = RootPayload)
    )
)]
pub(crate) async fn root(pool: web::Data<Pool>) -> ActixResult<HttpResponse> {
    let status = "ok".to_string();
    let docs_url = format!("{API_PREFIX}/docs/");

    let database = match pool.get().await {
        Ok(client) => match StatsRepository::get_stats(&client).await {
            Ok(stats) => Some(DatabaseStatsPayload {
                countries: stats.countries,
                population_cells: stats.population_cells,
                total_population: stats.total_population,
                geonames_places: stats.geonames_places,
                tables: stats
                    .tables
                    .into_iter()
                    .map(|t| TableSizePayload {
                        name: t.name,
                        size_bytes: t.size_bytes,
                    })
                    .collect(),
            }),
            Err(e) => {
                log::warn!("Failed to fetch database stats: {e}");
                None
            }
        },
        Err(e) => {
            log::warn!("Failed to get pool connection for stats: {e}");
            None
        }
    };

    Ok(ApiResponse::ok(RootPayload {
        status,
        docs_url,
        database,
    }))
}
