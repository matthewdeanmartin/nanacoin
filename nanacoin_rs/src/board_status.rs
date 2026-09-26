//! Pure status-light policy. Hardware output is optional and board-specific.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LedPin {
    Gpio38,
    Gpio48,
}

/// No autodetection or arbitrary pin probing: unknown settings disable output.
pub fn led_pin(value: Option<&str>) -> Result<Option<LedPin>, &'static str> {
    match value.map(str::trim) {
        None | Some("" | "off") => Ok(None),
        Some("38") => Ok(Some(LedPin::Gpio38)),
        Some("48") => Ok(Some(LedPin::Gpio48)),
        _ => Err("expected off, 38, or 48; LED disabled"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Starting,
    Connecting,
    Setup,
    Healthy,
    Degraded,
    Stalled,
    Fatal,
}

pub struct Health {
    pub ready: bool,
    pub wifi: bool,
    pub setup: bool,
    pub workers: bool,
    pub mdns: bool,
    pub fatal: bool,
    pub recent_error: bool,
}

impl Health {
    pub fn state(&self) -> State {
        if self.fatal {
            State::Fatal
        } else if self.ready && !self.workers {
            State::Stalled
        } else if !self.wifi {
            if self.setup {
                State::Setup
            } else {
                State::Connecting
            }
        } else if !self.ready {
            State::Starting
        } else if !self.mdns || self.recent_error {
            State::Degraded
        } else {
            State::Healthy
        }
    }
}

/// RGB, brightness capped at 12/255. Healthy's brief dark beat is meaningful
/// only while the caller supplies fresh progress from *both* server workers.
pub fn color(state: State, elapsed_ms: u64) -> [u8; 3] {
    match state {
        State::Healthy => {
            if elapsed_ms % 2000 < 150 {
                [0, 0, 0]
            } else {
                [0, 8, 0]
            }
        }
        State::Connecting => {
            let step = (elapsed_ms % 2000 / 100) as u8;
            [0, 0, 2 + if step < 10 { step } else { 19 - step }]
        }
        State::Setup => {
            if elapsed_ms % 2000 < 1000 {
                [0, 5, 8]
            } else {
                [0, 0, 0]
            }
        }
        State::Starting => [4, 4, 4],
        State::Degraded => [8, 5, 0],
        State::Stalled | State::Fatal => [8, 0, 0],
    }
}

pub fn reset_flashes(reason: &str) -> u64 {
    match reason {
        "power on" => 1,
        "software restart" | "reset pin" | "USB" => 2,
        "crash" => 3,
        "watchdog" => 4,
        "brownout (power dipped)" => 5,
        _ => 6,
    }
}

/// A visible 1.5-second white startup marker, a 0.5-second gap, then
/// short reset-class flashes. This can only run after the firmware starts.
pub fn reset_color(elapsed_ms: u64, flashes: u64) -> Option<[u8; 3]> {
    if elapsed_ms < 1500 {
        return Some([5, 5, 5]);
    }
    if elapsed_ms < 2000 {
        return Some([0, 0, 0]);
    }
    let elapsed_ms = elapsed_ms - 2000;
    if elapsed_ms >= flashes * 300 + 500 {
        None
    } else if elapsed_ms < flashes * 300 && elapsed_ms % 300 < 100 {
        Some([5, 5, 5])
    } else {
        Some([0, 0, 0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_board_defaults_to_no_pin() {
        assert_eq!(led_pin(None), Ok(None));
        assert_eq!(led_pin(Some("off")), Ok(None));
        assert_eq!(led_pin(Some("48")), Ok(Some(LedPin::Gpio48)));
        assert_eq!(led_pin(Some("38")), Ok(Some(LedPin::Gpio38)));
        for pin in ["19", "26", "0", "-1", "auto", "garbage"] {
            assert!(led_pin(Some(pin)).is_err());
        }
    }

    #[test]
    fn network_loss_and_stalls_cannot_show_green() {
        let mut h = Health {
            ready: true,
            wifi: true,
            setup: false,
            workers: true,
            mdns: true,
            fatal: false,
            recent_error: false,
        };
        assert_eq!(h.state(), State::Healthy);
        h.wifi = false;
        assert_eq!(h.state(), State::Connecting);
        h.setup = true;
        assert_eq!(h.state(), State::Setup);
        h.workers = false;
        assert_eq!(h.state(), State::Stalled);
        h.fatal = true;
        assert_eq!(h.state(), State::Fatal);
        h.fatal = false;
        h.wifi = true;
        h.workers = true;
        h.mdns = false;
        assert_eq!(h.state(), State::Degraded);
        h.mdns = true;
        h.recent_error = true;
        assert_eq!(h.state(), State::Degraded);
    }

    #[test]
    fn patterns_are_dim_and_stalled_has_no_healthy_heartbeat() {
        for state in [
            State::Starting,
            State::Connecting,
            State::Setup,
            State::Healthy,
            State::Degraded,
            State::Stalled,
            State::Fatal,
        ] {
            for ms in 0..4000 {
                assert!(color(state, ms).into_iter().all(|v| v <= 12));
            }
        }
        assert_eq!(color(State::Healthy, 0), [0, 0, 0]);
        assert_eq!(color(State::Healthy, 500), [0, 8, 0]);
        assert_eq!(color(State::Stalled, 0), color(State::Stalled, 500));
        assert_eq!(reset_color(0, reset_flashes("watchdog")), Some([5, 5, 5]));
        assert_eq!(reset_color(1400, 4), Some([5, 5, 5]));
        assert_eq!(reset_color(1700, 4), Some([0, 0, 0]));
        assert_eq!(reset_color(2000, 4), Some([5, 5, 5]));
        assert_eq!(reset_color(3700, 4), None);
    }
}
