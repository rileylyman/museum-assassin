use macroquad::prelude::*;

pub struct DebugDrawer {
    draw_calls: Vec<Box<dyn Fn() + Send + Sync + 'static>>,
}

impl DebugDrawer {
    pub fn new() -> Self {
        Self {
            draw_calls: Vec::new(),
        }
    }

    pub fn queue(&mut self, f: impl Fn() + Send + Sync + 'static) {
        self.draw_calls.push(Box::new(f));
    }

    pub fn draw(&mut self) {
        for f in self.draw_calls.drain(..) {
            f();
        }
    }

    pub fn clear(&mut self) {
        self.draw_calls.clear();
    }
}

pub fn draw_dotted_line(
    start_x: f32,
    start_y: f32,
    end_x: f32,
    end_y: f32,
    thickness: f32,
    dot_length: f32,
    dot_spacing: f32,
    color: Color,
) {
    let start = vec2(start_x, start_y);
    let end = vec2(end_x, end_y);
    let mut cursor = start;
    let d = (end - start).normalize() * dot_length;
    let spacer = (end - start).normalize() * dot_spacing;
    let original_sign = (end - start).signum();

    while (end - (cursor + d)).signum() == original_sign {
        draw_line(
            cursor.x,
            cursor.y,
            cursor.x + d.x,
            cursor.y + d.y,
            thickness,
            color,
        );
        cursor += d + spacer;
    }
}
