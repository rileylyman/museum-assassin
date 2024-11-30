
use macroquad::prelude::*;
use std::{collections::HashMap, fs::read_to_string, path::Path};

use serde::{Deserialize, Serialize};
use serde_json as json;

#[derive(Serialize, Deserialize, Debug)]
pub struct TexturePackerData {
    pub frames: HashMap<String, TexturePackerFrame>,
    pub meta: TexturePackerMeta,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TexturePackerFrame {
    pub frame: TexturePackerRect,
    pub rotated: bool,
    pub trimmed: bool,
    #[serde(rename = "spriteSourceSize")]
    pub sprite_source_size: TexturePackerRect,
    #[serde(rename = "sourceSize")]
    pub source_size: TexturePackerRect,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TexturePackerMeta {
    pub app: String,
    pub version: String,
    pub image: String,
    pub format: String,
    pub size: TexturePackerRect,
    pub scale: String,
    pub smartupdate: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct TexturePackerRect {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub w: i32,
    pub h: i32,
}

impl Into<Rect> for TexturePackerRect {
    fn into(self) -> Rect {
        Rect {
            x: self.x.unwrap_or_default() as f32,
            y: self.y.unwrap_or_default() as f32,
            w: self.w as f32,
            h: self.h as f32,
        }
    }
}

impl TexturePackerData {
    pub fn new(path: impl AsRef<Path>) -> Self {
        json::from_str(&read_to_string(path).unwrap()).unwrap()
    }
}