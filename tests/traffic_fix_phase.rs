mod def;

use def::{
    print_summary, run_simulation, run_simulation_with_profile, write_csv, write_surface_csv,
    ArrivalProfile, ControllerKind, SimulationParams, DEFAULT_SEED, PEAK_PLOT_SECONDS,
    PEDESTRIAN_RECOVERY_ARRIVAL_RATE, RECOVERY_SIMULATION_SECONDS, SIMULATION_SECONDS,
};

#[test]
fn export_fixed_phase_csv() {
    let params = SimulationParams::default();
    let result = run_simulation(
        ControllerKind::Fixed,
        params,
        DEFAULT_SEED,
        SIMULATION_SECONDS,
    );
    let csv_path = write_csv(&result, "fixed_phase.csv").expect("failed to write fixed-phase csv");

    assert!(
        !result.rows.is_empty(),
        "fixed-phase simulation produced no rows"
    );
    assert!(csv_path.exists(), "fixed-phase csv was not created");

    print_summary(&result.summary, &csv_path);
}

#[test]
fn export_fixed_peak_20min_csv() {
    let params = SimulationParams::default();
    let result = run_simulation(
        ControllerKind::Fixed,
        params,
        DEFAULT_SEED,
        PEAK_PLOT_SECONDS,
    );
    let csv_path =
        write_csv(&result, "fixed_peak_20min.csv").expect("failed to write fixed 20min csv");

    assert!(
        !result.rows.is_empty(),
        "fixed 20min simulation produced no rows"
    );
    assert!(csv_path.exists(), "fixed 20min csv was not created");

    print_summary(&result.summary, &csv_path);
}

#[test]
fn export_fixed_phase_surface_csv() {
    let mut summaries = Vec::new();

    for pedestrian_green_seconds in 15..=60 {
        for vehicle_green_seconds in 15..=60 {
            let params = SimulationParams {
                pedestrian_green_seconds,
                vehicle_green_seconds,
                ..SimulationParams::default()
            };

            let result = run_simulation(
                ControllerKind::Fixed,
                params,
                DEFAULT_SEED,
                SIMULATION_SECONDS,
            );
            summaries.push(result.summary);
        }
    }

    let csv_path = write_surface_csv(&summaries, "fixed_phase_surface.csv")
        .expect("failed to write fixed-phase surface csv");

    assert_eq!(
        summaries.len(),
        46 * 46,
        "unexpected number of parameter pairs"
    );
    assert!(csv_path.exists(), "fixed-phase surface csv was not created");

    println!(
        "\nfixed-phase parameter sweep exported to {} with {} rows",
        csv_path.display(),
        summaries.len()
    );
}

#[test]
fn export_fixed_peak_20min_surface_csv() {
    let mut summaries = Vec::new();

    for pedestrian_green_seconds in 15..=60 {
        for vehicle_green_seconds in 15..=60 {
            let params = SimulationParams {
                pedestrian_green_seconds,
                vehicle_green_seconds,
                ..SimulationParams::default()
            };

            let result = run_simulation(
                ControllerKind::Fixed,
                params,
                DEFAULT_SEED,
                PEAK_PLOT_SECONDS,
            );
            summaries.push(result.summary);
        }
    }

    let csv_path = write_surface_csv(&summaries, "fixed_peak_20min_surface.csv")
        .expect("failed to write fixed 20min surface csv");

    assert_eq!(
        summaries.len(),
        46 * 46,
        "unexpected number of parameter pairs"
    );
    assert!(csv_path.exists(), "fixed 20min surface csv was not created");

    println!(
        "\nfixed 20min parameter sweep exported to {} with {} rows",
        csv_path.display(),
        summaries.len()
    );
}

#[test]
fn export_fixed_phase_recovery_surface_csv() {
    let arrival_profile = ArrivalProfile {
        pedestrian_recovery_rate: PEDESTRIAN_RECOVERY_ARRIVAL_RATE,
        ..ArrivalProfile::default()
    };
    let mut summaries = Vec::new();

    for pedestrian_green_seconds in 15..=60 {
        for vehicle_green_seconds in 15..=60 {
            let params = SimulationParams {
                pedestrian_green_seconds,
                vehicle_green_seconds,
                ..SimulationParams::default()
            };

            let result = run_simulation_with_profile(
                ControllerKind::Fixed,
                params,
                arrival_profile,
                DEFAULT_SEED,
                RECOVERY_SIMULATION_SECONDS,
            );
            summaries.push(result.summary);
        }
    }

    let csv_path = write_surface_csv(&summaries, "fixed_phase_recovery_surface.csv")
        .expect("failed to write fixed-phase recovery surface csv");

    assert_eq!(
        summaries.len(),
        46 * 46,
        "unexpected number of parameter pairs"
    );
    assert!(
        csv_path.exists(),
        "fixed-phase recovery surface csv was not created"
    );

    println!(
        "\nfixed-phase recovery sweep exported to {} with {} rows",
        csv_path.display(),
        summaries.len()
    );
}

#[test]
fn export_fixed_recovery_csv() {
    let params = SimulationParams::default();
    let arrival_profile = ArrivalProfile {
        pedestrian_recovery_rate: PEDESTRIAN_RECOVERY_ARRIVAL_RATE,
        ..ArrivalProfile::default()
    };
    let result = run_simulation_with_profile(
        ControllerKind::Fixed,
        params,
        arrival_profile,
        DEFAULT_SEED,
        RECOVERY_SIMULATION_SECONDS,
    );
    let csv_path =
        write_csv(&result, "fixed_recovery.csv").expect("failed to write fixed recovery csv");

    assert!(
        !result.rows.is_empty(),
        "fixed recovery simulation produced no rows"
    );
    assert!(csv_path.exists(), "fixed recovery csv was not created");

    print_summary(&result.summary, &csv_path);
}
