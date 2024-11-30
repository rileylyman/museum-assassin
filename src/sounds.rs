use std::{collections::HashMap, sync::Mutex};

use lazy_static::lazy_static;
use macroquad::{
    audio::{load_sound_from_bytes, play_sound, stop_sound, PlaySoundParams, Sound},
    prelude::*,
};

use crate::draw_progress;

lazy_static! {
    static ref SOUNDS: Mutex<HashMap<String, Sound>> = Mutex::new(HashMap::new());
    static ref PLAYING: Mutex<HashMap<String, Vec<Sound>>> = Mutex::new(HashMap::new());
}

async fn load_sound_with_feedback(name: &str, sound_bytes: &[u8]) {
    let sound = load_sound_from_bytes(sound_bytes).await.unwrap();
    SOUNDS.lock().unwrap().insert(name.to_string(), sound);
    draw_progress("Loading Sounds", SOUNDS.lock().unwrap().len() as f32 / 11.0).await;
}

pub async fn load_sounds() {
    draw_progress("Loading Sounds", 0.0).await;
    load_sound_with_feedback("alarm", include_bytes!("../assets/sounds/alarm.wav")).await;
    load_sound_with_feedback("arrow_shoot", include_bytes!("../assets/sounds/arrow.wav")).await;
    load_sound_with_feedback("bg_music", include_bytes!("../assets/sounds/bg_music.wav")).await;
    load_sound_with_feedback("win", include_bytes!("../assets/sounds/Retro Event 49.wav")).await;
    load_sound_with_feedback(
        "menu_tick",
        include_bytes!("../assets/sounds/Retro Event Acute 11.wav"),
    )
    .await;
    load_sound_with_feedback(
        "menu_select",
        include_bytes!("../assets/sounds/Retro Event Acute 08.wav"),
    )
    .await;
    load_sound_with_feedback(
        "footstep",
        include_bytes!("../assets/sounds/Retro FootStep Grass 01.wav"),
    )
    .await;
    load_sound_with_feedback(
        "hit",
        include_bytes!("../assets/sounds/Retro Impact Punch 07.wav"),
    )
    .await;
    load_sound_with_feedback(
        "arrow_bounce",
        include_bytes!("../assets/sounds/Retro Water Drop 01.wav"),
    )
    .await;
    load_sound_with_feedback(
        "alert",
        include_bytes!("../assets/sounds/Retro Blop 07.wav"),
    )
    .await;
    load_sound_with_feedback(
        "wrong",
        include_bytes!("../assets/sounds/Retro Event Wrong Simple 03.wav"),
    )
    .await;
}

pub fn sound(name: &str) -> Sound {
    SOUNDS.lock().unwrap().get(name).unwrap().clone()
}

pub fn play(name: &str, volume: f32, looped: bool) {
    let volume = volume / 3.0;
    play_sound(&sound(name), PlaySoundParams { volume, looped });
}

pub fn stop(name: &str) {
    stop_sound(&sound(name));
}
