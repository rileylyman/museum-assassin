use itertools::izip;
use macroquad::prelude::*;
use schema::{EntityInstance, GridPoint, Ldtk};
use serde::de::DeserializeOwned;

use crate::{
    geometry_utils::GeoUtilsFloatExts,
    sprite::{Sprite, SpriteSheet},
    Draw, Light, PatrolNode, PatrolPath, Trigger, TriggerType,
};

mod schema;

pub struct TileSprite {
    pub sprite: Sprite,
    pub pos: Vec2,
}

impl TileSprite {
    fn rect(&self) -> Rect {
        Rect {
            x: self.pos.x,
            y: self.pos.y,
            ..self.sprite.src()
        }
    }
}

impl Draw for TileSprite {
    fn draw(&self) {
        self.sprite.draw(self.pos);
    }

    fn sort_order(&self) -> f32 {
        self.pos.y + self.sprite.size().y
    }
}

#[derive(Clone)]
pub struct PopUp {
    pub rect: Rect,
    pub text: String,
    pub triggered: bool,
}

impl PopUp {
    pub fn new(pos: Vec2, size: Vec2, text: String) -> Self {
        Self {
            rect: Rect {
                x: pos.x,
                y: pos.y,
                w: size.x,
                h: size.y,
            },
            text,
            triggered: false,
        }
    }
}

#[derive(Default)]
pub struct Level {
    pub player_spawn: Vec2,
    pub bg_color: Color,
    pub colliders: Vec<Rect>,
    pub lights: Vec<Light>,
    pub shadow_casters: Vec<Rect>,
    pub structure_sprites: Vec<TileSprite>,
    pub auto_sprites: Vec<TileSprite>,
    pub decoration_sprites: Vec<TileSprite>,
    pub triggers: Vec<Trigger>,
    pub patrol_paths: Vec<PatrolPath>,
    pub center: Vec2,
    pub transitions: Vec<(Vec2, String)>,
    pub bounds: Rect,
    pub level_name: String,
    pub camera_height: f32,
    pub camera_tracking: f32,
    pub popups: Vec<PopUp>,
}

pub async fn get_level_indices(ldtk_string: &str) -> Vec<usize> {
    let ldtk: Ldtk = serde_json::from_str(&ldtk_string).unwrap();
    ldtk.levels.iter().enumerate().map(|(idx, _)| idx).collect()
}

