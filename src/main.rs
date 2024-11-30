use bindata::preload_textures;
use lazy_static::lazy_static;
use macroquad::rand::ChooseRandom;
use pathfinder::Pathfinder;
use sounds::{load_sounds, play, stop};
use sprite::{anim_rects, Sprite, SpriteMap, SpriteSheet};
use std::{
    cell::RefCell,
    f32::consts::TAU,
    ptr::null_mut,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    vec,
};
use text_helpers::{draw_text_aligned, TextAlign};
use ui::{Ui, UiAction};

use draw_utils::{draw_dotted_line, DebugDrawer};
use geometry_utils::{
    intersect_lines, line_rect_intersect, rotate_vec2, shape_preserving_rect_merge,
    triangle_contains, GeoUtilsFloatExts,
};
use ldtk::{get_level_indices, load_ldtk, PopUp, TileSprite};
use macroquad::prelude::*;
use materials::shadow_postprocess;

mod bindata;
mod draw_utils;
mod geometry_utils;
mod ldtk;
mod materials;
mod pathfinder;
mod quadtree;
mod sounds;
mod sprite;
mod text_helpers;
mod texturepacker;
mod ui;

#[cfg(target_arch = "wasm32")]
mod getrandom_on_web;

// #E04D49
const MY_RED: Color = Color::new(0.878, 0.302, 0.286, 1.0);
// #CDCBE4
const MY_WHITE: Color = Color::new(0.804, 0.796, 0.894, 1.0);

type Time = f64;

trait Draw {
    fn draw(&self);
    fn sort_order(&self) -> f32;
}

fn tolerant_rect(rect: Rect) -> Rect {
    let x1_tolerance = 8.0;
    let x2_tolerance = 8.0;
    let y1_tolerance = 10.0;
    let y2_tolerance = 10.0;
    Rect {
        x: rect.x + x1_tolerance,
        y: rect.y + y1_tolerance,
        w: rect.w - x1_tolerance - x2_tolerance,
        h: rect.h - y1_tolerance - y2_tolerance,
    }
}

#[derive(Debug, Clone)]
struct Emotes {
    emote_smap: SpriteMap,
    base_intro: Sprite,
    base_final: Sprite,

    intial_phase: RefCell<bool>,
    last_drawn: RefCell<String>,
}

impl Emotes {
    fn new(emote_smap: SpriteMap) -> Self {
        let base_intro = emote_smap.get("base_intro").clone();
        let base_final = emote_smap.get("base_final").clone();
        Self {
            emote_smap,
            base_intro,
            base_final,
            last_drawn: RefCell::new("".to_owned()),
            intial_phase: RefCell::new(true),
        }
    }

    fn draw(&self, pos: Vec2, name: &str) {
        if *self.last_drawn.borrow() != name {
            *self.intial_phase.borrow_mut() = true;
            *self.last_drawn.borrow_mut() = name.to_string();
            self.base_intro.reset();
            self.base_final.reset();
        }
        if *self.intial_phase.borrow() {
            if self.base_intro.about_to_loop() {
                *self.intial_phase.borrow_mut() = false;
            }
            self.base_intro.draw(pos);
        } else {
            self.base_final.draw(pos);
            self.emote_smap.get(name).draw(pos);
        }
    }

    fn reset(&self) {
        *self.intial_phase.borrow_mut() = true;
        *self.last_drawn.borrow_mut() = "".to_owned();
        self.base_intro.reset();
        self.base_final.reset();
    }
}

#[derive(Debug, Eq, PartialEq)]
enum PlayerState {
    Idle,
    Moving,
    Caught,
}

#[derive(Debug)]
struct Player {
    pos: Vec2,
    strike_cone: Option<(Vec2, f32, Vec<Intersection>)>,
    smap: SpriteMap,
    emotes: Emotes,
    last_v: Vec2,
    is_moving: bool,
    // the iter the strike animation is currently on, so we know when to stop
    state: PlayerState,
    carrying: (*mut Enemy, Vec2),
    detected: bool,
    seen_mouse_pressed: bool,
    bow_charge_duration: Time,
}

impl Player {
    fn new(pos: Vec2, smap: SpriteMap, emotes: Emotes) -> Self {
        Self {
            pos,
            strike_cone: None,
            smap,
            emotes,
            last_v: vec2(0.0, 0.0),
            is_moving: false,
            state: PlayerState::Idle,
            carrying: (null_mut(), vec2(0.0, 0.0)),
            detected: false,
            seen_mouse_pressed: false,
            bow_charge_duration: 0.5,
        }
    }

    fn caught(&mut self, found_dead_body: bool) {
        if !debug() {
            if !found_dead_body {
                self.state = PlayerState::Caught;
            }
            self.detected = true;
        }
    }

    fn curr_sprite(&self) -> &Sprite {
        match self.state {
            PlayerState::Caught => self.smap.get(vel_to_name("sit", self.last_v)),
            PlayerState::Moving => self.smap.get(vel_to_name("run", self.last_v)),
            _ => self.smap.get(vel_to_name("idle", self.last_v)),
        }
    }

    fn rect(&self) -> Rect {
        let (width, height) = self.curr_sprite().size().into();
        Rect::new(
            self.pos.x - width / 2.0,
            self.pos.y - height / 2.0,
            width,
            height,
        )
    }

    fn tolerant_rect(&self) -> Rect {
        tolerant_rect(self.rect())
    }

    fn strike(&mut self, projectiles: &mut Vec<Projectile>, enemies: &mut [Enemy]) {
        assert!(self.strike_cone.is_some());
        play("arrow_shoot", 1.0, false);
        for e in enemies.iter_mut() {
            e.hightlight = false;
        }
        let (_, _, ints) = self.strike_cone.as_ref().unwrap();
        projectiles.push(Projectile::new(self.pos, &ints));
        self.strike_cone = None;
    }

    fn set_strike_cone(
        &mut self,
        new_strike_cone: Vec2,
        enemies: &mut [Enemy],
        colliders: &[Rect],
    ) {
        for e in enemies.iter_mut() {
            e.hightlight = false;
        }
        if let Some((mouse_pos, t, _)) = self.strike_cone.as_mut() {
            *mouse_pos = new_strike_cone;
            *t = (*t + get_frame_time() / self.bow_charge_duration as f32).min(1.0);
        } else {
            self.strike_cone = Some((new_strike_cone, 0.0, Vec::new()));
        }
        let dir = (self.strike_cone.as_ref().unwrap().0 - self.pos).normalize();
        let ints = get_intersections(self.pos, dir, enemies, colliders);
        self.strike_cone.as_mut().unwrap().2 = ints;
    }

    fn go(&mut self, mut v: Vec2, colliders: &[Rect]) {
        self.last_v = v;
        let new_rect_x = Rect {
            x: self.tolerant_rect().x + v.x,
            ..self.tolerant_rect()
        };
        let new_rect_y = Rect {
            y: self.tolerant_rect().y + v.y,
            ..self.tolerant_rect()
        };
        for c in colliders.iter() {
            let eps = 0.1;
            let c = Rect {
                x: c.x + eps,
                y: c.y + eps,
                w: c.w - 2.0 * eps,
                h: c.h - 2.0 * eps,
            };
            if new_rect_x.overlaps(&c) {
                v.x = 0.0;
            }
            if new_rect_y.overlaps(&c) {
                v.y = 0.0;
            }
        }
        self.is_moving = true;
        self.pos += v;
    }

