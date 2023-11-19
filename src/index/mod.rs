use std::str::FromStr;

use crate::post::{BooruPost, FileExt, Rating, Status};

// mod comment;
// pub use comment::{Comment, CommentIndex};
mod id;
pub use id::{IdIndex, IdIndexLoader};
// mod pool;
// pub use pool::{Pool, PoolCategory, PoolIndex};
mod tag;
pub use tag::TagIndexLoader;
// mod user;
// pub use user::{UserIndex, UserIndexLoader};

macro_rules! key_index {
    ($loader_name:ident, $index_name:ident, $key_type:ty, $get_key:expr) => {
        pub struct $loader_name {
            key_loader: ::booru_db::index::KeyIndexLoader<$key_type>,
        }

        impl ::std::default::Default for $loader_name {
            fn default() -> Self {
                Self {
                    key_loader: ::booru_db::index::KeyIndexLoader::new(),
                }
            }
        }

        #[allow(clippy::redundant_closure_call)]
        impl ::booru_db::index::IndexLoader<BooruPost> for $loader_name {
            fn add(&mut self, id: ::booru_db::ID, post: &BooruPost) {
                let key = $get_key(post);
                self.key_loader.add(id, &key);
            }

            fn load(
                self: ::std::boxed::Box<Self>,
            ) -> ::std::boxed::Box<dyn ::booru_db::index::Index<BooruPost>> {
                let key_index = self.key_loader.load();
                ::std::boxed::Box::new($index_name { key_index })
            }
        }

        pub struct $index_name {
            key_index: ::booru_db::index::KeyIndex<$key_type>,
        }

        #[allow(clippy::redundant_closure_call)]
        impl ::booru_db::index::Index<BooruPost> for $index_name {
            fn query<'s>(
                &'s self,
                _ident: std::option::Option<&::std::primitive::str>,
                text: &::std::primitive::str,
                inverse: ::std::primitive::bool,
            ) -> ::std::option::Option<::booru_db::Query<::booru_db::Queryable<'s>>> {
                if text.contains(',') {
                    let mut or_chain = ::std::vec::Vec::new();
                    for value in text.split(',') {
                        if let ::std::result::Result::Ok(key) = value.parse() {
                            if let ::std::option::Option::Some(queryable) = self.key_index.get(&key)
                            {
                                let item = ::booru_db::query::Item::Single(queryable);
                                or_chain.push(::booru_db::Query::new(item, false));
                            }
                        }
                    }
                    if or_chain.is_empty() {
                        return ::std::option::Option::None;
                    }
                    let item = ::booru_db::query::Item::OrChain(or_chain);
                    return ::std::option::Option::Some(::booru_db::Query::new(item, inverse));
                }
                if let ::std::result::Result::Ok(key) = text.parse() {
                    let queryable = self.key_index.get(&key)?;
                    let item = ::booru_db::query::Item::Single(queryable);
                    return ::std::option::Option::Some(::booru_db::query::Query::new(
                        item, inverse,
                    ));
                }
                ::std::option::Option::None
            }

            fn insert(&mut self, id: ::booru_db::ID, post: &BooruPost) {
                let key = $get_key(post);
                self.key_index.insert(id, &key);
            }

            fn remove(&mut self, id: ::booru_db::ID, post: &BooruPost) {
                let key = $get_key(post);
                self.key_index.remove(id, &key)
            }

            fn update(&mut self, id: ::booru_db::ID, old: &BooruPost, new: &BooruPost) {
                let old_key = $get_key(old);
                let new_key = $get_key(new);
                self.key_index.update(id, &old_key, &new_key);
            }
        }
    };
}

