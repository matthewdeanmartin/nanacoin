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

pub const DEFAULT_PHRASES: [&str; 3] = [
    "Katie, do you really need all of those drugs?",
    "Only if you also have a lemon square!",
    "Thank you for donating blood, Mr Thiel. Here's a shiny nana-nickel",
];
pub const DEFAULT_PHRASE: &str = DEFAULT_PHRASES[0];

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightConfig {
    pub phrases: [heapless::String<80>; 3],
}
impl Default for LightConfig {
    fn default() -> Self {
        Self {
            phrases: DEFAULT_PHRASES.map(|s| s.try_into().expect("bounded default")),
        }
    }
}
impl LightConfig {
    pub fn valid(&self) -> bool {
        self.phrases.iter().all(|p| valid_phrase(p))
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, crate::domain::Error> {
        let (config, used): (Self, _) =
            serde_json_core::from_slice(bytes).map_err(|_| crate::domain::Error::CorruptJournal)?;
        if !config.valid() || bytes[used..].iter().any(|b| !b.is_ascii_whitespace()) {
            return Err(crate::domain::Error::CorruptJournal);
        }
        Ok(config)
    }
    pub fn encode(&self) -> Result<heapless::Vec<u8, 512>, crate::domain::Error> {
        serde_json_core::to_vec(self).map_err(|_| crate::domain::Error::Storage)
    }
}

pub const MAX_PHRASE: usize = 80;
pub const MORSE_UNIT_MS: u64 = 200; // Six words/minute (PARIS convention).

fn morse(ch: u8) -> Option<&'static str> {
    Some(match ch.to_ascii_uppercase() {
        b'A' => ".-",
        b'B' => "-...",
        b'C' => "-.-.",
        b'D' => "-..",
        b'E' => ".",
        b'F' => "..-.",
        b'G' => "--.",
        b'H' => "....",
        b'I' => "..",
        b'J' => ".---",
        b'K' => "-.-",
        b'L' => ".-..",
        b'M' => "--",
        b'N' => "-.",
        b'O' => "---",
        b'P' => ".--.",
        b'Q' => "--.-",
        b'R' => ".-.",
        b'S' => "...",
        b'T' => "-",
        b'U' => "..-",
        b'V' => "...-",
        b'W' => ".--",
        b'X' => "-..-",
        b'Y' => "-.--",
        b'Z' => "--..",
        b'0' => "-----",
        b'1' => ".----",
        b'2' => "..---",
        b'3' => "...--",
        b'4' => "....-",
        b'5' => ".....",
        b'6' => "-....",
        b'7' => "--...",
        b'8' => "---..",
        b'9' => "----.",
        b'.' => ".-.-.-",
        b',' => "--..--",
        b'?' => "..--..",
        b'!' => "-.-.--",
        b'-' => "-....-",
        b'/' => "-..-.",
        b'\'' => ".----.",
        b'"' => ".-..-.",
        b'(' => "-.--.",
        b')' => "-.--.-",
        b':' => "---...",
        b';' => "-.-.-.",
        b'=' => "-...-",
        b'+' => ".-.-.",
        b'@' => ".--.-.",
        b'_' => "..--.-",
        _ => return None,
    })
}

pub fn valid_phrase(phrase: &str) -> bool {
    !phrase.trim().is_empty()
        && phrase.len() <= MAX_PHRASE
        && phrase.bytes().all(|c| c == b' ' || morse(c).is_some())
}

