use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

use lazy_static::lazy_static;
use macroquad::prelude::*;

use crate::draw_progress;

lazy_static! {
    static ref TEXTURE_BYTES: Mutex<HashMap<PathBuf, &'static [u8]>> = Mutex::new(
        [
            (
                "assets/22_Museum_32x32.png".into(),
                include_bytes!("../assets/22_Museum_32x32.png").as_slice()
            ),
            (
                "assets/emotes.png".into(),
                include_bytes!("../assets/emotes.png").as_slice()
            ),
            (
                "assets/enemy1.png".into(),
                include_bytes!("../assets/enemy1.png").as_slice()
            ),
            (
                "assets/enemy2.png".into(),
                include_bytes!("../assets/enemy2.png").as_slice()
            ),
            (
                "assets/enemy3.png".into(),
                include_bytes!("../assets/enemy3.png").as_slice()
            ),
            (
                "assets/player.png".into(),
                include_bytes!("../assets/player.png").as_slice()
            ),
            (
                "assets/Room_Builder_32x32.png".into(),
                include_bytes!("../assets/Room_Builder_32x32.png").as_slice()
            ),
            (
                "assets/ui.png".into(),
                include_bytes!("../assets/ui.png").as_slice()
            ),
        ]
        .into()
    );
    static ref LOADED_CACHE: Mutex<HashMap<PathBuf, Texture2D>> = Mutex::new(HashMap::new());
}

pub async fn load_fast_texture(path: impl AsRef<Path>) -> Texture2D {
    if let Some(tex) = LOADED_CACHE.lock().unwrap().get(path.as_ref()) {
        return tex.clone();
    }

    let ret = Texture2D::from_file_with_format(
        TEXTURE_BYTES.lock().unwrap().get(path.as_ref()).unwrap(),
        None,
    );
    LOADED_CACHE
        .lock()
        .unwrap()
        .insert(path.as_ref().into(), ret.clone());
    ret
}

async fn load_texture_with_feedback(path: &str) {
    load_fast_texture(path).await;
    draw_progress(
        "Caching Textures",
        LOADED_CACHE.lock().unwrap().len() as f32 / 8.0,
    )
    .await;
}

pub async fn preload_textures() {
    load_texture_with_feedback("assets/22_Museum_32x32.png").await;
    load_texture_with_feedback("assets/emotes.png").await;
    load_texture_with_feedback("assets/enemy1.png").await;
    load_texture_with_feedback("assets/enemy2.png").await;
    load_texture_with_feedback("assets/enemy3.png").await;
    load_texture_with_feedback("assets/player.png").await;
    load_texture_with_feedback("assets/Room_Builder_32x32.png").await;
    load_texture_with_feedback("assets/ui.png").await;
}
