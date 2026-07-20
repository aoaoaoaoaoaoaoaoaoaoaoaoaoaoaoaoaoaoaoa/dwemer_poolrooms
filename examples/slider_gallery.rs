#![expect(
    unused_crate_dependencies,
    reason = "the gallery deliberately consumes egui and wgpu through the crate's version-locked re-exports"
)]

mod support;

use anyhow::Result;
use dwemer_poolrooms::{
    chrome::{self, Rail},
    egui,
    water::Surface,
};

use support::Exhibit;

struct Sliders {
    free: u16,
    barred: u16,
    ceiling: u16,
}

impl Default for Sliders {
    fn default() -> Self {
        Self {
            free: 4,
            barred: 4,
            ceiling: 6,
        }
    }
}

impl Exhibit for Sliders {
    const TITLE: &'static str = "Poolrooms · slider gallery";
    const SIZE: [f64; 2] = [720.0, 390.0];

    fn ui(&mut self, ui: &mut egui::Ui, water: &mut Surface) {
        let _panel = egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(chrome::PAGE).inner_margin(28))
            .show_inside(ui, |ui| {
                let _title = ui.label(chrome::title("POOLROOMS SLIDER"));
                let _law = ui.label(chrome::muted(
                    "fixed guide · offset slider-crank · swept-volume water coupling",
                ));
                ui.add_space(24.0);

                let _label = ui.horizontal(|ui| {
                    let _name = ui.label(chrome::eyebrow("UNRESTRICTED TRAVEL"));
                    let _value =
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let _value = ui.label(chrome::muted(format!("{}", self.free)));
                        });
                });
                let rail = Rail::new(&mut self.free, 0..=10)
                    .detents(11)
                    .width(ui.available_width())
                    .show(ui);
                water.rail(&rail);

                ui.add_space(28.0);
                let _label = ui.horizontal(|ui| {
                    let _name = ui.label(chrome::eyebrow("DYNAMIC ADMISSIBLE TRAVEL"));
                    let _value =
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let _value = ui.label(chrome::muted(format!(
                                "{}  ∈  0..{}",
                                self.barred, self.ceiling
                            )));
                        });
                });
                let rail = Rail::new(&mut self.barred, 0..=10)
                    .allowed(0..=self.ceiling)
                    .detents(11)
                    .width(ui.available_width())
                    .show(ui);
                water.rail(&rail);

                ui.add_space(15.0);
                let _gate = ui.horizontal(|ui| {
                    let _caption = ui.label(chrome::muted("external stop"));
                    if chrome::glyph(ui, "−", false).clicked() {
                        self.ceiling = self.ceiling.saturating_sub(1);
                    }
                    if chrome::glyph(ui, "+", false).clicked() {
                        self.ceiling = self.ceiling.saturating_add(1).min(10);
                    }
                });
            });
    }
}

fn main() -> Result<()> {
    support::run(Sliders::default())
}
