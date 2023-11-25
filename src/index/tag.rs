use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use booru_db::{
    index::{
        Index, IndexLoader, KeyIndex, KeyIndexLoader, KeysIndex, KeysIndexLoader, NgramIndex,
        RangeIndex, RangeIndexLoader,
    },
    query::Item,
    Query, Queryable, RangeQuery, TextQuery, ID,
};

use crate::BooruPost;

pub struct Tag {
    name: Arc<str>,
    count: u32,
}
mod database {
    use super::Tag;
    use booru_db::db;
    db!(Tag);
}
use database::{Db as TagDb, DbLoader as TagDbLoader};

#[derive(Default)]
struct TagDbIdIndexLoader {
    name_to_id: HashMap<Arc<str>, ID>,
    id_to_name: HashMap<ID, Arc<str>>,
}

impl IndexLoader<Tag> for TagDbIdIndexLoader {
    fn add(&mut self, id: ID, tag: &Tag) {
        self.name_to_id.insert(tag.name.clone(), id);
        self.id_to_name.insert(id, tag.name.clone());
    }

    fn load(self: Box<Self>) -> Box<dyn Index<Tag>> {
        Box::new(TagDbIdIndex {
            name_to_id: self.name_to_id,
            id_to_name: self.id_to_name,
        })
    }
}

pub struct TagDbIdIndex {
    pub name_to_id: HashMap<Arc<str>, ID>,
    pub id_to_name: HashMap<ID, Arc<str>>,
}

impl Index<Tag> for TagDbIdIndex {
    fn query<'s>(
        &'s self,
        _ident: Option<&str>,
        _text: &str,
        _inverse: bool,
    ) -> Option<Query<Queryable<'s>>> {
        None
    }

    fn insert(&mut self, id: ID, tag: &Tag) {
        self.name_to_id.insert(tag.name.clone(), id);
        self.id_to_name.insert(id, tag.name.clone());
    }

    fn remove(&mut self, id: ID, tag: &Tag) {
        self.id_to_name.remove(&id);
        self.name_to_id.remove(&tag.name);
    }

    fn update(&mut self, id: ID, old: &Tag, new: &Tag) {
        if old.name == new.name {
            return;
        }
        self.remove(id, old);
        self.insert(id, new);
    }
}

#[derive(Default)]
struct TagDbCountIndexLoader {
    range_loader: RangeIndexLoader<u32>,
}

impl IndexLoader<Tag> for TagDbCountIndexLoader {
    fn add(&mut self, id: ID, tag: &Tag) {
        self.range_loader.add(id, tag.count);
    }

    fn load(self: Box<Self>) -> Box<dyn Index<Tag>> {
        Box::new(TagDbCountIndex {
            range_index: self.range_loader.load(),
        })
    }
}

pub struct TagDbCountIndex {
    pub range_index: RangeIndex<u32>,
}

impl Index<Tag> for TagDbCountIndex {
    fn query<'s>(
        &'s self,
        _ident: Option<&str>,
        text: &str,
        inverse: bool,
    ) -> Option<Query<Queryable<'s>>> {
        let query: RangeQuery<u32> = text.parse().ok()?;
        let mut query = self.range_index.get(query);
        query.inverse = inverse;
        Some(query)
    }

    fn insert(&mut self, id: ID, tag: &Tag) {
        self.range_index.insert(id, tag.count);
    }

    fn remove(&mut self, id: ID, tag: &Tag) {
        self.range_index.remove(id, tag.count);
    }

    fn update(&mut self, id: ID, old: &Tag, new: &Tag) {
        if old.count == new.count {
            return;
        }
        self.remove(id, old);
        self.insert(id, new);
    }
}

fn abbreviate(text: &str) -> String {
    text.replace(|c| ['(', ')'].contains(&c), "")
        .split('_')
        .filter_map(|w| w.chars().next())
        .collect()
}

#[derive(Default)]
struct TagDbNameIndexLoader {
    abbreviations: KeyIndexLoader<String>,
    n1gram_index: NgramIndex<1>,
    n2gram_index: NgramIndex<2>,
}

