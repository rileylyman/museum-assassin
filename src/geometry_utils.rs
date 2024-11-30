use std::f32::consts::TAU;

use approx::ulps_eq;
use macroquad::prelude::*;

pub trait GeoUtilsFloatExts {
    fn normalized_rads(self) -> Self;
}

impl GeoUtilsFloatExts for f32 {
    fn normalized_rads(self) -> Self {
        let theta = self % TAU;
        if theta < 0.0 {
            theta + TAU
        } else {
            theta
        }
    }
}

#[allow(dead_code)]
pub fn cast_rect(mut rect: Rect, to: Vec2, barriers: &[Rect]) -> bool {
    let step_size = 1.0;
    let accept_interval = 1.5 * step_size;
    let dir = match (to - rect.center()).try_normalize() {
        Some(dir) => dir,
        _ => return true,
    };
    while (to - rect.center()).length() > accept_interval
        && !barriers.iter().any(|b| {
            (Rect {
                x: rect.x + 0.1,
                y: rect.y + 0.1,
                w: rect.w - 0.2,
                h: rect.h - 0.2,
            })
            .overlaps(&b)
        })
    {
        rect.x += dir.x * step_size;
        rect.y += dir.y * step_size;
    }
    (to - rect.center()).length() <= accept_interval
}

pub fn rotate_vec2(v: Vec2, angle: f32) -> Vec2 {
    let c = angle.cos();
    let s = angle.sin();
    vec2(v.x * c - v.y * s, v.x * s + v.y * c)
}

pub fn triangle_contains(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let area = (b - a).perp_dot(c - a) * 0.5;
    let bary_a = (p - b).perp_dot(p - c) / area;
    let bary_b = (p - c).perp_dot(p - a) / area;
    let bary_c = (p - a).perp_dot(p - b) / area;
    bary_a > 0.0 && bary_b > 0.0 && bary_c > 0.0
}

pub fn shape_preserving_rect_merge(mut rects: Vec<Rect>) -> Vec<Rect> {
    if rects.is_empty() {
        return rects;
    }

    // sort by y value then by x
    rects.sort_by(|a, b| {
        if a.y == b.y {
            a.x.partial_cmp(&b.x).unwrap()
        } else {
            a.y.partial_cmp(&b.y).unwrap()
        }
    });

    let mut x_merged = Vec::new();
    let mut cur = rects.pop().unwrap();

    while let Some(back) = rects.last() {
        if back.y == cur.y && back.h == cur.h && back.x + back.w == cur.x {
            cur = cur.combine_with(*back);
            rects.pop();
        } else {
            x_merged.push(cur);
            cur = rects.pop().unwrap();
        }
    }
    x_merged.push(cur);

    x_merged.sort_by(|a, b| {
        if a.x == b.x {
            a.y.partial_cmp(&b.y).unwrap()
        } else {
            a.x.partial_cmp(&b.x).unwrap()
        }
    });

    let mut y_merged = Vec::new();
    let mut cur = x_merged.pop().unwrap();

    while let Some(back) = x_merged.last() {
        if back.x == cur.x && back.w == cur.w && back.y + back.h == cur.y {
            cur = cur.combine_with(*back);
            x_merged.pop();
        } else {
            y_merged.push(cur);
            cur = x_merged.pop().unwrap();
        }
    }
    y_merged.push(cur);

    y_merged
}

pub fn line_rect_intersect(start: Vec2, end: Vec2, rect: Rect) -> Option<(Vec2, Vec2, Vec2, Vec2)> {
    let r1 = vec2(rect.x, rect.y);
    let r2 = vec2(rect.x + rect.w, rect.y);
    let r3 = vec2(rect.x, rect.y + rect.h);
    let r4 = vec2(rect.x + rect.w, rect.y + rect.h);

    let ints = [
        (
            intersect_lines(r1, r2, start, end),
            (r1 - r2).perp().normalize(),
        ),
        (
            intersect_lines(r1, r3, start, end),
            (r1 - r3).perp().normalize(),
        ),
        (
            intersect_lines(r3, r4, start, end),
            (r3 - r4).perp().normalize(),
        ),
        (
            intersect_lines(r2, r4, start, end),
            (r2 - r4).perp().normalize(),
        ),
    ];
    let cmp = |a: &(Vec2, Vec2), b: &(Vec2, Vec2)| {
        (a.0 - start)
            .length()
            .partial_cmp(&(b.0 - start).length())
            .unwrap()
    };
    let min = ints
        .iter()
        .filter_map(|(i, n)| i.map(|i| (i, *n)))
        .min_by(cmp);
    let max = ints
        .iter()
        .filter_map(|(i, n)| i.map(|i| (i, *n)))
        .max_by(cmp);
    match (min, max) {
        (Some((min_i, min_n)), Some((max_i, max_n))) => Some((min_i, min_n, max_i, max_n)),
        _ => None,
    }
}

