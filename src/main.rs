use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    // traffic_light_control::sim::traffic_light_sim::run();

    if let Err(error) = run_all_simulations_and_plots() {
        eprintln!("Failed to generate simulation data and plots: {error}");
        std::process::exit(1);
    }
}

fn run_all_simulations_and_plots() -> Result<(), String> {
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    println!("Running fixed-phase simulation suite...");
    run_command(
        &project_root,
        "cargo",
        &["test", "--test", "traffic_fix_phase", "--", "--nocapture"],
    )?;

    println!("Running fuzzy-control simulation suite...");
    run_command(
        &project_root,
        "cargo",
        &["test", "--test", "traffic_with_fuzzy", "--", "--nocapture"],
    )?;

    println!("Rendering gnuplot figures...");
    for script in [
        "gnuplot/peak_20min_queue_lines.gp",
        "gnuplot/peak_20min_ped_queue_surface.gp",
        "gnuplot/recovery_queue_lines.gp",
    ] {
        run_command(&project_root, "gnuplot", &[script])?;
    }

    println!("\nCompleted simulation export and plot generation.");
    println!(
        "CSV and PNG files are available under {}",
        project_root.join("tests/simulation_results").display()
    );

    Ok(())
}

fn run_command(project_root: &Path, program: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(program)
        .current_dir(project_root)
        .args(args)
        .status()
        .map_err(|error| format!("failed to start `{program} {}`: {error}", args.join(" ")))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "`{program} {}` exited with status {status}",
            args.join(" ")
        ))
    }
}
