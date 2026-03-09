use crate::config;

#[derive(Debug, PartialEq)]
// Debug for printing, PartialEq for using == in set_sig() to avoid ped yellow light
enum TrafficSign {
    RED,
    YELLOW,
    GREEN,
}

#[derive(Debug)]
enum LightState {
    OFF,
    SOLID,
    BLINKING,
}

struct TrafficLight {
    pub sig_ped: (TrafficSign, LightState),
    pub sig_veh: (TrafficSign, LightState),
    pub time: f64,
}

impl TrafficLight {
    // sig_pos must be either "ped" or "veh"
    pub fn set_sig(&mut self, sig_pos: &str, next_sign: TrafficSign) {
        match sig_pos {
            "ped" => {
                // since ped have no yellow light, raise error when trying to do so
                if next_sign == TrafficSign::YELLOW {
                    panic!("Pedestrian signal cannot be yellow");
                }
                self.sig_ped.0 = next_sign
            },
            "veh" => self.sig_veh.0 = next_sign,
            _ => panic!("Invalid signal position: {}", sig_pos),
        }
    }

    // I am thinking something like a graphic drive
    // Program would not stop changing the light status, and when it is the time, show it to the screen
    // After changing, we call something like display() and light the light (hhh)
    pub fn display(&self) {
        // Just put these here... for now
        println!(
            "Pedestrian signal: {:?} ({:?}), Vehicle signal: {:?} ({:?}), Time: {:.1} seconds",
            self.sig_ped.0, self.sig_ped.1,
            self.sig_veh.0, self.sig_veh.1,
            self.time
        );
    }

    // before using it, remember to set time to self.time
    pub fn thread_wait(&self) {
        std::thread::sleep(std::time::Duration::from_secs_f64(self.time));
    }

    // workflow of ped -> red, veh -> green
    pub fn ped_to_red_veh_to_green(&mut self) {
        // ped go red
        self.sig_ped = (TrafficSign::RED, LightState::SOLID);
        self.display();
        // wait <ALL_RED> seconds of all red
        self.time = config::ALL_RED;
        self.thread_wait();
        // veh go green
        self.sig_veh = (TrafficSign::GREEN, LightState::SOLID);
        self.display();
    }

    // workflow of veh -> red, ped -> green
    pub fn veh_to_red_ped_to_green(&mut self) {
        // veh go yellow for <VEH_YELLOW> seconds
        self.sig_veh = (TrafficSign::YELLOW, LightState::SOLID);
        self.display();
        self.time = config::VEH_YELLOW;
        self.thread_wait();
        // veh go red
        self.sig_veh = (TrafficSign::RED, LightState::SOLID);
        self.display();
        // wait <ALL_RED> seconds of all red
        self.time = config::ALL_RED;
        self.thread_wait();
        // ped go green
        self.sig_ped = (TrafficSign::GREEN, LightState::SOLID);
        self.display();
    }
}