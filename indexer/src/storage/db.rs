use std::env;

use sqlx::{postgres::{PgPoolOptions, PgRow}, Pool, Postgres, Row};

use super::{BskyPostId, BskyPostRecord};

pub type DbPool = Pool<Postgres>;

pub async fn connect() -> Result<DbPool, sqlx::Error> {
    let db_url = env::var("DATABASE_URL").expect("Missing DATABASE_URL env var");
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url).await
}

pub async fn insert_bsky_post<S: AsRef<str>>(pool: &DbPool, post: BskyPostRecord<BskyPostId<S>>) -> Result<(), sqlx::Error> {
    let result = sqlx::query("SELECT COUNT(*) as count FROM eyewall.bsky_posts AS posts WHERE posts.author_did = $1 and posts.rkey = $2")
        .bind(post.id.did.as_ref())
        .bind(post.id.rkey.as_ref())
        .fetch_one(pool)
        .await?;
    let count: i64 = result.try_get("count")?;
    if count == 1 {
        return Ok(());
    }

    let created_at = post.created_at.map(|ts| (ts as f64) / 1e6);
    sqlx::query("INSERT INTO eyewall.bsky_posts (author_did, rkey, created_at) VALUES ($1, $2, to_timestamp($3))")
        .bind(post.id.did.as_ref())
        .bind(post.id.rkey.as_ref())
        .bind(created_at)
        .execute(pool)
        .await?;

    if let Some(reply) = post.reply_to {
        let result = sqlx::query("SELECT COUNT(*) as count FROM eyewall.bsky_posts AS posts WHERE posts.author_did = $1 and posts.rkey = $2")
            .bind(reply.target.did.as_ref())
            .bind(reply.target.rkey.as_ref())
            .fetch_one(pool)
            .await?;
        let count: i64 = result.try_get("count")?;
        if count == 0 {
            sqlx::query("INSERT INTO eyewall.bsky_posts (author_did, rkey) VALUES ($1, $2)")
                .bind(reply.target.did.as_ref())
                .bind(reply.target.rkey.as_ref())
                .execute(pool)
                .await?;
        }
        sqlx::query("INSERT INTO eyewall.bsky_post_replies (author_did, rkey, parent_author_did, parent_rkey) VALUES ($1, $2, $3, $4)")
            .bind(post.id.did.as_ref())
            .bind(post.id.rkey.as_ref())
            .bind(reply.target.did.as_ref())
            .bind(reply.target.rkey.as_ref())
            .execute(pool)
            .await?;
    }
    if let Some(quote) = post.quote_of {
        let result = sqlx::query("SELECT COUNT(*) as count FROM eyewall.bsky_posts AS posts WHERE posts.author_did = $1 and posts.rkey = $2")
            .bind(quote.did.as_ref())
            .bind(quote.rkey.as_ref())
            .fetch_one(pool)
            .await?;
        let count: i64 = result.try_get("count")?;
        if count == 0 {
            sqlx::query("INSERT INTO eyewall.bsky_posts (author_did, rkey) VALUES ($1, $2)")
                .bind(quote.did.as_ref())
                .bind(quote.rkey.as_ref())
                .execute(pool)
                .await?;
        }
        sqlx::query("INSERT INTO eyewall.bsky_post_quotes (author_did, rkey, parent_author_did, parent_rkey) VALUES ($1, $2, $3, $4)")
            .bind(post.id.did.as_ref())
            .bind(post.id.rkey.as_ref())
            .bind(quote.did.as_ref())
            .bind(quote.rkey.as_ref())
            .execute(pool)
            .await?;
    }
    Ok(())
}
