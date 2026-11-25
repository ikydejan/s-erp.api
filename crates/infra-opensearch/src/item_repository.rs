use async_trait::async_trait;
use cqrs::{ItemSearchQuery, ItemSearchRepository, ItemView, SearchIndexer};
use opensearch::indices::{IndicesCreateParts, IndicesExistsParts};
use opensearch::{DeleteParts, IndexParts, OpenSearch, SearchParts};
use serde_json::{json, Value};
use uuid::Uuid;

const INDEX: &str = "items";

pub struct OpenSearchItemRepository {
    client: OpenSearch,
}

impl OpenSearchItemRepository {
    pub fn new(client: OpenSearch) -> Self {
        Self { client }
    }

    /// Idempotent: creates the `items` index with an explicit mapping if it
    /// doesn't exist yet. Call once at `worker`/`api` startup.
    pub async fn ensure_index(&self) -> anyhow::Result<()> {
        let exists = self
            .client
            .indices()
            .exists(IndicesExistsParts::Index(&[INDEX]))
            .send()
            .await?;
        if exists.status_code().is_success() {
            return Ok(());
        }

        self.client
            .indices()
            .create(IndicesCreateParts::Index(INDEX))
            .body(json!({
                "mappings": {
                    "properties": {
                        "id": { "type": "keyword" },
                        "sku": { "type": "keyword" },
                        "name": { "type": "text" },
                        "description": { "type": "text" },
                        "is_active": { "type": "boolean" },
                        "created_at": { "type": "date" },
                        "updated_at": { "type": "date" }
                    }
                }
            }))
            .send()
            .await?
            .error_for_status_code()?;
        Ok(())
    }
}

#[async_trait]
impl SearchIndexer for OpenSearchItemRepository {
    async fn upsert_item(&self, item: &ItemView) -> anyhow::Result<()> {
        self.client
            .index(IndexParts::IndexId(INDEX, &item.id.to_string()))
            .body(item)
            .send()
            .await?
            .error_for_status_code()?;
        Ok(())
    }

    async fn delete_item(&self, id: Uuid) -> anyhow::Result<()> {
        let response = self
            .client
            .delete(DeleteParts::IndexId(INDEX, &id.to_string()))
            .send()
            .await?;
        // 404 just means it was already gone from the read side — not an error.
        if response.status_code().as_u16() == 404 {
            return Ok(());
        }
        response.error_for_status_code()?;
        Ok(())
    }
}

#[async_trait]
impl ItemSearchRepository for OpenSearchItemRepository {
    async fn search(&self, query: ItemSearchQuery) -> anyhow::Result<(Vec<ItemView>, u64)> {
        let mut must: Vec<Value> = Vec::new();
        if let Some(q) = query.q.as_deref().filter(|q| !q.is_empty()) {
            must.push(json!({ "multi_match": { "query": q, "fields": ["sku", "name"] } }));
        }
        let mut filter: Vec<Value> = Vec::new();
        if let Some(is_active) = query.is_active {
            filter.push(json!({ "term": { "is_active": is_active } }));
        }

        let es_query = if must.is_empty() && filter.is_empty() {
            json!({ "match_all": {} })
        } else {
            json!({ "bool": { "must": must, "filter": filter } })
        };

        let page = query.page.max(1);
        let per_page = query.per_page.clamp(1, 100);
        let from = (page - 1) * per_page;

        let response = self
            .client
            .search(SearchParts::Index(&[INDEX]))
            .from(from as i64)
            .size(per_page as i64)
            .body(json!({ "query": es_query, "sort": [{ "updated_at": "desc" }] }))
            .send()
            .await?
            .error_for_status_code()?;

        let body: Value = response.json().await?;
        let total = body["hits"]["total"]["value"].as_u64().unwrap_or(0);
        let items = body["hits"]["hits"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|hit| serde_json::from_value::<ItemView>(hit["_source"].clone()).ok())
            .collect();

        Ok((items, total))
    }
}