/// Precomputed only when the setting changes. No allocations or service lock
/// during playback. Marks are cyan; three slower green flashes end each cycle.
pub struct MorsePattern {
    marks: Vec<(u16, u16)>,
    message_end: u64,
    cycle: u64,
}
impl MorsePattern {
    pub fn new(phrase: &str) -> Self {
        let phrase = if valid_phrase(phrase) {
            phrase
        } else {
            DEFAULT_PHRASE
        };
        let mut marks = Vec::new();
        let mut t = 0;
        let mut first_word = true;
        for word in phrase.split_whitespace() {
            if !first_word {
                t += 7;
            }
            first_word = false;
            for (i, ch) in word.bytes().enumerate() {
                if i > 0 {
                    t += 3;
                }
                if let Some(code) = morse(ch) {
                    for (j, mark) in code.bytes().enumerate() {
                        if j > 0 {
                            t += 1;
                        }
                        let end = t + if mark == b'.' { 1 } else { 3 };
                        marks.push((t as u16, end as u16));
                        t = end;
                    }
                }
            }
        }
        Self {
            marks,
            message_end: t,
            cycle: t + 7 + 3 * 6 + 7,
        }
    }
    pub fn color(&self, elapsed_ms: u64) -> [u8; 3] {
        let t = elapsed_ms / MORSE_UNIT_MS % self.cycle;
        if t < self.message_end {
            if self
                .marks
                .iter()
                .any(|&(start, end)| t >= u64::from(start) && t < u64::from(end))
            {
                [0, 5, 8]
            } else {
                [0, 0, 0]
            }
        } else if t >= self.message_end + 7
            && t < self.message_end + 7 + 18
            && (t - self.message_end - 7) % 6 < 3
        {
            [0, 8, 0]
        } else {
            [0, 0, 0]
        }
    }
}

/// Each message includes its own three green flashes and pause.
pub struct Rotation {
    patterns: [MorsePattern; 3],
    cycle_ms: u64,
}
impl Rotation {
    pub fn new(config: &LightConfig) -> Self {
        let patterns = std::array::from_fn(|i| MorsePattern::new(&config.phrases[i]));
        let cycle_ms = patterns.iter().map(|p| p.cycle * MORSE_UNIT_MS).sum();
        Self { patterns, cycle_ms }
    }
    pub fn color(&self, elapsed_ms: u64) -> [u8; 3] {
        let mut t = elapsed_ms % self.cycle_ms;
        for p in &self.patterns {
            let length = p.cycle * MORSE_UNIT_MS;
            if t < length {
                return p.color(t);
            }
            t -= length;
        }
        [0, 0, 0]
    }
}

#[cfg(test)]
mod morse_tests {
    use super::*;
    #[test]
    fn timing_and_three_healthy_flashes() {
        let p = MorsePattern::new("E T");
        assert_eq!(p.marks, vec![(0, 1), (8, 11)]);
        assert_eq!(p.message_end, 11);
        assert_eq!(p.color(0), [0, 5, 8]);
        assert_eq!(p.color(200), [0, 0, 0]);
        assert_eq!(p.color(1600), [0, 5, 8]);
        let green: Vec<_> = (11..p.cycle)
            .filter(|t| p.color(t * 200) == [0, 8, 0])
            .collect();
        assert_eq!(green, vec![18, 19, 20, 24, 25, 26, 30, 31, 32]);
        assert_eq!(p.color(p.cycle * 200), p.color(0));
        assert_eq!(
            MorsePattern::new("AA").marks,
            vec![(0, 1), (2, 5), (8, 9), (10, 13)]
        );
    }
    #[test]
    fn validation_and_default() {
        assert!(LightConfig::default().valid());
        let config = LightConfig::default();
        assert_eq!(
            LightConfig::decode(&config.encode().unwrap()).unwrap(),
            config
        );
        assert!(valid_phrase(DEFAULT_PHRASE));
        assert!(valid_phrase(&"A".repeat(MAX_PHRASE)));
        for bad in ["", "   ", "hello 🌍", "a\nb", "a#b"] {
            assert!(!valid_phrase(bad));
        }
        assert!(!valid_phrase(&"A".repeat(MAX_PHRASE + 1)));
        assert_eq!(
            MorsePattern::new("e t").marks,
            MorsePattern::new("E T").marks
        );
        assert_eq!(morse(b'!'), Some("-.-.--"));
    }
}

#[cfg(test)]
mod rotation_tests {
    use super::*;
    #[test]
    fn visits_three_distinct_messages_in_order_then_wraps() {
        let config = LightConfig {
            phrases: ["E", "T", "I"].map(|p| p.try_into().unwrap()),
        };
        let rotation = Rotation::new(&config);
        let mut start = 0;
        for p in &rotation.patterns {
            for ms in (0..p.cycle * MORSE_UNIT_MS).step_by(100) {
                assert_eq!(rotation.color(start + ms), p.color(ms));
            }
            start += p.cycle * MORSE_UNIT_MS;
        }
        assert_eq!(start, rotation.cycle_ms);
        assert_eq!(rotation.color(start + 200), rotation.color(200));
    }
}
