use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    path::Path,
};

use macroquad::prelude::*;

use crate::{bindata::load_fast_texture, texturepacker::TexturePackerData, Time};

pub struct SpriteSheet {
    pub texture: Texture2D,
    pub data: Option<TexturePackerData>,
}

impl SpriteSheet {
    pub async fn from_json(path: impl AsRef<Path>) -> Self {
        let data = TexturePackerData::new(path.as_ref());
        let texture_path = path.as_ref().with_file_name(&data.meta.image);
        Self {
            data: Some(data),
            ..Self::from_texture_path(texture_path).await
        }
    }

    pub async fn from_texture_path(path: impl AsRef<Path>) -> Self {
        let texture = load_fast_texture(path.as_ref()).await;
        texture.set_filter(FilterMode::Nearest);
        Self {
            texture,
            data: None,
        }
    }

    pub fn sprite_named(&self, name: &str, frame_time: Option<Time>) -> Sprite {
        if let Some(frame_time) = frame_time {
            let data = &self.data.as_ref().unwrap().frames;
            let mut keys = data.keys().filter(|k| k.contains(name)).collect::<Vec<_>>();
            assert!(!keys.is_empty());

            keys.sort();
            let frames = keys
                .into_iter()
                .map(|k| data.get(k).unwrap().frame.into())
                .collect::<Vec<_>>();

            Sprite {
                frames: Frames::Multiple {
                    frames,
                    curr: Cell::new(0),
                    since: Cell::new(get_time()),
                    frame_time: Cell::new(frame_time),
                },
                texture: self.texture.clone(),
                flip_x: false,
                flip_y: false,
            }
        } else {
            let rect = self
                .data
                .as_ref()
                .unwrap()
                .frames
                .get(name)
                .unwrap()
                .frame
                .into();
            Sprite {
                frames: Frames::Single(rect),
                texture: self.texture.clone(),
                flip_x: false,
                flip_y: false,
            }
        }
    }

    pub fn sprite_rect(&self, rect: Rect) -> Sprite {
        Sprite {
            frames: Frames::Single(rect),
            texture: self.texture.clone(),
            flip_x: false,
            flip_y: false,
        }
    }

    pub fn sprite_rects(&self, frames: &[Rect], frame_time: Time) -> Sprite {
        Sprite {
            texture: self.texture.clone(),
            frames: Frames::Multiple {
                frames: frames.to_vec(),
                curr: Cell::new(0),
                since: Cell::new(get_time()),
                frame_time: Cell::new(frame_time),
            },
            flip_x: false,
            flip_y: false,
        }
    }
}

#[derive(Debug)]
pub struct Sprite {
    texture: Texture2D,
    frames: Frames,
    flip_x: bool,
    flip_y: bool,
}

impl Sprite {
    pub fn expand_src(&mut self, other: &Sprite) {
        if let Frames::Single(src) = self.frames {
            self.frames = Frames::Single(src.combine_with(other.frames.get()));
        } else {
            unreachable!()
        }
    }

    pub fn src(&self) -> Rect {
        self.frames.get()
    }

    pub fn draw(&self, pos: Vec2) {
        draw_texture_ex(
            &self.texture,
            pos.x,
            pos.y,
            WHITE,
            DrawTextureParams {
                source: Some(self.frames.get()),
                flip_x: self.flip_x,
                flip_y: self.flip_y,
                ..Default::default()
            },
        );
    }

    pub fn flip_y(self, flip: bool) -> Self {
        Self {
            flip_y: flip,
            ..self
        }
    }

    pub fn flip_x(self, flip: bool) -> Self {
        Self {
            flip_x: flip,
            ..self
        }
    }

    pub fn reset(&self) {
        self.frames.reset();
    }

    pub fn size(&self) -> Vec2 {
        self.frames.get().size()
    }

    pub fn about_to_loop(&self) -> bool {
        self.frames.about_to_loop()
    }
}