impl IndexLoader<Tag> for TagDbNameIndexLoader {
    fn add(&mut self, id: ID, tag: &Tag) {
        let abv = abbreviate(&tag.name);
        self.abbreviations.add(id, &abv);
        self.n1gram_index.insert(id, tag.name.clone());
        self.n2gram_index.insert(id, tag.name.clone());
    }

    fn load(self: Box<Self>) -> Box<dyn Index<Tag>> {
        Box::new(TagDbNameIndex {
            abbreviations: self.abbreviations.load(),
            n1gram_index: self.n1gram_index,
            n2gram_index: self.n2gram_index,
        })
    }
}

#[derive(Default)]
struct TagDbNameIndex {
    abbreviations: KeyIndex<String>,
    n1gram_index: NgramIndex<1>,
    n2gram_index: NgramIndex<2>,
}

impl Index<Tag> for TagDbNameIndex {
    fn query<'s>(
        &'s self,
        _ident: Option<&str>,
        text: &str,
        inverse: bool,
    ) -> Option<Query<Queryable<'s>>> {
        if let Some(abv) = text.strip_prefix('/') {
            return self
                .abbreviations
                .get(abv)
                .map(|q| Query::new(Item::Single(q), inverse));
        }
        let query: TextQuery = text.parse().ok()?;
        let text = query.text();
        let Some(smallest) = (match text.len() {
            0 => None,
            1 => self.n1gram_index.query(text),
            _ => self.n2gram_index.query(text),
        }) else {
            return Some(Query::new(
                Item::Single(Queryable::IDsOwned(vec![])),
                inverse,
            ));
        };
        let mut ids = Vec::new();
        match query {
            TextQuery::StartsWith(text) => {
                for (t, id) in smallest {
                    if t.starts_with(&text) {
                        ids.push(*id);
                    }
                }
            }
            TextQuery::Contains(text) => {
                for (t, id) in smallest {
                    if t.contains(&text) {
                        ids.push(*id);
                    }
                }
            }
            TextQuery::EndsWith(text) => {
                for (t, id) in smallest {
                    if t.ends_with(&text) {
                        ids.push(*id);
                    }
                }
            }
        }
        let queryable = Queryable::IDsOwned(ids);
        let item = Item::Single(queryable);
        Some(Query::new(item, inverse))
    }

    fn insert(&mut self, id: ID, tag: &Tag) {
        let abv = abbreviate(&tag.name);
        self.abbreviations.insert(id, &abv);
        self.n1gram_index.insert(id, tag.name.clone());
        self.n2gram_index.insert(id, tag.name.clone());
    }

    fn remove(&mut self, id: ID, tag: &Tag) {
        let abv = abbreviate(&tag.name);
        self.abbreviations.remove(id, &abv);
        self.n1gram_index.remove(id, tag.name.clone());
        self.n2gram_index.remove(id, tag.name.clone());
    }

    fn update(&mut self, id: ID, old: &Tag, new: &Tag) {
        if old.name == new.name {
            return;
        }
        self.remove(id, old);
        self.insert(id, new);
    }
}

pub struct TagIndexLoader {
    keys_loader: KeysIndexLoader<Arc<str>>,
}

impl Default for TagIndexLoader {
    fn default() -> Self {
        Self {
            keys_loader: KeysIndexLoader::new(),
        }
    }
}

impl IndexLoader<BooruPost> for TagIndexLoader {
    fn add(&mut self, id: ID, post: &BooruPost) {
        self.keys_loader.add(id, post.tags.iter());
    }

    fn load(self: Box<Self>) -> Box<dyn Index<BooruPost>> {
        let keys_index = self.keys_loader.load();

        let tag_db = {
            let tags = keys_index.items.iter().map(|(name, queryable)| Tag {
                // Create new Arc<str> instead of cloning. Makes initial tags close in memory.
                name: name.to_string().into(),
                count: queryable.matched() as u32,
            });
            TagDbLoader::new()
                .with_default(TagDbNameIndexLoader::default())
                .with_loader("count", TagDbCountIndexLoader::default())
                .with_loader("id", TagDbIdIndexLoader::default())
                .load(tags)
        };
        let index = TagIndex { keys_index, tag_db };
        Box::new(index)
    }
}

