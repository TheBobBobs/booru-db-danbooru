use std::str::FromStr;

use chrono::NaiveDateTime;
use sqlx::{postgres::PgRow, FromRow, Row};

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

    pub tags: Vec<String>,
    pub tag_count_general: u16,
    pub tag_count_artist: u16,
    pub tag_count_character: u16,
    pub tag_count_copyright: u16,
    pub tag_count_meta: u16,
}

impl FromRow<'_, PgRow> for BooruPost {
    fn from_row(row: &PgRow) -> sqlx::Result<Self> {
        let id: i32 = row.try_get("id")?;
        let parent_id: Option<i32> = row.try_get("parent_id")?;
        let pixiv_id: Option<i32> = row.try_get("pixiv_id")?;

        let uploader_id: i32 = row.try_get("uploader_id")?;
        let approver_id: Option<i32> = row.try_get("approver_id")?;
        let is_banned: bool = row.try_get("is_banned")?;
        let is_deleted: bool = row.try_get("is_deleted")?;
        let is_flagged: bool = row.try_get("is_flagged")?;
        let is_pending: bool = row.try_get("is_pending")?;

        let created_at: NaiveDateTime = row.try_get("created_at")?;
        let updated_at: NaiveDateTime = row.try_get("updated_at")?;

        let fav_count: i32 = row.try_get("fav_count")?;
        let up_score: i32 = row.try_get("up_score")?;
        let down_score: i32 = row.try_get("down_score")?;

        let source: String = row.try_get("source")?;
        let width: i32 = row.try_get("image_width")?;
        let height: i32 = row.try_get("image_height")?;
        let file_ext: String = row.try_get("file_ext")?;
        let file_size: i32 = row.try_get("file_size")?;

        let rating: String = row.try_get("rating")?;

        let tag_string: String = row.try_get("tag_string")?;
        let tag_count_general: i32 = row.try_get("tag_count_general")?;
        let tag_count_artist: i32 = row.try_get("tag_count_artist")?;
        let tag_count_character: i32 = row.try_get("tag_count_character")?;
        let tag_count_copyright: i32 = row.try_get("tag_count_copyright")?;
        let tag_count_meta: i32 = row.try_get("tag_count_meta")?;

        let post = Self {
            id: id as u32,
            parent_id: parent_id.map(|i| i as u32),
            pixiv_id: pixiv_id.map(|i| i as u32),

            uploader_id: uploader_id as u32,
            approver_id: approver_id.map(|i| i as u32),
            status: if is_banned {
                Status::Banned
            } else if is_deleted {
                Status::Deleted
            } else if is_flagged {
                Status::Flagged
            } else if is_pending {
                Status::Pending
            } else {
                Status::Active
            },

            created_at,
            updated_at,

            fav_count: fav_count as u32,
            up_score,
            down_score,

            source,
            width: width as u16,
            height: height as u16,
            file_ext: file_ext.parse().unwrap(),
            file_size: file_size as u32,

            rating: rating.parse().unwrap(),

            tags: tag_string
                .split_whitespace()
                .map(|t| t.to_string())
                .collect(),
            tag_count_general: tag_count_general as u16,
            tag_count_artist: tag_count_artist as u16,
            tag_count_character: tag_count_character as u16,
            tag_count_copyright: tag_count_copyright as u16,
            tag_count_meta: tag_count_meta as u16,
        };
        Ok(post)
    }
}