/// Get any intersection point between line segments.
/// Note that this function always detects endpoint-to-endpoint intersections.
/// Most of this is from <https://stackoverflow.com/a/565282>
///
/// Cut and pasted from https://github.com/eadf/intersect2d.rs/blob/main/src/lib.rs
pub fn intersect_lines(a_start: Vec2, a_end: Vec2, b_start: Vec2, b_end: Vec2) -> Option<Vec2> {
    {
        // AABB tests
        if a_end.x > b_end.x && a_end.x > b_start.x && a_start.x > b_end.x && a_start.x > b_start.x
        {
            return None;
        }
        if a_end.x < b_end.x && a_end.x < b_start.x && a_start.x < b_end.x && a_start.x < b_start.x
        {
            return None;
        }
        if a_end.y > b_end.y && a_end.y > b_start.y && a_start.y > b_end.y && a_start.y > b_start.y
        {
            return None;
        }
        if a_end.y < b_end.y && a_end.y < b_start.y && a_start.y < b_end.y && a_start.y < b_start.y
        {
            return None;
        }
    }
    let p = a_start;
    let q = b_start;
    let r = a_end - p;
    let s = b_end - q;

    let r_cross_s = r.perp_dot(s);
    let q_minus_p = q - p;
    let q_minus_p_cross_r = q_minus_p.perp_dot(r);

    // If r × s = 0 then the two lines are parallel
    if ulps_eq!(r_cross_s, 0.0) {
        // one (or both) of the lines may be a point
        let a_is_a_point = ulps_eq_vecs(a_start, a_end);
        let b_is_a_point = ulps_eq_vecs(b_start, b_end);
        if a_is_a_point || b_is_a_point {
            if a_is_a_point && b_is_a_point && ulps_eq_vecs(a_start, b_start) {
                return Some(a_start);
            }
            return if a_is_a_point {
                intersect_line_point(b_start, b_end, a_start)
            } else {
                intersect_line_point(a_start, a_end, b_start)
            };
        }

        // If r × s = 0 and (q − p) × r = 0, then the two lines are collinear.
        if ulps_eq!(q_minus_p_cross_r, 0.0) {
            // let r_dot_r = r.dot(r);
            // let r_div_r_dot_r = r / r_dot_r;
            // let s_dot_r = s.dot(r);
            // let t0 = q_minus_p.dot(r_div_r_dot_r);
            // let t1 = t0 + s_dot_r / r_dot_r;

            // TODO: differentiate overlaps instead of just returning point
            Some(a_start)
        } else {
            // If r × s = 0 and (q − p) × r ≠ 0,
            // then the two lines are parallel and non-intersecting.
            None
        }
    } else {
        // the lines are not parallel
        let t = q_minus_p.perp_dot(s / r_cross_s);
        let u = q_minus_p.perp_dot(r / r_cross_s);

        // If r × s ≠ 0 and 0 ≤ t ≤ 1 and 0 ≤ u ≤ 1,
        // the two line segments meet at the point p + t r = q + u s.
        if 0.0 <= t && t <= 1.0 && 0.0 <= u && u <= 1.0 {
            Some(p + r * t)
        } else {
            None
        }
    }
}

/// Get any intersection point between line segment and point.
/// Inspired by <https://stackoverflow.com/a/17590923>
///
/// Cut and pasted from https://github.com/eadf/intersect2d.rs/blob/main/src/lib.rs
pub fn intersect_line_point(line_start: Vec2, line_end: Vec2, point: Vec2) -> Option<Vec2> {
    // take care of end point equality
    if ulps_eq!(line_start.x, point.x) && ulps_eq!(line_start.y, point.y) {
        return Some(point);
    }
    if ulps_eq!(line_end.x, point.x) && ulps_eq!(line_end.y, point.y) {
        return Some(point);
    }

    let x1 = line_start.x;
    let x2 = line_end.x;
    let y1 = line_start.y;
    let y2 = line_end.y;
    let x = point.x;
    let y = point.y;

    let ab = ((x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1)).sqrt();
    let ap = ((x - x1) * (x - x1) + (y - y1) * (y - y1)).sqrt();
    let pb = ((x2 - x) * (x2 - x) + (y2 - y) * (y2 - y)).sqrt();

    if ulps_eq!(ab, ap + pb) {
        return Some(point);
    }
    None
}

pub fn ulps_eq_vecs(v1: Vec2, v2: Vec2) -> bool {
    ulps_eq!(v1.x, v2.x) && ulps_eq!(v1.y, v2.y)
}
