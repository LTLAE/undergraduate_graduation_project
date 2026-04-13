#![allow(dead_code)]

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::fmt;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use traffic_light_control::config;
use traffic_light_control::traffic::fuzzy_inference;

pub const SIMULATION_SECONDS: u32 = 15 * 60;
pub const PEAK_PLOT_SECONDS: u32 = 20 * 60;
pub const PEAK_DURATION_SECONDS: u32 = 15 * 60;
pub const RECOVERY_SIMULATION_SECONDS: u32 = 60 * 60;
pub const PEDESTRIAN_ARRIVAL_RATE: f64 = 140.0 / 60.0; // According to paper, 140 ped / min -> ped / s
pub const PEDESTRIAN_RECOVERY_ARRIVAL_RATE: f64 = 0.5;
pub const PEDESTRIAN_SERVICE_RATE: f64 = 5.2;
pub const VEHICLE_ARRIVAL_RATE_TOTAL: f64 = 0.17; // total, /3 for per lane, roughly 10 veh / min total
pub const VEHICLE_ARRIVAL_RATE_PER_LANE: f64 =
    VEHICLE_ARRIVAL_RATE_TOTAL / VEHICLE_LANE_COUNT as f64;
// Vehicle service rate is defined per lane.
pub const VEHICLE_LANE_COUNT: u32 = 3;
pub const VEHICLE_SERVICE_RATE_PER_LANE: f64 = 0.5;
pub const VEHICLE_SERVICE_RATE: f64 = VEHICLE_SERVICE_RATE_PER_LANE * VEHICLE_LANE_COUNT as f64;
pub const VEHICLE_STARTUP_LOSS_SECONDS: u32 = 2;
pub const STABILITY_QUEUE_THRESHOLD: u32 = 10;
pub const STABILITY_CONSECUTIVE_SECONDS: u32 = 5 * 60;
pub const DEFAULT_SEED: u64 = 114514;

#[derive(Clone, Copy, Debug)]
pub struct SimulationParams {
    pub pedestrian_green_seconds: u32,
    pub pedestrian_blinking_seconds: u32,
    pub vehicle_green_seconds: u32,
    pub vehicle_blinking_seconds: u32,
    pub vehicle_yellow_seconds: u32,
    pub all_red_seconds: u32,
}

