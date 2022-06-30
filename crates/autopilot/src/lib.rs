pub mod arguments;
pub mod database;

use crate::database::Postgres;
use shared::metrics::LivenessChecking;
use std::{sync::Arc, time::Duration};

/// Assumes tracing and metrics registry have already been set up.
pub async fn main(args: arguments::Arguments) {
    let serve_metrics = shared::metrics::serve_metrics(Arc::new(Liveness), args.metrics_address);
    let db = Postgres::new(args.db_url.as_str()).unwrap();
    let db_metrics = database_metrics(db);
    tokio::select! {
        result = serve_metrics => tracing::error!(?result, "serve_metrics exited"),
        _ = db_metrics => (),
    };
}

struct Liveness;
#[async_trait::async_trait]
impl LivenessChecking for Liveness {
    async fn is_alive(&self) -> bool {
        true
    }
}

async fn database_metrics(db: Postgres) -> ! {
    loop {
        if let Err(err) = db.update_table_rows_metric().await {
            tracing::error!(?err, "failed to update table rows metric");
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}
