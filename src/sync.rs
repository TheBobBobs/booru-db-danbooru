use std::{sync::Arc, time::Instant};

use serde::Deserialize;
use sqlx::{postgres::PgListener, Executor};
use tokio::sync::RwLock;

use crate::{
    index::IdIndex,
    post::{BooruPost, RawBooruPost},
    Db,
};

pub async fn create_listener(uri: &str, pool: &sqlx::PgPool) -> PgListener {
    pool.execute(
        r#"
        CREATE OR REPLACE FUNCTION posts_notify() RETURNS TRIGGER as $posts_notify$
        BEGIN
            CASE TG_OP
                WHEN 'UPDATE' THEN
                    PERFORM pg_notify('public_posts_update', '{"old":' || row_to_json(OLD)::text || ',"new":' || row_to_json(NEW)::text || '}');
                    RETURN NEW;
                WHEN 'INSERT' THEN
                    PERFORM pg_notify('public_posts_insert', row_to_json(NEW)::text);
                    RETURN NEW;
                WHEN 'DELETE' THEN
                    PERFORM pg_notify('public_posts_delete', row_to_json(OLD)::text);
                    RETURN OLD;
            END CASE;
        END;
        $posts_notify$ LANGUAGE plpgsql
        "#,
        )
        .await.unwrap();
    pool.execute(
        "CREATE OR REPLACE TRIGGER public_posts_trigger
            AFTER INSERT OR UPDATE OR DELETE ON public.posts
            FOR EACH ROW
            EXECUTE FUNCTION posts_notify()",
    )
    .await
    .unwrap();
    let mut listener = PgListener::connect(uri).await.unwrap();
    listener
        .listen_all(vec![
            "public_posts_insert",
            "public_posts_update",
            "public_posts_delete",
        ])
        .await
        .unwrap();
    listener
}

pub async fn handle_listener(db: Arc<RwLock<Db>>, mut pg_listener: PgListener) {
    #[derive(Deserialize)]
    struct Update {
        old: RawBooruPost,
        new: RawBooruPost,
    }
    while let Ok(notif) = pg_listener.recv().await {
        let channel = notif.channel();
        let payload = notif.payload();
        let start_time = Instant::now();
        match channel {
            "public_posts_update" => {
                let data: Update = serde_json::from_str(payload).unwrap();
                let old: BooruPost = data.old.into();
                let new = data.new.into();
                let mut db = db.write().await;
                let id_index: &IdIndex = db.index().unwrap();
                let id = id_index.post_id_to_id(old.id).unwrap();
                db.update(id, &old, &new);
            }
            "public_posts_insert" => {
                let raw: RawBooruPost = serde_json::from_str(payload).unwrap();
                let post = raw.into();
                let mut db = db.write().await;
                let id = db.next_id();
                db.insert(id, &post);
            }
            "public_posts_delete" => {
                let raw: RawBooruPost = serde_json::from_str(payload).unwrap();
                let post: BooruPost = raw.into();
                let mut db = db.write().await;
                let id_index: &IdIndex = db.index().unwrap();
                let id = id_index.post_id_to_id(post.id).unwrap();
                db.remove(id, &post);
            }
            _ => {
                unreachable!()
            }
        };
        let elapsed = start_time.elapsed().as_nanos();
        println!("{channel}: {:.3}ms", elapsed as f64 / 1000.0 / 1000.0);
    }
}
