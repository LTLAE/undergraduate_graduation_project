mod def;

use def::{
    print_summary, run_simulation, write_csv, ControllerKind, SimulationParams, DEFAULT_SEED,
    SIMULATION_SECONDS,
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
