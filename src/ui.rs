use macroquad::prelude::*;

use crate::{
    get_width_height,
    sounds::play,
    sprite::{anim_rects, SpriteMap, SpriteSheet},
    text_helpers::{draw_text_aligned, TextAlign},
};

#[derive(Clone, Copy)]
enum ButtonAction {
    StartGame(i32),
    GoToControls,
    GoToLevelSelect,
    GoHome,
    DisableUi,
    Quit,
}

enum UiState {
    Disabled,
    MainMenu,
    Controls,
    LevelSelect,
    PopUp(String),
}

pub enum UiAction {
    None,
    Quit,
    SwitchLevel(String),
}

pub struct Button {
    rect: Rect,
    action: ButtonAction,
}

pub struct Ui {
    pub camera: Camera2D,
    width: f32,
    height: f32,
    smap: SpriteMap,
    font: Option<Font>,
    buttons: Vec<Button>,
    selected_button: usize,
    ignore_mousepos: Option<Vec2>,
    state: UiState,
    levels: Vec<(String, RenderTarget)>,
    pub curr_level: usize,
}

impl Ui {
    pub fn resize(&mut self) {
        let (width, height) = get_width_height(512.0);
        self.width = width;
        self.height = height;
        self.camera =
            Camera2D::from_display_rect(Rect::new(0.0, self.height, self.width, -self.height));
    }
    pub async fn new(levels: Vec<(String, RenderTarget)>) -> Self {
        // TODO: resize camera
        let (width, height) = get_width_height(512.0);
        let camera = Camera2D::from_display_rect(Rect::new(0.0, height, width, -height));
        let ui_sheet = SpriteSheet::from_texture_path("assets/ui.png").await;
        let ui_anims = vec![
            (
                "container1_topleft".to_owned(),
                anim_rects(Rect::new(0.0 * 32.0, 14.0 * 32.0, 32.0, 32.0), 0.0, 1),
                0.1,
            ),
            (
                "container1_topmid".to_owned(),
                anim_rects(Rect::new(1.0 * 32.0, 14.0 * 32.0, 32.0, 32.0), 0.0, 1),
                0.1,
            ),
            (
                "container1_topright".to_owned(),
                anim_rects(Rect::new(2.0 * 32.0, 14.0 * 32.0, 32.0, 32.0), 0.0, 1),
                0.1,
            ),
            (
                "container1_leftmid".to_owned(),
                anim_rects(Rect::new(0.0 * 32.0, 15.0 * 32.0, 32.0, 32.0), 0.0, 1),
                0.1,
            ),
            (
                "container1_mid".to_owned(),
                anim_rects(Rect::new(1.0 * 32.0, 15.0 * 32.0, 32.0, 32.0), 0.0, 1),
                0.1,
            ),
            (
                "container1_rightmid".to_owned(),
                anim_rects(Rect::new(2.0 * 32.0, 15.0 * 32.0, 32.0, 32.0), 0.0, 1),
                0.1,
            ),
            (
                "container1_botleft".to_owned(),
                anim_rects(Rect::new(0.0 * 32.0, 16.0 * 32.0, 32.0, 32.0), 0.0, 1),
                0.1,
            ),
            (
                "container1_botmid".to_owned(),
                anim_rects(Rect::new(1.0 * 32.0, 16.0 * 32.0, 32.0, 32.0), 0.0, 1),
                0.1,
            ),
            (
                "container1_botright".to_owned(),
                anim_rects(Rect::new(2.0 * 32.0, 16.0 * 32.0, 32.0, 32.0), 0.0, 1),
                0.1,
            ),
            (
                "container2_topleft".to_owned(),
                anim_rects(
                    Rect::new((3.0 + 0.0) * 32.0, 14.0 * 32.0, 32.0, 32.0),
                    0.0,
                    1,
                ),
                0.1,
            ),
            (
                "container2_topmid".to_owned(),
                anim_rects(
                    Rect::new((3.0 + 1.0) * 32.0, 14.0 * 32.0, 32.0, 32.0),
                    0.0,
                    1,
                ),
                0.1,
            ),
            (
                "container2_topright".to_owned(),
                anim_rects(
                    Rect::new((3.0 + 2.0) * 32.0, 14.0 * 32.0, 32.0, 32.0),
                    0.0,
                    1,
                ),
                0.1,
            ),
            (
                "container2_leftmid".to_owned(),
                anim_rects(
                    Rect::new((3.0 + 0.0) * 32.0, 15.0 * 32.0, 32.0, 32.0),
                    0.0,
                    1,
                ),
                0.1,
            ),
            (
                "container2_mid".to_owned(),
                anim_rects(
                    Rect::new((3.0 + 1.0) * 32.0, 15.0 * 32.0, 32.0, 32.0),
                    0.0,
                    1,
                ),
                0.1,
            ),
            (
                "container2_rightmid".to_owned(),
                anim_rects(
                    Rect::new((3.0 + 2.0) * 32.0, 15.0 * 32.0, 32.0, 32.0),
                    0.0,
                    1,
                ),
                0.1,
            ),
            (
                "container2_botleft".to_owned(),
                anim_rects(
                    Rect::new((3.0 + 0.0) * 32.0, 16.0 * 32.0, 32.0, 32.0),
                    0.0,
                    1,
                ),
                0.1,
            ),
            (
                "container2_botmid".to_owned(),
                anim_rects(
                    Rect::new((3.0 + 1.0) * 32.0, 16.0 * 32.0, 32.0, 32.0),
                    0.0,
                    1,
                ),
                0.1,
            ),
            (
                "container2_botright".to_owned(),
                anim_rects(
                    Rect::new((3.0 + 2.0) * 32.0, 16.0 * 32.0, 32.0, 32.0),
                    0.0,
                    1,
                ),
                0.1,
            ),
            (
                "button_left".to_owned(),
                anim_rects(Rect::new(0.0 * 32.0, 22.0 * 32.0, 32.0, 32.0), 0.0, 1),
                0.1,
            ),
            (
                "button_mid".to_owned(),
                anim_rects(Rect::new(1.0 * 32.0, 22.0 * 32.0, 32.0, 32.0), 0.0, 1),
                0.1,
            ),
            (
                "button_right".to_owned(),
                anim_rects(Rect::new(2.0 * 32.0, 22.0 * 32.0, 32.0, 32.0), 0.0, 1),
                0.1,
            ),
            (
                "button_arrow".to_owned(),
                anim_rects(Rect::new(14.0 * 32.0, 20.0 * 32.0, 32.0, 32.0), 0.0, 1),
                0.1,
            ),
            (
                "right_arrow".to_owned(),
                anim_rects(Rect::new(28.0 * 32.0, 2.0 * 32.0, 32.0, 32.0), 0.0, 1),
                0.1,
            ),
            (
                "left_arrow".to_owned(),
                anim_rects(Rect::new(29.0 * 32.0, 2.0 * 32.0, 32.0, 32.0), 0.0, 1),
                0.1,
            ),
        ];
        let smap = SpriteMap::new(&ui_sheet, &ui_anims);
        Self {
            camera,
            width,
            height,
            smap,
            font: None,
            buttons: Vec::new(),
            selected_button: 0,
            state: UiState::MainMenu,
            levels,
            curr_level: 0,
            ignore_mousepos: None,
        }
    }

