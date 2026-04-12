mod def;

use def::{
    print_summary, run_simulation, write_csv, write_surface_csv, ControllerKind, SimulationParams,
    DEFAULT_SEED, SIMULATION_SECONDS,
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
