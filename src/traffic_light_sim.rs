// traffic_light_sim.rs — Traffic light simulation UI
// Uses egui/eframe for rendering; simulation loop runs on a background thread.
// Fuzzy extension values are mocked with random numbers for demonstration.

use std::sync::{Arc, Mutex};
use std::time::Duration;
use eframe::egui;
use egui::{Color32, Pos2, Stroke, Vec2};
use rand::Rng;
use crate::config;
use crate::traffic_light::{TrafficSign, TrafficLight, LightState};
use crate::fuzzy_inference::{get_extension_time};
use crate::call_py_yolo::{count_people, count_cars};
use std::path::Path;

// ---------------------------------------------------------------------------
// Shared simulation state
// ---------------------------------------------------------------------------



/// Top-level phase label shown in the info panel
#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    PedestrianGreen,
    AllRedBeforeVehicle,
    VehicleGreen,
    VehicleYellow,
    AllRedBeforePedestrian,
}

impl Phase {
    pub fn label(&self) -> &str {
        match self {
            Phase::PedestrianGreen     => "Phase 1 — Pedestrian Green",
            Phase::AllRedBeforeVehicle => "All Red — Preparing for Vehicles",
            Phase::VehicleGreen        => "Phase 2 — Vehicle Green",
            Phase::VehicleYellow       => "Vehicle Yellow",
            Phase::AllRedBeforePedestrian => "All Red — Preparing for Pedestrians",
        }
    }
}

#[derive(Clone)]
pub struct SimState {
    pub phase:         Phase,
    pub ped_sign:      TrafficSign,
    pub ped_blinking:  bool,
    pub veh_sign:      TrafficSign,
    pub veh_blinking:  bool,
    /// Seconds remaining in the current sub-phase
    pub countdown:     f64,
    /// Total cycle count
    pub cycle_count:   u32,
    /// Last computed fuzzy extension for pedestrians (seconds)
    pub ped_extension: f64,
    /// Speed multiplier: 0.5, 1.0, 2.0, 4.0
    pub speed:         f64,
    pub paused:        bool,
    /// Blink toggle driven by sim thread
    pub blink_on:      bool,
    // ── Manual fuzzy input (pedestrian only) ─────────────────────────────
    /// Raw text typed in the pedestrian count input box
    pub input_ped_text:  String,
    /// Raw text typed in the vehicle count input box (used as input to fuzzy, not for extension)
    pub input_veh_text:  String,
    /// Last successfully computed fuzzy extension for pedestrians (manual)
    pub manual_ped_ext:  Option<f64>,
    /// Whether the manual extension has been applied to the current phase
    pub manual_applied:  bool,
    /// Set to true to skip the remainder of the current phase immediately
    pub skip_phase:      bool,
}

impl Default for SimState {
    fn default() -> Self {
        Self {
            phase:         Phase::PedestrianGreen,
            ped_sign:      TrafficSign::Green,
            ped_blinking:  false,
            veh_sign:      TrafficSign::Red,
            veh_blinking:  false,
            countdown:     config::PHASE_BASIC_PED_GREEN,
            cycle_count:   0,
            ped_extension: 0.0,
            speed:         1.0,
            paused:        false,
            blink_on:      true,
            input_ped_text: String::new(),
            input_veh_text: String::new(),
            manual_ped_ext: None,
            manual_applied: false,
            skip_phase:     false,
        }
    }
}

// ---------------------------------------------------------------------------
// Simulation loop
// ---------------------------------------------------------------------------

