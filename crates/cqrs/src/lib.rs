//! Reference CQRS wiring for the "items" master-data module. Writes go
//! through Postgres (source of truth); reads for listing/searching go through
//! OpenSearch via the transactional outbox. Copy this module's shape
//! (`dto.rs` + `ports.rs` + the `infra-postgres`/`infra-opensearch` impls)
//! when adding the next ERP module.
pub mod dto;
pub mod ports;

pub use dto::{ItemSearchQuery, ItemView, NewItemCommand};
pub use ports::{ItemSearchRepository, ItemWriteRepository, SearchIndexer};