impl Default for SimulationParams {
    fn default() -> Self {
        Self {
            pedestrian_green_seconds: config::PHASE_BASIC_PED_GREEN.round() as u32,
            pedestrian_blinking_seconds: config::PED_GREEN_BLINKING.round() as u32,
            vehicle_green_seconds: config::PHASE_BASIC_VEH_GREEN.round() as u32,
            vehicle_blinking_seconds: config::VEH_GREEN_BLINKING.round() as u32,
            vehicle_yellow_seconds: config::VEH_YELLOW.round() as u32,
            all_red_seconds: config::ALL_RED.round() as u32,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ArrivalProfile {
    pub peak_duration_seconds: u32,
    pub pedestrian_peak_rate: f64,
    pub pedestrian_recovery_rate: f64,
    pub vehicle_rate: f64,
}

impl Default for ArrivalProfile {
    fn default() -> Self {
        Self {
            peak_duration_seconds: PEAK_DURATION_SECONDS,
            pedestrian_peak_rate: PEDESTRIAN_ARRIVAL_RATE,
            pedestrian_recovery_rate: PEDESTRIAN_ARRIVAL_RATE,
            vehicle_rate: VEHICLE_ARRIVAL_RATE_TOTAL,
        }
    }
}

impl ArrivalProfile {
    fn pedestrian_rate_at(&self, second: u32) -> f64 {
        if second < self.peak_duration_seconds {
            self.pedestrian_peak_rate
        } else {
            self.pedestrian_recovery_rate
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ControllerKind {
    Fixed,
    Fuzzy,
}

impl fmt::Display for ControllerKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ControllerKind::Fixed => write!(f, "fixed"),
            ControllerKind::Fuzzy => write!(f, "fuzzy"),
        }
    }
}

// Phases, with AI
#[derive(Clone, Copy, Debug)]
enum Phase {
    PedestrianGreen,
    PedestrianBlinking,
    AllRedBeforeVehicle,
    VehicleGreen,
    VehicleBlinking,
    VehicleYellow,
    AllRedBeforePedestrian,
}

impl Phase {
    fn as_str(self) -> &'static str {
        match self {
            Phase::PedestrianGreen => "pedestrian_green",
            Phase::PedestrianBlinking => "pedestrian_blinking",
            Phase::AllRedBeforeVehicle => "all_red_before_vehicle",
            Phase::VehicleGreen => "vehicle_green",
            Phase::VehicleBlinking => "vehicle_blinking",
            Phase::VehicleYellow => "vehicle_yellow",
            Phase::AllRedBeforePedestrian => "all_red_before_pedestrian",
        }
    }
}

#[derive(Clone, Debug)]
pub struct CsvRow {
    pub second: u32,
    pub cycle: u32,
    pub phase: &'static str,
    pub ped_arrival_rate: f64,
    pub ped_arrivals: u32,
    pub ped_departures: u32,
    pub ped_queue: u32,
    pub veh_arrivals: u32,
    pub veh_departures: u32,
    pub veh_queue: u32,
    pub ped_extension_seconds: u32,
}

#[derive(Clone, Debug)]
pub struct SimulationSummary {
    pub controller: ControllerKind,
    pub total_seconds: u32,
    pub pedestrian_green_seconds: u32,
    pub vehicle_green_seconds: u32,
    pub pedestrian_peak_rate: f64,
    pub pedestrian_recovery_rate: f64,
    pub peak_duration_seconds: u32,
    pub total_ped_arrivals: u32,
    pub total_ped_departures: u32,
    pub total_vehicle_arrivals: u32,
    pub total_vehicle_departures: u32,
    pub avg_ped_queue: f64,
    pub avg_vehicle_queue: f64,
    pub avg_ped_wait_seconds: f64,
    pub avg_vehicle_wait_seconds: f64,
    pub max_ped_queue: u32,
    pub max_vehicle_queue: u32,
    pub final_ped_queue: u32,
    pub final_vehicle_queue: u32,
    pub peak_end_ped_queue: u32,
    pub stabilization_time_seconds: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct SimulationResult {
    pub summary: SimulationSummary,
    pub rows: Vec<CsvRow>,
}

#[derive(Clone, Debug)]
struct EngineState {
    controller: ControllerKind,
    params: SimulationParams,
    arrival_profile: ArrivalProfile,
    rng: StdRng,
    ped_queue: u32,
    veh_queue: u32,
    ped_capacity_bank: f64,
    veh_capacity_bank: f64,
    phase: Phase,
    phase_remaining: u32,
    phase_elapsed: u32,
    cycle_index: u32,
    current_ped_extension: u32,
    total_ped_arrivals: u32,
    total_ped_departures: u32,
    total_vehicle_arrivals: u32,
    total_vehicle_departures: u32,
    total_ped_queue_seconds: f64,
    total_vehicle_queue_seconds: f64,
    max_ped_queue: u32,
    max_vehicle_queue: u32,
    peak_end_ped_queue: Option<u32>,
    stable_run_seconds: u32,
    stabilization_time_seconds: Option<u32>,
}

impl EngineState {
    fn new(
        controller: ControllerKind,
        params: SimulationParams,
        arrival_profile: ArrivalProfile,
        seed: u64,
    ) -> Self {
        let current_ped_extension = compute_ped_extension(controller, 0, 0);
        Self {
            controller,
            params,
            arrival_profile,
            rng: StdRng::seed_from_u64(seed),
            ped_queue: 0,
            veh_queue: 0,
            ped_capacity_bank: 0.0,
            veh_capacity_bank: 0.0,
            phase: Phase::PedestrianGreen,
            phase_remaining: ped_green_duration(params, current_ped_extension),
            phase_elapsed: 0,
            cycle_index: 0,
            current_ped_extension,
            total_ped_arrivals: 0,
            total_ped_departures: 0,
            total_vehicle_arrivals: 0,
            total_vehicle_departures: 0,
            total_ped_queue_seconds: 0.0,
            total_vehicle_queue_seconds: 0.0,
            max_ped_queue: 0,
            max_vehicle_queue: 0,
            peak_end_ped_queue: None,
            stable_run_seconds: 0,
            stabilization_time_seconds: None,
        }
    }

    fn step(&mut self, second: u32) -> CsvRow {
        let ped_arrival_rate = self.arrival_profile.pedestrian_rate_at(second);
        let ped_arrivals = sample_poisson(&mut self.rng, ped_arrival_rate);
        let veh_arrivals = sample_poisson(&mut self.rng, self.arrival_profile.vehicle_rate);

        self.ped_queue += ped_arrivals;
        self.veh_queue += veh_arrivals;
        self.total_ped_arrivals += ped_arrivals;
        self.total_vehicle_arrivals += veh_arrivals;

        self.total_ped_queue_seconds += self.ped_queue as f64;
        self.total_vehicle_queue_seconds += self.veh_queue as f64;

        let (ped_capacity, veh_capacity) = self.phase_capacity();
        self.ped_capacity_bank += ped_capacity;
        self.veh_capacity_bank += veh_capacity;

        let ped_departures = self.consume_capacity(true);
        let veh_departures = self.consume_capacity(false);

        self.max_ped_queue = self.max_ped_queue.max(self.ped_queue);
        self.max_vehicle_queue = self.max_vehicle_queue.max(self.veh_queue);

        let row = CsvRow {
            second,
            cycle: self.cycle_index,
            phase: self.phase.as_str(),
            ped_arrival_rate,
            ped_arrivals,
            ped_departures,
            ped_queue: self.ped_queue,
            veh_arrivals,
            veh_departures,
            veh_queue: self.veh_queue,
            ped_extension_seconds: self.current_ped_extension,
        };

        self.update_recovery_metrics(second);
        self.advance_phase();
        row
    }

    fn update_recovery_metrics(&mut self, second: u32) {
        if second + 1 == self.arrival_profile.peak_duration_seconds {
            self.peak_end_ped_queue = Some(self.ped_queue);
        }

        if second < self.arrival_profile.peak_duration_seconds
            || self.stabilization_time_seconds.is_some()
        {
            return;
        }

        if self.ped_queue <= STABILITY_QUEUE_THRESHOLD {
            self.stable_run_seconds += 1;
            if self.stable_run_seconds >= STABILITY_CONSECUTIVE_SECONDS {
                self.stabilization_time_seconds =
                    Some(second + 1 - self.arrival_profile.peak_duration_seconds);
            }
        } else {
            self.stable_run_seconds = 0;
        }
    }

    fn phase_capacity(&self) -> (f64, f64) {
        match self.phase {
            Phase::PedestrianGreen => (PEDESTRIAN_SERVICE_RATE, 0.0),
            Phase::VehicleGreen => {
                if self.phase_elapsed < VEHICLE_STARTUP_LOSS_SECONDS {
                    (0.0, 0.0)
                } else {
                    (0.0, VEHICLE_SERVICE_RATE)
                }
            }
            _ => (0.0, 0.0),
        }
    }

    fn consume_capacity(&mut self, pedestrian: bool) -> u32 {
        let (queue, bank, total_departures) = if pedestrian {
            (
                &mut self.ped_queue,
                &mut self.ped_capacity_bank,
                &mut self.total_ped_departures,
            )
        } else {
            (
                &mut self.veh_queue,
                &mut self.veh_capacity_bank,
                &mut self.total_vehicle_departures,
            )
        };

        let serviceable = bank.floor() as u32;
        let departures = (*queue).min(serviceable);
        *queue -= departures;
        *bank -= departures as f64;
        *total_departures += departures;
        departures
    }

    fn advance_phase(&mut self) {
        self.phase_elapsed += 1;

        if self.phase_remaining > 1 {
            self.phase_remaining -= 1;
            return;
        }

        self.phase = match self.phase {
            Phase::PedestrianGreen => {
                self.phase_remaining = self.params.pedestrian_blinking_seconds;
                Phase::PedestrianBlinking
            }
            Phase::PedestrianBlinking => {
                self.phase_remaining = self.params.all_red_seconds;
                Phase::AllRedBeforeVehicle
            }
            Phase::AllRedBeforeVehicle => {
                self.phase_remaining = self.params.vehicle_green_seconds;
                Phase::VehicleGreen
            }
            Phase::VehicleGreen => {
                self.phase_remaining = self.params.vehicle_blinking_seconds;
                Phase::VehicleBlinking
            }
            Phase::VehicleBlinking => {
                self.phase_remaining = self.params.vehicle_yellow_seconds;
                Phase::VehicleYellow
            }
            Phase::VehicleYellow => {
                self.phase_remaining = self.params.all_red_seconds;
                Phase::AllRedBeforePedestrian
            }
            Phase::AllRedBeforePedestrian => {
                self.cycle_index += 1;
                self.current_ped_extension =
                    compute_ped_extension(self.controller, self.ped_queue, self.veh_queue);
                self.phase_remaining = ped_green_duration(self.params, self.current_ped_extension);
                Phase::PedestrianGreen
            }
        };

        self.phase_elapsed = 0;
    }
}

fn ped_green_duration(params: SimulationParams, extension_seconds: u32) -> u32 {
    params.pedestrian_green_seconds + extension_seconds
}

fn compute_ped_extension(controller: ControllerKind, ped_queue: u32, veh_queue: u32) -> u32 {
    match controller {
        ControllerKind::Fixed => 0,
        ControllerKind::Fuzzy => fuzzy_inference::get_extension_time(
            ped_queue.min(i32::MAX as u32) as i32,
            veh_queue.min(i32::MAX as u32) as i32,
        )
        .round()
        .clamp(0.0, config::T_MAX) as u32,
    }
}

fn sample_poisson(rng: &mut StdRng, lambda: f64) -> u32 {
    let threshold = (-lambda).exp();
    let mut product = 1.0;
    let mut count = 0u32;

    loop {
        count += 1;
        product *= rng.random::<f64>();
        if product <= threshold {
            return count - 1;
        }
    }
}

pub fn run_simulation(
    controller: ControllerKind,
    params: SimulationParams,
    seed: u64,
    total_seconds: u32,
) -> SimulationResult {
    run_simulation_with_profile(
        controller,
        params,
        ArrivalProfile::default(),
        seed,
        total_seconds,
    )
}

pub fn run_simulation_with_profile(
    controller: ControllerKind,
    params: SimulationParams,
    arrival_profile: ArrivalProfile,
    seed: u64,
    total_seconds: u32,
) -> SimulationResult {
    let mut state = EngineState::new(controller, params, arrival_profile, seed);
    let mut rows = Vec::with_capacity(total_seconds as usize);

    for second in 0..total_seconds {
        rows.push(state.step(second));
    }

    let summary = SimulationSummary {
        controller,
        total_seconds,
        pedestrian_green_seconds: params.pedestrian_green_seconds,
        vehicle_green_seconds: params.vehicle_green_seconds,
        pedestrian_peak_rate: arrival_profile.pedestrian_peak_rate,
        pedestrian_recovery_rate: arrival_profile.pedestrian_recovery_rate,
        peak_duration_seconds: arrival_profile.peak_duration_seconds,
        total_ped_arrivals: state.total_ped_arrivals,
        total_ped_departures: state.total_ped_departures,
        total_vehicle_arrivals: state.total_vehicle_arrivals,
        total_vehicle_departures: state.total_vehicle_departures,
        avg_ped_queue: state.total_ped_queue_seconds / total_seconds as f64,
        avg_vehicle_queue: state.total_vehicle_queue_seconds / total_seconds as f64,
        avg_ped_wait_seconds: if state.total_ped_departures == 0 {
            0.0
        } else {
            state.total_ped_queue_seconds / state.total_ped_departures as f64
        },
        avg_vehicle_wait_seconds: if state.total_vehicle_departures == 0 {
            0.0
        } else {
            state.total_vehicle_queue_seconds / state.total_vehicle_departures as f64
        },
        max_ped_queue: state.max_ped_queue,
        max_vehicle_queue: state.max_vehicle_queue,
        final_ped_queue: state.ped_queue,
        final_vehicle_queue: state.veh_queue,
        peak_end_ped_queue: state.peak_end_ped_queue.unwrap_or(state.ped_queue),
        stabilization_time_seconds: state.stabilization_time_seconds,
    };

    SimulationResult { summary, rows }
}

pub fn write_csv(result: &SimulationResult, file_name: &str) -> std::io::Result<PathBuf> {
    let output_dir = Path::new("tests").join("simulation_results");
    fs::create_dir_all(&output_dir)?;

    let output_path = output_dir.join(file_name);
    let file = File::create(&output_path)?;
    let mut writer = BufWriter::new(file);

    writeln!(
        writer,
        "second,cycle,phase,ped_arrival_rate,ped_arrivals,ped_departures,ped_queue,veh_arrivals,veh_departures,veh_queue,ped_extension_seconds"
    )?;

    for row in &result.rows {
        writeln!(
            writer,
            "{},{},{},{:.4},{},{},{},{},{},{},{}",
            row.second,
            row.cycle,
            row.phase,
            row.ped_arrival_rate,
            row.ped_arrivals,
            row.ped_departures,
            row.ped_queue,
            row.veh_arrivals,
            row.veh_departures,
            row.veh_queue,
            row.ped_extension_seconds
        )?;
    }

    writer.flush()?;
    Ok(output_path)
}

pub fn write_surface_csv(
    summaries: &[SimulationSummary],
    file_name: &str,
) -> std::io::Result<PathBuf> {
    let output_dir = Path::new("tests").join("simulation_results");
    fs::create_dir_all(&output_dir)?;

    let output_path = output_dir.join(file_name);
    let file = File::create(&output_path)?;
    let mut writer = BufWriter::new(file);

    writeln!(
        writer,
        "controller,pedestrian_green_seconds,vehicle_green_seconds,pedestrian_peak_rate,pedestrian_recovery_rate,peak_duration_seconds,total_seconds,total_ped_arrivals,total_ped_departures,total_vehicle_arrivals,total_vehicle_departures,avg_ped_queue,avg_vehicle_queue,avg_ped_wait_seconds,avg_vehicle_wait_seconds,max_ped_queue,max_vehicle_queue,final_ped_queue,final_vehicle_queue,peak_end_ped_queue,stabilization_time_seconds"
    )?;

    for summary in summaries {
        writeln!(
            writer,
            "{},{},{},{:.4},{:.4},{},{},{},{},{},{},{:.4},{:.4},{:.4},{:.4},{},{},{},{},{},{}",
            summary.controller,
            summary.pedestrian_green_seconds,
            summary.vehicle_green_seconds,
            summary.pedestrian_peak_rate,
            summary.pedestrian_recovery_rate,
            summary.peak_duration_seconds,
            summary.total_seconds,
            summary.total_ped_arrivals,
            summary.total_ped_departures,
            summary.total_vehicle_arrivals,
            summary.total_vehicle_departures,
            summary.avg_ped_queue,
            summary.avg_vehicle_queue,
            summary.avg_ped_wait_seconds,
            summary.avg_vehicle_wait_seconds,
            summary.max_ped_queue,
            summary.max_vehicle_queue,
            summary.final_ped_queue,
            summary.final_vehicle_queue,
            summary.peak_end_ped_queue,
            summary
                .stabilization_time_seconds
                .map(|value| value.to_string())
                .unwrap_or_default()
        )?;
    }

    writer.flush()?;
    Ok(output_path)
}

pub fn print_summary(summary: &SimulationSummary, csv_path: &Path) {
    println!(
        "\ncontroller={}\nped_green={}s, veh_green={}s\ncsv={}\nped_arrivals={}, ped_departures={}, avg_ped_queue={:.2}, avg_ped_wait={:.2}s, max_ped_queue={}, final_ped_queue={}\nveh_arrivals={}, veh_departures={}, avg_veh_queue={:.2}, avg_veh_wait={:.2}s, max_veh_queue={}, final_veh_queue={}\npeak_end_ped_queue={}, stabilization_time={}s",
        summary.controller,
        summary.pedestrian_green_seconds,
        summary.vehicle_green_seconds,
        csv_path.display(),
        summary.total_ped_arrivals,
        summary.total_ped_departures,
        summary.avg_ped_queue,
        summary.avg_ped_wait_seconds,
        summary.max_ped_queue,
        summary.final_ped_queue,
        summary.total_vehicle_arrivals,
        summary.total_vehicle_departures,
        summary.avg_vehicle_queue,
        summary.avg_vehicle_wait_seconds,
        summary.max_vehicle_queue,
        summary.final_vehicle_queue,
        summary.peak_end_ped_queue,
        summary
            .stabilization_time_seconds
            .map(|value| value.to_string())
            .unwrap_or_else(|| "not_reached".to_string())
    );
}