pub async fn load_ldtk(ldtk_str: &str, idx: usize) -> Level {
    let mut ret = Level::default();

    let ldtk: Ldtk = serde_json::from_str(ldtk_str).unwrap();

    let level = &ldtk.levels[idx];
    ret.bg_color = hex_str_to_color(&level.bg_color);
    ret.level_name = level.identifier.clone();

    ret.center = vec2(level.px_wid as f32, level.px_hei as f32) / 2.0;
    ret.camera_height = (level.px_hei as f32 * 0.9).max(512.0);
    ret.camera_tracking = 0.5;

    ret.bounds = Rect {
        x: 0.0,
        y: 0.0,
        w: level.px_wid as f32,
        h: level.px_hei as f32,
    };

    let layers = level.layer_instances.as_ref().unwrap();
    for i in (0..layers.len()).rev() {
        let layer = &layers[i];
        let grid_size = layer.grid_size as f32;
        let layer_offset = vec2(
            layer.px_total_offset_x as f32,
            layer.px_total_offset_y as f32,
        );

        let spritesheet = if let Some(rel_path) = layer.tileset_rel_path.as_ref() {
            Some(SpriteSheet::from_texture_path(format!("assets/{}", rel_path)).await)
        } else {
            None
        };

        if layer.int_grid_csv.len() > 0 {
            let width_gr = layer.c_wid;
            let height_gr = layer.c_hei;

            for y in 0..height_gr {
                for x in 0..width_gr {
                    let pos = vec2(grid_size * x as f32, grid_size * y as f32) + layer_offset;
                    let is_wall = layer.int_grid_csv[(x + y * width_gr) as usize] == 1
                        && layer.identifier == "Collisions";
                    let is_sc = layer.int_grid_csv[(x + y * width_gr) as usize] == 1
                        && (layer.identifier == "ShadowCasters"
                            || layer.identifier == "StructureGrid");
                    let is_level_transition_trigger = layer.int_grid_csv[(x + y * width_gr) as usize] == 1
                        && layer.identifier == "Triggers";
                    let is_won_game_trigger = layer.int_grid_csv[(x + y * width_gr) as usize] == 2
                        && layer.identifier == "Triggers";
                    if is_wall {
                        ret.colliders
                            .push(Rect::new(pos.x, pos.y, grid_size, grid_size));
                    }
                    if is_sc {
                        ret.shadow_casters
                            .push(Rect::new(pos.x, pos.y, grid_size, grid_size));
                    }
                    if is_level_transition_trigger {
                        ret.triggers.push(Trigger {
                            rect: Rect::new(pos.x, pos.y, grid_size, grid_size),
                            ty: TriggerType::LevelTransition,
                        });
                    }
                    if is_won_game_trigger {
                        ret.triggers.push(Trigger {
                            rect: Rect::new(pos.x, pos.y, grid_size, grid_size),
                            ty: TriggerType::WonGame,
                        });
                    }
                }
            }
        }

        for tile in layer.auto_layer_tiles.iter().chain(layer.grid_tiles.iter()) {
            let pos = vec2(tile.px[0] as f32, tile.px[1] as f32) + layer_offset;
            let src = Rect {
                x: tile.src[0] as f32,
                y: tile.src[1] as f32,
                w: grid_size,
                h: grid_size,
            };
            let vec = match layer.identifier.as_str() {
                "Structure" => &mut ret.structure_sprites,
                "AutoLayer" => &mut ret.auto_sprites,
                "Decoration" => &mut ret.decoration_sprites,
                _ => unreachable!(),
            };
            vec.push(TileSprite {
                sprite: spritesheet
                    .as_ref()
                    .unwrap()
                    .sprite_rect(src)
                    .flip_x(tile.f & 0x1 != 0)
                    .flip_y(tile.f & 0x2 != 0),
                pos,
            });
        }

        let grid_point_to_vec2 = |p: &GridPoint| {
            vec2(p.cx as f32, p.cy as f32) * grid_size
                + vec2(grid_size, grid_size) / 2.0
                + layer_offset
        };

        for entity in layer.entity_instances.iter() {
            let size = vec2(entity.width as f32, entity.height as f32);
            let pos = vec2(entity.px[0] as f32, entity.px[1] as f32) + size / 2.0 + layer_offset;
            match entity.identifier.as_str() {
                "PopUp" => {
                    let text = get_entity_field::<String>(entity, "Text");
                    ret.popups.push(PopUp::new(pos - size / 2.0, size, text));
                }
                "LevelTransition" => ret
                    .transitions
                    .push((pos, get_entity_field::<String>(entity, "Level"))),
                "Light" => {
                    let color = hex_str_to_color(&get_entity_field::<String>(entity, "Color"));
                    ret.lights.push(Light {
                        pos,
                        radius: size.x / 2.0,
                        color,
                    });
                }
                "CameraHeight" => {
                    let height = get_entity_field::<i32>(entity, "Height");
                    ret.camera_height = height as f32;

                    let camera_tracking = get_entity_field::<Option<f32>>(entity, "CameraTracking");
                    if let Some(t) = camera_tracking {
                        ret.camera_tracking = t;
                    }
                }
                "PatrolPath" => {
                    let mut locs = vec![pos];
                    locs.extend(
                        get_entity_field::<Vec<GridPoint>>(entity, "Path")
                            .iter()
                            .map(grid_point_to_vec2),
                    );
                    let full_circle = get_entity_field::<bool>(entity, "FullCircle");
                    let facings = get_entity_field::<Vec<GridPoint>>(entity, "Facing")
                        .iter()
                        .enumerate()
                        .map(|(i, p)| {
                            let p = grid_point_to_vec2(p);
                            (p - locs[i])
                                .normalize()
                                .angle_between(vec2(1.0, 0.0))
                                .normalized_rads()
                        })
                        .collect::<Vec<_>>();
                    let wait_timings = get_entity_field::<Vec<f32>>(entity, "WaitTiming")
                        .iter()
                        .map(|&t| t as f64)
                        .collect::<Vec<_>>();
                    let walk_timings = get_entity_field::<Vec<f32>>(entity, "WalkTiming")
                        .iter()
                        .map(|&t| t as f64)
                        .collect::<Vec<_>>();
                    let mut walk_timings = walk_timings
                        .into_iter()
                        .map(|t| Some(t))
                        .collect::<Vec<_>>();
                    if !full_circle {
                        walk_timings.push(None);
                    }
                    let start = get_entity_field::<i32>(entity, "Start");
                    let extra_spawns = get_entity_field::<Vec<i32>>(entity, "ExtraSpawnAt");
                    assert!(
                        !extra_spawns.contains(&start),
                        "{}: extra_spawns contains start",
                        level.identifier
                    );
                    assert!(
                        locs.len() == facings.len()
                            && locs.len() == wait_timings.len()
                            && locs.len() == walk_timings.len(),
                        "{}, locs={}, facings={}, wait={}, walk={}",
                        level.identifier,
                        locs.len(),
                        facings.len(),
                        wait_timings.len(),
                        walk_timings.len()
                    );
                    let patrol_nodes = izip!(locs, facings, wait_timings, walk_timings)
                        .map(|(pos, facing, wait, walk)| PatrolNode {
                            pos,
                            facing,
                            wait,
                            walk,
                        })
                        .collect::<Vec<_>>();
                    for &curr in [start].iter().chain(extra_spawns.iter()) {
                        ret.patrol_paths.push(PatrolPath {
                            nodes: patrol_nodes.clone(),
                            curr: curr as isize,
                            timer: None,
                            forwards: true,
                            full_circle,
                        });
                    }
                }
                "PlayerSpawn" => {
                    ret.player_spawn = pos;
                }
                _ => {}
            }
        }
    }

    ret.triggers = merge_triggers(ret.triggers);
    ret.decoration_sprites = merge_tile_layer(&mut ret.decoration_sprites);
    ret
}

