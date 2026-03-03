use deadpool_postgres::Object;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct TableStats {
    pub name: String,
    pub size_bytes: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct DatabaseStats {
    pub countries: i64,
    pub population_cells: i64,
    pub total_population: f64,
    pub geonames_places: i64,
    pub tables: Vec<TableStats>,
}

pub(crate) struct StatsRepository;

impl StatsRepository {
    pub async fn get_stats(client: &Object) -> Result<DatabaseStats, tokio_postgres::Error> {
        let countries: i64 = client
            .query_one("SELECT COUNT(*)::bigint FROM countries", &[])
            .await?
            .get(0);

        let pop_row = client
            .query_one(
                "SELECT COUNT(*)::bigint, COALESCE(SUM(pop), 0)::float8 FROM population",
                &[],
            )
            .await?;
        let population_cells: i64 = pop_row.get(0);
        let total_population: f64 = pop_row.get(1);

        let geonames_places: i64 = client
            .query_one("SELECT COUNT(*)::bigint FROM geonames", &[])
            .await?
            .get(0);

        let table_rows = client
            .query(
                r#"
                SELECT relname, pg_total_relation_size(relid)::bigint
                FROM pg_catalog.pg_statio_user_tables
                WHERE schemaname = 'public'
                ORDER BY pg_total_relation_size(relid) DESC
                "#,
                &[],
            )
            .await?;

        let tables: Vec<TableStats> = table_rows
            .iter()
            .map(|r| TableStats {
                name: r.get(0),
                size_bytes: r.get(1),
            })
            .collect();

        Ok(DatabaseStats {
            countries,
            population_cells,
            total_population,
            geonames_places,
            tables,
        })
    }
}
