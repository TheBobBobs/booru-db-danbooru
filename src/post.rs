use std::{str::FromStr, sync::Arc};

use chrono::NaiveDateTime;
use serde::Deserialize;
use sqlx::FromRow;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Rating {
    G,
    S,
    Q,
    E,
}

impl FromStr for Rating {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "g" => Ok(Self::G),
            "s" => Ok(Self::S),
            "q" => Ok(Self::Q),
            "e" => Ok(Self::E),
            _ => Err(()),
        }
    }
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum FileExt {
    AVIF,
    BMP,
    GIF,
    JPG,
    MP4,
    PNG,
    SWF,
    WEBM,
    WEBP,
    ZIP,
}

impl FromStr for FileExt {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "avif" => Ok(Self::AVIF),
            "bmp" => Ok(Self::BMP),
            "gif" => Ok(Self::GIF),
            "jpg" => Ok(Self::JPG),
            "png" => Ok(Self::PNG),
            "mp4" => Ok(Self::MP4),
            "swf" => Ok(Self::SWF),
            "webm" => Ok(Self::WEBM),
            "webp" => Ok(Self::WEBP),
            "zip" => Ok(Self::ZIP),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    Active,
    Banned,
    Deleted,
    Flagged,
    Pending,
}

impl FromStr for Status {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "banned" => Ok(Self::Banned),
            "deleted" => Ok(Self::Deleted),
            "flagged" => Ok(Self::Flagged),
            "pending" => Ok(Self::Pending),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BooruPost {
    pub id: u32,
    pub parent_id: Option<u32>,
    pub pixiv_id: Option<u32>,

    pub uploader_id: u32,
    pub approver_id: Option<u32>,
    pub status: Status,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,

    pub fav_count: u32,
    pub up_score: i32,
    pub down_score: i32,

    pub source: String,
    pub width: u16,
    pub height: u16,
    pub file_ext: FileExt,
    pub file_size: u32,

    pub rating: Rating,

    pub tags: Vec<Arc<str>>,
    pub tag_count_general: u16,
    pub tag_count_artist: u16,
    pub tag_count_character: u16,
    pub tag_count_copyright: u16,
    pub tag_count_meta: u16,
}

#[derive(Clone, Debug, Deserialize, FromRow)]
pub struct RawBooruPost {
    pub id: i32,
    pub parent_id: Option<i32>,
    pub pixiv_id: Option<i32>,

    pub uploader_id: i32,
    pub approver_id: Option<i32>,
    pub is_banned: bool,
    pub is_deleted: bool,
    pub is_flagged: bool,
    pub is_pending: bool,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,

    pub fav_count: i32,
    pub up_score: i32,
    pub down_score: i32,

    pub source: String,
    pub image_width: i32,
    pub image_height: i32,
    pub file_ext: String,
    pub file_size: i32,

    pub rating: String,

    pub tag_string: String,
    pub tag_count_general: i32,
    pub tag_count_artist: i32,
    pub tag_count_character: i32,
    pub tag_count_copyright: i32,
    pub tag_count_meta: i32,
}

impl From<RawBooruPost> for BooruPost {
    fn from(raw: RawBooruPost) -> Self {
        Self {
            id: raw.id as u32,
            parent_id: raw.parent_id.map(|i| i as u32),
            pixiv_id: raw.pixiv_id.map(|i| i as u32),
            uploader_id: raw.uploader_id as u32,
            approver_id: raw.approver_id.map(|i| i as u32),
            status: if raw.is_banned {
                Status::Banned
            } else if raw.is_deleted {
                Status::Deleted
            } else if raw.is_flagged {
                Status::Flagged
            } else if raw.is_pending {
                Status::Pending
            } else {
                Status::Active
            },
            created_at: raw.created_at,
            updated_at: raw.updated_at,
            fav_count: raw.fav_count as u32,
            up_score: raw.up_score,
            down_score: raw.down_score,
            source: raw.source,
            width: raw.image_width as u16,
            height: raw.image_height as u16,
            file_ext: raw.file_ext.parse().unwrap(),
            file_size: raw.file_size as u32,
            rating: raw.rating.parse().unwrap(),
            tags: raw
                .tag_string
                .split_whitespace()
                .map(|t| t.to_string().into())
                .collect(),
            tag_count_general: raw.tag_count_general as u16,
            tag_count_artist: raw.tag_count_artist as u16,
            tag_count_character: raw.tag_count_character as u16,
            tag_count_copyright: raw.tag_count_copyright as u16,
            tag_count_meta: raw.tag_count_meta as u16,
        }
    }
}
