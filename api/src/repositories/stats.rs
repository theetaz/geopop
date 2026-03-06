use deadpool_postgres::Object;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct TableStats {
    pub name: String,
    pub estimated_rows: i64,
}

pub(crate) struct StatsRepository;

impl StatsRepository {
    pub async fn get_stats(client: &Object) -> Result<Vec<TableStats>, tokio_postgres::Error> {
        let rows = client
            .query(
                r#"
                SELECT relname::text, GREATEST(reltuples::bigint, 0)
                FROM pg_class
                WHERE relname IN ('population', 'countries', 'geonames', 'admin1_codes', 'admin2_codes')
                  AND relkind = 'r'
                "#,
                &[],
            )
            .await?;
        Ok(rows.iter().map(|r| TableStats { name: r.get(0), estimated_rows: r.get(1) }).collect())
    }
}
