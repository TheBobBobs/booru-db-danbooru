use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use booru_db::{
    index::{
        Index, IndexLoader, KeysIndex, KeysIndexLoader, NgramIndex, RangeIndex, RangeIndexLoader,
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

#[derive(Default)]
struct TagDbNameIndexLoader {
    n1gram_index: NgramIndex<1>,
    n2gram_index: NgramIndex<2>,
}

impl IndexLoader<Tag> for TagDbNameIndexLoader {
    fn add(&mut self, id: ID, tag: &Tag) {
        self.n1gram_index.insert(id, tag.name.clone());
        self.n2gram_index.insert(id, tag.name.clone());
    }

    fn load(self: Box<Self>) -> Box<dyn Index<Tag>> {
        Box::new(TagDbNameIndex {
            n1gram_index: self.n1gram_index,
            n2gram_index: self.n2gram_index,
        })
    }
}

#[derive(Default)]
struct TagDbNameIndex {
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
        self.n1gram_index.insert(id, tag.name.clone());
        self.n2gram_index.insert(id, tag.name.clone());
    }

    fn remove(&mut self, id: ID, tag: &Tag) {
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

#[derive(Default)]
struct TagAbbreviations {
    items: HashMap<String, Vec<(u32, String)>>,
}

impl TagAbbreviations {
    fn abbreviate(text: &str) -> String {
        text.replace(|c| ['(', ')'].contains(&c), "")
            .split('_')
            .filter_map(|w| w.chars().next())
            .collect()
    }

    fn get(&self, abbreviation: &str) -> Option<&str> {
        let items = self.items.get(abbreviation)?;
        items.first().map(|(_, tag)| tag.as_str())
    }

    fn insert(&mut self, text: &str) {
        let a = Self::abbreviate(text);
        let items = self.items.entry(a).or_default();
        if let Some(item) = items.iter_mut().find(|(_, t)| t == text) {
            item.0 += 1;
        } else {
            let item = (1, text.to_string());
            if let Err(index) = items.binary_search_by(|probe| probe.cmp(&item).reverse()) {
                items.insert(index, item);
            };
        }
    }

    fn insert_item(&mut self, text: &str, count: u32) {
        let a = Self::abbreviate(text);
        let items = self.items.entry(a).or_default();
        let item = (count, text.to_string());
        let index = items
            .binary_search_by(|probe| probe.cmp(&item).reverse())
            .unwrap_or_else(|e| e);
        items.insert(index, item);
    }

    fn remove(&mut self, text: &str) {
        let a = Self::abbreviate(text);
        let Some(items) = self.items.get_mut(&a) else {
            return;
        };
        if let Some((index, (count, _))) =
            items.iter_mut().enumerate().find(|(_, (_, t))| t == text)
        {
            *count -= 1;
            if *count == 0 {
                items.remove(index);
                if items.is_empty() {
                    self.items.remove(&a);
                }
            }
        }
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
        let mut abbreviations = TagAbbreviations::default();
        for (tag, q) in &keys_index.items {
            abbreviations.insert_item(tag.as_ref(), q.matched() as u32);
        }

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
        let index = TagIndex {
            abbreviations,
            keys_index,
            tag_db,
        };
        Box::new(index)
    }
}

pub struct TagIndex {
    abbreviations: TagAbbreviations,
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
        let queryable = if let Some(text) = text.strip_prefix('/') {
            self.abbreviations
                .get(text)
                .and_then(|tag| self.keys_index.get(tag))
        } else {
            self.keys_index.get(text)
        }?;
        let item = Item::Single(queryable);
        Some(Query::new(item, inverse))
    }

    fn insert(&mut self, id: ID, post: &BooruPost) {
        self.keys_index.insert(id, post.tags.iter());
        for tag in &post.tags {
            self.abbreviations.insert(tag);

            let name = tag.clone();
            self.add_tag(name);
        }
    }

    fn remove(&mut self, id: ID, post: &BooruPost) {
        self.keys_index.remove(id, post.tags.iter());
        for tag in &post.tags {
            self.abbreviations.remove(tag);

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
            self.abbreviations.insert(tag);

            let name = tag.clone();
            self.add_tag(name);
        }
        for &tag in removed {
            self.abbreviations.remove(tag);

            let name = tag.clone();
            self.remove_tag(name);
        }
    }
}