/// Advance simulated time by `sim_secs`, updating `countdown` and `blink_on`
/// in shared state.  Respects `paused` and `speed` changes mid-sleep.
fn sim_sleep(sim_secs: f64, state: &Arc<Mutex<SimState>>) {
    let tick = Duration::from_millis(50);
    let mut simulated_elapsed = 0.0_f64;
    let mut blink_acc         = 0.0_f64;

    loop {
        let (speed, paused, skip) = {
            let s = state.lock().unwrap();
            (s.speed, s.paused, s.skip_phase)
        };

        // Skip: clear flag, re-pause, and exit immediately
        if skip {
            let mut s = state.lock().unwrap();
            s.skip_phase = false;
            s.countdown  = 0.0;
            s.paused     = true;
            break;
        }

        std::thread::sleep(tick);

        if paused {
            continue;
        }

        let delta = tick.as_secs_f64() * speed;
        simulated_elapsed += delta;
        blink_acc         += delta;

        // Toggle blink every 0.5 simulated seconds
        if blink_acc >= 0.5 {
            blink_acc -= 0.5;
            let mut s = state.lock().unwrap();
            s.blink_on = !s.blink_on;
        }

        // Update countdown
        {
            let mut s     = state.lock().unwrap();
            s.countdown   = (sim_secs - simulated_elapsed).max(0.0);
        }

        if simulated_elapsed >= sim_secs {
            break;
        }
    }
}

/// Generate a mock fuzzy extension time (replaces real camera + fuzzy inference).
fn mock_fuzzy_extension(max_ext: f64) -> f64 {
    let mut rng = rand::rng();
    let r: f64  = rng.random();
    (r * r * max_ext).min(max_ext)
}

pub fn run_sim_loop(state: Arc<Mutex<SimState>>) {
    loop {
        // ── Phase 1: Pedestrian Green ─────────────────────────────────────
        let base_ped  = config::PHASE_BASIC_PED_GREEN;
        let half_ped  = base_ped / 2.0;
        // Sequence: half_ped | ext | (half_ped - PED_GREEN_BLINKING) | PED_GREEN_BLINKING

        {
            let mut s    = state.lock().unwrap();
            s.phase      = Phase::PedestrianGreen;
            s.ped_sign   = TrafficSign::Green;
            s.ped_blinking = false;
            s.veh_sign   = TrafficSign::Red;
            s.veh_blinking = false;
            s.countdown  = half_ped;
            s.blink_on   = true;
        }
        // First half
        sim_sleep(half_ped, &state);

        // Fuzzy inference: use manual input if set, otherwise mock
        let ped_ext = {
            let mut s = state.lock().unwrap();
            if let Some(ext) = s.manual_ped_ext.take() {
                s.ped_extension = ext;
                ext
            } else {
                let ext = mock_fuzzy_extension(config::T_MAX / 4.0);
                s.ped_extension = ext;
                ext
            }
        };

        // Extension
        if ped_ext > 0.1 {
            { state.lock().unwrap().countdown = ped_ext; }
            sim_sleep(ped_ext, &state);
        }

        // Second half minus blinking time
        let second_solid = (half_ped - config::PED_GREEN_BLINKING).max(0.0);
        if second_solid > 0.0 {
            { state.lock().unwrap().countdown = second_solid; }
            sim_sleep(second_solid, &state);
        }

        // Blinking
        {
            let mut s      = state.lock().unwrap();
            s.ped_blinking = true;
            s.countdown    = config::PED_GREEN_BLINKING;
        }
        sim_sleep(config::PED_GREEN_BLINKING, &state);

        // ── All-Red before vehicles ────────────────────────────────────────
        {
            let mut s      = state.lock().unwrap();
            s.phase        = Phase::AllRedBeforeVehicle;
            s.ped_sign     = TrafficSign::Red;
            s.ped_blinking = false;
            s.countdown    = config::ALL_RED;
        }
        sim_sleep(config::ALL_RED, &state);

        // ── Phase 2: Vehicle Green (fixed duration, no fuzzy extension) ──────
        let base_veh  = config::PHASE_BASIC_VEH_GREEN;
        let veh_solid = base_veh - config::VEH_GREEN_BLINKING; // solid green portion only

        {
            let mut s      = state.lock().unwrap();
            s.phase        = Phase::VehicleGreen;
            s.veh_sign     = TrafficSign::Green;
            s.veh_blinking = false;
            s.countdown    = veh_solid;
            s.blink_on     = true;
        }
        sim_sleep(veh_solid, &state);

        // Blinking
        {
            let mut s      = state.lock().unwrap();
            s.veh_blinking = true;
            s.countdown    = config::VEH_GREEN_BLINKING;
        }
        sim_sleep(config::VEH_GREEN_BLINKING, &state);

        // ── Vehicle Yellow ─────────────────────────────────────────────────
        {
            let mut s      = state.lock().unwrap();
            s.phase        = Phase::VehicleYellow;
            s.veh_sign     = TrafficSign::Yellow;
            s.veh_blinking = false;
            s.countdown    = config::VEH_YELLOW;
        }
        sim_sleep(config::VEH_YELLOW, &state);

        // ── All-Red before pedestrians ────────────────────────────────────
        {
            let mut s      = state.lock().unwrap();
            s.phase        = Phase::AllRedBeforePedestrian;
            s.veh_sign     = TrafficSign::Red;
            s.countdown    = config::ALL_RED;
        }
        sim_sleep(config::ALL_RED, &state);

        // Cycle complete
        { state.lock().unwrap().cycle_count += 1; }
    }
}

