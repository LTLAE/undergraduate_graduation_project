mod call_py_yolo;
pub mod fuzzy_inference;
mod membership_fns;
mod sql_ops;
pub(crate) mod traffic_light;

use pyo3::PyResult;

pub use sql_ops::SQLError;
pub(crate) use traffic_light::{LightState, TrafficLight, TrafficLightPosition, TrafficSign};

pub(crate) fn calculate_extension_time(ped_count: i32, veh_count: i32) -> f64 {
    fuzzy_inference::get_extension_time(ped_count, veh_count)
}

pub(crate) fn detect_people(image_path: &str) -> PyResult<i32> {
    call_py_yolo::count_people(image_path)
}

pub(crate) fn detect_cars(image_path: &str) -> PyResult<i32> {
    call_py_yolo::count_cars(image_path)
}

pub(crate) fn init_history_store(init_sql_path: &str) -> Result<(), SQLError> {
    sql_ops::init_sqlite_db(init_sql_path)
}

pub(crate) fn record_pedestrian_count(obj_count: i32) -> Result<(), SQLError> {
    sql_ops::insert_pedestrian_count(obj_count)
}

pub(crate) fn record_vehicle_count(obj_count: i32) -> Result<(), SQLError> {
    sql_ops::insert_vehicle_count(obj_count)
}

pub(crate) fn get_historical_pedestrian_average() -> Result<i32, SQLError> {
    sql_ops::get_avg_pedestrian_count()
}

pub(crate) fn get_historical_vehicle_average() -> Result<i32, SQLError> {
    sql_ops::get_avg_vehicle_count()
}
