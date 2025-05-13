use std::{collections::HashMap, sync::{Arc, Mutex}};

use anyhow::Result;
use async_trait::async_trait;
use log::{error, info};
use rocketman::{connection::JetstreamConnection, endpoints::JetstreamEndpoints, handler, ingestion::LexiconIngestor, options::JetstreamOptions, types::event::Event};
use serde_json::Value;
use tokio::{select, sync::Notify, time::{self, Duration}};

static BSKY_POST_NSID: &'static str = "app.bsky.feed.post";

struct PostIngestor;

#[async_trait]
impl LexiconIngestor for PostIngestor {
    async fn ingest(&self, message: Event<Value>) ->  Result<()> {
        info!("{:?}", message);
        Ok(())
    }
}

async fn persist_cursor_task(cursor: Arc<Mutex<Option<u64>>>, shutdown: Arc<Notify>, interval: Duration) {
    let mut ticker = time::interval(interval);

    loop {
        select! {
            _ = ticker.tick() => {
                if let Some(c) = *cursor.lock().unwrap() {
                    println!("cursor = {c}");
                }
            }
            _ = shutdown.notified() => {
                if let Some(c) = *cursor.lock().unwrap() {
                    println!("final cursor = {c}");
                }
                break;
            }
        }
    }

}


pub async fn consume() -> Result<()> {
    let opts = JetstreamOptions::builder()
        .wanted_collections(vec![BSKY_POST_NSID.to_string()])
        .ws_url(JetstreamEndpoints::Public(rocketman::endpoints::JetstreamEndpointLocations::UsEast, 2))
        .build();

    let js_conn = JetstreamConnection::new(opts);

    let mut ingestors: HashMap<String, Box<dyn LexiconIngestor + Send + Sync>> = HashMap::new();
    ingestors.insert(
        BSKY_POST_NSID.to_string(),
        Box::new(PostIngestor)
    );

    let cursor = Arc::new(Mutex::new(None));

    let shutdown = Arc::new(Notify::new());
    let persist_handle = {
        let c = cursor.clone();
        let s = shutdown.clone();
        tokio::spawn(persist_cursor_task(c, s, Duration::from_millis(500)))
    };

    let msg_rx = js_conn.get_msg_rx();
    let reconnect_tx = js_conn.get_reconnect_tx();
    let c_cursor = cursor.clone();
    let ingest_handle = tokio::spawn(async move {
        while let Ok(msg) = msg_rx.recv_async().await {
            let handle_result = handler::handle_message(msg, &ingestors, reconnect_tx.clone(), c_cursor.clone()).await;
            if let Err(e) = handle_result{
                error!("Error processing message: {}", e);
            };
        }
        anyhow::Ok(())
    });

    info!("About to connect");

    if let Err(e) = js_conn.connect(cursor.clone()).await {
        error!("Error connecting: {}", e);
        std::process::exit(1);
    }

    info!("After connect");

    select! {
        _ = tokio::signal::ctrl_c() => info!("ctrl-c"),
        res = ingest_handle => { res??; }
    }

    shutdown.notify_waiters();
    persist_handle.await?;
    Ok(())
}