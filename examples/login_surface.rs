#![expect(
    unused_crate_dependencies,
    reason = "the example deliberately consumes egui and wgpu through the crate's version-locked re-exports"
)]

use dwemer_poolrooms::{
    chrome, egui,
    water::{Domain, Poke, Surface, Wetness},
};

fn main() {
    let ctx = egui::Context::default();
    chrome::install(&ctx);
    let mut water = Surface::new(Wetness::Wet);
    let mut account = "name@example.com".to_owned();
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(420.0, 240.0),
            )),
            ..egui::RawInput::default()
        },
        |ui| {
            water.begin(Domain::shelf(ui.max_rect()));
            let _title = ui.label(chrome::title("REMINDERS LOGIN"));
            let _account = ui.text_edit_singleline(&mut account);
            let login = chrome::glyph(ui, "LOGIN", false);
            water.hover("login", login.rect);
            if login.clicked() {
                water.click(login.rect);
            }
        },
    );
    water.poke(
        egui::Rect::from_center_size(egui::pos2(210.0, 120.0), egui::vec2(80.0, 28.0)),
        Poke::ring(1.0),
    );
    let frame = water.frame(&ctx, output.pixels_per_point, &[], None);
    assert!(frame.live());
}
