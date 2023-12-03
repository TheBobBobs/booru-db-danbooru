use std::{sync::Arc, time::Instant};

use axum::{
    extract::{Query as RQuery, State},
    Json,
};
use booru_db::Query;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::{
    index::{TagDbCountIndex, TagDbIdIndex, TagIndex},
    Db,
};

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagsSort {
    CountAsc,
    #[default]
    #[serde(alias = "count")]
    CountDesc,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GetTagsQuery {
    #[serde(default, alias = "q")]
    query: String,
    #[serde(default)]
    sort: TagsSort,

    #[serde(default)]
    page: usize,
    #[serde(default = "tags_default_limit")]
    limit: usize,
}

const fn tags_default_limit() -> usize {
    20
}

#[derive(Default, Serialize)]
pub struct TagsResponseTimings {
    query: u64,
    sort: u64,
}

#[derive(Serialize)]
pub struct TagsResponse {
    tags: Vec<(Arc<str>, u32)>,
    matched: usize,
    timings: TagsResponseTimings,
}

pub async fn get_tags(
    State(db): State<Arc<RwLock<Db>>>,
    RQuery(GetTagsQuery {
        query,
        sort,
        page,
        limit,
    }): RQuery<GetTagsQuery>,
) -> Json<TagsResponse> {
    let mut timings = TagsResponseTimings::default();

    let mut query = Query::parse(&query).unwrap(); // TODO
    query.simplify();

    let db = db.read().await;
    let tag_index: &TagIndex = db.index().unwrap();
    let tag_db = &tag_index.tag_db;

    let start_time = Instant::now();
    let result = tag_db.query(&query).unwrap(); // TODO
    let elapsed = start_time.elapsed().as_nanos();
    timings.query = elapsed as u64;

    let index = page * limit;
    let start_time = Instant::now();
    let ids = match sort {
        TagsSort::CountAsc | TagsSort::CountDesc => {
            let reverse = matches!(sort, TagsSort::CountDesc);
            let count_index: &TagDbCountIndex = tag_db.index().unwrap();
            let sort = count_index.range_index.ids().iter().copied();
            result.get_sorted(sort, index, limit, reverse)
        }
    };
    let elapsed = start_time.elapsed().as_nanos();
    timings.sort = elapsed as u64;

    let id_index: &TagDbIdIndex = tag_db.index().unwrap();
    let tags: Vec<_> = ids
        .into_iter()
        .map(|id| {
            let name = id_index.id_to_name.get(&id).unwrap();
            let count = tag_index.keys_index.items.get(name).unwrap().matched() as u32;
            (name.clone(), count)
        })
        .collect();
    drop(db);

    let matched = result.matched();
    let response = TagsResponse {
        tags,
        matched,
        timings,
    };
    response.into()
}