// ---------------------------------------------------------------------------
// Helper: draw a single circular light bulb
// ---------------------------------------------------------------------------

fn draw_bulb(painter: &egui::Painter, center: Pos2, radius: f32, lit: bool, color: Color32) {
    let fill = if lit { color } else { Color32::from_rgb(45, 45, 45) };
    painter.circle_filled(center, radius, fill);
    if lit {
        painter.circle_stroke(center, radius, Stroke::new(2.5, color.linear_multiply(1.5)));
        // Simple glow: larger faint circle
        painter.circle_stroke(center, radius + 4.0, Stroke::new(1.0, color.gamma_multiply(0.35)));
    } else {
        painter.circle_stroke(center, radius, Stroke::new(1.0, Color32::from_rgb(70, 70, 70)));
    }
}

// ---------------------------------------------------------------------------
// Draw vehicle traffic light (R / Y / G)
// ---------------------------------------------------------------------------

fn draw_vehicle_light(ui: &mut egui::Ui, snap: &SimState) {
    let r   = 20.0_f32;
    let pad = 10.0_f32;
    let w   = r * 2.0 + pad * 2.0;
    let h   = r * 6.0 + pad * 4.0 + 24.0; // 3 bulbs + label

    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::hover());
    let painter   = ui.painter_at(rect);

    painter.rect_filled(rect, egui::CornerRadius::same(12), Color32::from_rgb(28, 28, 28));
    painter.rect_stroke(rect, egui::CornerRadius::same(12), Stroke::new(2.0, Color32::from_rgb(70, 70, 70)), egui::StrokeKind::Outside);

    let blink_on = snap.blink_on;
    let cx       = rect.center().x;

    let red_lit    = snap.veh_sign == TrafficSign::Red    && (!snap.veh_blinking || blink_on);
    let yellow_lit = snap.veh_sign == TrafficSign::Yellow && (!snap.veh_blinking || blink_on);
    let green_lit  = snap.veh_sign == TrafficSign::Green  && (!snap.veh_blinking || blink_on);

    draw_bulb(&painter, Pos2::new(cx, rect.min.y + pad + r),             r, red_lit,    Color32::from_rgb(220, 40, 40));
    draw_bulb(&painter, Pos2::new(cx, rect.min.y + pad * 2.0 + r * 3.0), r, yellow_lit, Color32::from_rgb(220, 185, 20));
    draw_bulb(&painter, Pos2::new(cx, rect.min.y + pad * 3.0 + r * 5.0), r, green_lit,  Color32::from_rgb(30, 200, 60));

    painter.text(
        Pos2::new(cx, rect.max.y - 16.0),
        egui::Align2::CENTER_CENTER,
        "Veh Sig",
        egui::FontId::proportional(13.0),
        Color32::GRAY,
    );
}

// ---------------------------------------------------------------------------
// Draw pedestrian traffic light (R / G)
// ---------------------------------------------------------------------------

