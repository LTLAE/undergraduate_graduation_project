//! Public service entrypoint.
//!
//! This module centralizes the interfaces that will later be mounted onto the
//! Docker-exposed HTTP server.

use pyo3::PyResult;

pub use crate::traffic::SQLError;

pub fn calculate_extension_time(ped_count: i32, veh_count: i32) -> f64 {
    crate::traffic::calculate_extension_time(ped_count, veh_count)
}

pub fn detect_people(image_path: &str) -> PyResult<i32> {
    crate::traffic::detect_people(image_path)
}

pub fn detect_cars(image_path: &str) -> PyResult<i32> {
    crate::traffic::detect_cars(image_path)
}

pub fn init_history_store(init_sql_path: &str) -> Result<(), SQLError> {
    crate::traffic::init_history_store(init_sql_path)
}

pub fn record_pedestrian_count(obj_count: i32) -> Result<(), SQLError> {
    crate::traffic::record_pedestrian_count(obj_count)
}

pub fn record_vehicle_count(obj_count: i32) -> Result<(), SQLError> {
    crate::traffic::record_vehicle_count(obj_count)
}

pub fn get_historical_pedestrian_average() -> Result<i32, SQLError> {
    crate::traffic::get_historical_pedestrian_average()
}

pub fn get_historical_vehicle_average() -> Result<i32, SQLError> {
    crate::traffic::get_historical_vehicle_average()
}
