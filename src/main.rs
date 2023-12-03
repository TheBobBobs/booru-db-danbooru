use std::{
    net::SocketAddr,
    sync::{mpsc::sync_channel, Arc},
    time::Instant,
};

use axum::{routing::get, Router};
use booru_db::db;
use futures::StreamExt;
use tokio::sync::RwLock;

mod index;
use index::*;
mod post;
use post::{BooruPost, RawBooruPost};
mod routes;
use routes::{posts::get_posts, tags::get_tags};
mod sync;
use sync::{create_listener, handle_listener};

db!(BooruPost);

// Create a trigger on postgres to notify us of changes.
const SYNC: bool = true;

#[tokio::main]
async fn main() {
    let (tx, rx) = sync_channel::<BooruPost>(1024);
    let pg_listener = tokio::spawn(async move {
        let uri = std::env::args().nth(1).unwrap();
        let pool = sqlx::PgPool::connect(&uri).await.unwrap();

        let listener = if SYNC {
            Some(create_listener(&uri, &pool).await)
        } else {
            None
        };

        let mut posts = sqlx::query_as::<_, RawBooruPost>("SELECT * FROM posts").fetch(&pool);
        let mut count = 0;
        while let Some(Ok(post)) = posts.next().await {
            tx.send(post.into()).unwrap();
            count += 1;
            if count % 50_000 == 0 {
                println!("{count}");
            }
        }

        listener
    });

    let posts = rx.iter();
    let start_time = Instant::now();
    let db = DbLoader::new()
        .with_loader("id", IdIndexLoader::default())
        .with_loader("parent_id", ParentIdIndexLoader::default())
        .with_loader("pixiv_id", PixivIdIndexLoader::default())
        .with_loader("approver", ApproverIdIndexLoader::default())
        .with_loader("status", StatusIndexLoader::default())
        .with_loader("created_at", CreatedAtIndexLoader::default())
        .with_loader("updated_at", UpdatedAtIndexLoader::default())
        .with_loader("favcount", FavCountIndexLoader::default())
        .with_loader("score", ScoreIndexLoader::default())
        .with_loader("upvotes", UpScoreIndexLoader::default())
        .with_loader("downvotes", DownScoreIndexLoader::default())
        .with_loader("width", WidthIndexLoader::default())
        .with_loader("height", HeightIndexLoader::default())
        .with_loader("ratio", AspectRatioIndexLoader::default())
        .with_loader("mpixel", MPixelsIndexLoader::default())
        .with_loader("file_ext", FileExtIndexLoader::default())
        .with_loader("file_size", FileSizeIndexLoader::default())
        .with_loader("rating", RatingIndexLoader::default())
        .with_default(TagIndexLoader::default())
        .with_loader("tagcount", TagCountIndexLoader::default())
        .with_loader("gentags", TagCountGeneralIndexLoader::default())
        .with_loader("arttags", TagCountArtistIndexLoader::default())
        .with_loader("chartags", TagCountCharacterIndexLoader::default())
        .with_loader("copytags", TagCountCopyrightIndexLoader::default())
        .with_loader("metatags", TagCountMetaIndexLoader::default())
        .load(posts);
    let elapsed = start_time.elapsed().as_nanos();
    println!("Index: {:.3}s", elapsed as f64 / 1000.0 / 1000.0 / 1000.0);

    let db = Arc::new(RwLock::new(db));
    if let Some(pg_listener) = pg_listener.await.unwrap() {
        let db = db.clone();
        tokio::spawn(async move {
            handle_listener(db, pg_listener).await;
        });
    }

    let app = Router::new()
        .route("/posts", get(get_posts))
        .route("/tags", get(get_tags))
        .with_state(db.clone());
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let _ = axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await;
}
