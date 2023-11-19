use std::collections::{HashMap, HashSet};

use booru_db::{
    index::{Index, IndexLoader, KeysIndex, KeysIndexLoader, TextIndex, TextIndexLoader},
    query::Item,
    Query, Queryable, TextQuery, ID,
};

use crate::BooruPost;

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

#[derive(Default)]
pub struct TagIndexLoader {
    keys_loader: KeysIndexLoader<String>,
}

impl IndexLoader<BooruPost> for TagIndexLoader {
    fn add(&mut self, id: ID, post: &BooruPost) {
        self.keys_loader.add(id, post.tags.iter());
    }

    fn load(self: Box<Self>) -> Box<dyn Index<BooruPost>> {
        let keys_index = self.keys_loader.load();
        let mut abbreviations = TagAbbreviations::default();
        let mut text_loader = TextIndexLoader::default();
        for (tag, q) in &keys_index.items {
            abbreviations.insert_item(tag.as_str(), q.matched() as u32);
            text_loader.add(tag.clone());
        }

        let index = TagIndex {
            abbreviations,
            keys_index,
            text_index: text_loader.load(),
        };
        Box::new(index)
    }
}

pub struct TagIndex {
    abbreviations: TagAbbreviations,
    keys_index: KeysIndex<String>,
    text_index: TextIndex,
}

impl Index<BooruPost> for TagIndex {
    fn query<'s>(
        &'s self,
        _ident: Option<&str>,
        text: &str,
        inverse: bool,
    ) -> Option<Query<Queryable<'s>>> {
        if text.starts_with('*') || text.ends_with('*') {
            let text_query: TextQuery = text.parse().unwrap();
            let tags = self.text_index.get(&text_query);
            let tags: Vec<_> = tags
                .into_iter()
                .filter_map(|t| {
                    self.keys_index
                        .get(t.as_ref())
                        .map(|q| Query::new(Item::Single(q), false))
                })
                .collect();
            dbg!(tags.len());
            let item = Item::OrChain(tags);
            return Some(Query::new(item, inverse));
        }
        let queryable = if let Some(text) = text.strip_prefix('/') {
            self.abbreviations
                .get(text)
                .and_then(|tag| self.keys_index.get(&tag.to_string()))
        } else {
            self.keys_index.get(&text.to_string())
        }?;
        let item = Item::Single(queryable);
        Some(Query::new(item, inverse))
    }

    fn insert(&mut self, id: ID, post: &BooruPost) {
        self.keys_index.insert(id, post.tags.iter());
        for tag in &post.tags {
            self.abbreviations.insert(tag);
        }
    }

    fn remove(&mut self, id: ID, post: &BooruPost) {
        self.keys_index.remove(id, post.tags.iter());
        for tag in &post.tags {
            self.abbreviations.remove(tag);
        }
    }

    fn update(&mut self, id: ID, old: &BooruPost, new: &BooruPost) {
        if old.tags == new.tags {
            return;
        }
        self.keys_index.update(id, &old.tags, &new.tags);
        let old_tags: HashSet<&str> = old.tags.iter().map(|s| s.as_str()).collect();
        let new_tags: HashSet<&str> = new.tags.iter().map(|s| s.as_str()).collect();
        let added = new_tags.difference(&old_tags);
        let removed = old_tags.difference(&new_tags);
        for tag in added {
            self.abbreviations.insert(tag);
        }
        for tag in removed {
            self.abbreviations.remove(tag);
        }
    }
}