    fn dispatch_action(&mut self, action: ButtonAction) -> UiAction {
        self.selected_button = 0;
        match action {
            ButtonAction::StartGame(idx) => {
                self.state = UiState::Disabled;
                return UiAction::SwitchLevel(self.levels[idx as usize].0.clone());
            }
            ButtonAction::GoToControls => self.state = UiState::Controls,
            ButtonAction::GoToLevelSelect => self.state = UiState::LevelSelect,
            ButtonAction::DisableUi => self.state = UiState::Disabled,
            ButtonAction::GoHome => self.state = UiState::MainMenu,
            ButtonAction::Quit => return UiAction::Quit,
        }
        UiAction::None
    }

    pub fn is_enabled(&self) -> bool {
        !matches!(self.state, UiState::Disabled)
    }

    pub fn set_curr_level(&mut self, level: &str) {
        if level == "Menu" {
            self.curr_level = 0;
        } else {
            self.curr_level = self.levels.iter().position(|lev| lev.0 == level).unwrap();
        }
    }

    pub fn tick(&mut self, main_menu: bool) -> UiAction {
        if matches!(self.state, UiState::Disabled) && is_key_pressed(KeyCode::Escape) {
            self.selected_button = 0;
            self.state = UiState::MainMenu;
        } else if !main_menu && !matches!(self.state, UiState::PopUp(_)) && is_key_pressed(KeyCode::Escape) {
            self.state = UiState::Disabled;
        }
        if !self.is_enabled() {
            return UiAction::None;
        }
        let mouse_pos = self.camera.screen_to_world(mouse_position().into());
        if !self
            .ignore_mousepos
            .map(|p| p == mouse_pos)
            .unwrap_or(false)
        {
            if let Some((idx, _)) = self
                .buttons
                .iter()
                .enumerate()
                .find(|(_, b)| b.rect.contains(mouse_pos))
            {
                if self.selected_button != idx {
                    play("menu_tick", 1.0, false);
                }
                self.selected_button = idx;
                self.ignore_mousepos = Some(mouse_pos);
            }
        }

        if is_key_pressed(KeyCode::Up) {
            play("menu_tick", 1.0, false);
            self.selected_button = (self.selected_button as isize - 1 + self.buttons.len() as isize)
                as usize
                % self.buttons.len();
        }
        if is_key_pressed(KeyCode::Down) {
            play("menu_tick", 1.0, false);
            self.selected_button = (self.selected_button + 1) % self.buttons.len();
        }

        if is_key_pressed(KeyCode::Enter)
            || (is_mouse_button_pressed(MouseButton::Left)
                && self.buttons[self.selected_button].rect.contains(mouse_pos))
        {
            play("menu_select", 1.0, false);
            return self.dispatch_action(self.buttons[self.selected_button].action);
        }
        UiAction::None
    }

