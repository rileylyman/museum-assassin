use std::{
    cell::RefCell,
    collections::{BinaryHeap, HashMap},
    hash::Hash,
};

use macroquad::prelude::*;

use crate::debug_draw;

pub struct Pathfinder {
    cells: Vec<bool>,
    colliders_cache: Vec<IVec2>,
    cell_size: f32,
    cells_width: i32,
    cells_height: i32,
}

impl Pathfinder {
    pub fn new(level_width: f32, level_height: f32, colliders: &[Rect]) -> Self {
        let mut cells = Vec::new();
        let mut colliders_cache = Vec::new();
        let cell_size = 8.0;
        let cells_width = (level_width / cell_size) as i32;
        let cells_height = (level_height / cell_size) as i32;

        for y in 0..cells_height {
            for x in 0..cells_width {
                let p = vec2(x as f32 * cell_size, y as f32 * cell_size)
                    + vec2(cell_size / 2.0, cell_size / 2.0);
                // let rect = Rect::new(
                //     x as f32 * cell_size,
                //     y as f32 * cell_size,
                //     cell_size,
                //     cell_size,
                // );
                if colliders.iter().any(|c| c.contains(p)) {
                    cells.push(true);
                    colliders_cache.push(ivec2(x, y));
                } else {
                    cells.push(false);
                }
            }
        }

        Self {
            cells,
            colliders_cache,
            cell_size,
            cells_width,
            cells_height,
        }
    }

    pub fn get_path(&self, rect: Rect, to: Vec2) -> Option<Vec<Vec2>> {
        let from_cell = self.vec2_to_cell(rect.center());
        if self.is_oob(from_cell) {
            return None;
        }

        let mut to_cell = self.vec2_to_cell(to);
        let to_candidates = [
            to_cell,
            ivec2(to_cell.x + 1, to_cell.y),
            ivec2(to_cell.x - 1, to_cell.y),
            ivec2(to_cell.x, to_cell.y + 1),
            ivec2(to_cell.x, to_cell.y - 1),
        ];

        if let Some(c) = to_candidates.iter().find(|&&p| {
            let real_pos = self.cell_to_vec2(p);
            let r = Rect::new(
                real_pos.x - rect.w / 2.0,
                real_pos.y - rect.h / 2.0,
                rect.w,
                rect.h,
            );
            !self.is_oob(p) && !self.is_rect_colliding(r)
        }) {
            to_cell = *c;
        } else {
            return None;
        }

        let h = |p: IVec2| (to_cell - p).length_squared();

        let mut heap = BinaryHeap::<Node>::new();
        let mut g_scores = HashMap::<Node, i32>::new();
        let f_scores = RefCell::new(HashMap::<Node, i32>::new());
        let mut came_from = HashMap::<IVec2, IVec2>::new();

        #[derive(Clone, Copy)]
        pub struct Node<'f> {
            pos: IVec2,
            f_scores: &'f RefCell<HashMap<Node<'f>, i32>>,
        }

