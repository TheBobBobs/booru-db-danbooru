use booru_db::{
    index::{Index, IndexLoader, RangeIndex, RangeIndexLoader},
    query::Item,
    Query, Queryable, ID,
};

use crate::BooruPost;

#[derive(Default)]
pub struct IdIndexLoader {
    post_id_to_id: fxhash::FxHashMap<u32, ID>,
    range_index_loader: RangeIndexLoader<u32>,
}

impl IndexLoader<BooruPost> for IdIndexLoader {
    fn add(&mut self, id: ID, post: &BooruPost) {
        self.post_id_to_id.insert(post.id, id);
        self.range_index_loader.add(id, post.id);
    }

    fn load(self: Box<Self>) -> Box<(dyn Index<BooruPost>)> {
        let index = IdIndex {
            post_id_to_id: self.post_id_to_id,
            range_index: self.range_index_loader.load(),
        };
        Box::new(index)
    }
}

pub struct IdIndex {
    post_id_to_id: fxhash::FxHashMap<u32, ID>,
    pub range_index: RangeIndex<u32>,
}

impl IdIndex {
    pub fn id_to_post_id(&self, id: ID) -> Option<u32> {
        self.range_index.id_values().get(&id).copied()
    }

    pub fn post_id_to_id(&self, post_id: u32) -> Option<ID> {
        self.post_id_to_id.get(&post_id).copied()
    }
}

impl Index<BooruPost> for IdIndex {
    fn query<'s>(
        &'s self,
        _ident: Option<&str>,
        text: &str,
        inverse: bool,
    ) -> Option<Query<Queryable<'s>>> {
        if text.contains(',') {
            let ids: Vec<ID> = text
                .split(',')
                .filter_map(|v| {
                    v.parse::<u32>()
                        .ok()
                        .and_then(|post_id| self.post_id_to_id(post_id))
                })
                .collect();
            if ids.is_empty() {
                return None;
            }
            let queryable = Queryable::IDsOwned(ids);
            let item = Item::Single(queryable);
            return Some(Query::new(item, inverse));
        }
        if let Ok(range_query) = text.parse() {
            let mut query = self.range_index.get(range_query);
            query.inverse = inverse;
            return Some(query);
        }
        None
    }

    fn insert(&mut self, id: ID, post: &BooruPost) {
        self.post_id_to_id.insert(post.id, id);
        self.range_index.insert(id, post.id);
    }

    fn remove(&mut self, id: ID, post: &BooruPost) {
        self.post_id_to_id.remove(&post.id);
        self.range_index.remove(id, post.id);
    }

    fn update(&mut self, id: ID, old: &BooruPost, new: &BooruPost) {
        if old.id == new.id {
            return;
        }
        self.remove(id, old);
        self.insert(id, new);
    }
}
