use core::f32;

use egui_backend::GuiFn;

use crate::{event_loop::SharedState, game::GameState};

pub(crate) mod egui_backend;
pub(crate) mod egui_sdl3_event;

fn fade_in(t: f32, delay: f32, fade_in_time: f32) -> f32 {
    ((t - delay) / fade_in_time).clamp(0.0, 1.0).powi(3)
}

fn fade_in_out(t: f32, delay: f32, fade_in_time: f32, hold_time: f32, fade_out_time: f32) -> f32 {
    ((t - delay) / fade_in_time).clamp(0.0, 1.0).powi(3)
        - (1.0
            - (1.0 - ((t - (fade_in_time + delay + hold_time)) / fade_out_time).clamp(0.0, 1.0))
                .powi(3))
}

pub fn create_gui() -> Box<GuiFn> {
    let mut temp_gui_scale = 1.5;

    let mut main_menu = move |ctx: &egui::Context, shared_state: &mut SharedState| {
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
                let total_height = ui.available_height();
                let big_font_size = shared_state.gui_scale * total_height / 18.0;

                let elapsed = (std::time::Instant::now() - shared_state.game_state_change_time())
                    .as_secs_f32();

                if shared_state.previous_game_state() == GameState::Splash {
                    ui.set_opacity(fade_in(elapsed, 0.3, 1.0));
                }

                {
                    // The rect you want to fill (e.g. entire content area)
                    let target_rect = ui.ctx().content_rect();

                    let image = egui::Image::new(egui::include_image!("../assets/beach.png"));

                    // Get texture dimensions
                    let tex_size = image
                        .load_and_calc_size(ui, egui::Vec2::splat(f32::INFINITY))
                        .unwrap_or(egui::Vec2::splat(1.0));

                    let image_aspect = tex_size.x / tex_size.y;
                    let rect_aspect = target_rect.width() / target_rect.height();

                    // Compute UV rect that crops to maintain aspect
                    let uv_rect = if rect_aspect > image_aspect {
                        // The rect is wider than the image → crop horizontally
                        let crop = 0.5 * (1.0 - image_aspect / rect_aspect);
                        egui::Rect::from_min_max(egui::pos2(0.0, crop), egui::pos2(1.0, 1.0 - crop))
                    } else {
                        // The rect is taller than the image → crop vertically
                        let crop = 0.5 * (1.0 - rect_aspect / image_aspect);
                        egui::Rect::from_min_max(egui::pos2(crop, 0.0), egui::pos2(1.0 - crop, 1.0))
                    };

                    // Draw cropped, aspect-preserving image
                    image.uv(uv_rect).paint_at(ui, target_rect);
                }

                ui.add_space(ui.available_height() * 0.15);

                ui.scope(|ui| {
                    ui.style_mut().text_styles.insert(
                        egui::TextStyle::Body,
                        egui::FontId::new(big_font_size, egui::FontFamily::Proportional),
                    );

                    ui.vertical_centered(|ui| ui.add(egui::Label::new("Sole Searching")));
                });

                ui.add_space(ui.available_height() * 0.2);

                ui.style_mut().text_styles.insert(
                    egui::TextStyle::Button,
                    egui::FontId::new(big_font_size, egui::FontFamily::Proportional),
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

                    if ui
                        .add(
                            egui::Slider::new(&mut temp_gui_scale, 0.5..=2.0)
                                .text("Gui Scale")
                                .show_value(false)
                                .logarithmic(true)
                                .update_while_editing(false),
                        )
                        .drag_stopped()
                    {
                        shared_state.gui_scale = temp_gui_scale;
                    };
                });
            });
    };

    let playing_menu = move |ctx: &egui::Context, shared_state: &mut SharedState| {
        ctx.input(|input| {
            if input.key_pressed(egui::Key::Q) {
                shared_state.set_game_state(GameState::Menu);
            }
        });

        egui::CentralPanel::default()
            .frame(egui::Frame {
                inner_margin: egui::Margin::symmetric(4, 4),
                fill: egui::Color32::from_black_alpha(0),
                stroke: egui::Stroke::NONE,
                corner_radius: egui::CornerRadius::ZERO,
                outer_margin: egui::Margin::ZERO,
                shadow: egui::Shadow::NONE,
            })
            .show(ctx, |ui| {
                let big_font_size = shared_state.gui_scale * ui.available_height() / 32.0;

                if shared_state.winner {
                    ui.set_opacity(1.0);
                } else {
                    ui.set_opacity(0.0);
                }
                
                ui.add_space(ui.available_height() * 0.4);
                ui.scope(|ui| {
                    ui.style_mut().text_styles.insert(
                        egui::TextStyle::Body,
                        egui::FontId::new(big_font_size, egui::FontFamily::Proportional),
                    );

                    ui.vertical_centered(|ui| {
                        ui.add(egui::Label::new(
                            "You're Winner!",
                        ))
                    });
                });
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
                // Load image early so it's avaliable in time for the main menu
                //let _image = egui::Image::new(egui::include_image!("../test-objects/beach.png")).load_for_size(ctx, egui::Vec2::new(f32::INFINITY, f32::INFINITY));

                let big_font_size = shared_state.gui_scale * ui.available_height() / 32.0;

                let elapsed = (std::time::Instant::now() - shared_state.game_state_change_time())
                    .as_secs_f32();

                let delay_time = 1.0;
                let fade_in_time = 2.0;
                let hold_time = 1.0;
                let fade_out_time = 1.0;

                ui.set_opacity(fade_in_out(
                    elapsed,
                    delay_time,
                    fade_in_time,
                    hold_time,
                    fade_out_time,
                ));

                ui.add_space(ui.available_height() * 0.4);
                ui.scope(|ui| {
                    ui.style_mut().text_styles.insert(
                        egui::TextStyle::Body,
                        egui::FontId::new(big_font_size, egui::FontFamily::Proportional),
                    );

                    ui.vertical_centered(|ui| {
                        ui.add(egui::Label::new(
                            "Help Me I'm Stuck In a Blender Holdings Inc.",
                        ))
                    });
                    ui.add_space(ui.available_height() * 0.05);
                    ui.vertical_centered(|ui| ui.add(egui::Label::new("Present")));
                });

                if elapsed > delay_time + fade_in_time + hold_time + fade_out_time {
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
