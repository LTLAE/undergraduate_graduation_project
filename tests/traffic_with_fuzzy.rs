mod def;

use def::{
    print_summary, run_simulation, run_simulation_with_profile, write_csv, ArrivalProfile,
    ControllerKind, SimulationParams, DEFAULT_SEED, PEDESTRIAN_RECOVERY_ARRIVAL_RATE,
    RECOVERY_SIMULATION_SECONDS, SIMULATION_SECONDS,
};

#[test]
fn export_fuzzy_phase_csv() {
    let params = SimulationParams::default();
    let result = run_simulation(
        ControllerKind::Fuzzy,
        params,
        DEFAULT_SEED,
        SIMULATION_SECONDS,
    );
    let csv_path = write_csv(&result, "fuzzy_phase.csv").expect("failed to write fuzzy-phase csv");

    assert!(!result.rows.is_empty(), "fuzzy simulation produced no rows");
    assert!(csv_path.exists(), "fuzzy csv was not created");

    print_summary(&result.summary, &csv_path);
}

#[test]
fn export_fuzzy_recovery_csv() {
    let params = SimulationParams::default();
    let arrival_profile = ArrivalProfile {
        pedestrian_recovery_rate: PEDESTRIAN_RECOVERY_ARRIVAL_RATE,
        ..ArrivalProfile::default()
    };
    let result = run_simulation_with_profile(
        ControllerKind::Fuzzy,
        params,
        arrival_profile,
        DEFAULT_SEED,
        RECOVERY_SIMULATION_SECONDS,
    );
    let csv_path =
        write_csv(&result, "fuzzy_recovery.csv").expect("failed to write fuzzy recovery csv");

    assert!(
        !result.rows.is_empty(),
        "fuzzy recovery simulation produced no rows"
    );
    assert!(csv_path.exists(), "fuzzy recovery csv was not created");

    print_summary(&result.summary, &csv_path);
}
