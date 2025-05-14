use std::{collections::HashMap, sync::{Arc, Mutex}};

use anyhow::{Result};
use async_trait::async_trait;
use log::{debug, error, info};
use rocketman::{connection::JetstreamConnection, endpoints::JetstreamEndpoints, handler, ingestion::LexiconIngestor, options::JetstreamOptions, types::event::{Event, Kind}};
use serde_json::Value;
use tokio::{select, sync::Notify, time::{self, Duration}};

use crate::storage::{self, BskyPostRecord};

static BSKY_POST_NSID: &'static str = "app.bsky.feed.post";
static BSKY_EMBED_RECORD_NSID: &'static str = "app.bsky.embed.record";


struct PostIngestor {
    pool: storage::db::DbPool
}

impl PostIngestor {
    async fn new() -> Result<PostIngestor> {
        let pool = storage::db::connect().await?;
        Ok(PostIngestor { pool })
    }
}

#[async_trait]
impl LexiconIngestor for PostIngestor {

    async fn ingest(&self, message: Event<Value>) ->  Result<()> {
        if message.commit.is_none() {
            return Ok(());
        }
        let commit = message.commit.as_ref().unwrap();
        if commit.record.is_none() {
            return Ok(());
        }
        let record = commit.record.as_ref().unwrap();

        match record {
            Value::Object(map) => {
                let reply = map.get("reply");
                let has_reply = reply.is_some();

                let mut embed_record = None;
                if let Some(Value::Object(embed_map)) = map.get("embed") {
                    if let Some(Value::String(ty)) = embed_map.get("$type") {
                        if ty == BSKY_EMBED_RECORD_NSID {
                            embed_record = embed_map.get("record");
                        }
                    }
                }
                let has_quote = embed_record.is_none();

                if !has_reply && !has_quote {
                    return Ok(());
                }

                debug!("{:?}", message);

                let post_id = storage::BskyPostId::new(&message.did[..], &commit.rkey[..]);

                let reply_to = reply.map(|val| {
                    if let Value::Object(map) = val {
                        if let Some(Value::Object(parent_map)) = map.get("parent") {
                            if let Some(Value::String(parent_uri)) = parent_map.get("uri") {
                                let parts: Vec<_> = parent_uri.split('/').collect();
                                let parent_id = storage::BskyPostId::new(parts[2], parts[4]);
                                if let Some(Value::Object(root_map)) = map.get("root") {
                                    if let Some(Value::String(root_uri)) = root_map.get("uri") {
                                        let parts: Vec<_> = root_uri.split('/').collect();
                                        let root_id = storage::BskyPostId::new(parts[2], parts[4]);
                                        return storage::BskyPostReplyTo::new(parent_id, root_id);
                                    }
                                }
                            }
                        }
                    }
                    panic!("Expected reply to be object");
                });

                let mut quote_of = None;
                if let Some(Value::Object(embed_record)) = embed_record {
                    if let Some(Value::String(uri)) = embed_record.get("uri") {
                        let parts: Vec<_> = uri.split('/').collect();
                        let author_did = parts[2];
                        let rkey = parts[4];
                        quote_of = Some(storage::BskyPostId::new(author_did, rkey));
                    }
                }

                let post = BskyPostRecord::new(post_id, reply_to, quote_of, message.time_us);
                storage::db::insert_bsky_post(&self.pool, post).await?;

            },
            _ => {
                return Ok(());
            }
        }

        // Event {
        //   did: "did:plc:i5o7ybb4yg45zhnhigrzeiqn", time_us: Some(1746977528899856), kind: Commit, commit: Some(Commit {
        //   rev: "3lovrnkjkf32t", operation: Create, collection: "app.bsky.feed.post",
        //   rkey: "3lovrnkbesk2z",
        //   record: Some(Object {
        //      "$type": String("app.bsky.feed.post"), "createdAt": String("2025-05-11T15:32:08.458Z"), "langs": Array [String("en")],
        //      "reply": Object {
        //          "parent": Object {
        //              "cid": String("bafyreifhgwuhg25cdh7r4xg54c7zyt3miq37caeixkfdwlpsxbsucdedey"),
        //              "uri": String("at://did:plc:qsmmhv4u2ygx2thvepti77zc/app.bsky.feed.post/3lovq4eaoxc2y")
        //          },
        //          "root": Object {
        //              "cid": String("bafyreifhgwuhg25cdh7r4xg54c7zyt3miq37caeixkfdwlpsxbsucdedey"), "uri": String("at://did:plc:qsmmhv4u2ygx2thvepti77zc/app.bsky.feed.post/3lovq4eaoxc2y")
        //          }
        //      }, "text": String("Military security is a concept of the past for Trump. Anything and everything is for sale.")}), cid: Some("bafyreicf5n2zehmldcvn6twlmoadq6awsdbofhizq2saxstdik5d4yvckm") }),
        // identity: None }


        /*
         *
         * Event {
         *  did: "did:plc:qinfhaxhsd3kwga35ldgc4zu", time_us: Some(1746985087246643), kind: Commit,
         *  commit: Some(Commit {
         *      rev: "3lovyosmtu32v", operation: Create, collection: "app.bsky.feed.post", rkey: "3lovyosgpvc2t",
         *      record: Some(Object {
         *          "$type": String("app.bsky.feed.post"), "createdAt": String("2025-05-11T17:38:06.770Z"),
         *          "embed": Object {
         *              "$type": String("app.bsky.embed.record"),
         *              "record": Object {
         *                   "cid": String("bafyreifpwkwnmcrkmvjhp6kbpzmxe3cpua4dcjwcfpsqz4bkxjiypj2fn4"), "uri": String("at://did:plc:ib6pplehueaytqk3q7kpllwh/app.bsky.feed.post/3lovugh5g4s2s")
         *              }
         *          }, "langs": Array [String("en")], "text": String("I am old enough to remember when Jimmie Carter gave up his beloved peanut farm voluntarily even before anyone complained, simply to not risk any accidental infraction of the Emoluments clause")}), cid: Some("bafyreih52kyfdiagt3lzrl5ekg5ulc632624hws4xanndhsbzne7yoxfhm") }), identity: None }
         */


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


pub async fn consume(cursor_val: Option<u64>) -> Result<()> {
    let opts = JetstreamOptions::builder()
        .wanted_collections(vec![BSKY_POST_NSID.to_string()])
        .ws_url(JetstreamEndpoints::Public(rocketman::endpoints::JetstreamEndpointLocations::UsEast, 2))
        .build();

    let js_conn = JetstreamConnection::new(opts);

    let mut ingestors: HashMap<String, Box<dyn LexiconIngestor + Send + Sync>> = HashMap::new();
    ingestors.insert(
        BSKY_POST_NSID.to_string(),
        Box::new(PostIngestor::new().await.expect("Could not connect to DB"))
    );

    let cursor = Arc::new(Mutex::new(cursor_val));

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
            }
        }
        anyhow::Ok(())
    });

    if let Err(e) = js_conn.connect(cursor.clone()).await {
        error!("Error connecting: {}", e);
        std::process::exit(1);
    }

    select! {
        _ = tokio::signal::ctrl_c() => info!("ctrl-c"),
        res = ingest_handle => { res??; }
    }

    shutdown.notify_waiters();
    persist_handle.await?;
    Ok(())
}