    fn tick(
        &mut self,
        camera: &mut Camera2D,
        colliders: &[Rect],
        projectiles: &mut Vec<Projectile>,
        enemies: &mut [Enemy],
    ) {
        let was_moving = self.is_moving;
        self.is_moving = false;
        let speed = if self.carrying.0.is_null() {
            100.0
        } else {
            75.0
        };
        match self.state {
            PlayerState::Caught => {}
            _ => {
                self.state = PlayerState::Idle;

                if self.strike_cone.is_some() && is_mouse_button_released(MouseButton::Left) {
                    if self.strike_cone.as_ref().unwrap().1 == 1.0 {
                        self.strike(projectiles, enemies);
                    } else {
                        play("wrong", 1.0, false);
                        self.strike_cone = None;
                    }
                }
                self.seen_mouse_pressed =
                    self.seen_mouse_pressed || is_mouse_button_pressed(MouseButton::Left);
                if self.seen_mouse_pressed && is_mouse_button_down(MouseButton::Left) {
                    self.set_strike_cone(
                        camera.screen_to_world(mouse_position().into()),
                        enemies,
                        colliders,
                    );
                }

                if is_key_pressed(KeyCode::E) {
                    if self.carrying.0.is_null() {
                        if let Some(e) = enemies
                            .iter_mut()
                            .filter(|e| e.dead() && (e.pos - self.pos).length() < 48.0)
                            .min_by(|a, b| {
                                (a.pos - self.pos)
                                    .length()
                                    .partial_cmp(&(b.pos - self.pos).length())
                                    .unwrap()
                            })
                        {
                            play("hit", 1.0, false);
                            self.carrying = (e, e.pos - self.pos);
                        }
                    } else {
                        play("hit", 1.0, false);
                        self.carrying = (null_mut(), vec2(0.0, 0.0));
                    }
                }

                let mut v = vec2(0.0, 0.0);
                if is_key_down(KeyCode::W) {
                    v.y = -1.0;
                }
                if is_key_down(KeyCode::S) {
                    v.y = 1.0;
                }
                if is_key_down(KeyCode::A) {
                    v.x = -1.0;
                }
                if is_key_down(KeyCode::D) {
                    v.x = 1.0;
                }
                if v.length() > 0.0 {
                    if !was_moving {
                        play("footstep", 1.0, true);
                    }
                    self.state = PlayerState::Moving;
                    v = v.normalize() * speed * get_frame_time();
                    self.go(v, colliders);
                    unsafe {
                        let (e, off) = self.carrying;
                        if !e.is_null() {
                            e.as_mut().unwrap().pos = self.pos + off;
                        }
                    }
                }
            }
        }
        if was_moving && !self.is_moving {
            stop("footstep");
        }
    }

