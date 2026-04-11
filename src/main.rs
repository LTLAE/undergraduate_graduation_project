mod config;
mod traffic_light;
mod fuzzy_inference;
mod membership_fns;
mod call_py_yolo;
mod camera;
mod sql_ops;
mod traffic_light_sim;

fn main() {
    traffic_light_sim::run();
}
