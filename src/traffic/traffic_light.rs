#[derive(Debug, Clone, PartialEq)]
// Debug for printing, PartialEq for using == in set_sig() to avoid ped yellow light
pub(crate) enum TrafficSign {
    Red,
    Yellow,
    Green,
}

#[derive(Debug, Clone)]
pub(crate) enum LightState {
    Off,
    Solid,
    Blinking,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum TrafficLightPosition {
    Ped1,
    Veh1,
}

#[derive(Clone)]
pub(crate) struct TrafficLight {
    pub(crate) sig_ped: (TrafficSign, LightState),
    pub(crate) sig_veh: (TrafficSign, LightState),
    pub(crate) time: f64,
}

impl TrafficLight {
    // Sets the signal color and state
    pub(crate) fn set_sig(&mut self, sig_pos: TrafficLightPosition, next_sign: TrafficSign, next_state: LightState) {
        println!("Changing {:?} signal to {:?} ({:?})", sig_pos, next_sign, next_state);
        match sig_pos {
            TrafficLightPosition::Ped1 => {
                // since ped have no yellow light, raise error when trying to do so
                if next_sign == TrafficSign::Yellow {
                    panic!("Pedestrian signal cannot be yellow");
                }
                self.sig_ped = (next_sign, next_state);
            },
            TrafficLightPosition::Veh1 => {
                self.sig_veh = (next_sign, next_state);
            },
        }
    }
}
