// UI mostly build by ChatGPT or Claude Haiku & GitHub Copilot
// traffic_light_sim.rs — Traffic light simulation UI
// Uses egui/eframe for rendering; simulation loop runs on a background thread.

use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use eframe::egui;
use egui::{Color32, Pos2, Stroke, Vec2, Rect};
use crate::config;
use crate::traffic_light::{TrafficSign, TrafficLight, LightState, TrafficLightPosition};
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
    PedestrianGreenExtension,  // Extension time during ped green
    AllRedBeforeVehicle,
    VehicleGreen,
    VehicleYellow,
    AllRedBeforePedestrian,
}

impl Phase {
    pub fn label(&self) -> &str {
        match self {
            Phase::PedestrianGreen     => "Phase 1 — Pedestrian Green",
            Phase::PedestrianGreenExtension => "Phase 1 — Pedestrian Green (Extended)",
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
    pub ped_blinking:  bool,
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
    // ── Traffic light state (source of truth) ────────────────────────────
    pub traffic_light: TrafficLight,
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
    // ── Manual control mode ──────────────────────────────────────────────
    /// Enable manual control: timer frozen, manual phase switching allowed
    pub manual_control_enabled: bool,
    /// Request to switch to next phase (during manual control)
    pub manual_switch_phase: bool,
    /// Current blinking duration during manual phase switch (in seconds)
    pub manual_blink_countdown: f64,
    /// Flag indicating we are exiting manual mode and waiting for all red before enabling auto cycle
    pub exiting_manual_mode: bool,
    /// All Red Hold mode: keeps all lights red until manually disabled
    pub all_red_hold_enabled: bool,
    /// Flag to transition to all red hold after blink sequence
    pub transitioning_to_all_red: bool,
    /// Flag to transition to all red immediately after current blinking phase
    pub all_red_after_blinking: bool,
    /// Flag to add yellow light before all red (for vehicle green -> all red)
    pub need_yellow_before_all_red: bool,
}

impl Default for SimState {
    fn default() -> Self {
        Self {
            phase:         Phase::PedestrianGreen,
            ped_blinking:  false,
            veh_blinking:  false,
            countdown:     config::PHASE_BASIC_PED_GREEN,
            cycle_count:   0,
            ped_extension: 0.0,
            speed:         1.0,
            paused:        false,
            blink_on:      true,
            traffic_light: TrafficLight {
                sig_ped: (TrafficSign::Green, LightState::Solid),
                sig_veh: (TrafficSign::Red, LightState::Solid),
                time: config::PHASE_BASIC_PED_GREEN,
            },
            input_ped_text: String::new(),
            input_veh_text: String::new(),
            manual_ped_ext: None,
            manual_applied: false,
            skip_phase:     false,
            manual_control_enabled: false,
            manual_switch_phase: false,
            manual_blink_countdown: 0.0,
            exiting_manual_mode: false,
            all_red_hold_enabled: false,
            transitioning_to_all_red: false,
            all_red_after_blinking: false,
            need_yellow_before_all_red: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Simulation loop
// ---------------------------------------------------------------------------

/// Advance simulated time by `sim_secs`, updating `countdown` and `blink_on`
/// in shared state. Respects `paused`, `speed`, and `manual_control_enabled` changes mid-sleep.
fn sim_sleep(sim_secs: f64, state: &Arc<Mutex<SimState>>) {
    let tick = Duration::from_millis(50);
    let mut simulated_elapsed = 0.0_f64;
    let mut blink_acc         = 0.0_f64;

    loop {
        let (speed, paused, skip, manual_enabled, transitioning_to_all_red) = {
            let s = state.lock().unwrap();
            (s.speed, s.paused, s.skip_phase, s.manual_control_enabled, s.transitioning_to_all_red)
        };

        // If manual control is enabled, just break (preserve current state)
        if manual_enabled {
            break;
        }

        // If transitioning to all red, break immediately to apply the state change
        if transitioning_to_all_red {
            break;
        }

        // Skip: clear flag and exit immediately to move to next phase
        if skip {
            let mut s = state.lock().unwrap();
            s.skip_phase = false;
            s.countdown  = 0.0;
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

/// Sleep function for manual phase transitions - executes full countdown regardless of manual_control_enabled
/// This is used during manual Next Phase transitions to show proper animations/blinks
fn manual_sim_sleep(sim_secs: f64, state: &Arc<Mutex<SimState>>) {
    let tick = Duration::from_millis(50);
    let mut simulated_elapsed = 0.0_f64;
    let mut blink_acc         = 0.0_f64;

    loop {
        let speed = state.lock().unwrap().speed;

        std::thread::sleep(tick);

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

/// Generate a fuzzy extension time based on pedestrian and vehicle counts.
fn get_fuzzy_extension() -> f64 {
    // Use default counts (0, 0) for automatic cycle
    // In real deployment, this would pull from actual camera/sensor data
    get_extension_time(0, 0)
}

/// Transition to the next phase by executing full transition sequences.
/// This function performs the complete ped->red->veh->green or veh->red->ped->green sequence.
/// Returns early if the user disables manual control mid-transition.
fn manual_phase_transition(state: &Arc<Mutex<SimState>>) {
    // Snapshot current phase and signals to decide if transition is allowed
    let (phase, ped_sig, veh_sig) = {
        let s = state.lock().unwrap();
        (s.phase.clone(), s.traffic_light.sig_ped.0.clone(), s.traffic_light.sig_veh.0.clone())
    };

    // Helper to clear the request flag
    let clear_switch_flag = || {
        state.lock().unwrap().manual_switch_phase = false;
    };

    match (phase, ped_sig, veh_sig) {
        // Pedestrian green -> Vehicle green sequence
        (Phase::PedestrianGreen, TrafficSign::Green, TrafficSign::Red) => {
            // Ped green blinking
            {
                let mut s = state.lock().unwrap();
                s.ped_blinking = true;
                s.countdown = config::PED_GREEN_BLINKING;
                s.traffic_light.set_sig(TrafficLightPosition::Ped1, TrafficSign::Green, LightState::Blinking);
            }
            manual_sim_sleep(config::PED_GREEN_BLINKING, state);

            // Ped to red, all-red interval
            {
                let mut s = state.lock().unwrap();
                s.ped_blinking = false;
                s.phase = Phase::AllRedBeforeVehicle;
                s.countdown = config::ALL_RED;
                s.traffic_light.set_sig(TrafficLightPosition::Ped1, TrafficSign::Red, LightState::Solid);
            }
            manual_sim_sleep(config::ALL_RED, state);

            // Vehicle to green
            {
                let mut s = state.lock().unwrap();
                s.phase = Phase::VehicleGreen;
                s.veh_blinking = false;
                s.countdown = 0.0;
                s.traffic_light.set_sig(TrafficLightPosition::Veh1, TrafficSign::Green, LightState::Solid);
                s.manual_switch_phase = false;
            }
        }

        // Vehicle green -> Pedestrian green sequence
        (Phase::VehicleGreen, TrafficSign::Red, TrafficSign::Green) => {
            // Vehicle green blinking
            {
                let mut s = state.lock().unwrap();
                s.veh_blinking = true;
                s.countdown = config::VEH_GREEN_BLINKING;
                s.traffic_light.set_sig(TrafficLightPosition::Veh1, TrafficSign::Green, LightState::Blinking);
            }
            manual_sim_sleep(config::VEH_GREEN_BLINKING, state);

            // Vehicle yellow solid
            {
                let mut s = state.lock().unwrap();
                s.veh_blinking = false;
                s.phase = Phase::VehicleYellow;
                s.countdown = config::VEH_YELLOW;
                s.traffic_light.set_sig(TrafficLightPosition::Veh1, TrafficSign::Yellow, LightState::Solid);
            }
            manual_sim_sleep(config::VEH_YELLOW, state);

            // All red before pedestrians
            {
                let mut s = state.lock().unwrap();
                s.phase = Phase::AllRedBeforePedestrian;
                s.countdown = config::ALL_RED;
                s.traffic_light.set_sig(TrafficLightPosition::Veh1, TrafficSign::Red, LightState::Solid);
            }
            manual_sim_sleep(config::ALL_RED, state);

            // Pedestrian green
            {
                let mut s = state.lock().unwrap();
                s.phase = Phase::PedestrianGreen;
                s.ped_blinking = false;
                s.countdown = 0.0;
                s.traffic_light.set_sig(TrafficLightPosition::Ped1, TrafficSign::Green, LightState::Solid);
                s.manual_switch_phase = false;
            }
        }

        // Other states: ignore the request
        _ => {
            clear_switch_flag();
        }
    }
}

pub fn run_sim_loop(state: Arc<Mutex<SimState>>) {
    loop {
        // Check current modes
        let (manual_enabled, all_red_enabled) = {
            let s = state.lock().unwrap();
            (s.manual_control_enabled, s.all_red_hold_enabled)
        };

        // All Red Hold mode
        if all_red_enabled {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }

        if manual_enabled {
            // In manual mode, check for phase switch request
            let should_switch = state.lock().unwrap().manual_switch_phase;
            if should_switch {
                manual_phase_transition(&state);
            } else {
                // Wait a bit before checking again
                std::thread::sleep(Duration::from_millis(100));
            }
            continue;
        }

        // ── Phase 1: Pedestrian Green ─────────────────────────────────────
        let base_ped  = config::PHASE_BASIC_PED_GREEN;
        let half_ped  = base_ped / 2.0;
        // Sequence: half_ped | ext | (half_ped - PED_GREEN_BLINKING) | PED_GREEN_BLINKING

        {
            let mut s    = state.lock().unwrap();
            s.phase      = Phase::PedestrianGreen;
            s.ped_blinking = false;
            s.veh_blinking = false;
            s.countdown  = half_ped;
            s.blink_on   = true;
            s.traffic_light.set_sig(TrafficLightPosition::Ped1, TrafficSign::Green, LightState::Solid);
            s.traffic_light.set_sig(TrafficLightPosition::Veh1, TrafficSign::Red, LightState::Solid);
        }

        // Check if we are exiting manual mode or transitioning to all_red - if so, skip to blinking immediately
        // BUT: if all_red_after_blinking is set, we've already done the blinking setup in UI, so don't re-enter blinking
        let skip_to_ped_blinking = {
            let s = state.lock().unwrap();
            (s.exiting_manual_mode || s.transitioning_to_all_red) && !s.all_red_after_blinking
        };

        if !skip_to_ped_blinking {
            // First half
            sim_sleep(half_ped, &state);
            if state.lock().unwrap().manual_control_enabled { continue; }
            if state.lock().unwrap().transitioning_to_all_red {
                // Skip remaining pedestrian green phases, go straight to all red
            } else {
                // Fuzzy inference: use manual input if set, otherwise compute from sensor data
                let ped_ext = {
                    let mut s = state.lock().unwrap();
                    if let Some(ext) = s.manual_ped_ext.take() {
                        s.ped_extension = ext;
                        ext
                    } else {
                        let ext = get_fuzzy_extension();
                        s.ped_extension = ext;
                        ext
                    }
                };

                // Extension
                if ped_ext > 0.1 {
                    {
                        let mut s = state.lock().unwrap();
                        s.phase = Phase::PedestrianGreenExtension;
                        s.countdown = ped_ext;
                    }
                    sim_sleep(ped_ext, &state);
                    if state.lock().unwrap().manual_control_enabled { continue; }
                    if state.lock().unwrap().transitioning_to_all_red {
                        // Skip to all red
                    } else {
                        // Second half minus blinking time
                        let second_solid = (half_ped - config::PED_GREEN_BLINKING).max(0.0);
                        if second_solid > 0.0 {
                            {
                                let mut s = state.lock().unwrap();
                                s.phase = Phase::PedestrianGreen;
                            }
                            { state.lock().unwrap().countdown = second_solid; }
                            sim_sleep(second_solid, &state);
                            if state.lock().unwrap().manual_control_enabled { continue; }
                        }
                    }
                } else {
                    // Second half minus blinking time
                    let second_solid = (half_ped - config::PED_GREEN_BLINKING).max(0.0);
                    if second_solid > 0.0 {
                        { state.lock().unwrap().countdown = second_solid; }
                        sim_sleep(second_solid, &state);
                        if state.lock().unwrap().manual_control_enabled { continue; }
                    }
                }
            }
        }

        // Blinking
        {
            let mut s      = state.lock().unwrap();
            s.ped_blinking = true;
            s.countdown    = config::PED_GREEN_BLINKING;
            s.traffic_light.set_sig(TrafficLightPosition::Ped1, TrafficSign::Green, LightState::Blinking);
        }
        sim_sleep(config::PED_GREEN_BLINKING, &state);
        if state.lock().unwrap().manual_control_enabled { continue; }

        // Check if all_red_after_blinking was set during ped blinking
        {
            let mut s = state.lock().unwrap();
            if s.all_red_after_blinking {
                s.all_red_after_blinking = false;
                s.transitioning_to_all_red = true;
                s.ped_blinking = false;
            }
        }

        if state.lock().unwrap().transitioning_to_all_red {
            // Continue to all red phase where transition will be handled
        }

        // Check if we should interrupt for all_red_hold
        {
            let s = state.lock().unwrap();
            if s.transitioning_to_all_red {
                // Continue to the all red phase where the transition will be handled
            }
        }

        // ── All-Red before vehicles ────────────────────────────────────────
        {
            let mut s      = state.lock().unwrap();
            s.phase        = Phase::AllRedBeforeVehicle;
            s.ped_blinking = false;
            s.countdown    = config::ALL_RED;
            s.traffic_light.set_sig(TrafficLightPosition::Ped1, TrafficSign::Red, LightState::Solid);
        }
        sim_sleep(config::ALL_RED, &state);
        if state.lock().unwrap().manual_control_enabled { continue; }

        // Clear exiting_manual_mode flag after reaching all red
        {
            let mut s = state.lock().unwrap();
            if s.exiting_manual_mode {
                s.exiting_manual_mode = false;
            }

            // Check if transitioning to all red hold
            if s.transitioning_to_all_red {
                s.transitioning_to_all_red = false;
                s.all_red_hold_enabled = true;
                continue;
            }

            // Check if all red hold was enabled during the all red phase
            if s.all_red_hold_enabled {
                continue;
            }
        }

        // ── Phase 2: Vehicle Green (fixed duration, no fuzzy extension) ──────
        let base_veh  = config::PHASE_BASIC_VEH_GREEN;
        let veh_solid = base_veh - config::VEH_GREEN_BLINKING; // solid green portion only

        {
            let mut s      = state.lock().unwrap();
            s.phase        = Phase::VehicleGreen;
            s.veh_blinking = false;
            s.countdown    = veh_solid;
            s.blink_on     = true;
            s.traffic_light.set_sig(TrafficLightPosition::Veh1, TrafficSign::Green, LightState::Solid);
        }

        // Check if we are exiting manual mode - if so, skip to blinking immediately
        let skip_to_veh_blinking = state.lock().unwrap().exiting_manual_mode;

        if !skip_to_veh_blinking {
            sim_sleep(veh_solid, &state);
            if state.lock().unwrap().manual_control_enabled { continue; }
            if state.lock().unwrap().transitioning_to_all_red {
                // Skip remaining vehicle green phases, go straight to all red
            }
        }

        // Blinking
        {
            let mut s      = state.lock().unwrap();
            s.veh_blinking = true;
            s.countdown    = config::VEH_GREEN_BLINKING;
            s.traffic_light.set_sig(TrafficLightPosition::Veh1, TrafficSign::Green, LightState::Blinking);
        }
        sim_sleep(config::VEH_GREEN_BLINKING, &state);
        if state.lock().unwrap().manual_control_enabled { continue; }

        // Check if all_red_after_blinking was set during vehicle blinking
        {
            let mut s = state.lock().unwrap();
            if s.all_red_after_blinking {
                s.all_red_after_blinking = false;
                s.veh_blinking = false;
                // If need_yellow_before_all_red is set, go to yellow; otherwise go to all_red
                if s.need_yellow_before_all_red {
                    // Will execute yellow light below
                } else {
                    s.transitioning_to_all_red = true;
                }
            }
        }

        if state.lock().unwrap().transitioning_to_all_red {
            // Skip vehicle yellow, go straight to all red before pedestrians
        } else {
            // ── Vehicle Yellow ─────────────────────────────────────────────────
            {
                let mut s      = state.lock().unwrap();
                s.phase        = Phase::VehicleYellow;
                s.veh_blinking = false;
                s.countdown    = config::VEH_YELLOW;
                s.traffic_light.set_sig(TrafficLightPosition::Veh1, TrafficSign::Yellow, LightState::Solid);
            }
            sim_sleep(config::VEH_YELLOW, &state);
            if state.lock().unwrap().manual_control_enabled { continue; }

            // Check if need_yellow_before_all_red was set (yellow after all_red blinking)
            {
                let mut s = state.lock().unwrap();
                if s.need_yellow_before_all_red {
                    s.need_yellow_before_all_red = false;
                    s.transitioning_to_all_red = true;
                }
            }
        }

        // ── All-Red before pedestrians ────────────────────────────────────
        {
            let mut s      = state.lock().unwrap();
            s.phase        = Phase::AllRedBeforePedestrian;
            s.countdown    = config::ALL_RED;
            s.traffic_light.set_sig(TrafficLightPosition::Veh1, TrafficSign::Red, LightState::Solid);
        }
        sim_sleep(config::ALL_RED, &state);
        if state.lock().unwrap().manual_control_enabled { continue; }

        // Clear exiting_manual_mode flag after reaching all red
        {
            let mut s = state.lock().unwrap();
            if s.exiting_manual_mode {
                s.exiting_manual_mode = false;
            }

            // Check if transitioning to all red hold
            if s.transitioning_to_all_red {
                s.transitioning_to_all_red = false;
                s.all_red_hold_enabled = true;
                continue;
            }

            // Check if all red hold was enabled during the all red phase
            if s.all_red_hold_enabled {
                continue;
            }
        }

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

fn draw_vehicle_light(ui: &mut egui::Ui, traffic_light: &TrafficLight, blink_on: bool, veh_blinking: bool) {
    let r   = 20.0_f32;
    let pad = 10.0_f32;
    let w   = r * 2.0 + pad * 2.0;
    let h   = r * 6.0 + pad * 4.0 + 24.0; // 3 bulbs + label

    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::hover());
    let painter   = ui.painter_at(rect);

    painter.rect_filled(rect, egui::CornerRadius::same(12), Color32::from_rgb(28, 28, 28));
    painter.rect_stroke(rect, egui::CornerRadius::same(12), Stroke::new(2.0, Color32::from_rgb(70, 70, 70)), egui::StrokeKind::Outside);

    let cx = rect.center().x;

    let red_lit    = traffic_light.sig_veh.0 == TrafficSign::Red    && (!veh_blinking || blink_on);
    let yellow_lit = traffic_light.sig_veh.0 == TrafficSign::Yellow && (!veh_blinking || blink_on);
    let green_lit  = traffic_light.sig_veh.0 == TrafficSign::Green  && (!veh_blinking || blink_on);

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

fn draw_pedestrian_light(ui: &mut egui::Ui, traffic_light: &TrafficLight, blink_on: bool, ped_blinking: bool) {
    let r   = 16.0_f32;
    let pad = 8.0_f32;
    let w   = 120.0_f32;
    let h   = r * 4.0 + pad * 3.0 + 24.0; // 2 bulbs + gap + label

    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::hover());
    let painter   = ui.painter_at(rect);

    painter.rect_filled(rect, egui::CornerRadius::same(12), Color32::from_rgb(28, 28, 28));
    painter.rect_stroke(rect, egui::CornerRadius::same(12), Stroke::new(2.0, Color32::from_rgb(70, 70, 70)), egui::StrokeKind::Outside);

    let cx = rect.center().x;

    let red_lit   = traffic_light.sig_ped.0 == TrafficSign::Red   && (!ped_blinking || blink_on);
    let green_lit = traffic_light.sig_ped.0 == TrafficSign::Green && (!ped_blinking || blink_on);

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
    /// Available camera devices
    camera_devices: Vec<(usize, String)>,
    /// Selected camera index
    selected_camera: usize,
    /// Last captured camera image path
    camera_img_path: Option<String>,
    /// Cached dimensions for the latest captured image
    camera_img_dimensions: Option<(u32, u32)>,
    /// Currently previewing image (full-screen modal)
    preview_image_path: Option<String>,
    /// Background capture task receiver
    camera_capture_rx: Option<mpsc::Receiver<Result<std::path::PathBuf, String>>>,
    /// Whether a photo capture is currently in progress
    camera_capture_in_progress: bool,
}

impl TrafficLightApp {
    fn new(state: Arc<Mutex<SimState>>) -> Self {
        let camera_devices = crate::camera::get_camera_devices();
        Self {
            state,
            ui_ped_text: String::new(),
            ui_veh_text: String::new(),
            fuzzy_ped_result: None,
            ped_err: None,
            ped_img_path: None,
            veh_img_path: None,
            yolo_msg: None,
            camera_devices,
            selected_camera: 0,
            camera_img_path: None,
            camera_img_dimensions: None,
            preview_image_path: None,
            camera_capture_rx: None,
            camera_capture_in_progress: false,
        }
    }

    fn selected_camera_id(&self) -> usize {
        self.camera_devices
            .get(self.selected_camera)
            .map(|(camera_id, _)| *camera_id)
            .unwrap_or(0)
    }

    fn refresh_camera_devices(&mut self) {
        let previous_camera_id = self.selected_camera_id();
        self.camera_devices = crate::camera::get_camera_devices();
        self.selected_camera = self
            .camera_devices
            .iter()
            .position(|(camera_id, _)| *camera_id == previous_camera_id)
            .unwrap_or(0);
    }

    fn poll_camera_capture(&mut self) {
        let Some(rx) = self.camera_capture_rx.take() else {
            return;
        };

        match rx.try_recv() {
            Ok(result) => {
                self.camera_capture_in_progress = false;
                match result {
                    Ok(path) => {
                        let path_str = path.to_string_lossy().into_owned();
                        self.camera_img_dimensions = crate::camera::get_image_dimensions(&path_str).ok();
                        self.ped_img_path = Some(path_str.clone());
                        self.camera_img_path = Some(path_str);
                        self.yolo_msg = Some("Photo captured successfully! Use detection buttons above.".to_string());
                    }
                    Err(err) => {
                        self.camera_img_dimensions = None;
                        self.yolo_msg = Some(format!("Capture failed: {}", err));
                    }
                }
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.camera_capture_rx = Some(rx);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.camera_capture_in_progress = false;
                self.camera_img_dimensions = None;
                self.yolo_msg = Some("Capture failed: background task disconnected".to_string());
            }
        }
    }
}

impl eframe::App for TrafficLightApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(50));
        self.poll_camera_capture();

        // ── Image Preview Modal ──────────────────────────────────────────
        if let Some(ref img_path) = self.preview_image_path.clone() {
            let mut is_open = true;
            egui::Window::new("📷 Image Preview")
                .open(&mut is_open)
                .resizable(true)
                .collapsible(false)
                .vscroll(true)
                .hscroll(true)
                .default_width(600.0)
                .default_height(500.0)
                .show(ctx, |ui| {
                    let file_name = Path::new(img_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(img_path);
                    let dims = self.camera_img_dimensions;

                    ui.label(
                        egui::RichText::new(format!("Captured: {}", file_name))
                            .size(12.0)
                            .color(Color32::LIGHT_GRAY),
                    );
                    ui.add_space(8.0);

                    if let Some((width, height)) = dims {
                        let max_width = 550.0;
                        let max_height = 450.0;
                        let (w, h) = (width as f32, height as f32);
                        let scale = (max_width / w).min(max_height / h).min(1.0);
                        let display_width = w * scale;
                        let display_height = h * scale;

                        ui.label(
                            egui::RichText::new(format!("Resolution: {}x{}", width, height))
                                .size(11.0)
                                .color(Color32::DARK_GRAY),
                        );
                        ui.add_space(12.0);

                        // Try to load and display the actual image
                        match image::open(img_path) {
                            Ok(img) => {
                                let rgb_img = img.to_rgb8();
                                let pixels: Vec<Color32> = rgb_img
                                    .pixels()
                                    .map(|p| Color32::from_rgb(p[0], p[1], p[2]))
                                    .collect();

                                let color_image = egui::ColorImage {
                                    size: [width as usize, height as usize],
                                    pixels,
                                };

                                let texture = ctx.load_texture(
                                    "preview_image",
                                    color_image,
                                    Default::default(),
                                );

                                ui.image(egui::load::SizedTexture::new(
                                    texture.id(),
                                    Vec2::new(display_width, display_height),
                                ));
                            }
                            Err(e) => {
                                ui.label(
                                    egui::RichText::new(format!("Failed to load image: {}", e))
                                        .size(11.0)
                                        .color(Color32::RED),
                                );
                                let (rect, _) = ui.allocate_exact_size(
                                    Vec2::new(display_width, display_height),
                                    egui::Sense::hover(),
                                );
                                ui.painter_at(rect)
                                    .rect_filled(rect, 4.0, Color32::from_rgb(60, 90, 140));
                            }
                        }
                    } else {
                        ui.label("Image captured, but preview metadata is unavailable");
                    }
                });

            if !is_open {
                self.preview_image_path = None;
            }
        }

        let snap = self.state.lock().unwrap().clone();

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
                        Phase::PedestrianGreen
                        | Phase::PedestrianGreenExtension => Color32::from_rgb(60, 210, 80),
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
                                            draw_vehicle_light(ui, &snap.traffic_light, snap.blink_on, snap.veh_blinking);
                                        });

                                        ui.add_space(16.0);

                                        ui.vertical(|ui| {
                                            ui.set_min_width(125.0);
                                            draw_pedestrian_light(ui, &snap.traffic_light, snap.blink_on, snap.ped_blinking);
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
                                                    info_row(ui, "  Ped Signal", &format!("{:?}/{:?}", snap.traffic_light.sig_ped.0, snap.traffic_light.sig_ped.1), Color32::from_rgb(80, 210, 100));
                                                    info_row(ui, "  Veh Signal", &format!("{:?}/{:?}", snap.traffic_light.sig_veh.0, snap.traffic_light.sig_veh.1), Color32::from_rgb(80, 170, 255));
                                                    info_row(ui, "  Time", &format!("{:.1} s", snap.traffic_light.time), Color32::LIGHT_GRAY);
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

                            ui.add_space(10.0);

                            egui::Frame::default()
                                .fill(Color32::from_rgb(20, 24, 32))
                                .corner_radius(egui::CornerRadius::same(10))
                                .inner_margin(egui::Margin::same(12))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(55, 55, 90)))
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new("📷 Camera Module")
                                            .size(14.0)
                                            .color(Color32::LIGHT_GRAY)
                                            .strong(),
                                    );
                                    ui.add_space(8.0);

                                    ui.horizontal_top(|ui| {
                                        ui.vertical(|ui| {
                                            ui.set_min_width(230.0);
                                            ui.label(
                                                egui::RichText::new("Select Camera:")
                                                    .size(12.0)
                                                    .color(Color32::LIGHT_GRAY),
                                            );

                                            let mut device_labels: Vec<String> = self
                                                .camera_devices
                                                .iter()
                                                .map(|(_, label)| label.clone())
                                                .collect();
                                            if device_labels.is_empty() {
                                                device_labels.push("No camera found".to_string());
                                            }

                                            let selected_label = device_labels
                                                .get(self.selected_camera)
                                                .cloned()
                                                .unwrap_or_else(|| "No camera found".to_string());
                                            let mut selected = self.selected_camera;

                                            ui.horizontal(|ui| {
                                                egui::ComboBox::from_label("")
                                                    .selected_text(&selected_label)
                                                    .show_ui(ui, |ui| {
                                                        for (idx, label) in device_labels.iter().enumerate() {
                                                            ui.selectable_value(&mut selected, idx, label);
                                                        }
                                                    });

                                                let refresh_btn = egui::Button::new("🔄 Refresh")
                                                    .min_size(Vec2::new(92.0, 28.0));
                                                if ui.add(refresh_btn).clicked() {
                                                    self.refresh_camera_devices();
                                                    let device_count = self.camera_devices.len();
                                                    self.yolo_msg = Some(format!(
                                                        "Camera list refreshed: {} device(s) available",
                                                        device_count
                                                    ));
                                                } else {
                                                    self.selected_camera = selected;
                                                }
                                            });

                                            ui.add_space(12.0);

                                            let capture_btn = egui::Button::new(
                                                egui::RichText::new("📸 Capture Photo")
                                                    .size(13.0)
                                                    .color(Color32::WHITE),
                                            )
                                            .fill(Color32::from_rgb(100, 150, 200))
                                            .min_size(Vec2::new(150.0, 32.0));

                                            if ui
                                                .add_enabled(!self.camera_capture_in_progress, capture_btn)
                                                .clicked()
                                            {
                                                let camera_id = self.selected_camera_id();
                                                let (tx, rx) = mpsc::channel();
                                                self.camera_capture_rx = Some(rx);
                                                self.camera_capture_in_progress = true;
                                                self.yolo_msg = Some(format!(
                                                    "Capturing photo from Camera {}...",
                                                    camera_id
                                                ));

                                                std::thread::spawn(move || {
                                                    let result = crate::camera::capture_frame(camera_id)
                                                        .map_err(|err| err.to_string());
                                                    let _ = tx.send(result);
                                                });
                                            }

                                            if self.camera_capture_in_progress {
                                                ui.add_space(6.0);
                                                ui.label(
                                                    egui::RichText::new("Capturing photo in background...")
                                                        .size(11.0)
                                                        .color(Color32::YELLOW),
                                                );
                                            }
                                        });

                                        ui.add_space(20.0);

                                        ui.vertical(|ui| {
                                            ui.set_min_width(180.0);
                                            ui.label(
                                                egui::RichText::new("📷 Preview")
                                                    .size(12.0)
                                                    .color(Color32::LIGHT_GRAY),
                                            );
                                            ui.add_space(4.0);

                                            if let Some(ref img_path) = self.camera_img_path {
                                                let width = 150.0;
                                                let height = 110.0;
                                                let (rect, response) = ui.allocate_exact_size(
                                                    Vec2::new(width, height),
                                                    egui::Sense::click(),
                                                );

                                                // Try to load and display the actual image thumbnail
                                                match image::open(img_path) {
                                                    Ok(img) => {
                                                        // Scale image to fit in the preview area
                                                        let img_w = img.width() as f32;
                                                        let img_h = img.height() as f32;
                                                        let scale = (width / img_w).min(height / img_h).min(1.0);
                                                        let display_w = img_w * scale;
                                                        let display_h = img_h * scale;

                                                        // Convert image to RGB pixels
                                                        let rgb_img = img.to_rgb8();
                                                        let pixels: Vec<Color32> = rgb_img
                                                            .pixels()
                                                            .map(|p| Color32::from_rgb(p[0], p[1], p[2]))
                                                            .collect();

                                                        let color_image = egui::ColorImage {
                                                            size: [img.width() as usize, img.height() as usize],
                                                            pixels,
                                                        };

                                                        let texture = ctx.load_texture(
                                                            "camera_preview_thumb",
                                                            color_image,
                                                            Default::default(),
                                                        );

                                                        // Draw border and background
                                                        let painter = ui.painter_at(rect);
                                                        painter.rect_filled(
                                                            rect,
                                                            egui::CornerRadius::same(4),
                                                            Color32::from_rgb(50, 80, 120),
                                                        );
                                                        painter.rect_stroke(
                                                            rect,
                                                            egui::CornerRadius::same(4),
                                                            Stroke::new(2.0, Color32::from_rgb(100, 150, 200)),
                                                            egui::StrokeKind::Outside,
                                                        );

                                                        // Center the image within the preview box
                                                        let center_x = rect.center().x;
                                                        let center_y = rect.center().y;
                                                        let image_rect = egui::Rect {
                                                            min: Pos2::new(center_x - display_w / 2.0, center_y - display_h / 2.0),
                                                            max: Pos2::new(center_x + display_w / 2.0, center_y + display_h / 2.0),
                                                        };

                                                        ui.painter_at(image_rect).image(
                                                            texture.id(),
                                                            image_rect,
                                                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                                            Color32::WHITE,
                                                        );
                                                    }
                                                    Err(_) => {
                                                        // Fallback if image loading fails
                                                        let painter = ui.painter_at(rect);
                                                        painter.rect_filled(
                                                            rect,
                                                            egui::CornerRadius::same(4),
                                                            Color32::from_rgb(50, 80, 120),
                                                        );
                                                        painter.rect_stroke(
                                                            rect,
                                                            egui::CornerRadius::same(4),
                                                            Stroke::new(2.0, Color32::from_rgb(100, 150, 200)),
                                                            egui::StrokeKind::Outside,
                                                        );
                                                    }
                                                }

                                                if response.hovered() {
                                                    ui.ctx().output_mut(|o| {
                                                        o.cursor_icon = egui::CursorIcon::PointingHand;
                                                    });
                                                }

                                                if response.clicked() {
                                                    self.preview_image_path = Some(img_path.clone());
                                                }

                                                let file_name = Path::new(img_path)
                                                    .file_name()
                                                    .and_then(|n| n.to_str())
                                                    .unwrap_or(img_path);
                                                ui.add_space(4.0);
                                                ui.label(
                                                    egui::RichText::new(file_name)
                                                        .size(10.0)
                                                        .color(Color32::from_rgb(100, 200, 100)),
                                                );
                                                if let Some((img_w, img_h)) = self.camera_img_dimensions {
                                                    ui.label(
                                                        egui::RichText::new(format!("{}x{}", img_w, img_h))
                                                            .size(10.0)
                                                            .color(Color32::DARK_GRAY),
                                                    );
                                                }
                                                ui.label(
                                                    egui::RichText::new("Click to view preview details")
                                                        .size(10.0)
                                                        .color(Color32::DARK_GRAY),
                                                );
                                            } else {
                                                let (rect, _) = ui.allocate_exact_size(
                                                    Vec2::new(150.0, 110.0),
                                                    egui::Sense::hover(),
                                                );
                                                ui.painter_at(rect).rect_filled(
                                                    rect,
                                                    egui::CornerRadius::same(4),
                                                    Color32::from_rgb(35, 42, 58),
                                                );
                                                ui.add_space(4.0);
                                                ui.label(
                                                    egui::RichText::new("No captured photo yet")
                                                        .size(10.0)
                                                        .color(Color32::DARK_GRAY),
                                                );
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
                let snap = self.state.lock().unwrap().clone();
                let pause_controls_disabled = snap.exiting_manual_mode || snap.transitioning_to_all_red || snap.all_red_after_blinking || snap.need_yellow_before_all_red;

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
                            let manual_enabled = snap.manual_control_enabled;
                            let (lbl, fill, text_color) = {
                                let s = self.state.lock().unwrap();
                                if pause_controls_disabled {
                                    // Exiting manual mode: all pause controls disabled
                                    if s.paused {
                                        ("▶  Resume", Color32::from_rgb(100, 100, 120), Color32::from_rgb(100, 100, 120))
                                    } else {
                                        ("⏸  Pause", Color32::from_rgb(100, 100, 120), Color32::from_rgb(100, 100, 120))
                                    }
                                } else if manual_enabled {
                                    // Manual mode: button is disabled
                                    if s.paused {
                                        ("▶  Resume", Color32::from_rgb(100, 100, 120), Color32::from_rgb(100, 100, 120))
                                    } else {
                                        ("⏸  Pause", Color32::from_rgb(100, 100, 120), Color32::from_rgb(100, 100, 120))
                                    }
                                } else {
                                    // Auto mode: button is enabled
                                    if s.paused {
                                        ("▶  Resume", Color32::from_rgb(45, 185, 75), Color32::WHITE)
                                    } else {
                                        ("⏸  Pause", Color32::from_rgb(195, 70, 45), Color32::WHITE)
                                    }
                                }
                            };
                            let btn = egui::Button::new(
                                egui::RichText::new(lbl).size(14.0).color(text_color),
                            )
                            .fill(fill)
                            .min_size(Vec2::new(96.0, 32.0));
                            let button_response = ui.add_enabled(!manual_enabled && !pause_controls_disabled, btn);
                            if !manual_enabled && !pause_controls_disabled && button_response.clicked() {
                                let mut s = self.state.lock().unwrap();
                                s.paused = !s.paused;
                            }

                            if snap.paused && !manual_enabled && !pause_controls_disabled {
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

                    ui.add_space(28.0);

                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("🎛 Manual Mode").size(13.0).color(Color32::LIGHT_GRAY));
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let manual_enabled = snap.manual_control_enabled;
                            let (toggle_lbl, toggle_fill, toggle_text_col) = if pause_controls_disabled {
                                // Exiting manual mode: button disabled
                                ("● Manual ON", Color32::from_rgb(100, 100, 120), Color32::from_rgb(100, 100, 120))
                            } else if manual_enabled {
                                ("● Manual ON", Color32::from_rgb(200, 80, 80), Color32::WHITE)
                            } else {
                                ("○ Manual OFF", Color32::from_rgb(50, 50, 68), Color32::WHITE)
                            };
                            let toggle_btn = egui::Button::new(
                                egui::RichText::new(toggle_lbl).size(14.0).color(toggle_text_col),
                            )
                            .fill(toggle_fill)
                            .min_size(Vec2::new(130.0, 32.0));
                            let toggle_response = ui.add_enabled(!pause_controls_disabled, toggle_btn);
                            if !pause_controls_disabled && toggle_response.clicked() {
                                let mut s = self.state.lock().unwrap();
                                if s.manual_control_enabled {
                                    // Disable manual control: set flag to exit manual mode after reaching all red
                                    s.manual_control_enabled = false;
                                    s.exiting_manual_mode = true;
                                    // Ensure simulation is not paused so it can continue
                                    s.paused = false;

                                    // Set current green light to blinking state
                                    if s.traffic_light.sig_ped.0 == TrafficSign::Green {
                                        s.traffic_light.set_sig(TrafficLightPosition::Ped1, TrafficSign::Green, LightState::Blinking);
                                        s.ped_blinking = true;
                                    }
                                    if s.traffic_light.sig_veh.0 == TrafficSign::Green {
                                        s.traffic_light.set_sig(TrafficLightPosition::Veh1, TrafficSign::Green, LightState::Blinking);
                                        s.veh_blinking = true;
                                    }
                                } else {
                                    // Enable manual control: pause the simulation at current state
                                    s.manual_control_enabled = true;
                                    s.paused = true;
                                    s.countdown = 0.0;
                                }
                            }

                            // Phase switch button: only enabled in manual mode
                            let switch_enabled = manual_enabled && !pause_controls_disabled;
                            let switch_fill = if switch_enabled {
                                Color32::from_rgb(100, 150, 255)
                            } else if pause_controls_disabled {
                                Color32::from_rgb(70, 70, 85)
                            } else {
                                Color32::from_rgb(50, 50, 68)
                            };
                            let switch_text_col = if switch_enabled {
                                Color32::WHITE
                            } else {
                                Color32::from_rgb(100, 100, 120)
                            };
                            let switch_btn = egui::Button::new(
                                egui::RichText::new("⏩ Next Phase").size(14.0).color(switch_text_col),
                            )
                            .fill(switch_fill)
                            .min_size(Vec2::new(120.0, 32.0));

                            let button_response = ui.add_enabled(switch_enabled, switch_btn);
                            if switch_enabled && button_response.clicked() {
                                let mut s = self.state.lock().unwrap();
                                if s.manual_control_enabled && !s.manual_switch_phase {
                                    s.manual_switch_phase = true;
                                }
                            }
                        });
                    });

                    ui.add_space(28.0);

                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("🔴 Special Modes").size(13.0).color(Color32::LIGHT_GRAY));
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let all_red_enabled = snap.all_red_hold_enabled;
                            let modes_transitioning = snap.transitioning_to_all_red || snap.all_red_after_blinking;

                            // All Red button
                            let (all_red_lbl, all_red_fill, all_red_text_col) = if modes_transitioning {
                                ("All Red...", Color32::from_rgb(100, 100, 120), Color32::from_rgb(100, 100, 120))
                            } else if all_red_enabled {
                                ("● All Red ON", Color32::from_rgb(180, 60, 60), Color32::WHITE)
                            } else {
                                ("○ All Red OFF", Color32::from_rgb(50, 50, 68), Color32::WHITE)
                            };
                            let all_red_btn = egui::Button::new(
                                egui::RichText::new(all_red_lbl).size(13.0).color(all_red_text_col),
                            )
                            .fill(all_red_fill)
                            .min_size(Vec2::new(130.0, 32.0));
                            let all_red_response = ui.add_enabled(!modes_transitioning, all_red_btn);
                            if !modes_transitioning && all_red_response.clicked() {
                                let mut s = self.state.lock().unwrap();
                                if s.all_red_hold_enabled {
                                    // Turn off all red, continue auto cycle
                                    s.all_red_hold_enabled = false;
                                    // Automatically resume simulation
                                    s.paused = false;
                                    // Reset to all red state so auto cycle continues
                                    s.traffic_light.set_sig(TrafficLightPosition::Ped1, TrafficSign::Red, LightState::Solid);
                                    s.traffic_light.set_sig(TrafficLightPosition::Veh1, TrafficSign::Red, LightState::Solid);
                                    // Reset blinking flags to avoid duplicate blinking on next cycle
                                    s.ped_blinking = false;
                                    s.veh_blinking = false;
                                    s.all_red_after_blinking = false;
                                    s.need_yellow_before_all_red = false;
                                } else {
                                    // Exit manual mode if in manual mode, and allow transition to run
                                    if s.manual_control_enabled {
                                        s.manual_control_enabled = false;
                                    }
                                    s.paused = false;

                                    // Reset transition helper flags before applying one deterministic path.
                                    s.all_red_after_blinking = false;
                                    s.need_yellow_before_all_red = false;

                                    match s.phase {
                                        // 1 & 2. Ped green solid / extension solid -> immediate ped blinking -> all red.
                                        Phase::PedestrianGreen | Phase::PedestrianGreenExtension => {
                                            if s.ped_blinking {
                                                // 3. Already in ped blinking -> wait blinking end -> all red.
                                                s.all_red_after_blinking = true;
                                            } else {
                                                s.traffic_light.set_sig(TrafficLightPosition::Ped1, TrafficSign::Green, LightState::Blinking);
                                                s.ped_blinking = true;
                                                s.countdown = config::PED_GREEN_BLINKING;
                                                s.all_red_after_blinking = true;
                                                s.skip_phase = true;
                                            }
                                        }

                                        // 5 & 6. Vehicle green solid / blinking -> blinking then yellow then all red.
                                        Phase::VehicleGreen => {
                                            s.need_yellow_before_all_red = true;
                                            if s.veh_blinking {
                                                s.all_red_after_blinking = true;
                                            } else {
                                                s.traffic_light.set_sig(TrafficLightPosition::Veh1, TrafficSign::Green, LightState::Blinking);
                                                s.veh_blinking = true;
                                                s.countdown = config::VEH_GREEN_BLINKING;
                                                s.all_red_after_blinking = true;
                                                s.skip_phase = true;
                                            }
                                        }

                                        // During yellow, finish yellow then go all red.
                                        Phase::VehicleYellow => {
                                            s.need_yellow_before_all_red = true;
                                        }

                                        // 4 & 7. Already in all-red state -> keep all red hold.
                                        Phase::AllRedBeforeVehicle | Phase::AllRedBeforePedestrian => {
                                            s.all_red_hold_enabled = true;
                                        }
                                    }
                                }
                            }

                            ui.add_space(12.0);
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
