use egui_backend::GuiFn;

use crate::{event_loop::SharedState, game::GameState};

pub(crate) mod egui_backend;
pub(crate) mod egui_sdl3_event;

pub fn create_gui() -> Box<GuiFn> {
    let main_menu = move |ctx: &egui::Context, shared_state: &mut SharedState| {
        egui::CentralPanel::default()
            .frame(egui::Frame {
                inner_margin: egui::Margin::symmetric(4, 4),
                fill: egui::Color32::from_black_alpha(200),
                stroke: egui::Stroke::NONE,
                corner_radius: egui::CornerRadius::ZERO,
                outer_margin: egui::Margin::ZERO,
                shadow: egui::Shadow::NONE,
            })
            .show(ctx, |ui| {
                let elapsed = (std::time::Instant::now() - shared_state.game_state_change_time())
                    .as_secs_f32();

                ui.set_opacity(elapsed.clamp(0.0, 1.0).powi(3));

                ui.add_space(ui.available_height() * 0.2);

                ui.scope(|ui| {
                    ui.style_mut().text_styles.insert(
                        egui::TextStyle::Body,
                        egui::FontId::new(32.0, egui::FontFamily::Proportional),
                    );

                    ui.vertical_centered(|ui| ui.add(egui::Label::new("Sole Searching")));
                });

                ui.add_space(ui.available_height() * 0.2);

                ui.style_mut().text_styles.insert(
                    egui::TextStyle::Button,
                    egui::FontId::new(32.0, egui::FontFamily::Proportional),
                );

                ui.vertical_centered(|ui| {
                    if ui.button("Play").clicked() {
                        shared_state.set_game_state(GameState::Playing);
                        shared_state.last_mouse_position =
                            ctx.input(|inputs| inputs.pointer.latest_pos()).map(|pos| {
                                let pos = pos * shared_state.gui_scale;
                                (pos.x, pos.y)
                            });
                    }
                });

                ui.add_space(ui.available_height() * 0.3);

                ui.columns(3, |columns| {
                    let ui = &mut columns[1];

                    ui.style_mut().spacing.slider_width = ui.available_width();

                    ui.add(
                        egui::Slider::new(&mut shared_state.volume, 0.0..=1.0)
                            .text("Volume")
                            .show_value(false),
                    );
                });
            });
    };

    let playing_menu = move |ctx: &egui::Context, shared_state: &mut SharedState| {
        ctx.input(|input| {
            if input.key_pressed(egui::Key::Q) {
                shared_state.set_game_state(GameState::Menu);
            }
        });
    };

    let splash_screen = move |ctx: &egui::Context, shared_state: &mut SharedState| {
        egui::CentralPanel::default()
            .frame(egui::Frame {
                inner_margin: egui::Margin::symmetric(4, 4),
                fill: egui::Color32::from_black_alpha(200),
                stroke: egui::Stroke::NONE,
                corner_radius: egui::CornerRadius::ZERO,
                outer_margin: egui::Margin::ZERO,
                shadow: egui::Shadow::NONE,
            })
            .show(ctx, |ui| {
                let elapsed = (std::time::Instant::now() - shared_state.game_state_change_time())
                    .as_secs_f32();

                ui.set_opacity(
                    ((elapsed - 0.3) * 0.5).clamp(0.0, 1.0).powi(3)
                        - (1.0 - (1.0 - ((elapsed - 3.0) * 0.3).clamp(0.0, 1.0)).powi(3)),
                );

                ui.add_space(ui.available_height() * 0.4);
                ui.scope(|ui| {
                    ui.style_mut().text_styles.insert(
                        egui::TextStyle::Body,
                        egui::FontId::new(20.0, egui::FontFamily::Proportional),
                    );

                    ui.vertical_centered(|ui| {
                        ui.add(egui::Label::new(
                            "Help Me I'm Stuck In a Blender Holdings Inc.",
                        ))
                    });
                    ui.add_space(ui.available_height() * 0.05);
                    ui.vertical_centered(|ui| ui.add(egui::Label::new("Present")));
                });

                if elapsed > 6.0 {
                    shared_state.set_game_state(GameState::Menu);
                }
            });
    };

    Box::new(move |ctx, shared_state| match shared_state.game_state() {
        GameState::Menu => main_menu(ctx, shared_state),
        GameState::Playing => playing_menu(ctx, shared_state),
        GameState::Splash => splash_screen(ctx, shared_state),
    })
}
