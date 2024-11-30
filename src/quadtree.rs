use macroquad::prelude::*;

use crate::geometry_utils::{intersect_lines, line_rect_intersect};

type LineSeg = (Vec2, Vec2);

pub struct QuadTree {
    root: QuadNode,
}

struct QuadNode {
    rect: Rect,
    children: Option<Vec<QuadNode>>,
    segments: Option<Vec<LineSeg>>,
}

impl QuadTree {
    pub fn build(rect: Rect, min_size: Vec2, line_segments: &[LineSeg]) -> Self {
        Self {
            root: QuadNode::build(rect, min_size, line_segments),
        }
    }

    pub fn filter_by_segment(&self, line_segment: LineSeg) -> Vec<LineSeg> {
        self.root.filter(&|rect| {
            rect.contains(line_segment.0)
                || rect.contains(line_segment.1)
                || line_rect_intersect(line_segment.0, line_segment.1, rect).is_some()
        })
    }

    pub fn filter_by_radius(&self, pos: Vec2, radius: f32) -> Vec<LineSeg> {
        self.root.filter(&|rect| {
            let dist_to_center = (rect.center() - pos).length();
            let dir = (rect.center() - pos).normalize_or_zero() * dist_to_center.min(radius);
            rect.contains(pos + dir)
        })
    }

    pub fn debug_draw(&self) {
        self.root.debug_draw();
    }

    pub fn intersect(&self, from: Vec2, to: Vec2) -> Option<Vec2> {
        if self.root.does_intersect(from, to) {
            self.root.intersect(from, to)
        } else {
            None
        }
    }
}

impl QuadNode {
    fn debug_draw(&self) {
        draw_rectangle_lines(
            self.rect.x,
            self.rect.y,
            self.rect.w,
            self.rect.h,
            1.0,
            GREEN,
        );
        if let Some(children) = self.children.as_ref() {
            for child in children {
                child.debug_draw();
            }
        }
    }

    fn does_intersect(&self, from: Vec2, to: Vec2) -> bool {
        self.rect.contains(from)
            || self.rect.contains(to)
            || line_rect_intersect(from, to, self.rect).is_some()
    }

    fn intersect(&self, from: Vec2, to: Vec2) -> Option<Vec2> {
        if let Some(children) = self.children.as_ref() {
            let mut children_ints = children
                .iter()
                .filter_map(|c| {
                    if c.rect.contains(from) {
                        Some((c, from))
                    } else {
                        line_rect_intersect(from, to, c.rect).map(|i| (c, i.0))
                    }
                })
                .collect::<Vec<_>>();
            children_ints.sort_by(|&a, &b| {
                (a.1 - from)
                    .length()
                    .partial_cmp(&(b.1 - from).length())
                    .unwrap()
            });
            for i in children_ints.iter() {
                draw_circle(i.1.x, i.1.y, 2.0, BLUE);
            }
            let dir = (to - from).normalize() * 0.1;
            children_ints
                .into_iter()
                .map(|(c, i)| c.intersect(i + dir, to))
                .find_map(|i| i)
        } else if let Some(segments) = self.segments.as_ref() {
            let mut ints = segments
                .iter()
                .filter_map(|li| intersect_lines(from, to, li.0, li.1))
                .collect::<Vec<_>>();
            ints.sort_by(|&a, &b| {
                (a - from)
                    .length()
                    .partial_cmp(&(b - from).length())
                    .unwrap()
            });
            let ret = ints.into_iter().next();
            if let Some(ret) = ret {
                draw_circle(ret.x, ret.y, 2.0, RED);
            }
            ret
        } else {
            unreachable!()
        }
    }

    fn filter(&self, filter_fn: &impl Fn(Rect) -> bool) -> Vec<LineSeg> {
        assert!(self.children.is_some() || self.segments.is_some());
        assert!(!self.children.is_some() || !self.segments.is_some());

        if !filter_fn(self.rect) {
            return Vec::with_capacity(0);
        }

        if let Some(children) = self.children.as_ref() {
            let mut ret = Vec::new();
            for child in children {
                ret.extend(child.filter(filter_fn));
            }
            return ret;
        }

        if let Some(segments) = self.segments.as_ref() {
            return segments.clone();
        }

        unreachable!()
    }

    fn build(rect: Rect, min_size: Vec2, line_segments: &[LineSeg]) -> Self {
        let (segments, children) = if rect.size().length_squared() <= min_size.length_squared() {
            (
                Some(
                    line_segments
                        .iter()
                        .filter(|li| {
                            rect.contains(li.0)
                                || rect.contains(li.1)
                                || line_rect_intersect(li.0, li.1, rect).is_some()
                        })
                        .map(|li| *li)
                        .collect::<Vec<_>>(),
                ),
                None,
            )
        } else {
            (
                None,
                Some(
                    (0..=1)
                        .map(|h| {
                            (0..=1).map(move |v| {
                                QuadNode::build(
                                    Rect {
                                        x: rect.x + h as f32 * rect.w / 2.0,
                                        y: rect.y + v as f32 * rect.h / 2.0,
                                        w: rect.w / 2.0,
                                        h: rect.h / 2.0,
                                    },
                                    min_size,
                                    line_segments,
                                )
                            })
                        })
                        .flatten()
                        .collect::<Vec<_>>(),
                ),
            )
        };
        Self {
            rect,
            children,
            segments,
        }
    }
}
