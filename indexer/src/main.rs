use eyewall_indexer;
static CURSOR_VAL_US: Option<u64> = Some(1746075600_000000);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Hello, world!");
    env_logger::init();
    eyewall_indexer::ingestion::consume(CURSOR_VAL_US).await?;
    Ok(())
}
