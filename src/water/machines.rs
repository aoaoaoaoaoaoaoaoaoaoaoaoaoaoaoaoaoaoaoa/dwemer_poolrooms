use std::time::{Duration, Instant};

use super::gpu;

const RAFT_RATE: f32 = 1.7;
const RAFT_RISE: Duration = Duration::from_millis(70);
const RAFT_SINK_TAU: f32 = 0.5;
const RAFT_PEAK_MIN: f32 = 13.0;
const RAFT_PEAK_SPAN: f32 = 10.0;
const DRAIN_COLS: usize = 3;
const DRAIN_ROWS: usize = 2;
const DRAIN_CELLS: usize = DRAIN_COLS * DRAIN_ROWS;
const DRAIN_RATE: f32 = 1.15;
const DRAIN_AMP_MIN: f32 = -0.52;
const DRAIN_AMP_SPAN: f32 = -0.48;

pub(super) struct DrainPulse {
    pub rect: egui::Rect,
    pub amp: f32,
}

/// A bilinear high-tension membrane pulled by four independent Poisson pistons.
pub(super) struct LoadingRaft {
    rect: egui::Rect,
    corners: [RaftPiston; 4],
    rng: u64,
    visible: bool,
}

impl LoadingRaft {
    pub fn new() -> Self {
        let now = Instant::now();
        let mut raft = Self {
            rect: egui::Rect::NOTHING,
            corners: [RaftPiston::new(now); 4],
            rng: 0x2b99_2751_d6e8_4d31,
            visible: false,
        };
        for slot in 0..raft.corners.len() {
            let wait = raft.wait();
            raft.corners[slot].next = now + Duration::from_secs_f32(wait);
        }
        raft
    }

    pub fn show(&mut self, ctx: &egui::Context, rect: egui::Rect) {
        self.visible = true;
        self.rect = rect;
        self.tick(Instant::now());
        ctx.request_repaint_after(Duration::from_millis(16));
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn source(
        &mut self,
        ctx: &egui::Context,
        pixels_per_point: f32,
        amplitude: f32,
    ) -> Option<gpu::Raft> {
        if !self.visible {
            return None;
        }
        self.tick(Instant::now());
        ctx.request_repaint_after(Duration::from_millis(16));
        Some(gpu::Raft {
            rect: scale_rect(self.rect, pixels_per_point),
            corners: self
                .corners
                .map(|corner| corner.height() * pixels_per_point * amplitude),
        })
    }

    fn tick(&mut self, now: Instant) {
        for slot in 0..self.corners.len() {
            if now < self.corners[slot].next {
                continue;
            }
            let peak = RAFT_PEAK_MIN + self.unit() * RAFT_PEAK_SPAN;
            self.corners[slot].fire(now, peak);
            let wait = self.wait();
            self.corners[slot].next = now + Duration::from_secs_f32(wait);
        }
    }

    fn wait(&mut self) -> f32 {
        -(1.0 - self.unit()).ln() / RAFT_RATE
    }

    fn unit(&mut self) -> f32 {
        self.rng ^= self.rng << 7;
        self.rng ^= self.rng >> 9;
        self.rng ^= self.rng << 8;
        ((self.rng >> 40) as u32 as f32 + 0.5) / 16_777_216.0
    }
}

#[derive(Clone, Copy)]
struct RaftPiston {
    fired: Instant,
    next: Instant,
    base: f32,
    peak: f32,
}

impl RaftPiston {
    fn new(now: Instant) -> Self {
        Self {
            fired: now,
            next: now,
            base: 0.0,
            peak: 0.0,
        }
    }

    fn fire(&mut self, now: Instant, peak: f32) {
        self.base = self.height_at(now);
        self.peak = peak;
        self.fired = now;
    }

    fn height(self) -> f32 {
        self.height_at(Instant::now())
    }

    fn height_at(self, now: Instant) -> f32 {
        let age = now.saturating_duration_since(self.fired);
        if age <= RAFT_RISE {
            let t = age.as_secs_f32() / RAFT_RISE.as_secs_f32();
            return self.base + (self.peak - self.base) * t;
        }
        let sink = age.saturating_sub(RAFT_RISE).as_secs_f32();
        self.peak * (-sink / RAFT_SINK_TAU).exp()
    }
}

/// A quiet six-cell drain field with independent Poisson clocks.
pub(super) struct EmptyDrain {
    cells: [Instant; DRAIN_CELLS],
    rng: u64,
    visible: bool,
}

impl EmptyDrain {
    pub fn new() -> Self {
        let now = Instant::now();
        let mut drain = Self {
            cells: [now; DRAIN_CELLS],
            rng: 0x83f6_1d05_a04d_e357,
            visible: false,
        };
        for slot in 0..DRAIN_CELLS {
            drain.cells[slot] = now + Duration::from_secs_f32(drain.wait());
        }
        drain
    }

    pub fn show(&mut self, ctx: &egui::Context, rect: egui::Rect) -> Vec<DrainPulse> {
        self.visible = true;
        let now = Instant::now();
        let mut pulses = Vec::new();
        for slot in 0..DRAIN_CELLS {
            if now < self.cells[slot] {
                continue;
            }
            pulses.push(DrainPulse {
                rect: drain_cell(rect, slot),
                amp: DRAIN_AMP_MIN + self.unit() * DRAIN_AMP_SPAN,
            });
            self.cells[slot] = now + Duration::from_secs_f32(self.wait());
        }
        ctx.request_repaint_after(Duration::from_millis(33));
        pulses
    }

    pub fn hide(&mut self) {
        if !self.visible {
            return;
        }
        self.visible = false;
        let now = Instant::now();
        for slot in 0..DRAIN_CELLS {
            self.cells[slot] = now + Duration::from_secs_f32(self.wait());
        }
    }

    fn wait(&mut self) -> f32 {
        -(1.0 - self.unit()).ln() / DRAIN_RATE
    }

    fn unit(&mut self) -> f32 {
        self.rng ^= self.rng << 7;
        self.rng ^= self.rng >> 9;
        self.rng ^= self.rng << 8;
        ((self.rng >> 40) as u32 as f32 + 0.5) / 16_777_216.0
    }
}

fn drain_cell(rect: egui::Rect, slot: usize) -> egui::Rect {
    let col = slot % DRAIN_COLS;
    let row = slot / DRAIN_COLS;
    let cell = egui::vec2(
        rect.width() / DRAIN_COLS as f32,
        rect.height() / DRAIN_ROWS as f32,
    );
    let min = rect.min + egui::vec2(col as f32 * cell.x, row as f32 * cell.y);
    egui::Rect::from_min_size(min, cell).shrink(6.0)
}

pub(super) fn scale_rect(rect: egui::Rect, scale: f32) -> egui::Rect {
    egui::Rect::from_min_max(
        (rect.min.to_vec2() * scale).to_pos2(),
        (rect.max.to_vec2() * scale).to_pos2(),
    )
}