    fn draw_container(&mut self, rect: Rect, name: &str) {
        assert!(rect.w % 32.0 == 0.0);
        assert!(rect.h % 32.0 == 0.0);
        let topleft = rect.point();
        let topright = topleft + vec2(rect.w, 0.0);
        let botleft = topleft + vec2(0.0, rect.h);
        let botright = topleft + vec2(rect.w, rect.h);

        self.smap
            .get(&format!("{name}_topleft"))
            .draw(topleft - vec2(32.0, 32.0));
        self.smap
            .get(&format!("{name}_topright"))
            .draw(topright - vec2(0.0, 32.0));
        self.smap
            .get(&format!("{name}_botleft"))
            .draw(botleft - vec2(32.0, 0.0));
        self.smap.get(&format!("{name}_botright")).draw(botright);

        let num_rows = (rect.h / 32.0) as i32;
        let num_cols = (rect.w / 32.0) as i32;
        for row in 0..num_rows {
            for col in 0..num_cols {
                if row == 0 {
                    self.smap
                        .get(&format!("{name}_topmid"))
                        .draw(topleft + vec2(col as f32 * 32.0, -32.0));
                } else if row == num_rows - 1 {
                    self.smap
                        .get(&format!("{name}_botmid"))
                        .draw(botleft + vec2(col as f32 * 32.0, 0.0));
                }
                if col == 0 {
                    self.smap
                        .get(&format!("{name}_leftmid"))
                        .draw(topleft + vec2(-32.0, row as f32 * 32.0));
                } else if col == num_cols - 1 {
                    self.smap
                        .get(&format!("{name}_rightmid"))
                        .draw(topright + vec2(0.0, row as f32 * 32.0));
                }

                self.smap
                    .get(&format!("{name}_mid"))
                    .draw(topleft + vec2(col as f32 * 32.0, 32.0 * row as f32));
            }
        }
    }

    pub fn popup(&mut self, text: &str) {
        self.state = UiState::PopUp(text.to_string());
    }