    fn draw_bow(&self) {
        if let Some((mouse_pos, strike_t, ints)) = &self.strike_cone {
            let tr = self.tolerant_rect();
            let bar_off = vec2(-16.0, 7.0);
            let bar_pos = vec2(tr.x, tr.y + tr.h) + bar_off;
            let bar_width = tr.w + bar_off.x.abs() * 2.0;
            let bar_height = 6.0;
            draw_rectangle_lines(
                bar_pos.x - 1.0,
                bar_pos.y - 1.0,
                bar_width + 2.0,
                bar_height + 2.0,
                1.0,
                BLACK,
            );
            draw_rectangle(
                bar_pos.x,
                bar_pos.y,
                bar_width * strike_t,
                bar_height,
                WHITE,
            );

            let (_, height) = self.curr_sprite().size().into();
            let max_len = 48.0;
            let len = strike_t * max_len;
            let mouse_dir = (*mouse_pos - self.pos).normalize() * len;
            draw_dotted_line(
                self.pos.x,
                self.pos.y,
                self.pos.x + mouse_dir.x,
                self.pos.y + mouse_dir.y,
                3.0,
                5.0,
                3.0,
                BLACK,
            );
            draw_dotted_line(
                self.pos.x,
                self.pos.y,
                self.pos.x + mouse_dir.x,
                self.pos.y + mouse_dir.y,
                2.0,
                4.0,
                4.0,
                WHITE,
            );
            let angle = mouse_dir.angle_between(vec2(0.0, -1.0)).to_degrees();
            let arc_len_deg = 90.0;
            draw_arc(
                self.pos.x,
                self.pos.y,
                32,
                height,
                180.0 - angle + arc_len_deg / 2.0 - 1.0,
                3.0,
                arc_len_deg + 2.0,
                BLACK,
            );
            draw_arc(
                self.pos.x,
                self.pos.y,
                32,
                height,
                180.0 - angle + arc_len_deg / 2.0,
                2.0,
                arc_len_deg,
                WHITE,
            );

            for (idx, int) in ints.iter().enumerate() {
                let pos = int.pos;
                match int.entity {
                    IsectType::Enemy(_) => {
                        draw_circle(pos.x, pos.y, 3.0, BLACK);
                        draw_circle(pos.x, pos.y, 2.0, WHITE);
                    }
                    IsectType::Collider(_) if idx < ints.len() - 1 => {
                        let new_dir = int.new_dir;
                        let old_dir = rotate_vec2(int.normal, -int.normal.angle_between(new_dir));
                        draw_dotted_line(
                            pos.x,
                            pos.y,
                            pos.x + new_dir.x * 25.0,
                            pos.y + new_dir.y * 25.0,
                            3.0,
                            5.0,
                            3.0,
                            BLACK,
                        );
                        draw_dotted_line(
                            pos.x,
                            pos.y,
                            pos.x + old_dir.x * 25.0,
                            pos.y + old_dir.y * 25.0,
                            3.0,
                            5.0,
                            3.0,
                            BLACK,
                        );
                        draw_dotted_line(
                            pos.x,
                            pos.y,
                            pos.x + new_dir.x * 25.0,
                            pos.y + new_dir.y * 25.0,
                            2.0,
                            4.0,
                            4.0,
                            WHITE,
                        );
                        draw_dotted_line(
                            pos.x,
                            pos.y,
                            pos.x + old_dir.x * 25.0,
                            pos.y + old_dir.y * 25.0,
                            2.0,
                            4.0,
                            4.0,
                            WHITE,
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    fn draw_emotes(&self) {
        let size = self.curr_sprite().size();
        let mut top_left = self.pos - size / 2.0;
        top_left.y -= 40.0;
        top_left.x += 16.0;
        if !self.carrying.0.is_null() {
            self.emotes.draw(top_left, "sweat");
        } else {
            self.emotes.reset();
        }
    }
}

impl Draw for Player {
    fn draw(&self) {
        let size = self.curr_sprite().size();
        let mut top_left = self.pos - size / 2.0;
        top_left.y -= 6.0;
        self.curr_sprite().draw(top_left);
        {
            let tolerant_rect = self.tolerant_rect();
            debug_draw(move || {
                draw_rectangle_lines(top_left.x, top_left.y, size.x, size.y, 1.0, RED);
                draw_rectangle_lines(
                    tolerant_rect.x,
                    tolerant_rect.y,
                    tolerant_rect.w,
                    tolerant_rect.h,
                    1.0,
                    BLUE,
                );
            });
        }
    }

    fn sort_order(&self) -> f32 {
        self.pos.y + self.curr_sprite().size().y / 2.0
    }
}

#[derive(Debug)]
pub struct Polygon {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

impl Polygon {
    pub fn contains(&self, p: Vec2) -> bool {
        for chunk in self.indices.chunks_exact(3) {
            let a: Vec2 = self.vertices[chunk[0] as usize].position.xy();
            let b: Vec2 = self.vertices[chunk[1] as usize].position.xy();
            let c: Vec2 = self.vertices[chunk[2] as usize].position.xy();
            if triangle_contains(p, a, b, c) {
                return true;
            }
        }
        false
    }

    pub fn draw_outline(&self, color: Color, thickness: f32, _t: f32) {
        let first = self.vertices[self.indices[0] as usize].position.xy();
        let second = self.vertices[self.indices[1] as usize].position.xy();
        let last = self.vertices[self.indices[self.indices.len() - 1] as usize]
            .position
            .xy();

        draw_line(first.x, first.y, second.x, second.y, thickness, color);
        draw_line(first.x, first.y, last.x, last.y, thickness, color);

        for idxs in self.indices.chunks_exact(3) {
            let b = self.vertices[idxs[1] as usize].position.xy();
            let c = self.vertices[idxs[2] as usize].position.xy();

            draw_line(b.x, b.y, c.x, c.y, 0.5, color);
        }
    }

    pub fn draw_eye(&self, color: Color, thickness: f32, radius: f32) {
        let first = self.vertices[self.indices[0] as usize].position.xy();
        let second = self.vertices[self.indices[1] as usize].position.xy();
        let last = self.vertices[self.indices[self.indices.len() - 1] as usize]
            .position
            .xy();

        let second = first + (second - first).normalize() * radius;
        let last = first + (last - first).normalize() * radius;
        draw_line(first.x, first.y, second.x, second.y, thickness, color);
        draw_line(first.x, first.y, last.x, last.y, thickness, color);
    }

    pub fn debug_draw(&self) {
        self.draw_outline(RED, 1.0, 0.5);
    }
}

#[derive(Debug, Clone)]
struct PatrolNode {
    pos: Vec2,
    facing: f32,
    wait: Time,
    walk: Option<Time>,
}

#[derive(Debug)]
struct PatrolPath {
    nodes: Vec<PatrolNode>,
    curr: isize,
    timer: Option<Time>,
    forwards: bool,
    full_circle: bool,
}

#[derive(Debug)]
enum EnemyState {
    Dead(bool),
    Patrolling,
    Assessing(f32),
    Reporting(bool),
}

#[derive(Debug)]
struct Enemy {
    pos: Vec2,
    hightlight: bool,
    detection_radius: f32,
    view_angle: f32,
    facing: f32,
    smap: SpriteMap,
    emotes: Emotes,
    cone: Option<(Vec2, f32, Polygon)>,
    state: EnemyState,
    patrol_path: PatrolPath,
    astar_path: Option<Vec<Vec2>>,
    last_v: Vec2,
    is_moving: bool,
    walk_speed: f32,
}

impl Enemy {
    fn new(path: PatrolPath, sprite: SpriteMap, emotes: Emotes) -> Self {
        Self {
            pos: path.nodes[path.curr as usize].pos,
            hightlight: false,
            detection_radius: 512.0,
            facing: path.nodes[path.curr as usize].facing,
            view_angle: 90.0f32.to_radians(),
            cone: None,
            state: EnemyState::Patrolling,
            patrol_path: path,
            astar_path: None,
            smap: sprite,
            emotes,
            last_v: vec2(0.0, 0.0),
            is_moving: false,
            walk_speed: 50.0,
        }
    }

    fn dead(&self) -> bool {
        if let EnemyState::Dead(_) = self.state {
            true
        } else {
            false
        }
    }

    fn die(&mut self) {
        match self.state {
            EnemyState::Dead(_) => {}
            _ => self.state = EnemyState::Dead(false),
        }
    }

    fn curr_sprite(&self) -> &Sprite {
        let mut v = rotate_vec2(vec2(1.0, 0.0), -self.facing);
        if v.x.abs() > v.y.abs() {
            v.y = 0.0;
        } else {
            v.x = 0.0;
        }
        match self.state {
            EnemyState::Reporting(_) if !self.is_moving => self.smap.get("use_phone"),
            EnemyState::Dead(true) => self.smap.get(vel_to_name("sit", v)),
            EnemyState::Dead(false) => self.smap.get(vel_to_name("damage", v)),
            _ => {
                if self.is_moving {
                    self.smap.get(vel_to_name("run", v))
                } else {
                    self.smap.get(vel_to_name("idle", v))
                }
            }
        }
    }

    fn is_point_in_full_cone(&self, p: Vec2) -> bool {
        if (p - self.pos).length() > self.detection_radius {
            return false;
        }
        let angle = (p - self.pos)
            .normalize()
            .angle_between(vec2(1.0, 0.0))
            .normalized_rads();
        let max = (self.facing + self.view_angle / 2.0).normalized_rads();
        let min = (self.facing - self.view_angle / 2.0).normalized_rads();

        if max > min && angle < max && angle > min {
            true
        } else if min > max && (angle > min || angle < max) {
            true
        } else {
            false
        }
    }

    fn generate_cone(&self, segments: &[(Vec2, Vec2)], color: Color, max_distance: f32) -> Polygon {
        let mut directions = Vec::new();
        let degree_step = 2;
        let mut max = 0.0;
        for d in (0..(self.view_angle.to_degrees()) as i32).step_by(degree_step) {
            let angle =
                (self.facing + self.view_angle / 2.0 - (d as f32).to_radians()).normalized_rads();
            if d == 0 {
                max = angle;
            }
            directions.push(angle);
        }
        let min = *directions.last().unwrap();
        for p in segments
            .iter()
            .map(|(a, b)| [*a, *b])
            .flatten()
            .filter(|p| self.is_point_in_full_cone(*p))
        {
            let angle = (p - self.pos)
                .normalize()
                .angle_between(vec2(1.0, 0.0))
                .normalized_rads();
            if (max > min && angle < max && angle > min)
                || (min > max && (angle > min || angle < max))
            {
                directions.push(angle);
            }
        }
        // TODO: don't need to regenerate this if the facing angle hasn't changed
        generate_shadow_polygon(
            self.pos,
            &segments,
            directions,
            max_distance,
            false,
            color,
            Some(-self.facing - self.view_angle / 2.0),
        )
    }

    fn get_suspicion(
        &mut self,
        shadow_segments: &[(Vec2, Vec2)],
        dead_enemy_rects: &[Rect],
        player: &Player,
    ) -> Option<(Vec2, bool)> {
        if self.dead() {
            return None;
        }
        let mut should_regen = false;
        if let Some(&(pos, facing, _)) = self.cone.as_ref() {
            if self.pos != pos || self.facing != facing {
                should_regen = true;
            }
        } else {
            should_regen = true;
        }
        if should_regen {
            self.cone = Some((
                self.pos,
                self.facing,
                self.generate_cone(shadow_segments, BLACK, self.detection_radius),
            ));
        }

        for (idx, r) in [player.tolerant_rect()]
            .iter()
            .chain(dead_enemy_rects.iter())
            .enumerate()
        {
            let points = [
                vec2(r.x, r.y),
                vec2(r.x + r.w, r.y),
                vec2(r.x, r.y + r.h),
                vec2(r.x + r.w, r.y + r.h),
            ];
            if !points.iter().any(|p| self.is_point_in_full_cone(*p)) {
                continue;
            }
            for p in points {
                if self.cone.as_ref().unwrap().2.contains(p) {
                    return Some((r.center(), idx == 0));
                }
            }
        }
        None
    }

    fn tick(
        &mut self,
        pathfinder: &Pathfinder,
        colliders: &[Rect],
        shadow_segments: &[(Vec2, Vec2)],
        dead_enemy_rects: &[Rect],
        player: &mut Player,
    ) {
        self.is_moving = false;

        // self.goto(
        //     pathfinder
        //         .get_path(self.tolerant_rect(), player.pos)
        //         .map(|p| *p.get(1).unwrap_or(&self.pos))
        //         .unwrap_or(self.pos),
        //     colliders,
        // );
        // return;

        let suspicion = self.get_suspicion(shadow_segments, dead_enemy_rects, player);
        match self.state {
            EnemyState::Patrolling if suspicion.is_some() => {
                play("alert", 1.0, false);
                self.state = EnemyState::Assessing(0.0);
            }
            EnemyState::Patrolling => {
                let curr = self.patrol_path.curr as usize;
                let reached_goal = {
                    if let Some(astar) = self.astar_path.as_ref() {
                        astar.len() == 0
                    } else {
                        (self.pos - self.patrol_path.nodes[curr].pos).length().abs() < 16.0
                    }
                };
                if !reached_goal {
                    if let Some(path) = self.astar_path.as_mut() {
                        if debug() {
                            for &p in path.iter() {
                                debug_draw(move || {
                                    draw_circle(p.x, p.y, 4.0, RED);
                                });
                            }
                        }
                        while let Some(first) = path.first() {
                            if *first == self.pos {
                                path.remove(0);
                            } else {
                                break;
                            }
                        }
                        if let Some(&target) = path.first() {
                            self.goto(target, colliders);
                        }
                    } else {
                        let target = self.patrol_path.nodes[curr].pos;
                        self.goto(target, colliders);
                    }
                } else {
                    if self.patrol_path.timer.is_none() {
                        self.patrol_path.timer = Some(get_time());
                    }
                    self.astar_path = None;
                    let timer = self.patrol_path.timer.unwrap();
                    self.facing = self.patrol_path.nodes[curr].facing;
                    if get_time() - timer > self.patrol_path.nodes[curr].wait {
                        let next_idx = if self.patrol_path.full_circle {
                            (curr + 1) % self.patrol_path.nodes.len()
                        } else {
                            if self.patrol_path.forwards && curr == self.patrol_path.nodes.len() - 1
                            {
                                self.patrol_path.forwards = false;
                            }
                            if !self.patrol_path.forwards && curr == 0 {
                                self.patrol_path.forwards = true;
                            }
                            if self.patrol_path.forwards {
                                (curr + 1) % self.patrol_path.nodes.len()
                            } else {
                                (curr - 1) % self.patrol_path.nodes.len()
                            }
                        };
                        if next_idx != curr {
                            let walk_time = if self.patrol_path.forwards {
                                self.patrol_path.nodes[curr].walk.unwrap()
                            } else {
                                self.patrol_path.nodes[next_idx].walk.unwrap()
                            };
                            self.walk_speed = (self.patrol_path.nodes[next_idx].pos - self.pos)
                                .length()
                                / walk_time as f32;
                            self.patrol_path.curr = next_idx as isize;
                            self.astar_path = pathfinder.get_path(
                                self.tolerant_rect(),
                                self.patrol_path.nodes[next_idx].pos,
                            );
                            self.patrol_path.timer = None;
                        }
                    }
                }
            }
            EnemyState::Assessing(t) => {
                // XXX
                // self.face_towards_player(pos);
                let assess_duration = 2.0;
                let reset_duration = 2.0;

                if let Some((sus_pos, is_player)) = suspicion {
                    let dist_t = (sus_pos - self.pos).length() / self.detection_radius;
                    let t = t + get_frame_time() / assess_duration;
                    if t >= dist_t.min(1.0) {
                        self.state = EnemyState::Reporting(is_player);
                    } else {
                        self.state = EnemyState::Assessing(t);
                    }
                } else {
                    let t = t - get_frame_time() / reset_duration;
                    if t <= 0.0 {
                        self.state = EnemyState::Patrolling;
                    } else {
                        self.state = EnemyState::Assessing(t);
                    }
                }
            }
            EnemyState::Reporting(is_player) => {
                player.caught(!is_player);
            }
            EnemyState::Dead(has_played_death_anim) => {
                if !has_played_death_anim && self.curr_sprite().about_to_loop() {
                    self.state = EnemyState::Dead(true);
                }
            }
        }

        {
            let px = self.pos.x;
            let py = self.pos.y;
            let facing = self.pos + rotate_vec2(vec2(1.0, 0.0), -self.facing) * 15.0;
            debug_draw(move || {
                draw_circle(px, py, 1.0, GREEN);
                draw_circle(facing.x, facing.y, 1.0, RED);
            })
        }
    }

    fn speed(&self) -> f32 {
        match self.state {
            EnemyState::Reporting(_) => 125.0,
            _ => self.walk_speed,
        }
    }

    fn goto(&mut self, target: Vec2, colliders: &[Rect]) {
        let dir = target - self.pos;
        self.facing = dir
            .normalize_or(vec2(1.0, 0.0))
            .angle_between(vec2(1.0, 0.0))
            .normalized_rads();
        let dist = dir.length();
        let mut v = dir.normalize_or_zero() * (self.speed() * get_frame_time()).min(dist);
        self.is_moving = true;
        self.last_v = v;
        let new_rect_x = Rect {
            x: self.tolerant_rect().x + v.x,
            ..self.tolerant_rect()
        };
        let new_rect_y = Rect {
            y: self.tolerant_rect().y + v.y,
            ..self.tolerant_rect()
        };
        for c in colliders.iter() {
            let eps = 0.1;
            let c = Rect {
                x: c.x + eps,
                y: c.y + eps,
                w: c.w - 2.0 * eps,
                h: c.h - 2.0 * eps,
            };
            if new_rect_x.overlaps(&c) {
                v.x = 0.0;
            }
            if new_rect_y.overlaps(&c) {
                v.y = 0.0;
            }
        }
        self.pos += v;
    }

    fn draw_cone_outline(&self) {
        match self.state {
            EnemyState::Dead(_) => return,
            _ => {}
        }
        if let Some((_, _, cone)) = self.cone.as_ref() {
            cone.draw_eye(MY_RED, 1.0, 48.0);
        }
    }

    fn draw_cone(&self) {
        match self.state {
            EnemyState::Dead(_) => return,
            _ => {}
        }
        if let Some((_, _, cone)) = self.cone.as_ref() {
            draw_custom_shape(&cone.vertices, &cone.indices);
        }
    }

    fn draw_red_cone(&self, shadow_segments: &[(Vec2, Vec2)]) {
        match self.state {
            EnemyState::Dead(_) => return,
            _ => {}
        }
        if let EnemyState::Assessing(t) = self.state {
            if t > 0.01 {
                let small_cone = self.generate_cone(
                    shadow_segments,
                    Color::from_rgba(255, 0, 0, 255),
                    self.detection_radius * t,
                );
                draw_custom_shape(&small_cone.vertices, &small_cone.indices);
            }
        }
    }

    fn rect(&self) -> Rect {
        let size = self.curr_sprite().size();
        Rect::new(
            self.pos.x - size.x / 2.0,
            self.pos.y - size.y / 2.0,
            size.x,
            size.y,
        )
    }

    fn tolerant_rect(&self) -> Rect {
        tolerant_rect(self.rect())
    }

    fn draw_emotes(&self) {
        let mut top_left = self.pos - self.curr_sprite().size() / 2.0;
        top_left.y -= 40.0;
        top_left.x += 16.0;
        match self.state {
            EnemyState::Assessing(_) => {
                self.emotes.draw(top_left, "question");
            }
            EnemyState::Reporting(_) => {
                self.emotes.draw(top_left, "exclamation");
            }
            _ => {
                self.emotes.reset();
            }
        }
    }

    fn move_to_first_node(&mut self, pathfinder: &Pathfinder) {
        if let Some(path) = pathfinder.get_path(self.tolerant_rect(), self.pos) {
            self.pos = *path.last().unwrap_or(&self.pos);
        }
    }
}

impl Draw for Enemy {
    fn draw(&self) {
        let mut top_left = self.pos - self.curr_sprite().size() / 2.0;
        top_left.y -= 6.0;
        self.curr_sprite().draw(top_left);

        {
            let r = self.rect();
            let t = self.tolerant_rect();
            debug_draw(move || {
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, RED);
                draw_rectangle_lines(t.x, t.y, t.w, t.h, 1.0, BLUE);
            });
        }

        // if let Some(cone) = self.cone.as_ref() {
        //     cone.draw_eye(MY_WHITE, 1.0, 64.0);
        // }
    }

    fn sort_order(&self) -> f32 {
        self.pos.y + self.curr_sprite().size().y / 2.0
    }
}

fn vel_to_name(name: &str, v: Vec2) -> String {
    if v.y < 0.0 {
        format!("{}_up", name)
    } else if v.y > 0.0 {
        format!("{}_down", name)
    } else if v.x < 0.0 {
        format!("{}_left", name)
    } else {
        format!("{}_right", name)
    }
}

#[derive(Debug, Clone)]
struct Intersection {
    pos: Vec2,
    normal: Vec2,
    entity: IsectType,
    new_dir: Vec2,
}

#[derive(Debug, Clone, Copy)]
enum IsectType {
    Enemy(*mut Enemy),
    Collider(*const Rect),
    Air,
}

impl IsectType {
    fn rect(&self) -> Rect {
        unsafe {
            match self {
                Self::Enemy(e) => e.as_ref().unwrap().rect(),
                Self::Collider(c) => **c,
                _ => Rect::default(),
            }
        }
    }
}

fn get_intersections(
    mut start: Vec2,
    mut dir: Vec2,
    enemies: &mut [Enemy],
    colliders: &[Rect],
) -> Vec<Intersection> {
    let mut all: Vec<Intersection> = Vec::new();
    let max_bounces = 4;
    let mut enemies = enemies.iter_mut().collect::<Vec<_>>();
    let mut have_bounced = false;
    for _ in 0..max_bounces {
        let endpoint = start + dir * 1000.0;
        let mut closest_int = (
            endpoint,
            Vec2::ZERO,
            endpoint,
            Vec2::ZERO,
            0,
            IsectType::Air,
        );
        for (idx, e) in enemies
            .iter_mut()
            .map(|e| IsectType::Enemy(*e))
            .chain(colliders.iter().map(|c| IsectType::Collider(c)))
            .enumerate()
        {
            match e {
                IsectType::Enemy(e) => unsafe {
                    if e.as_mut().unwrap().dead() {
                        continue;
                    }
                },
                _ => {}
            }
            let rect = e.rect();
            if let Some((pb, nb, pe, ne)) = line_rect_intersect(start, endpoint, rect) {
                if (pb - start).length() < (closest_int.0 - start).length() {
                    closest_int = (pb, nb, pe, ne, idx, e);
                }
            }
        }

        match closest_int {
            (_, _, pe, _, idx, IsectType::Enemy(_)) => {
                enemies.remove(idx);
                start = pe + dir * 1.0;
            }
            (pb, nb, _, _, _, IsectType::Collider(_)) => {
                let from = (start - pb).normalize();
                let angle = 2.0 * from.angle_between(nb);
                let rotator = vec2(angle.cos(), angle.sin());
                dir = from.rotate(rotator);
                start = pb + dir * 1.0;
            }
            (pb, _, _, _, _, IsectType::Air) => {
                start = pb;
            }
        }
        let int = Intersection {
            pos: closest_int.0,
            normal: closest_int.1,
            entity: closest_int.5,
            new_dir: dir,
        };

        all.push(int);
        match closest_int.5 {
            IsectType::Air => break,
            IsectType::Enemy(_) => break,
            IsectType::Collider(_) if have_bounced => break,
            IsectType::Collider(_) => have_bounced = true,
        }
    }
    all
}

fn rects_to_segments<'a>(rects: &'a [Rect]) -> impl Iterator<Item = (Vec2, Vec2)> + 'a {
    rects
        .iter()
        .map(|&Rect { x, y, w, h }| {
            [
                (vec2(x, y), vec2(x + w, y)),
                (vec2(x + w, y), vec2(x + w, y + h)),
                (vec2(x, y), vec2(x, y + h)),
                (vec2(x, y + h), vec2(x + w, y + h)),
            ]
        })
        .flatten()
}

fn generate_shadow_polygon(
    pos: Vec2,
    segments: &[(Vec2, Vec2)],
    directions: impl IntoIterator<Item = f32>,
    max_distance: f32,
    full_circle: bool,
    color: Color,
    start_from: Option<f32>,
) -> Polygon {
    let start_from = rotate_vec2(vec2(1.0, 0.0), start_from.unwrap_or(0.0) - TAU / 32.0);
    let mut ints = Vec::new();
    for angle in directions {
        let mut add_int = |dir: Vec2| {
            let endpoint = pos + dir * max_distance;
            let mut closest_int = endpoint;
            for ls in segments.iter() {
                if let Some(int) = intersect_lines(pos, endpoint, ls.0, ls.1) {
                    if int == pos {
                        continue;
                    }
                    if (closest_int - pos).length() > (int - pos).length() {
                        closest_int = int;
                    }
                }
            }
            ints.push(closest_int);
        };
        let dir = rotate_vec2(vec2(1.0, 0.0), -angle).normalize();
        add_int(dir);
        add_int(rotate_vec2(dir, 0.0001));
        add_int(rotate_vec2(dir, -0.0001));
    }

    ints.sort_by(|a, b| {
        let da = (*a - pos).normalize();
        let db = (*b - pos).normalize();
        let da = -da.angle_between(start_from).normalized_rads();
        let db = -db.angle_between(start_from).normalized_rads();
        let cmp = da.partial_cmp(&db);
        cmp.unwrap()
    });

    let mut vertices = Vec::new();
    let mut indices = Vec::<u16>::new();
    vertices.push(Vertex::new(pos.x, pos.y, 0.0, 0.0, 0.0, color));

    for i in 0..ints.len() {
        let x = ints[i].x;
        let y = ints[i].y;
        let len = ints.len();
        debug_draw(move || {
            draw_circle(
                x,
                y,
                2.0,
                Color {
                    r: i as f32 / len as f32,
                    ..BLUE
                },
            );
        });
        let p = ints[i];
        vertices.push(Vertex::new(p.x, p.y, 0.0, 0.0, 0.0, color));
        if i < ints.len() - 1 {
            let p = ints[i + 1];
            vertices.push(Vertex::new(p.x, p.y, 0.0, 0.0, 0.0, color));
            indices.extend_from_slice(&[
                0,
                (vertices.len() - 2) as u16,
                (vertices.len() - 1) as u16,
            ]);
        } else if full_circle {
            indices.extend_from_slice(&[0, (vertices.len() - 1) as u16, 1]);
        }
    }

    Polygon { vertices, indices }
}

struct Light {
    pos: Vec2,
    radius: f32,
    color: Color,
}

impl Draw for Light {
    fn draw(&self) {
        draw_circle(self.pos.x, self.pos.y, self.radius, self.color);
    }

    fn sort_order(&self) -> f32 {
        0.0
    }
}

static mut ROTATING: bool = true;

lazy_static! {
    static ref DEBUG_DRAWER: Mutex<DebugDrawer> = Mutex::new(DebugDrawer::new());
    static ref DEBUG: AtomicBool = AtomicBool::new(false);
}

fn debug_draw(f: impl Fn() + Send + Sync + 'static) {
    if DEBUG.load(Ordering::Relaxed) {
        DEBUG_DRAWER.lock().unwrap().queue(f);
    }
}

fn debug_flush() {
    if DEBUG.load(Ordering::Relaxed) {
        DEBUG_DRAWER.lock().unwrap().draw();
    } else {
        DEBUG_DRAWER.lock().unwrap().clear();
    }
}

fn debug() -> bool {
    DEBUG.load(Ordering::Relaxed)
}

fn debug_toggle() {
    DEBUG.store(!DEBUG.load(Ordering::Relaxed), Ordering::Relaxed);
}

fn get_camera_target(center: Vec2, tracking: f32, player: &Player) -> Vec2 {
    center.lerp(player.pos, tracking)
}

#[derive(Clone)]
struct Projectile {
    pos: Vec2,
    dir: Vec2,
    path: Vec<Intersection>,
    length: f32,
}

impl Projectile {
    fn new(pos: Vec2, path: &[Intersection]) -> Self {
        assert!(!path.is_empty());
        let dir = (path[0].pos - pos).normalize_or(vec2(1.0, 0.0));
        Self {
            pos,
            dir,
            path: path.to_vec(),
            length: 16.0,
        }
    }

    fn tick(&mut self, colliders: &[Rect], enemies: &mut [Enemy]) -> bool {
        let speed = 500.0;
        let v = self.dir * speed * get_frame_time();
        if !self.path.is_empty() && (self.path[0].pos - self.pos).length() <= v.length() {
            self.pos = self.path[0].pos;
            let removed = self.path.remove(0);
            match removed.entity {
                IsectType::Air => return true,
                IsectType::Collider(_) => play("arrow_bounce", 1.0, false),
                _ => {}
            }
            if let Some(&next) = self.path.get(0).as_ref() {
                self.dir = (next.pos - self.pos).normalize_or(vec2(1.0, 0.0));
            }
        } else {
            self.pos += v;
        }
        let head = self.pos + self.length * self.dir;
        if self.path.is_empty() {
            if let Some(_) = colliders
                .iter()
                .find(|c| c.contains(head) || c.contains(self.pos))
            {
                return true;
            }
        }
        if let Some(e) = enemies
            .iter_mut()
            .find(|e| !e.dead() && (e.rect().contains(head) || e.rect().contains(self.pos)))
        {
            e.die();
            play("hit", 1.0, false);
            return true;
        }
        false
    }
}

impl Draw for Projectile {
    fn draw(&self) {
        let target = self.pos + self.dir * self.length;
        draw_line(self.pos.x, self.pos.y, target.x, target.y, 2.0, WHITE);
    }

    fn sort_order(&self) -> f32 {
        100000.0
    }
}

enum ResetStage {
    Initial,
    ResetNow,
    Done,
}

#[derive(Clone)]
struct ResetHandler {
    swipe_t: f32,
    hit_middle: bool,
    started: Time,
    alarm_t: Time,
    play_alarm: bool,
}

impl ResetHandler {
    fn new(play_alarm: bool) -> Self {
        Self {
            swipe_t: -1.0,
            alarm_t: 0.0,
            hit_middle: false,
            started: get_time(),
            play_alarm,
        }
    }

    fn tick(&mut self) -> ResetStage {
        if self.play_alarm && self.alarm_t == 0.0 {
            play("alarm", 1.0, true);
        }
        if self.play_alarm {
            self.alarm_t += get_frame_time() as Time;
        }
        if self.play_alarm && get_time() - self.started < 1.5 {
            return ResetStage::Initial;
        }
        self.swipe_t += get_frame_time() * 2.0;
        if self.swipe_t > 0.0 && !self.hit_middle {
            self.hit_middle = true;
            stop("alarm");
            return ResetStage::ResetNow;
        } else if self.swipe_t > 1.0 {
            return ResetStage::Done;
        }
        ResetStage::Initial
    }

    fn alarm_t(&self) -> Time {
        if self.hit_middle {
            0.0
        } else {
            self.alarm_t
        }
    }
}

struct Scene {
    idx: usize,
    enemies: Vec<Enemy>,
    player: Player,
    level_center: Vec2,
    colliders: Vec<Rect>,
    shadow_segments: Vec<(Vec2, Vec2)>,
    bg_color: Color,
    structure_sprites: Vec<TileSprite>,
    auto_sprites: Vec<TileSprite>,
    decoration_sprites: Vec<TileSprite>,
    popups: Vec<PopUp>,
    transitions: Vec<(Vec2, String)>,
    triggers: Vec<Trigger>,
    pathfinder: Pathfinder,
    projectiles: Vec<Projectile>,
    reset_handler: Option<ResetHandler>,
    level_name: String,
    camera_height: f32,
    camera_tracking: f32,
    stage_cleared: bool,
    player_in_trigger: bool,
}

impl Scene {
    async fn new(
        ldtk_str: &str,
        idx: usize,
        player_smap: &SpriteMap,
        enemy_smaps: &[SpriteMap],
        emote_smap: &SpriteMap,
    ) -> Self {
        let level = load_ldtk(ldtk_str, idx).await;
        let level_center = level.center;

        let emotes = Emotes::new(emote_smap.clone());

        let player = Player::new(level.player_spawn, player_smap.clone(), emotes.clone());

        let mut enemies = level
            .patrol_paths
            .into_iter()
            .map(|p| Enemy::new(p, enemy_smaps.choose().unwrap().clone(), emotes.clone()))
            .collect::<Vec<_>>();
        let colliders = shape_preserving_rect_merge(level.colliders);
        let mut shadow_casters = shape_preserving_rect_merge(level.shadow_casters);
        shadow_casters.push(level.bounds);
        let shadow_segments = rects_to_segments(&shadow_casters).collect::<Vec<_>>();
        let structure_sprites = level.structure_sprites;
        let auto_sprites = level.auto_sprites;
        let decoration_sprites = level.decoration_sprites;
        let transitions = level.transitions;
        let triggers = level.triggers;
        let bounds = level.bounds;
        let pathfinder = Pathfinder::new(bounds.w, bounds.h, &colliders);
        let level_name = level.level_name;
        let camera_height = level.camera_height;
        let camera_tracking = level.camera_tracking;
        let popups = level.popups;

        for e in enemies.iter_mut() {
            e.move_to_first_node(&pathfinder);
        }

        Self {
            idx,
            enemies,
            player,
            level_center,
            colliders,
            shadow_segments,
            bg_color: level.bg_color,
            structure_sprites,
            auto_sprites,
            decoration_sprites,
            transitions,
            popups,
            triggers,
            pathfinder,
            projectiles: Vec::new(),
            reset_handler: None,
            level_name,
            camera_height,
            camera_tracking,
            stage_cleared: false,
            player_in_trigger: false,
        }
    }

    fn check_trigger(&mut self, won_game: &mut bool) -> Option<String> {
        if self.player.detected {
            return None;
        }
        for trigger in self.triggers.iter() {
            if trigger.rect.overlaps(&self.player.tolerant_rect()) {
                match trigger.ty {
                    TriggerType::LevelTransition => {
                        if !self.player_in_trigger {
                            self.player_in_trigger = true;
                            return self.get_level_transition();
                        } else {
                            return None;
                        }
                    }
                    TriggerType::WonGame => {
                        *won_game = true;
                    }
                }
            }
        }
        self.player_in_trigger = false;
        None
    }

    fn get_level_transition(&self) -> Option<String> {
        self.transitions
            .iter()
            .min_by(|a, b| {
                (a.0 - self.player.pos)
                    .length()
                    .partial_cmp(&(b.0 - self.player.pos).length())
                    .unwrap()
            })
            .map(|(_, name)| name.clone())
    }

    fn get_sorted_drawables(&self) -> Vec<&dyn Draw> {
        let mut drawables = self
            .decoration_sprites
            .iter()
            .map(|t| t as &dyn Draw)
            .chain(self.enemies.iter().map(|e| e as &dyn Draw))
            .chain([&self.player as &dyn Draw].into_iter())
            .collect::<Vec<_>>();
        drawables.sort_by(|a, b| a.sort_order().partial_cmp(&b.sort_order()).unwrap());
        drawables
    }

    fn render_preview_texture(&self, width: f32, height: f32) -> RenderTarget {
        let tex = render_target(width as u32, height as u32);
        let mut camera = Camera2D::from_display_rect(Rect::new(0.0, height, width, -height));
        camera.target = self.level_center;
        camera.render_target = Some(tex.clone());
        set_camera(&camera);

        clear_background(self.bg_color);
        for t in self.auto_sprites.iter() {
            t.draw();
        }
        for t in self.structure_sprites.iter() {
            t.draw();
        }
        for d in self.get_sorted_drawables() {
            d.draw();
        }

        set_default_camera();
        tex
    }

    fn tick(&mut self) {
        if !self.stage_cleared && self.enemies.iter().all(|e| e.dead()) {
            play("win", 0.6, false);
            self.stage_cleared = true;
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum TriggerType {
    LevelTransition,
    WonGame,
}

struct Trigger {
    rect: Rect,
    ty: TriggerType,
}

async fn get_smaps() -> (SpriteMap, [SpriteMap; 3], SpriteMap) {
    let enemy_sheets = {
        let enemy1_sheet = SpriteSheet::from_texture_path("assets/enemy1.png").await;
        let enemy2_sheet = SpriteSheet::from_texture_path("assets/enemy2.png").await;
        let enemy3_sheet = SpriteSheet::from_texture_path("assets/enemy3.png").await;
        [enemy1_sheet, enemy2_sheet, enemy3_sheet]
    };
    let player_sheet = SpriteSheet::from_texture_path("assets/player.png").await;
    let emote_sheet = SpriteSheet::from_texture_path("assets/emotes.png").await;

    let emote_anims = vec![
        (
            "base_intro".to_owned(),
            anim_rects(Rect::new(0.0, 0.0, 32.0, 64.0), 32.0, 4),
            0.1,
        ),
        (
            "base_final".to_owned(),
            anim_rects(Rect::new(32.0 * 3.0, 0.0, 32.0, 64.0), 32.0, 1),
            0.3,
        ),
        (
            "exclamation".to_owned(),
            anim_rects(Rect::new(0.0, 32.0 * 5.0, 32.0, 32.0), 32.0, 2),
            0.3,
        ),
        (
            "question".to_owned(),
            anim_rects(Rect::new(32.0 * 2.0, 32.0 * 6.0, 32.0, 32.0), 32.0, 2),
            0.3,
        ),
        (
            "knife".to_owned(),
            anim_rects(Rect::new(32.0 * 2.0, 32.0 * 7.0, 32.0, 32.0), 32.0, 2),
            0.3,
        ),
        (
            "sweat".to_owned(),
            anim_rects(Rect::new(32.0 * 6.0, 32.0 * 9.0, 32.0, 32.0), 32.0, 2),
            0.3,
        ),
    ];

    let row_base = 12.0;
    let row_height = 64.0;
    let col_width = 32.0;
    let sprite_width = 32.0;
    let sprite_height = 52.0;

    macro_rules! anim_rect {
        ($name: expr, $row: expr, $col: expr, $count: expr, $time: expr) => {
            (
                $name.to_owned(),
                anim_rects(
                    Rect::new(
                        $col as f32 * col_width,
                        row_base + $row as f32 * row_height,
                        sprite_width,
                        sprite_height,
                    ),
                    col_width,
                    $count,
                ),
                $time,
            )
        };
    }

    let char_anims = vec![
        anim_rect!("idle_right", 1, 0, 6, 0.2),
        anim_rect!("idle_up", 1, 6, 6, 0.2),
        anim_rect!("idle_left", 1, 12, 6, 0.2),
        anim_rect!("idle_down", 1, 18, 6, 0.2),
        anim_rect!("run_right", 2, 0, 6, 0.1),
        anim_rect!("run_up", 2, 6, 6, 0.1),
        anim_rect!("run_left", 2, 12, 6, 0.1),
        anim_rect!("run_down", 2, 18, 6, 0.1),
        anim_rect!("use_phone", 6, 0, 12, 0.2),
        anim_rect!("sit_right", 4, 0, 1, 0.1),
        anim_rect!("sit_down", 4, 0, 1, 0.1),
        anim_rect!("sit_left", 4, 6, 1, 0.1),
        anim_rect!("sit_up", 4, 6, 1, 0.1),
        anim_rect!("strike_right", 14, 0, 6, 0.1),
        anim_rect!("strike_up", 14, 6, 6, 0.1),
        anim_rect!("strike_left", 14, 12, 6, 0.1),
        anim_rect!("strike_down", 14, 18, 6, 0.1),
        anim_rect!("damage_right", 19, 0, 3, 0.2),
        anim_rect!("damage_up", 19, 3, 3, 0.2),
        anim_rect!("damage_left", 19, 6, 3, 0.2),
        anim_rect!("damage_down", 19, 9, 3, 0.2),
    ];

    let player_smap = SpriteMap::new(&player_sheet, &char_anims);
    let enemy_smaps = [
        SpriteMap::new(&enemy_sheets[0], &char_anims),
        SpriteMap::new(&enemy_sheets[1], &char_anims),
        SpriteMap::new(&enemy_sheets[2], &char_anims),
    ];
    let emote_smap = SpriteMap::new(&emote_sheet, &emote_anims);

    (player_smap, enemy_smaps, emote_smap)
}

fn get_width_height(desired_height: f32) -> (f32, f32) {
    if screen_height() * 16.0 / 9.0 < screen_width() {
        let height = desired_height * screen_dpi_scale();
        let width = height * screen_width() / screen_height();
        (width, height)
    } else {
        let width = desired_height * 16.0 / 9.0 * screen_dpi_scale();
        let height = width * screen_height() / screen_width();
        (width, height)
    }
}

pub async fn draw_progress(text: &str, t: f32) {
    clear_background(Color::from_hex(0x404059));
    draw_text_aligned(
        text,
        TextAlign::Center,
        vec2(screen_width(), screen_height()) / 2.0 - vec2(0.0, 200.0),
        None,
        false,
        TextParams {
            font_size: 48,
            ..Default::default()
        },
    );
    let w = screen_width() / 2.0;
    let h = 50.0;
    let pos = vec2(screen_width(), screen_height()) / 2.0 - vec2(w / 2.0, h / 2.0);
    draw_rectangle_lines(pos.x, pos.y, w, h, 2.0, WHITE);
    draw_rectangle(pos.x, pos.y, w * t, h, WHITE);
    next_frame().await;
}

#[derive(Default)]
struct AverageFps {
    sum: i32,
    count: i32,
    last_reset: Time,
    last_fps: f32,
}

impl AverageFps {
    fn new() -> Self {
        Self {
            sum: 0,
            count: 0,
            last_reset: get_time(),
            last_fps: 0.0,
        }
    }

    fn tick(&mut self) {
        if get_time() - self.last_reset > 1.0 {
            self.last_fps = self.sum as f32 / self.count as f32;
            self.last_reset = get_time();
            self.sum = 0;
            self.count = 0;
        }
        self.sum += get_fps();
        self.count += 1;
    }

    fn get(&self) -> f32 {
        self.last_fps
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // macroquad::rand::srand(::rand::random::<u64>());
    load_sounds().await;
    preload_textures().await;

    let (player_smap, enemy_smaps, emote_smap) = get_smaps().await;

    let mut vis_target = render_target(screen_width() as u32, screen_height() as u32);
    vis_target.texture.set_filter(FilterMode::Nearest);
    let mut cones_target = render_target(screen_width() as u32, screen_height() as u32);
    let mut postprocess_material = shadow_postprocess();

    let mut shadows = false;
    let mut timer = Option::<Time>::None;
    let mut won_game = false;
    let mut is_debug_paused = false;

    let mut scenes = Vec::new();
    let ldtk_str = include_str!("../assets/level.ldtk");
    for idx in get_level_indices(ldtk_str).await {
        scenes.push(Scene::new(ldtk_str, idx, &player_smap, &enemy_smaps, &emote_smap).await);
    }
    scenes.sort_by(|a, b| a.level_name.cmp(&b.level_name));
    let scene_previews = {
        let (width, height) = get_width_height(512.0);
        scenes
            .iter()
            .filter(|s| s.level_name.starts_with("Stage_"))
            .map(|s| {
                (
                    s.level_name.clone(),
                    s.render_preview_texture(width, height),
                )
            })
            .collect::<Vec<_>>()
    };
    let mut ui = Ui::new(scene_previews.clone()).await;
    let mut scene = scenes.iter_mut().find(|s| s.level_name == "Menu").unwrap();
    scene.player.pos = vec2(-100.0, -100.0);

    let new_camera = |scene: &Scene| {
        let (width, height) = get_width_height(scene.camera_height);
        Camera2D::from_display_rect(Rect::new(0.0, height, width, -height))
    };
    let mut camera = new_camera(scene);

    let mut last_screen_size = vec2(screen_width(), screen_height());

    play("bg_music", 1.0, true);

    let mut avg_fps = AverageFps::new();

    loop {
        avg_fps.tick();
        if let Some(timer) = timer.as_mut() {
            if !won_game && !ui.is_enabled() {
                *timer += get_frame_time() as Time;
            }
        }

        if last_screen_size != vec2(screen_width(), screen_height()) {
            set_default_camera();
            camera.render_target = None;
            gl_use_default_material();
            last_screen_size = vec2(screen_width(), screen_height());
            let old_target = camera.target;
            camera = new_camera(scene);
            camera.target = old_target;
            drop(postprocess_material);
            drop(vis_target);
            drop(cones_target);
            next_frame().await;
            postprocess_material = shadow_postprocess();
            vis_target = render_target(screen_width() as u32, screen_height() as u32);
            vis_target.texture.set_filter(FilterMode::Nearest);
            cones_target = render_target(screen_width() as u32, screen_height() as u32);
            ui.resize();
        }

        if cfg!(debug_assertions) {
            if is_key_pressed(KeyCode::Q) {
                break;
            }
            if is_key_pressed(KeyCode::Space) {
                is_debug_paused = !is_debug_paused;
            }
        }

        let mut new_scene = None;
        if !ui.is_enabled() && !is_debug_paused {
            if is_key_pressed(KeyCode::R) {
                scene.reset_handler = Some(ResetHandler::new(false));
            }
            if cfg!(debug_assertions) {
                if is_key_pressed(KeyCode::T) {
                    shadows = !shadows;
                }
                if is_key_pressed(KeyCode::G) {
                    unsafe {
                        ROTATING = !ROTATING;
                    }
                }
                if is_key_pressed(KeyCode::H) {
                    debug_toggle();
                }
            }

            scene.player.tick(
                &mut camera,
                &scene.colliders,
                &mut scene.projectiles,
                &mut scene.enemies,
            );
            camera.target =
                get_camera_target(scene.level_center, scene.camera_tracking, &scene.player);
            let dead_enemy_rects = scene
                .enemies
                .iter()
                .filter(|e| e.dead())
                .map(|e| e.tolerant_rect())
                .collect::<Vec<_>>();
            for e in scene.enemies.iter_mut() {
                e.tick(
                    &scene.pathfinder,
                    &scene.colliders,
                    &scene.shadow_segments,
                    &dead_enemy_rects,
                    &mut scene.player,
                );
            }
            scene.projectiles = scene
                .projectiles
                .drain(..)
                .filter_map(|mut p| {
                    if !p.tick(&scene.colliders, &mut scene.enemies) {
                        Some(p)
                    } else {
                        None
                    }
                })
                .collect();

            if scene.player.detected {
                if scene.reset_handler.is_none() {
                    scene.reset_handler = Some(ResetHandler::new(true));
                }
            }
            scene.tick();

            for pop in scene.popups.iter_mut().filter(|p| !p.triggered) {
                if !ui.is_enabled() && pop.rect.overlaps(&scene.player.tolerant_rect()) {
                    scene.player.state = PlayerState::Idle;
                    scene.player.is_moving = false;
                    stop("footstep");
                    pop.triggered = true;
                    ui.popup(&pop.text);
                }
            }

            if let Some(handler) = scene.reset_handler.as_mut() {
                match handler.tick() {
                    ResetStage::Initial => {}
                    ResetStage::ResetNow => {
                        let handler = handler.clone();
                        let old_popups = scene.popups.clone();
                        *scene = Scene::new(
                            &ldtk_str,
                            scene.idx,
                            &player_smap,
                            &enemy_smaps,
                            &emote_smap,
                        )
                        .await;
                        scene.popups = old_popups;
                        scene.reset_handler = Some(handler);
                    }
                    ResetStage::Done => {
                        scene.reset_handler = None;
                    }
                }
            }
        }

        //
        // DRAW TO THE VISIBILITY TEXTURE
        //

        camera.render_target = Some(vis_target.clone());
        set_camera(&camera);
        clear_background(Color::new(0.0, 0.0, 0.0, 0.0));

        for t in scene.auto_sprites.iter() {
            t.draw();
        }
        for t in scene.structure_sprites.iter() {
            t.draw();
        }

        for d in scene.get_sorted_drawables() {
            d.draw();
        }

        scene.player.draw_emotes();
        for e in scene.enemies.iter() {
            e.draw_emotes();
            e.draw_cone_outline();
        }

        //
        // DRAW TO THE CONES TEXTURE
        //
        camera.render_target = Some(cones_target.clone());
        set_camera(&camera);
        clear_background(Color::new(0.0, 0.0, 0.0, 0.0));

        for e in scene.enemies.iter() {
            e.draw_cone();
        }
        for e in scene.enemies.iter() {
            e.draw_red_cone(&scene.shadow_segments);
        }

        // let max_distance = 1024.0;
        // let directions = scene
        //     .shadow_segments
        //     .iter()
        //     .map(|(a, b)| [*a, *b])
        //     .flatten()
        //     .map(|p| {
        //         (p - scene.player.pos)
        //             .angle_between(vec2(1.0, 0.0))
        //             .normalized_rads()
        //     });
        // let poly = generate_shadow_polygon(
        //     scene.player.pos,
        //     &scene.shadow_segments,
        //     directions,
        //     max_distance,
        //     true,
        //     WHITE,
        //     None,
        // );
        // draw_custom_shape(&poly.vertices, &poly.indices);

        //
        // BLIT THE TEXTURES TOGETHER
        //

        postprocess_material.set_texture("VisibleTexture", vis_target.texture.clone());
        postprocess_material.set_texture("ConesTexture", cones_target.texture.clone());
        postprocess_material.set_uniform::<f32>(
            "AlarmTime",
            scene
                .reset_handler
                .as_ref()
                .map(|r| r.alarm_t())
                .unwrap_or(0.0) as f32,
        );
        postprocess_material.set_uniform::<[f32; 4]>("BgColor", Color::from_hex(0x404059).into());
        postprocess_material.set_uniform::<f32>(
            "SwipeT",
            scene
                .reset_handler
                .as_ref()
                .map(|r| r.swipe_t)
                .unwrap_or(-2.0),
        );
        gl_use_material(&postprocess_material);
        set_default_camera();
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), WHITE);
        gl_use_default_material();

        //
        // DRAW DEBUG INFO
        //

        camera.render_target = None;
        set_camera(&camera);
        debug_flush();
        if debug() {
            for c in scene.colliders.iter() {
                draw_rectangle_lines(c.x, c.y, c.w, c.h, 1.0, BLUE);
            }
            for p in scene.popups.iter() {
                draw_rectangle_lines(p.rect.x, p.rect.y, p.rect.w, p.rect.h, 1.0, PURPLE);
            }
        }

        for p in scene.projectiles.iter() {
            p.draw();
        }
        scene.player.draw_bow();

        //
        // DRAW UI
        //

        set_default_camera();
        draw_text(
            &format!("FPS: {:.0}", avg_fps.get()),
            10.0,
            25.0,
            32.0,
            MY_WHITE,
        );

        {
            let timer = timer.unwrap_or(0.0);
            let minutes = (timer / 60.0) as i32;
            let seconds = timer - minutes as f64 * 60.0;
            draw_text(
                &format!("Timer: {:02}:{:05.2}", minutes, seconds),
                10.0,
                50.0,
                32.0,
                MY_WHITE,
            );
        }

        if scene.player.detected {
            draw_text_aligned(
                if let PlayerState::Caught = scene.player.state {
                    "You were found!"
                } else {
                    "Dead body found!"
                },
                TextAlign::Center,
                vec2(screen_width() / 2.0, screen_height() / 2.0),
                None,
                false,
                TextParams {
                    font_size: 128,
                    ..Default::default()
                },
            );
        }

        set_camera(&ui.camera);
        if let Some(s) = scene.check_trigger(&mut won_game) {
            if scene.enemies.iter().all(|e| e.dead()) {
                new_scene = Some(s);
            } else {
                scene.player.state = PlayerState::Idle;
                scene.player.is_moving = false;
                stop("footstep");
                ui.popup("You cannot progress until all enemies have been dispatched.");
            }
        }
        match ui.tick(scene.level_name == "Menu") {
            UiAction::SwitchLevel(name) => {
                timer = Some(0.0);
                won_game = false;
                new_scene = Some(name);
            }
            UiAction::Quit => {
                if scene.level_name == "Menu" {
                    break;
                } else {
                    new_scene = Some("Menu".into());
                }
            }
            UiAction::None => {}
        }
        ui.draw(scene.level_name == "Menu");

        set_default_camera();

        if let Some(new_scene) = new_scene {
            scene = scenes
                .iter_mut()
                .find(|s| s.level_name == new_scene)
                .unwrap();
            *scene = Scene::new(
                &ldtk_str,
                scene.idx,
                &player_smap,
                &enemy_smaps,
                &emote_smap,
            )
            .await;
            camera = new_camera(scene);

            if scene.level_name == "Menu" {
                scene.player.pos = vec2(-10000.0, -10000.0);
            }
            ui.set_curr_level(&scene.level_name);

            stop("footstep");
            stop("alarm");
        }

        next_frame().await
    }
}

fn window_conf() -> macroquad::conf::Conf {
    macroquad::conf::Conf {
        miniquad_conf: miniquad::conf::Conf {
            window_title: "Museum Assassin".to_owned(),
            window_width: 1920,
            window_height: 1080,
            high_dpi: true, // https://docs.rs/good-web-game/latest/good_web_game/
            ..Default::default()
        },
        draw_call_index_capacity: 10000,
        default_filter_mode: FilterMode::Nearest,
        ..Default::default()
    }
}
