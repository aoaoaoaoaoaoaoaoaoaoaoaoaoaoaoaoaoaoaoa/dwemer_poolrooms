#![expect(
    unused_crate_dependencies,
    reason = "the gallery deliberately consumes egui and wgpu through the crate's version-locked re-exports"
)]

mod support;

use anyhow::Result;
use dwemer_poolrooms::{
    chrome::{self, DateSpool, GregorianDay},
    egui,
    water::Surface,
};

use support::Exhibit;

#[derive(Clone, Copy, Eq, PartialEq)]
struct Day {
    year: i32,
    month: u32,
    day: u32,
}

impl GregorianDay for Day {
    fn ymd(self) -> (i32, u32, u32) {
        (self.year, self.month, self.day)
    }

    fn from_ymd(year: i32, month: u32, day: u32) -> Self {
        Self { year, month, day }
    }
}

struct Dates {
    value: Option<Day>,
}

impl Default for Dates {
    fn default() -> Self {
        Self {
            value: Some(Day {
                year: 2026,
                month: 7,
                day: 20,
            }),
        }
    }
}

impl Exhibit for Dates {
    const TITLE: &'static str = "Poolrooms · date-spool gallery";
    const SIZE: [f64; 2] = [520.0, 310.0];

    fn ui(&mut self, ui: &mut egui::Ui, water: &mut Surface) {
        let _panel = egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(chrome::PAGE).inner_margin(28))
            .show_inside(ui, |ui| {
                let _title = ui.label(chrome::title("DATE TRANSPORT"));
                let _law = ui.label(chrome::muted(
                    "drag or wheel a reel · tape, roller and spring share one motion",
                ));
                ui.add_space(22.0);
                let spool = DateSpool::new(
                    &mut self.value,
                    Day {
                        year: 2026,
                        month: 7,
                        day: 20,
                    },
                    2005..=2027,
                )
                .label("DATE")
                .show(ui, "gallery-date");
                water.date_spool(&spool);
            });
    }
}

fn main() -> Result<()> {
    support::run(Dates::default())
}