macro_rules! range_index {
    ($loader_name:ident, $index_name:ident, $value_type:ty, $get_value:expr) => {
        pub struct $loader_name {
            range_loader: ::booru_db::index::RangeIndexLoader<$value_type>,
        }

        impl Default for $loader_name {
            fn default() -> Self {
                Self {
                    range_loader: ::booru_db::index::RangeIndexLoader::new(),
                }
            }
        }

        #[allow(clippy::redundant_closure_call)]
        impl ::booru_db::index::IndexLoader<BooruPost> for $loader_name {
            fn add(&mut self, id: ::booru_db::ID, post: &BooruPost) {
                let value = $get_value(post);
                self.range_loader.add(id, value);
            }

            fn load(
                self: ::std::boxed::Box<Self>,
            ) -> ::std::boxed::Box<dyn ::booru_db::index::Index<BooruPost>> {
                let range_index = self.range_loader.load();
                ::std::boxed::Box::new($index_name { range_index })
            }
        }

        pub struct $index_name {
            pub range_index: ::booru_db::index::RangeIndex<$value_type>,
        }

        #[allow(clippy::redundant_closure_call)]
        impl ::booru_db::index::Index<BooruPost> for $index_name {
            fn query<'s>(
                &'s self,
                _ident: std::option::Option<&::std::primitive::str>,
                text: &::std::primitive::str,
                inverse: ::std::primitive::bool,
            ) -> ::std::option::Option<::booru_db::Query<::booru_db::Queryable<'s>>> {
                if let ::std::result::Result::Ok(range_query) = text.parse() {
                    let mut query = self.range_index.get(range_query);
                    query.inverse = inverse;
                    return ::std::option::Option::Some(query);
                }
                ::std::option::Option::None
            }

            fn insert(&mut self, id: ::booru_db::ID, post: &BooruPost) {
                let value = $get_value(post);
                self.range_index.insert(id, value);
            }

            fn remove(&mut self, id: ::booru_db::ID, post: &BooruPost) {
                let value = $get_value(post);
                self.range_index.remove(id, value)
            }

            fn update(&mut self, id: ::booru_db::ID, old: &BooruPost, new: &BooruPost) {
                let old_value = $get_value(old);
                let new_value = $get_value(new);
                self.range_index.update(id, old_value, new_value);
            }
        }
    };
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ParentId(Option<u32>);
impl FromStr for ParentId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "none" {
            return Ok(ParentId(None));
        }
        s.parse::<u32>().map(|i| Self(Some(i))).map_err(|_| ())
    }
}
#[rustfmt::skip]
key_index!(
    ParentIdIndexLoader,
    ParentIdIndex,
    ParentId,
    |p: &BooruPost| ParentId(p.parent_id)
);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PixivId(Option<u32>);
impl FromStr for PixivId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "none" {
            return Ok(PixivId(None));
        }
        s.parse::<u32>().map(|i| Self(Some(i))).map_err(|_| ())
    }
}
#[rustfmt::skip]
range_index!(
    PixivIdIndexLoader,
    PixivIdIndex,
    PixivId,
    |p: &BooruPost| PixivId(p.pixiv_id)
);

#[rustfmt::skip]
key_index!(
    UploaderIdIndexLoader,
    UploaderIdIndex,
    u32,
    |p: &BooruPost| p.uploader_id
);

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ApproverId(Option<u32>);
impl FromStr for ApproverId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "none" {
            return Ok(ApproverId(None));
        }
        s.parse::<u32>().map(|i| Self(Some(i))).map_err(|_| ())
    }
}
#[rustfmt::skip]
key_index!(
    ApproverIdIndexLoader,
    ApproverIdIndex,
    ApproverId,
    |p: &BooruPost| ApproverId(p.approver_id)
);

#[rustfmt::skip]
key_index!(
    StatusIndexLoader,
    StatusIndex,
    Status,
    |p: &BooruPost| p.status
);

#[rustfmt::skip]
range_index!(
    CreatedAtIndexLoader,
    CreatedAtIndex,
    i64,
    |p: &BooruPost| p.created_at.timestamp_millis()
);

#[rustfmt::skip]
range_index!(
    UpdatedAtIndexLoader,
    UpdatedAtIndex,
    i64,
    |p: &BooruPost| p.updated_at.timestamp_millis()
);