impl Clone for Sprite {
    fn clone(&self) -> Self {
        let new = Self {
            texture: self.texture.clone(),
            frames: self.frames.clone(),
            flip_x: self.flip_x,
            flip_y: self.flip_y,
        };
        new.reset();
        new
    }
}

#[derive(Debug, Clone)]
pub enum Frames {
    Single(Rect),
    Multiple {
        frames: Vec<Rect>,
        curr: Cell<usize>,
        since: Cell<Time>,
        frame_time: Cell<Time>,
    },
}

impl Frames {
    pub fn get(&self) -> Rect {
        match self {
            Frames::Single(r) => *r,
            Frames::Multiple {
                frames,
                curr,
                since,
                frame_time,
            } => {
                if get_time() - since.get() > frame_time.get() {
                    curr.set((curr.get() + 1) % frames.len());
                    since.set(get_time());
                }
                frames[curr.get()]
            }
        }
    }

    pub fn about_to_loop(&self) -> bool {
        match self {
            Frames::Single(_) => false,
            Frames::Multiple {
                frames,
                curr,
                since,
                frame_time,
            } => {
                if curr.get() == frames.len() - 1 {
                    get_time() - since.get() + 2.0 * get_frame_time() as Time >= frame_time.get()
                } else {
                    false
                }
            }
        }
    }

    pub fn reset(&self) {
        match self {
            Frames::Single(_) => {}
            Frames::Multiple { curr, since, .. } => {
                curr.set(0);
                since.set(get_time());
            }
        }
    }
}

pub struct AnimationAtlas {
    col_base: f32,
    row_base: f32,
    col_width: f32,
    row_height: f32,
    sprite_width: f32,
    sprite_height: f32,
    anims: HashMap<String, (f32, f32, i32)>,
}

impl AnimationAtlas {
    pub fn new(
        col_base: f32,
        row_base: f32,
        col_width: f32,
        row_height: f32,
        sprite_width: f32,
        sprite_height: f32,
    ) -> Self {
        Self {
            col_base,
            row_base,
            col_width,
            row_height,
            sprite_width,
            sprite_height,
            anims: HashMap::new(),
        }
    }

    pub fn add(&mut self, name: String, row: f32, col: f32, count: i32) {
        self.anims.insert(name, (row, col, count));
    }

    pub fn get(&self, name: &str) -> Vec<Rect> {
        let (row, col, count) = self.anims[name];
        let y = row * self.row_height + self.row_base;
        let mut x = col * self.col_width + self.col_base;
        let mut rects = Vec::new();
        for _ in 0..count {
            rects.push(Rect {
                x,
                y,
                w: self.sprite_width,
                h: self.sprite_height,
            });
            x += self.col_width;
        }
        rects
    }
}

pub fn anim_rects(rect: Rect, stride_x: f32, count: i32) -> Vec<Rect> {
    let mut rects = Vec::new();
    let mut x = rect.x;
    for _ in 0..count {
        rects.push(Rect {
            x,
            y: rect.y,
            w: rect.w,
            h: rect.h,
        });
        x += stride_x;
    }
    rects
}

#[derive(Debug, Clone)]
pub struct SpriteMap {
    map: HashMap<String, Sprite>,
    last_accessed: RefCell<String>,
}

impl SpriteMap {
    pub fn new(sheet: &SpriteSheet, anims: &[(String, Vec<Rect>, f64)]) -> Self {
        let map = anims
            .into_iter()
            .map(|(n, v, t)| (n.to_owned(), sheet.sprite_rects(v, *t)))
            .collect::<HashMap<_, _>>();
        Self {
            map,
            last_accessed: RefCell::new(String::new()),
        }
    }

    pub fn get(&self, name: impl AsRef<str>) -> &Sprite {
        if name.as_ref() != self.last_accessed.borrow().as_str() {
            self.map.get(name.as_ref()).unwrap().reset();
            self.last_accessed.replace(name.as_ref().to_string());
        }
        &self.map[name.as_ref()]
    }
}
