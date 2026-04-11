mod config;
mod http;
mod sim;
mod traffic;

fn main() {
    sim::traffic_light_sim::run();
}
