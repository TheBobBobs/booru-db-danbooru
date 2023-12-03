use std::{sync::Arc, time::Instant};

use axum::{
    extract::{Query as RQuery, State},
    Json,
};
use booru_db::Query;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::{
    index::{IdIndex, ScoreIndex},
    Db,
};

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
    #[serde(default, alias = "q")]
    query: String,
    #[serde(default)]
    sort: Sort,

    #[serde(default)]
    page: usize,
    #[serde(default = "posts_default_limit")]
    limit: usize,
}

const fn posts_default_limit() -> usize {
    20
}

#[derive(Default, Serialize)]
pub struct PostsResponseTimings {
    query: u64,
    sort: u64,
}

#[derive(Serialize)]
pub struct PostsResponse {
    matched: usize,
    url: String,
    timings: PostsResponseTimings,
}

pub async fn get_posts(
    State(db): State<Arc<RwLock<Db>>>,
    RQuery(GetPostsQuery {
        query,
        sort,
        page,
        limit,
    }): RQuery<GetPostsQuery>,
) -> Json<PostsResponse> {
    let mut timings = PostsResponseTimings::default();

    let mut query = Query::parse(&query).unwrap(); // TODO
    query.simplify();

    let db = db.read().await;

    let start_time = Instant::now();
    let result = db.query(&query).unwrap(); // TODO
    let elapsed = start_time.elapsed().as_nanos();
    timings.query = elapsed as u64;

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
    timings.sort = elapsed as u64;

    let id_index: &IdIndex = db.index().unwrap();
    let post_ids: Vec<_> = ids
        .into_iter()
        .map(|id| id_index.id_to_post_id(id).unwrap().to_string())
        .collect();
    drop(db);

    let id_search = post_ids.join(",");
    let url = format!("https://danbooru.donmai.us/posts?tags=id:{id_search}+order:custom");

    let matched = result.matched();
    let response = PostsResponse {
        matched,
        url,
        timings,
    };
    response.into()
}
