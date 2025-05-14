use eyewall_indexer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    eyewall_indexer::ingestion::consume().await?;
    Ok(())
}
