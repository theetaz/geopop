use actix_web::{web, HttpResponse, Result as ActixResult};
use deadpool_postgres::Pool;

use crate::config::API_PREFIX;
use crate::models::{RootPayload, TableRowCount};
use crate::repositories::StatsRepository;
use crate::response::ApiResponse;

/// Root endpoint: health status, Swagger docs link, and estimated table row counts.
#[utoipa::path(
    get,
    path = "/",
    tag = "System",
    summary = "Root / landing",
    description = "Returns health status, link to Swagger docs, and estimated row counts per table.",
    responses(
        (status = 200, description = "Service info with table row counts", body = RootPayload)
    )
)]
pub(crate) async fn root(pool: web::Data<Pool>) -> ActixResult<HttpResponse> {
    let tables = match pool.get().await {
        Ok(client) => match StatsRepository::get_stats(&client).await {
            Ok(stats) => Some(
                stats
                    .into_iter()
                    .map(|t| TableRowCount {
                        name: t.name,
                        estimated_rows: t.estimated_rows,
                    })
                    .collect(),
            ),
            Err(e) => {
                log::warn!("Failed to fetch table stats: {e}");
                None
            }
        },
        Err(e) => {
            log::warn!("Failed to get pool connection for stats: {e}");
            None
        }
    };

    Ok(ApiResponse::ok(RootPayload {
        status: "ok".into(),
        docs_url: format!("{API_PREFIX}/docs/"),
        tables,
    }))
}
