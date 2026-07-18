#[cfg(feature = "water")]
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tension {
    pub id: u64,
    pub rect: egui::Rect,
    pub pointer: egui::Pos2,
    pub grip: f32,
    pub omega: f32,
}

#[cfg(feature = "water")]
pub(crate) fn push(
    ctx: &egui::Context,
    id: u64,
    rect: egui::Rect,
    pointer: egui::Pos2,
    grip: f32,
    omega: f32,
) {
    let tension = Tension {
        id,
        rect,
        pointer,
        grip,
        omega,
    };
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<Vec<Tension>>(egui::Id::new("dwemer-tension-field"))
            .push(tension);
    });
}

#[cfg(not(feature = "water"))]
pub(crate) fn push(
    _ctx: &egui::Context,
    _id: u64,
    _rect: egui::Rect,
    _pointer: egui::Pos2,
    _grip: f32,
    _omega: f32,
) {
}

#[cfg(feature = "water")]
pub(crate) fn take(ctx: &egui::Context) -> Vec<Tension> {
    ctx.data_mut(|data| data.remove_temp::<Vec<Tension>>(egui::Id::new("dwemer-tension-field")))
        .unwrap_or_default()
}
