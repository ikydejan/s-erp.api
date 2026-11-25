use opensearch::http::transport::Transport;
use opensearch::OpenSearch;

pub fn build_client(url: &str) -> anyhow::Result<OpenSearch> {
    let transport =
        Transport::single_node(url).map_err(|e| anyhow::anyhow!("invalid OPENSEARCH_URL: {e}"))?;
    Ok(OpenSearch::new(transport))
}