pub struct TagIndex {
    pub keys_index: KeysIndex<Arc<str>>,
    pub tag_db: TagDb,
}

impl TagIndex {
    fn add_tag(&mut self, name: Arc<str>) {
        let count = self.keys_index.items.get(&name).unwrap().matched() as u32;
        let tag = Tag { name, count };
        let id_index: &TagDbIdIndex = self.tag_db.index().unwrap();
        if let Some(&id) = id_index.name_to_id.get(&tag.name) {
            let old = Tag {
                name: tag.name.clone(),
                count: tag.count - 1,
            };
            self.tag_db.update(id, &old, &tag);
        } else {
            let id = self.tag_db.next_id();
            self.tag_db.insert(id, &tag);
        }
    }

    fn remove_tag(&mut self, name: Arc<str>) {
        let count = self
            .keys_index
            .items
            .get(&name)
            .map(|q| q.matched() as u32)
            .unwrap_or(0);
        let tag = Tag { name, count };
        let id_index: &TagDbIdIndex = self.tag_db.index().unwrap();
        if let Some(&id) = id_index.name_to_id.get(&tag.name) {
            if tag.count == 0 {
                self.tag_db.remove(id, &tag);
            } else {
                let old = Tag {
                    name: tag.name.clone(),
                    count: tag.count + 1,
                };
                self.tag_db.update(id, &old, &tag);
            }
        }
    }
}

impl Index<BooruPost> for TagIndex {
    fn query<'s>(
        &'s self,
        _ident: Option<&str>,
        text: &str,
        inverse: bool,
    ) -> Option<Query<Queryable<'s>>> {
        if text.starts_with('*') || text.ends_with('*') {
            let id_index: &TagDbIdIndex = self.tag_db.index().unwrap();
            let result = self
                .tag_db
                .query(&Query::new(Item::Single(text.into()), false))
                .ok()?;
            let ids = result.get(0, result.matched(), false);
            let tags: Vec<_> = ids
                .into_iter()
                .map(|id| {
                    let name = id_index.id_to_name.get(&id).unwrap();
                    let queryable = self.keys_index.get(name).unwrap();
                    Query::new(Item::Single(queryable), false)
                })
                .collect();
            let item = Item::OrChain(tags);
            return Some(Query::new(item, inverse));
        }
        let queryable = if text.starts_with('/') {
            let query = Query::new(Item::Single(text.to_string()), false);
            let result = self.tag_db.query(&query).ok()?;
            let count_index: &TagDbCountIndex = self.tag_db.index().unwrap();
            let sort = count_index.range_index.ids().iter().copied();
            let id = *result.get_sorted(sort, 0, 1, true).first()?;
            let id_index: &TagDbIdIndex = self.tag_db.index().unwrap();
            let name = id_index.id_to_name.get(&id)?;
            self.keys_index.get(name)
        } else {
            self.keys_index.get(text)
        }?;
        let item = Item::Single(queryable);
        Some(Query::new(item, inverse))
    }

    fn insert(&mut self, id: ID, post: &BooruPost) {
        self.keys_index.insert(id, post.tags.iter());
        for tag in &post.tags {
            let name = tag.clone();
            self.add_tag(name);
        }
    }

    fn remove(&mut self, id: ID, post: &BooruPost) {
        self.keys_index.remove(id, post.tags.iter());
        for tag in &post.tags {
            let name = tag.clone();
            self.remove_tag(name);
        }
    }

    fn update(&mut self, id: ID, old: &BooruPost, new: &BooruPost) {
        if old.tags == new.tags {
            return;
        }
        self.keys_index.update(id, &old.tags, &new.tags);
        let old_tags: HashSet<&Arc<str>> = old.tags.iter().collect();
        let new_tags: HashSet<&Arc<str>> = new.tags.iter().collect();
        let added = new_tags.difference(&old_tags);
        let removed = old_tags.difference(&new_tags);
        for &tag in added {
            let name = tag.clone();
            self.add_tag(name);
        }
        for &tag in removed {
            let name = tag.clone();
            self.remove_tag(name);
        }
    }
}
