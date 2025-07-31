use egui_backend::GuiFn;

pub(crate) mod egui_backend;
pub(crate) mod egui_sdl3_event;

pub fn create_gui() -> Box<GuiFn> {
    #[derive(PartialEq)]
    enum Enum {
        First,
        Second,
        Third,
    }

    let mut my_string = String::new();
    let mut my_f32 = 0.0f32;
    let mut my_boolean = false;
    let mut my_enum = Enum::First;
    Box::new(move |ctx| {
        egui::SidePanel::left("my_left_panel")
            .frame(egui::Frame {
                inner_margin: egui::Margin::symmetric(4, 4),
                fill: egui::Color32::from_black_alpha(200),
                stroke: egui::Stroke::NONE,
                corner_radius: egui::CornerRadius::ZERO,
                outer_margin: egui::Margin::ZERO,
                shadow: egui::Shadow::NONE,
            })
            .show(ctx, |ui| {
                ui.label("This is a label");
                ui.hyperlink("https://github.com/emilk/egui");
                ui.text_edit_singleline(&mut my_string);
                if ui.button("Click me").clicked() {
                    println!("Clicked!!");
                }
                ui.add(egui::Slider::new(&mut my_f32, 0.0..=100.0));
                ui.add(egui::DragValue::new(&mut my_f32));

                ui.checkbox(&mut my_boolean, "Checkbox");

                ui.horizontal(|ui| {
                    ui.radio_value(&mut my_enum, Enum::First, "First");
                    ui.radio_value(&mut my_enum, Enum::Second, "Second");
                    ui.radio_value(&mut my_enum, Enum::Third, "Third");
                });

                ui.separator();

                ui.collapsing("Click to see what is hidden!", |ui| {
                    ui.label("Not much, as it turns out");
                });
            });
    })
}