fn draw_pedestrian_light(ui: &mut egui::Ui, snap: &SimState) {
    let r   = 16.0_f32;
    let pad = 8.0_f32;
    let w   = 120.0_f32;
    let h   = r * 4.0 + pad * 3.0 + 24.0; // 2 bulbs + gap + label

    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::hover());
    let painter   = ui.painter_at(rect);

    painter.rect_filled(rect, egui::CornerRadius::same(12), Color32::from_rgb(28, 28, 28));
    painter.rect_stroke(rect, egui::CornerRadius::same(12), Stroke::new(2.0, Color32::from_rgb(70, 70, 70)), egui::StrokeKind::Outside);

    let blink_on = snap.blink_on;
    let cx       = rect.center().x;

    let red_lit   = snap.ped_sign == TrafficSign::Red   && (!snap.ped_blinking || blink_on);
    let green_lit = snap.ped_sign == TrafficSign::Green && (!snap.ped_blinking || blink_on);

    // bulb 1 (red):   centre at pad + r
    // bulb 2 (green): centre at pad * 2 + r * 3  (same spacing as vehicle light)
    draw_bulb(&painter, Pos2::new(cx, rect.min.y + pad + r),              r, red_lit,   Color32::from_rgb(220, 40, 40));
    draw_bulb(&painter, Pos2::new(cx, rect.min.y + pad * 2.0 + r * 3.0), r, green_lit, Color32::from_rgb(30, 200, 60));

    painter.text(
        Pos2::new(cx, rect.max.y - 16.0),
        egui::Align2::CENTER_CENTER,
        "Ped Sig",
        egui::FontId::proportional(13.0),
        Color32::GRAY,
    );
}

// ---------------------------------------------------------------------------
// Info row helper
// ---------------------------------------------------------------------------

fn info_row(ui: &mut egui::Ui, label: &str, value: &str, val_color: Color32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{}:", label)).size(13.0).color(Color32::GRAY));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).size(13.0).color(val_color).strong());
        });
    });
    ui.add_space(2.0);
}

// ---------------------------------------------------------------------------
// eframe App
// ---------------------------------------------------------------------------

struct TrafficLightApp {
    state:       Arc<Mutex<SimState>>,
    traffic_light: TrafficLight,
    /// Local (UI-thread) text buffers for the input boxes
    ui_ped_text: String,
    ui_veh_text: String,
    /// Computed fuzzy result ready to display (None = not yet computed)
    fuzzy_ped_result: Option<f64>,
    /// Error message from bad input
    ped_err: Option<String>,
    /// Selected image paths for YOLO detection
    ped_img_path: Option<String>,
    veh_img_path: Option<String>,
    /// Latest YOLO detection status
    yolo_msg: Option<String>,
}

impl TrafficLightApp {
    fn new(state: Arc<Mutex<SimState>>) -> Self {
        Self {
            state,
            traffic_light: TrafficLight {
                sig_ped: (TrafficSign::Green, LightState::Solid),
                sig_veh: (TrafficSign::Red,   LightState::Solid),
                time: config::PHASE_BASIC_PED_GREEN,
            },
            ui_ped_text: String::new(),
            ui_veh_text: String::new(),
            fuzzy_ped_result: None,
            ped_err: None,
            ped_img_path: None,
            veh_img_path: None,
            yolo_msg: None,
        }
    }
}