        impl PartialEq for Node<'_> {
            fn eq(&self, other: &Self) -> bool {
                self.pos == other.pos
            }
        }

        impl Eq for Node<'_> {}

        impl Hash for Node<'_> {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.pos.hash(state);
            }
        }

        impl Ord for Node<'_> {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.f_scores.borrow()[self]
                    .cmp(&self.f_scores.borrow()[other])
                    .reverse()
            }
        }

        impl PartialOrd for Node<'_> {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        {
            let n = Node {
                pos: self.vec2_to_cell(rect.center()),
                f_scores: &f_scores,
            };
            g_scores.insert(n, 0);
            f_scores.borrow_mut().insert(n, h(n.pos));
            heap.push(n);
        }

        let mut found = None;
        while let Some(curr) = heap.pop() {
            if curr.pos == to_cell {
                found = Some(curr);
                break;
            }
            let curr_real_pos = self.cell_to_vec2(curr.pos) - vec2(rect.w / 2.0, rect.h / 2.0);
            let curr_rect = Rect {
                x: curr_real_pos.x,
                y: curr_real_pos.y,
                ..rect
            };
            for x in -1..=1 {
                for y in -1..=1 {
                    let off = ivec2(x, y);
                    let pos = curr.pos + off;
                    if (off.x == 0 && off.y == 0) || off.x.abs() == off.y.abs() {
                        continue;
                    }
                    if self.is_oob(pos) {
                        continue;
                    }

                    let neib_real_pos = self.cell_to_vec2(pos);
                    if self.is_direct_path_blocked(curr_rect, neib_real_pos) {
                        continue;
                    }

                    let neib = Node {
                        pos,
                        f_scores: &f_scores,
                    };

                    if crate::debug() {
                        let neib_v2 = self.cell_to_vec2(neib.pos);
                        let curr_v2 = self.cell_to_vec2(curr.pos);
                        debug_draw(move || {
                            draw_line(neib_v2.x, neib_v2.y, curr_v2.x, curr_v2.y, 1.0, GREEN);
                        });
                    }

                    let neib_g = g_scores[&curr] + 1;
                    if neib_g < *g_scores.get(&neib).unwrap_or(&i32::MAX) {
                        g_scores.insert(neib, neib_g);
                        f_scores.borrow_mut().insert(neib, neib_g + h(neib.pos));
                        came_from.insert(neib.pos, curr.pos);
                        heap.push(neib);
                    }
                }
            }
        }
        if found.is_none() {
            return None;
        }

        let found = found.unwrap();
        let mut path = vec![found.pos];

        while let Some(n) = came_from.get(&path[path.len() - 1]) {
            path.push(*n);
        }

        if crate::debug() {
            for (p1, p2) in path.iter().zip(path.iter().skip(1)) {
                let p1 = self.cell_to_vec2(*p1);
                let p2 = self.cell_to_vec2(*p2);
                debug_draw(move || {
                    draw_line(p1.x, p1.y, p2.x, p2.y, 1.0, RED);
                });
            }
        }

        Some(
            self.cleanup_path_redundancies(
                rect,
                path.into_iter()
                    .rev()
                    .map(|p| self.cell_to_vec2(p))
                    .collect(),
            ),
        )
    }

    fn cleanup_path_redundancies(&self, rect: Rect, mut path: Vec<Vec2>) -> Vec<Vec2> {
        let mut new_path = Vec::new();
        let first = path.remove(0);
        new_path.push(Rect {
            x: first.x - rect.w / 2.0,
            y: first.y - rect.h / 2.0,
            ..rect
        });
        while !path.is_empty() {
            let mut last = None;
            while !self.is_direct_path_blocked(*new_path.last().unwrap(), path[0]) {
                last = {
                    let first = path.remove(0);
                    let new_rect = Rect {
                        x: first.x - rect.w / 2.0,
                        y: first.y - rect.h / 2.0,
                        ..rect
                    };
                    Some(new_rect)
                };
                if path.is_empty() {
                    break;
                }
            }
            assert!(last.is_some());
            new_path.push(last.unwrap());
        }
        new_path.into_iter().map(|r| r.center()).collect()
    }

    pub fn is_direct_path_blocked(&self, mut rect: Rect, to: Vec2) -> bool {
        let orig_dist = to - rect.center();
        let dir = orig_dist.try_normalize();
        if dir.is_none() {
            return false;
        }
        let dir = dir.unwrap() * self.cell_size;

        while (to - rect.center()).signum() == orig_dist.signum() {
            if self.is_rect_colliding(rect) {
                return true;
            }
            rect = Rect {
                x: rect.x + dir.x,
                y: rect.y + dir.y,
                ..rect
            }
        }

        false
    }

    pub fn is_rect_colliding(&self, rect: Rect) -> bool {
        for IVec2 { x, y } in self.colliders_cache.iter() {
            let cell = Rect::new(
                *x as f32 * self.cell_size,
                *y as f32 * self.cell_size,
                self.cell_size,
                self.cell_size,
            );
            if cell.overlaps(&rect) {
                return true;
            }
        }
        false
    }

    pub fn is_collider(&self, x: i32, y: i32) -> bool {
        self.cells[(y * self.cells_width + x) as usize]
    }

    fn is_oob(&self, p: IVec2) -> bool {
        p.x < 0 || p.x >= self.cells_width || p.y < 0 || p.y >= self.cells_height
    }

    pub fn vec2_to_cell(&self, v: Vec2) -> IVec2 {
        (v / self.cell_size).as_ivec2()
    }

    pub fn cell_to_vec2(&self, v: IVec2) -> Vec2 {
        v.as_vec2() * self.cell_size + vec2(self.cell_size / 2.0, self.cell_size / 2.0)
    }
}