#[rustfmt::skip]
range_index!(
    FavCountIndexLoader,
    FavCountIndex,
    u32,
    |p: &BooruPost| p.fav_count
);

#[rustfmt::skip]
range_index!(
    ScoreIndexLoader,
    ScoreIndex,
    i32,
    |p: &BooruPost| p.up_score + p.down_score
);

#[rustfmt::skip]
range_index!(
    UpScoreIndexLoader,
    UpScoreIndex,
    i32,
    |p: &BooruPost| p.up_score
);

#[rustfmt::skip]
range_index!(
    DownScoreIndexLoader,
    DownScoreIndex,
    i32,
    |p: &BooruPost| p.down_score
);

#[rustfmt::skip]
range_index!(
    WidthIndexLoader,
    WidthIndex,
    u16,
    |p: &BooruPost| p.width
);

#[rustfmt::skip]
range_index!(
    HeightIndexLoader,
    HeightIndex,
    u16,
    |p: &BooruPost| p.height
);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AspectRatio(u32);

impl FromStr for AspectRatio {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ratio = if let Some((a, b)) = s.split_once('/') {
            let a: f32 = a.parse().map_err(|_| ())?;
            let b: f32 = b.parse().map_err(|_| ())?;
            AspectRatio((a / b * 1_000.0) as u32)
        } else {
            let ratio: f32 = s.parse().map_err(|_| ())?;
            AspectRatio((ratio * 1_000.0) as u32)
        };
        Ok(ratio)
    }
}

#[rustfmt::skip]
range_index!(
    AspectRatioIndexLoader,
    AspectRatioIndex,
    AspectRatio,
    |p: &BooruPost| AspectRatio((p.width as f32 / p.height as f32 * 1_000.0) as u32)
);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MPixel(u32);

impl FromStr for MPixel {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut float: f32 = s.parse().map_err(|_| ())?;
        float = float.clamp(0.0, 1000.0);
        Ok(MPixel((float * 1_000_000.0) as u32))
    }
}

#[rustfmt::skip]
range_index!(
    MPixelsIndexLoader,
    MPixelsIndex,
    MPixel,
    |p: &BooruPost| MPixel(p.width as u32 * p.height as u32)
);

#[rustfmt::skip]
key_index!(
    FileExtIndexLoader,
    FileExtIndex,
    FileExt,
    |p: &BooruPost| p.file_ext
);

#[rustfmt::skip]
range_index!(
    FileSizeIndexLoader,
    FileSizeIndex,
    u32,
    |p: &BooruPost| p.file_size
);

#[rustfmt::skip]
key_index!(
    RatingIndexLoader,
    RatingIndex,
    Rating,
    |p: &BooruPost| p.rating
);

#[rustfmt::skip]
range_index!(
    TagCountIndexLoader,
    TagCountIndex,
    u16,
    |p: &BooruPost| p.tags.len() as u16
);

#[rustfmt::skip]
range_index!(
    TagCountGeneralIndexLoader,
    TagCountGeneralIndex,
    u16,
    |p: &BooruPost| p.tag_count_general
);

#[rustfmt::skip]
range_index!(
    TagCountArtistIndexLoader,
    TagCountArtistIndex,
    u16,
    |p: &BooruPost| p.tag_count_artist
);

#[rustfmt::skip]
range_index!(
    TagCountCharacterIndexLoader,
    TagCountCharacterIndex,
    u16,
    |p: &BooruPost| p.tag_count_character
);

#[rustfmt::skip]
range_index!(
    TagCountCopyrightIndexLoader,
    TagCountCopyrightIndex,
    u16,
    |p: &BooruPost| p.tag_count_copyright
);

#[rustfmt::skip]
range_index!(
    TagCountMetaIndexLoader,
    TagCountMetaIndex,
    u16,
    |p: &BooruPost| p.tag_count_meta
);
