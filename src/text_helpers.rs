use macroquad::prelude::*;

#[allow(dead_code)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

pub fn draw_text_aligned(
    text: &str,
    align: TextAlign,
    pos: Vec2,
    wrap_width: Option<f32>,
    measure_only: bool,
    params: TextParams,
) -> Rect {
    // Just used for the final rect measurement, not the actual text placing algorithm
    let mut first_line_height = 0.0;
    let mut last_line_height = 0.0;
    let mut max_width = 0.0;

    // These are the alg vars
    let mut start = 0;
    let mut end = 0;
    let mut y_offset = 0.0;
    let max_line_h = 2.0
        + measure_text(
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
            params.font,
            params.font_size,
            params.font_scale,
        )
        .height;

    while start < text.len() {
        if let Some(wrap_width) = wrap_width {
            loop {
                let curr_size = measure_text(
                    &text[start..end],
                    params.font,
                    params.font_size,
                    params.font_scale,
                );
                if curr_size.width > wrap_width {
                    let mut new_end = end;
                    while !text.chars().nth(new_end).unwrap().is_whitespace() && new_end > start {
                        new_end -= 1;
                    }
                    if new_end == start {
                        new_end = end;
                        while !text.chars().nth(new_end).unwrap().is_whitespace()
                            && new_end < text.len()
                        {
                            new_end += 1;
                        }
                    }
                    end = new_end;
                    break;
                }
                if end == text.len() {
                    break;
                }
                end += 1;
            }
        } else {
            end = text.len();
        };

        let size = measure_text(
            &text[start..end],
            params.font,
            params.font_size,
            params.font_scale,
        );
        if size.width > max_width {
            max_width = size.width;
        }
        if start == 0 {
            first_line_height = size.height;
        }
        last_line_height = size.height;
        let x = match align {
            TextAlign::Right => pos.x - size.width,
            TextAlign::Center => pos.x - size.width / 2.0,
            TextAlign::Left => pos.x,
        };

        if !measure_only {
            draw_text_ex(&text[start..end], x, pos.y + y_offset, params.clone());
        }
        start = end + 1;
        end = start;
        y_offset += max_line_h;
    }

    let pos = vec2(
        pos.x
            - match align {
                TextAlign::Center => max_width / 2.0,
                TextAlign::Right => max_width,
                TextAlign::Left => 0.0,
            },
        pos.y - first_line_height,
    );
    let size = vec2(max_width, y_offset + first_line_height - last_line_height);
    Rect {
        x: pos.x,
        y: pos.y,
        w: size.x,
        h: size.y,
    }
}
