use sqlx::{Executor, PgPool, Row};

#[derive(Clone)]
pub struct Postgres(pub PgPool);

impl Postgres {
    pub fn new(url: &str) -> sqlx::Result<Self> {
        Ok(Self(PgPool::connect_lazy(url)?))
    }

    async fn count_rows_in_table(&self, table: &str) -> sqlx::Result<i64> {
        let query = format!("SELECT COUNT(*) FROM {};", table);
        let row = self.0.fetch_one(query.as_str()).await?;
        row.try_get(0).map_err(Into::into)
    }

    pub async fn update_table_rows_metric(&self) -> sqlx::Result<()> {
        let metrics = Metrics::get();
        for &table in database::ALL_TABLES {
            let count = self.count_rows_in_table(table).await?;
            metrics.table_rows.with_label_values(&[table]).set(count);
        }
        Ok(())
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Number of rows in db tables.
    #[metric(labels("table"))]
    table_rows: prometheus::IntGaugeVec,

    /// Timing of db queries.
    #[metric(labels("type"))]
    database_queries: prometheus::HistogramVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(shared::metrics::get_metric_storage_registry()).unwrap()
    }
}
