//! Optional semantic anchors for deterministic external UI choreography.

const STORE: &str = "dwemer-poolroom-anchors";

#[derive(Clone, Debug)]
pub struct Anchor {
    pub name: String,
    pub rect: egui::Rect,
}

pub fn record(ui: &egui::Ui, name: impl Into<String>, rect: egui::Rect) {
    ui.ctx().data_mut(|data| {
        data.get_temp_mut_or_default::<Vec<Anchor>>(egui::Id::new(STORE))
            .push(Anchor {
                name: name.into(),
                rect,
            });
    });
}

pub fn take(ctx: &egui::Context) -> Vec<Anchor> {
    ctx.data_mut(|data| data.remove_temp::<Vec<Anchor>>(egui::Id::new(STORE)))
        .unwrap_or_default()
}