impl eframe::App for TrafficLightApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(50));

        let snap = self.state.lock().unwrap().clone();

        // Keep traffic_light state in sync with simulation
        {
            self.traffic_light.sig_ped.0 = snap.ped_sign.clone();
            self.traffic_light.sig_ped.1 = if snap.ped_blinking { LightState::Blinking } else { LightState::Solid };
            self.traffic_light.sig_veh.0 = snap.veh_sign.clone();
            self.traffic_light.sig_veh.1 = if snap.veh_blinking { LightState::Blinking } else { LightState::Solid };
            self.traffic_light.time      = snap.countdown.max(0.0);
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(Color32::from_rgb(16, 16, 22))
                    .inner_margin(egui::Margin::same(14)),
            )
            .show(ctx, |ui| {
                ui.visuals_mut().override_text_color = Some(Color32::WHITE);

                // ── Header ───────────────────────────────────────────────────
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("🚦 Traffic Light Control System Simulation")
                            .size(22.0)
                            .color(Color32::WHITE)
                            .strong(),
                    );
                    let phase_color = match snap.phase {
                        Phase::PedestrianGreen        => Color32::from_rgb(60, 210, 80),
                        Phase::VehicleGreen           => Color32::from_rgb(60, 180, 255),
                        Phase::VehicleYellow          => Color32::from_rgb(230, 190, 20),
                        Phase::AllRedBeforeVehicle
                        | Phase::AllRedBeforePedestrian => Color32::from_rgb(210, 70, 70),
                    };
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(snap.phase.label())
                            .size(17.0)
                            .color(phase_color)
                            .strong(),
                    );
                });
                ui.add_space(10.0);

                // ── Main content: horizontal-first layout ───────────────────
                egui::ScrollArea::both().id_salt("main_content_scroll").show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        // Left side: lights + runtime info
                        ui.vertical(|ui| {

                            egui::Frame::default()
                                .fill(Color32::from_rgb(22, 22, 34))
                                .corner_radius(egui::CornerRadius::same(10))
                                .inner_margin(egui::Margin::same(12))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(55, 55, 80)))
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new("🚥 Signals & Runtime")
                                            .size(14.0)
                                            .color(Color32::LIGHT_GRAY)
                                            .strong(),
                                    );
                                    ui.add_space(8.0);

                                    ui.horizontal_top(|ui| {
                                        ui.vertical(|ui| {
                                            ui.set_min_width(70.0);
                                            draw_vehicle_light(ui, &snap);
                                        });

                                        ui.add_space(16.0);

                                        ui.vertical(|ui| {
                                            ui.set_min_width(125.0);
                                            draw_pedestrian_light(ui, &snap);
                                        });

                                        ui.add_space(16.0);

                                        ui.vertical(|ui| {
                                            ui.set_min_width(200.0);
                                            ui.set_max_width(350.0);
                                            egui::Frame::default()
                                                .fill(Color32::from_rgb(26, 26, 36))
                                                .corner_radius(egui::CornerRadius::same(10))
                                                .inner_margin(egui::Margin::same(12))
                                                .stroke(Stroke::new(1.0, Color32::from_rgb(55, 55, 80)))
                                                .show(ui, |ui| {
                                                    ui.set_min_width(220.0);
                                                    ui.label(
                                                        egui::RichText::new("📊 Runtime Info")
                                                            .size(14.0)
                                                            .color(Color32::LIGHT_GRAY)
                                                            .strong(),
                                                    );
                                                    ui.separator();
                                                    ui.add_space(4.0);

                                                    info_row(ui, "Completed Cycles", &snap.cycle_count.to_string(), Color32::WHITE);
                                                    ui.add_space(4.0);
                                                    ui.label(egui::RichText::new("Fuzzy Extension").size(12.0).color(Color32::from_rgb(150,150,180)));
                                                    info_row(ui, "  Pedestrian Ext.", &format!("{:.1} s", snap.ped_extension), Color32::from_rgb(80, 210, 100));
                                                    ui.add_space(4.0);
                                                    ui.label(egui::RichText::new("TrafficLight State").size(12.0).color(Color32::from_rgb(150,150,180)));
                                                    info_row(ui, "  Ped Signal", &format!("{:?}/{:?}", self.traffic_light.sig_ped.0, self.traffic_light.sig_ped.1), Color32::from_rgb(80, 210, 100));
                                                    info_row(ui, "  Veh Signal", &format!("{:?}/{:?}", self.traffic_light.sig_veh.0, self.traffic_light.sig_veh.1), Color32::from_rgb(80, 170, 255));
                                                    info_row(ui, "  Time", &format!("{:.1} s", self.traffic_light.time), Color32::LIGHT_GRAY);
                                                    ui.add_space(4.0);
                                                    ui.label(egui::RichText::new("Timing Parameters").size(12.0).color(Color32::from_rgb(150,150,180)));
                                                    info_row(ui, "  Ped. Green", &format!("{} s", config::PHASE_BASIC_PED_GREEN as u32), Color32::LIGHT_GRAY);
                                                    info_row(ui, "  Veh. Green", &format!("{} s", config::PHASE_BASIC_VEH_GREEN as u32), Color32::LIGHT_GRAY);
                                                    info_row(ui, "  Ped. Blink", &format!("{} s", config::PED_GREEN_BLINKING as u32), Color32::LIGHT_GRAY);
                                                    info_row(ui, "  Veh. Blink", &format!("{} s", config::VEH_GREEN_BLINKING as u32), Color32::LIGHT_GRAY);
                                                    info_row(ui, "  Veh. Yellow", &format!("{} s", config::VEH_YELLOW as u32), Color32::LIGHT_GRAY);
                                                    info_row(ui, "  All Red", &format!("{} s", config::ALL_RED as u32), Color32::LIGHT_GRAY);
                                                });
                                        });
                                    });
                                });
                        });

                        ui.add_space(12.0);

                        // Right side: fuzzy + YOLO stacked
                        ui.vertical(|ui| {
                            ui.set_min_width(460.0);

                            egui::Frame::default()
                                .fill(Color32::from_rgb(22, 22, 34))
                                .corner_radius(egui::CornerRadius::same(10))
                                .inner_margin(egui::Margin::same(12))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(55, 55, 90)))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("🧮 Manual Fuzzy Inference Input")
                                                .size(14.0)
                                                .color(Color32::LIGHT_GRAY)
                                                .strong(),
                                        );
                                        ui.add_space(12.0);
                                        let cd_text = format!("⏱ {:.1}s", snap.countdown.max(0.0));
                                        ui.label(egui::RichText::new(cd_text).size(20.0).color(Color32::WHITE).strong());
                                        if snap.veh_blinking {
                                            ui.label(egui::RichText::new("🟢 Veh Blinking").size(12.0).color(Color32::YELLOW));
                                        } else if snap.ped_blinking {
                                            ui.label(egui::RichText::new("🟢 Ped Blinking").size(12.0).color(Color32::YELLOW));
                                        }
                                    });
                                    ui.label(
                                        egui::RichText::new(
                                            "Enter pedestrian and vehicle counts to compute the pedestrian green extension. Vehicle green remains fixed.",
                                        )
                                        .size(11.0)
                                        .color(Color32::DARK_GRAY),
                                    );
                                    ui.add_space(8.0);

                                    ui.horizontal_top(|ui| {
                                        ui.vertical(|ui| {
                                            ui.set_min_width(150.0);
                                            ui.label(egui::RichText::new("🚶 Pedestrian Count").size(13.0).color(Color32::from_rgb(80, 210, 100)));
                                            let ped_edit = egui::TextEdit::singleline(&mut self.ui_ped_text)
                                                .hint_text("e.g. 50")
                                                .desired_width(120.0);
                                            ui.add(ped_edit);
                                            if let Some(ref err) = self.ped_err {
                                                ui.label(egui::RichText::new(err).size(11.0).color(Color32::from_rgb(230, 80, 80)));
                                            }
                                            if let Some(result) = self.fuzzy_ped_result {
                                                ui.label(
                                                    egui::RichText::new(format!("→ Extension: {:.1} s", result))
                                                        .size(13.0)
                                                        .color(Color32::from_rgb(80, 210, 100))
                                                        .strong(),
                                                );
                                                let queued = self.state.lock().unwrap().manual_ped_ext.is_some();
                                                if queued {
                                                    ui.label(egui::RichText::new("⏳ Will apply in next Ped. phase").size(11.0).color(Color32::YELLOW));
                                                }
                                            }
                                        });

                                        ui.add_space(20.0);

                                        ui.vertical(|ui| {
                                            ui.set_min_width(160.0);
                                            ui.label(egui::RichText::new("🚗 Vehicle Count (input only)").size(13.0).color(Color32::from_rgb(80, 170, 255)));
                                            let veh_edit = egui::TextEdit::singleline(&mut self.ui_veh_text)
                                                .hint_text("e.g. 8")
                                                .desired_width(120.0);
                                            ui.add(veh_edit);
                                            ui.label(egui::RichText::new("Vehicle green time is fixed").size(11.0).color(Color32::DARK_GRAY));
                                        });

                                        ui.add_space(20.0);

                                        ui.vertical(|ui| {
                                            ui.add_space(18.0);
                                            let btn = egui::Button::new(
                                                egui::RichText::new("⚡ Compute & Queue").size(14.0).color(Color32::WHITE),
                                            )
                                            .fill(Color32::from_rgb(70, 100, 200))
                                            .min_size(Vec2::new(145.0, 36.0));

                                            if ui.add(btn).clicked() {
                                                match self.ui_ped_text.trim().parse::<i32>() {
                                                    Ok(ped_count) if ped_count >= 0 => {
                                                        let veh_count = self.ui_veh_text.trim().parse::<i32>().unwrap_or(0).max(0);
                                                        let ext = get_extension_time(ped_count, veh_count);
                                                        self.fuzzy_ped_result = Some(ext);
                                                        self.ped_err = None;
                                                        self.state.lock().unwrap().manual_ped_ext = Some(ext);
                                                    }
                                                    _ => {
                                                        self.ped_err = Some("Invalid count (use ≥0)".to_string());
                                                        self.fuzzy_ped_result = None;
                                                    }
                                                }
                                            }
                                        });
                                    });
                                });

                            ui.add_space(10.0);

                            egui::Frame::default()
                                .fill(Color32::from_rgb(20, 24, 32))
                                .corner_radius(egui::CornerRadius::same(10))
                                .inner_margin(egui::Margin::same(12))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(55, 55, 90)))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("🖼️ Image Inputs & YOLO Detection")
                                            .size(14.0).color(Color32::LIGHT_GRAY).strong());
                                        if let Some(msg) = &self.yolo_msg {
                                            ui.add_space(12.0);
                                            ui.label(egui::RichText::new(msg).size(12.0).color(Color32::from_rgb(180, 200, 255)));
                                        }
                                    });
                                    ui.add_space(8.0);

                                    ui.horizontal_top(|ui| {
                                        ui.vertical(|ui| {
                                            let select_btn = egui::Button::new("📁 Select pedestrian image").min_size(Vec2::new(180.0, 32.0));
                                            if ui.add(select_btn).clicked() {
                                                if let Some(path) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "webp"]).pick_file() {
                                                    self.ped_img_path = Some(path.display().to_string());
                                                    self.yolo_msg = Some(format!("Ped image selected: {}", Path::new(&path).file_name().and_then(|n| n.to_str()).unwrap_or("unknown")));
                                                }
                                            }
                                            let detect_btn = egui::Button::new("🔎 YOLO detect pedestrians").min_size(Vec2::new(180.0, 32.0));
                                            if ui.add(detect_btn).clicked() {
                                                match self.ped_img_path.clone() {
                                                    Some(p) => match count_people(&p) {
                                                        Ok(cnt) => {
                                                            self.ui_ped_text = cnt.to_string();
                                                            self.yolo_msg = Some(format!("Pedestrians detected: {}", cnt));
                                                            self.ped_err = None;
                                                        }
                                                        Err(e) => {
                                                            self.yolo_msg = Some(format!("Pedestrian detection failed: {}", e));
                                                        }
                                                    },
                                                    None => {
                                                        self.yolo_msg = Some("Select a pedestrian image first".to_string());
                                                    }
                                                }
                                            }
                                            if let Some(path) = &self.ped_img_path {
                                                ui.label(egui::RichText::new(Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or(path)).size(11.0).color(Color32::DARK_GRAY));
                                            }
                                        });

                                        ui.add_space(24.0);

                                        ui.vertical(|ui| {
                                            let select_btn = egui::Button::new("📁 Select vehicle image").min_size(Vec2::new(180.0, 32.0));
                                            if ui.add(select_btn).clicked() {
                                                if let Some(path) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "webp"]).pick_file() {
                                                    self.veh_img_path = Some(path.display().to_string());
                                                    self.yolo_msg = Some(format!("Vehicle image selected: {}", Path::new(&path).file_name().and_then(|n| n.to_str()).unwrap_or("unknown")));
                                                }
                                            }
                                            let detect_btn = egui::Button::new("🔎 YOLO detect vehicles").min_size(Vec2::new(180.0, 32.0));
                                            if ui.add(detect_btn).clicked() {
                                                match self.veh_img_path.clone() {
                                                    Some(p) => match count_cars(&p) {
                                                        Ok(cnt) => {
                                                            self.ui_veh_text = cnt.to_string();
                                                            self.yolo_msg = Some(format!("Vehicles detected: {}", cnt));
                                                        }
                                                        Err(e) => {
                                                            self.yolo_msg = Some(format!("Vehicle detection failed: {}", e));
                                                        }
                                                    },
                                                    None => {
                                                        self.yolo_msg = Some("Select a vehicle image first".to_string());
                                                    }
                                                }
                                            }
                                            if let Some(path) = &self.veh_img_path {
                                                ui.label(egui::RichText::new(Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or(path)).size(11.0).color(Color32::DARK_GRAY));
                                            }
                                        });
                                    });
                                });
                        });
                    });
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(8.0);

                // ── Bottom controls ─────────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("⚡ Speed").size(13.0).color(Color32::LIGHT_GRAY));
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            for &spd in &[0.5_f64, 1.0, 2.0, 4.0] {
                                let label = if spd == 0.5 { "0.5×".to_string() } else { format!("{}×", spd as u32) };
                                let selected = {
                                    let s = self.state.lock().unwrap();
                                    (s.speed - spd).abs() < 0.01
                                };
                                let (fill, text_col) = if selected {
                                    (Color32::from_rgb(50, 130, 240), Color32::WHITE)
                                } else {
                                    (Color32::from_rgb(50, 50, 68), Color32::LIGHT_GRAY)
                                };
                                let btn = egui::Button::new(
                                    egui::RichText::new(&label).size(14.0).color(text_col),
                                )
                                .fill(fill)
                                .min_size(Vec2::new(52.0, 32.0));
                                if ui.add(btn).clicked() {
                                    self.state.lock().unwrap().speed = spd;
                                }
                            }
                        });
                    });

                    ui.add_space(28.0);

                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("▶ Control").size(13.0).color(Color32::LIGHT_GRAY));
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let (lbl, fill) = {
                                let s = self.state.lock().unwrap();
                                if s.paused {
                                    ("▶  Resume", Color32::from_rgb(45, 185, 75))
                                } else {
                                    ("⏸  Pause", Color32::from_rgb(195, 70, 45))
                                }
                            };
                            let btn = egui::Button::new(
                                egui::RichText::new(lbl).size(14.0).color(Color32::WHITE),
                            )
                            .fill(fill)
                            .min_size(Vec2::new(96.0, 32.0));
                            if ui.add(btn).clicked() {
                                let mut s = self.state.lock().unwrap();
                                s.paused = !s.paused;
                            }

                            if snap.paused {
                                let skip_btn = egui::Button::new(
                                    egui::RichText::new("⏭  Skip Phase").size(14.0).color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(160, 100, 20))
                                .min_size(Vec2::new(120.0, 32.0));
                                if ui.add(skip_btn).clicked() {
                                    let mut s = self.state.lock().unwrap();
                                    s.skip_phase = true;
                                    s.paused = false;
                                }
                            }
                        });
                    });
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(4.0);

                ui.label(
                    egui::RichText::new(
                        "UI mostly by Claude Sonnet 4.6, logic by Longtail Amethyst Eralbrunia and Demitail Amethyst Eralbrunia",
                    )
                    .size(10.0)
                    .color(Color32::DARK_GRAY),
                );
            });
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn run() {
    let state = Arc::new(Mutex::new(SimState::default()));

    let state_for_thread = Arc::clone(&state);
    std::thread::spawn(move || {
        run_sim_loop(state_for_thread);
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Traffic Light Control System Simulation")
            .with_inner_size([1280.0, 650.0])
            .with_min_inner_size([1280.0, 650.0])
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "Traffic Light Control System Simulation",
        options,
        Box::new(move |_cc| Ok(Box::new(TrafficLightApp::new(Arc::clone(&state))))),
    )
    .expect("Failed to start eframe application");
}