fn merge_triggers(mut triggers: Vec<Trigger>) -> Vec<Trigger> {
    let mut ret = Vec::new();
    while let Some(mut curr) = triggers.pop() {
        while let Some((i, other)) = triggers
            .iter()
            .enumerate()
            .find(|(_, other)| curr.rect.overlaps(&other.rect) && curr.ty == other.ty)
        {
            curr.rect = curr.rect.combine_with(other.rect);
            triggers.remove(i);
        }
        ret.push(curr);
    }
    ret
}

fn merge_tile_layer(sprites: &mut Vec<TileSprite>) -> Vec<TileSprite> {
    let mut ret = Vec::new();
    while let Some(mut curr) = sprites.pop() {
        while let Some((i, other)) = sprites.iter().enumerate().find(|(_, other)| {
            curr.rect().overlaps(&other.rect()) && curr.sprite.src().overlaps(&other.sprite.src())
        }) {
            curr.sprite.expand_src(&other.sprite);
            curr.pos = curr.pos.min(other.pos);
            sprites.remove(i);
        }
        ret.push(curr);
    }
    ret
}

pub fn hex_str_to_color(s: &str) -> Color {
    let mut i = 0;
    if s.chars().nth(0).unwrap() == '#' {
        i += 1;
    }
    let r = u32::from_str_radix(&s[i..i + 2], 16).unwrap() as f32 / 255.0;
    let g = u32::from_str_radix(&s[i + 2..i + 4], 16).unwrap() as f32 / 255.0;
    let b = u32::from_str_radix(&s[i + 4..i + 6], 16).unwrap() as f32 / 255.0;
    let a = if s.chars().count() - i > 6 {
        u32::from_str_radix(&s[i + 6..i + 8], 16).unwrap() as f32 / 255.0
    } else {
        1.0
    };
    Color { r, g, b, a }
}

fn get_entity_field<T>(entity: &EntityInstance, id: &str) -> T
where
    T: DeserializeOwned,
{
    let field = entity
        .field_instances
        .iter()
        .find(|f| f.identifier.as_str() == id)
        .unwrap();
    serde_json::from_value(field.value.clone().unwrap()).unwrap()
}
