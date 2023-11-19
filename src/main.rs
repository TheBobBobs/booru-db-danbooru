use std::{
    net::SocketAddr,
    sync::{mpsc::sync_channel, Arc},
    time::Instant,
};

use axum::{extract::Query as RQuery, routing::get, Extension, Json, Router};
use booru_db::{db, Packed, Query, PACKED_SIZE};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgListener, Executor};
use tokio::sync::RwLock;

mod index;
use index::*;
mod post;
use post::{BooruPost, RawBooruPost};

db!(BooruPost);

// Create a trigger on postgres to notify us of changes.
const SYNC: bool = true;

#[tokio::main]
async fn main() {
    let (tx, rx) = sync_channel::<BooruPost>(1024);
    let pg_listener = tokio::spawn(async move {
        let uri = std::env::args().nth(1).unwrap();
        let pool = sqlx::PgPool::connect(&uri).await.unwrap();

        let mut listener = None;
        if SYNC {
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
            let mut l = PgListener::connect(&uri).await.unwrap();
            l.listen_all(vec![
                "public_posts_insert",
                "public_posts_update",
                "public_posts_delete",
            ])
            .await
            .unwrap();
            listener = Some(l);
        }

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
    if SYNC {
        #[derive(Deserialize)]
        struct Update {
            old: RawBooruPost,
            new: RawBooruPost,
        }
        let db = db.clone();
        let mut pg_listener = pg_listener.await.unwrap().unwrap();
        tokio::spawn(async move {
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
                        let checks = db.checks();
                        let mut id = checks.len() as u32 * PACKED_SIZE;
                        'outer: for (index, &c) in checks.iter().enumerate() {
                            if c != Packed::MAX {
                                for i in 0..PACKED_SIZE {
                                    if (c & (1 << i)) == 0 {
                                        id = (index as u32 * PACKED_SIZE) + i;
                                        break 'outer;
                                    }
                                }
                            }
                        }
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
        });
    }

    let app = Router::new()
        .route("/posts", get(get_posts))
        .layer(Extension(db.clone()));
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let _ = axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await;
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    IdAsc,
    #[default]
    #[serde(alias = "id")]
    IdDesc,
    ScoreAsc,
    #[serde(alias = "score")]
    ScoreDesc,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GetPostsQuery {
    #[serde(default)]
    query: String,
    #[serde(default)]
    sort: Sort,

    #[serde(default)]
    page: usize,
    #[serde(default = "default_limit")]
    limit: usize,
}

const fn default_limit() -> usize {
    20
}

#[derive(Serialize)]
pub struct PostsResponse {
    matched: usize,
    url: String,
}

async fn get_posts(
    Extension(db): Extension<Arc<RwLock<Db>>>,
    RQuery(GetPostsQuery {
        query,
        sort,
        page,
        limit,
    }): RQuery<GetPostsQuery>,
) -> Json<PostsResponse> {
    println!("{}", &query);
    let mut query = Query::parse(&query).unwrap(); // TODO
    query.simplify();
    let db = db.read().await;
    let start_time = Instant::now();
    let result = db.query(&query).unwrap(); // TODO
    let elapsed = start_time.elapsed().as_nanos();
    println!("Query: {:.3}ms", elapsed as f64 / 1000.0 / 1000.0);

    let index = page * limit;
    let start_time = Instant::now();
    let ids = match sort {
        Sort::IdAsc | Sort::IdDesc => {
            let reverse = matches!(sort, Sort::IdDesc);
            let id_index: &IdIndex = db.index().unwrap();
            let sort = id_index.range_index.ids().iter().copied();
            result.get_sorted(sort, index, limit, reverse)
        }
        Sort::ScoreAsc | Sort::ScoreDesc => {
            let reverse = matches!(sort, Sort::ScoreDesc);
            let score_index: &ScoreIndex = db.index().unwrap();
            let sort = score_index.range_index.ids().iter().copied();
            result.get_sorted(sort, index, limit, reverse)
        }
    };
    let elapsed = start_time.elapsed().as_nanos();
    println!("Sort-{sort:?}: {:.3}ms", elapsed as f64 / 1000.0 / 1000.0);
    let id_index: &IdIndex = db.index().unwrap();
    let post_ids: Vec<_> = ids
        .into_iter()
        .map(|id| id_index.id_to_post_id(id).unwrap().to_string())
        .collect();
    drop(db);

    let id_search = post_ids.join(",");
    let url = format!("https://danbooru.donmai.us/posts?tags=id:{id_search}+order:custom");

    let matched = result.matched();
    let response = PostsResponse { matched, url };
    response.into()
}