    pub fn draw(&mut self, main_menu: bool) {
        if let UiState::Disabled = self.state {
            return;
        }
        self.buttons.clear();

        let container_height = 384.0;
        let container_width = 416.0;

        let button_width = 192.0;

        let topleft = vec2(
            (self.width - container_width) / 2.0,
            (self.height - container_height) / 2.0,
        );

        match self.state {
            UiState::LevelSelect | UiState::Controls | UiState::MainMenu => {
                self.draw_container(
                    Rect::new(topleft.x, topleft.y, container_width, container_height),
                    "container1",
                );
            }
            _ => {}
        }

        match &self.state {
            UiState::PopUp(text) => {
                let text = text.clone();
                let text_rect = draw_text_aligned(
                    &text,
                    TextAlign::Left,
                    vec2(0.0, 0.0),
                    Some(container_width),
                    true,
                    TextParams {
                        font_size: 24,
                        font: self.font.as_ref(),
                        ..Default::default()
                    },
                );

                let container_height = ((text_rect.h / 32.0) as i32) as f32 * 32.0 + 32.0 * 2.0;
                let container_width = 416.0;
                let topleft = vec2(
                    (self.width - container_width) / 2.0,
                    self.height - container_height - 30.0,
                );

                self.draw_container(
                    Rect::new(topleft.x, topleft.y, container_width, container_height),
                    "container1",
                );

                draw_text_aligned(
                    &text,
                    TextAlign::Left,
                    topleft + vec2(0.0, 20.0),
                    Some(container_width),
                    false,
                    TextParams {
                        font_size: 32,
                        font_scale: 0.75,
                        font: self.font.as_ref(),
                        ..Default::default()
                    },
                );

                let right_arrow_pos =
                    topleft + vec2(container_width - 32.0, container_height - 32.0);
                let right_arrow = Rect::new(right_arrow_pos.x, right_arrow_pos.y, 32.0, 32.0);
                self.smap.get("right_arrow").draw(right_arrow_pos);
                self.buttons.push(Button {
                    rect: right_arrow,
                    action: ButtonAction::DisableUi,
                });
            }
            UiState::LevelSelect => {
                draw_text_aligned(
                    self.levels[self.curr_level].0.as_str(),
                    TextAlign::Center,
                    topleft + vec2(container_width / 2.0, 32.0),
                    None,
                    false,
                    TextParams {
                        font_size: 32,
                        font: self.font.as_ref(),
                        ..Default::default()
                    },
                );

                let margin = vec2(32.0, 96.0);
                let mut in_topleft = topleft + margin;
                in_topleft.y -= 32.0;
                let in_container_rect = Rect::new(
                    in_topleft.x,
                    in_topleft.y,
                    container_width - margin.x * 2.0,
                    container_height - margin.y * 2.0,
                );
                self.draw_container(in_container_rect, "container2");
                draw_texture_ex(
                    &self.levels[self.curr_level].1.texture,
                    in_topleft.x,
                    in_topleft.y,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(in_container_rect.w, in_container_rect.h)),
                        ..Default::default()
                    },
                );

                let left_arrow_pos = in_topleft + vec2(0.0, in_container_rect.h - 32.0);
                let left_arrow = Rect::new(left_arrow_pos.x, left_arrow_pos.y, 32.0, 32.0);
                let right_arrow_pos =
                    in_topleft + vec2(in_container_rect.w - 32.0, in_container_rect.h - 32.0);
                let right_arrow = Rect::new(right_arrow_pos.x, right_arrow_pos.y, 32.0, 32.0);
                self.smap.get("left_arrow").draw(left_arrow_pos);
                self.smap.get("right_arrow").draw(right_arrow_pos);

                let mouse_pos = self.camera.screen_to_world(mouse_position().into());
                if left_arrow.contains(mouse_pos) && is_mouse_button_pressed(MouseButton::Left)
                    || is_key_pressed(KeyCode::Left)
                {
                    play("menu_tick", 1.0, false);
                    self.curr_level = (self.curr_level + self.levels.len() - 1) % self.levels.len();
                }
                if right_arrow.contains(mouse_pos) && is_mouse_button_pressed(MouseButton::Left)
                    || is_key_pressed(KeyCode::Right)
                {
                    play("menu_tick", 1.0, false);
                    self.curr_level = (self.curr_level + 1) % self.levels.len();
                }

                self.draw_button(
                    topleft
                        + vec2(
                            container_width / 2.0 - button_width / 2.0,
                            container_height - 96.0,
                        ),
                    button_width,
                    "Start",
                    ButtonAction::StartGame(self.curr_level as i32),
                );
                self.draw_button(
                    topleft
                        + vec2(
                            container_width / 2.0 - button_width / 2.0,
                            container_height - 48.0,
                        ),
                    button_width,
                    "Back",
                    ButtonAction::GoHome,
                );
            }
            UiState::Controls => {
                draw_text_aligned(
                    "Controls",
                    TextAlign::Center,
                    topleft + vec2(container_width / 2.0, 48.0),
                    None,
                    false,
                    TextParams {
                        font_size: 32,
                        font: self.font.as_ref(),
                        ..Default::default()
                    },
                );
                let xoff = container_width / 5.0;

                draw_text_aligned(
                    "WASD: Movement",
                    TextAlign::Left,
                    topleft + vec2(xoff, 96.0),
                    None,
                    false,
                    TextParams {
                        font_size: 32,
                        font: self.font.as_ref(),
                        ..Default::default()
                    },
                );

                draw_text_aligned(
                    "MB1:  Fire bow",
                    TextAlign::Left,
                    topleft + vec2(xoff, 128.0),
                    None,
                    false,
                    TextParams {
                        font_size: 32,
                        font: self.font.as_ref(),
                        ..Default::default()
                    },
                );

                draw_text_aligned(
                    "E:    Pick up enemy",
                    TextAlign::Left,
                    topleft + vec2(xoff, 160.0),
                    None,
                    false,
                    TextParams {
                        font_size: 32,
                        font: self.font.as_ref(),
                        ..Default::default()
                    },
                );

                draw_text_aligned(
                    "R:    Reset Level",
                    TextAlign::Left,
                    topleft + vec2(xoff, 192.0),
                    None,
                    false,
                    TextParams {
                        font_size: 32,
                        font: self.font.as_ref(),
                        ..Default::default()
                    },
                );

                draw_text_aligned(
                    "ESC:  Open Menu",
                    TextAlign::Left,
                    topleft + vec2(xoff, 224.0),
                    None,
                    false,
                    TextParams {
                        font_size: 32,
                        font: self.font.as_ref(),
                        ..Default::default()
                    },
                );

                self.draw_button(
                    topleft + vec2(container_width / 2.0 - button_width / 2.0, 256.0),
                    button_width,
                    "Back",
                    ButtonAction::GoHome,
                );
            }
            UiState::MainMenu => {
                draw_text_aligned(
                    if main_menu {
                        "Museum Assassin"
                    } else {
                        "Game Paused"
                    },
                    TextAlign::Center,
                    topleft + vec2(container_width / 2.0, 48.0),
                    None,
                    false,
                    TextParams {
                        font_size: 32,
                        font: self.font.as_ref(),
                        ..Default::default()
                    },
                );

                if main_menu {
                    self.draw_button(
                        topleft + vec2(container_width / 2.0 - button_width / 2.0, 96.0),
                        button_width,
                        "New Game",
                        ButtonAction::StartGame(0),
                    );
                } else {
                    self.draw_button(
                        topleft + vec2(container_width / 2.0 - button_width / 2.0, 96.0),
                        button_width,
                        "Resume",
                        ButtonAction::DisableUi,
                    );
                }

                self.draw_button(
                    topleft + vec2(container_width / 2.0 - button_width / 2.0, 160.0),
                    button_width,
                    "Select Level",
                    ButtonAction::GoToLevelSelect,
                );

                self.draw_button(
                    topleft + vec2(container_width / 2.0 - button_width / 2.0, 224.0),
                    button_width,
                    "Controls",
                    ButtonAction::GoToControls,
                );

                self.draw_button(
                    topleft + vec2(container_width / 2.0 - button_width / 2.0, 284.0),
                    button_width,
                    if !main_menu { "Main Menu" } else { "Quit" },
                    ButtonAction::Quit,
                );
            }
            _ => {}
        }
    }

    fn draw_button(&mut self, pos: Vec2, width: f32, text: &str, action: ButtonAction) {
        assert!(width % 32.0 == 0.0);
        self.smap.get("button_left").draw(pos - vec2(32.0, 0.0));
        let num_cols = (width / 32.0) as i32;
        for col in 0..num_cols {
            self.smap
                .get("button_mid")
                .draw(pos + vec2(col as f32 * 32.0, 0.0));
        }
        self.smap.get("button_right").draw(pos + vec2(width, 0.0));

        draw_text_aligned(
            text,
            TextAlign::Center,
            pos + vec2(width / 2.0, 24.0),
            None,
            false,
            TextParams {
                font_size: 32,
                font: self.font.as_ref(),
                color: Color::from_hex(0x3a3a50),
                ..Default::default()
            },
        );

        if self.buttons.len() == self.selected_button as usize {
            self.smap.get("button_arrow").draw(pos + vec2(-32.0, 0.0));
        }

        self.buttons.push(Button {
            rect: Rect::new(pos.x, pos.y, width, 32.0),
            action,
        });
    }
}
