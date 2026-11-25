use std::time::Duration;

use cqrs::{ItemView, SearchIndexer};
use infra_postgres::{PendingSearchEvent, SearchOutboxRepository};

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const BATCH_SIZE: i64 = 50;

/// Drains `search_outbox` into OpenSearch — the CQRS read-side sync. Adding a
/// new module's events just means matching its `aggregate_type` below.
pub async fn run(outbox: SearchOutboxRepository, indexer: impl SearchIndexer) {
    loop {
        let events = match outbox.fetch_pending(BATCH_SIZE).await {
            Ok(events) => events,
            Err(e) => {
                tracing::error!(error = %e, "failed to fetch pending outbox events");
                tokio::time::sleep(POLL_INTERVAL).await;
                continue;
            }
        };

        if events.is_empty() {
            tokio::time::sleep(POLL_INTERVAL).await;
            continue;
        }

        for event in events {
            if let Err(e) = dispatch(&outbox, &indexer, &event).await {
                tracing::error!(error = %e, event_id = event.id, "outbox dispatch bookkeeping failed");
            }
        }
    }
}

async fn dispatch(
    outbox: &SearchOutboxRepository,
    indexer: &impl SearchIndexer,
    event: &PendingSearchEvent,
) -> anyhow::Result<()> {
    let result = match (event.aggregate_type.as_str(), event.event_type.as_str()) {
        ("items", "upsert") => match serde_json::from_value::<ItemView>(event.payload.clone()) {
            Ok(item) => indexer.upsert_item(&item).await,
            Err(e) => Err(anyhow::anyhow!("bad payload: {e}")),
        },
        ("items", "delete") => indexer.delete_item(event.aggregate_id).await,
        (agg, ev) => Err(anyhow::anyhow!("unknown outbox event {agg}/{ev}")),
    };

    match result {
        Ok(()) => outbox.mark_dispatched(event.id).await,
        Err(e) => {
            tracing::warn!(error = %e, event_id = event.id, "indexing failed, will retry");
            outbox.mark_failed(event.id, &e.to_string()).await
        }
    }
}
