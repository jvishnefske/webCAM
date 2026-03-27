//! G-code parser and validator.
//!
//! Ported from cnc-sender (cnc-types + cnc-gcode crates), combined into a
//! single module for simplicity. Strips types we don't need for webCAM
//! (MachinePosition, WorkPosition, Axis, Direction).

use crate::units::SpindleSpeed;

// ── Command types ────────────────────────────────────────────────────

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MotionMode {
    #[default]
    Rapid = 0,
    Linear = 1,
    ClockwiseArc = 2,
    CounterClockwiseArc = 3,
}

impl MotionMode {
    #[must_use]
    pub const fn gcode_number(self) -> u8 {
        match self {
            Self::Rapid => 0,
            Self::Linear => 1,
            Self::ClockwiseArc => 2,
            Self::CounterClockwiseArc => 3,
        }
    }
}

impl std::fmt::Display for MotionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "G{:02}", self.gcode_number())
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DistanceMode {
    #[default]
    Absolute = 0,
    Incremental = 1,
}

impl DistanceMode {
    #[must_use]
    pub const fn gcode_number(self) -> u8 {
        match self {
            Self::Absolute => 90,
            Self::Incremental => 91,
        }
    }
}

impl std::fmt::Display for DistanceMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "G{}", self.gcode_number())
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum UnitMode {
    Inches = 0,
    #[default]
    Millimeters = 1,
}

impl UnitMode {
    #[must_use]
    pub const fn gcode_number(self) -> u8 {
        match self {
            Self::Inches => 20,
            Self::Millimeters => 21,
        }
    }
}

impl std::fmt::Display for UnitMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "G{}", self.gcode_number())
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum SpindleControl {
    #[default]
    Off,
    Clockwise(SpindleSpeed),
    CounterClockwise(SpindleSpeed),
}

impl SpindleControl {
    #[must_use]
    pub const fn is_running(&self) -> bool {
        !matches!(self, Self::Off)
    }

    #[must_use]
    pub const fn speed(&self) -> Option<SpindleSpeed> {
        match self {
            Self::Off => None,
            Self::Clockwise(s) | Self::CounterClockwise(s) => Some(*s),
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CoolantControl {
    #[default]
    Off = 0,
    Mist = 1,
    Flood = 2,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct AxisMask {
    pub x: bool,
    pub y: bool,
    pub z: bool,
}

impl AxisMask {
    pub const ALL: Self = Self {
        x: true,
        y: true,
        z: true,
    };

    #[must_use]
    pub const fn any(&self) -> bool {
        self.x || self.y || self.z
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum WorkOffset {
    #[default]
    G54 = 0,
    G55 = 1,
    G56 = 2,
    G57 = 3,
    G58 = 4,
    G59 = 5,
}

impl WorkOffset {
    #[must_use]
    pub const fn gcode_number(self) -> u8 {
        match self {
            Self::G54 => 54,
            Self::G55 => 55,
            Self::G56 => 56,
            Self::G57 => 57,
            Self::G58 => 58,
            Self::G59 => 59,
        }
    }

    #[must_use]
    pub const fn from_gcode(code: u8) -> Option<Self> {
        match code {
            54 => Some(Self::G54),
            55 => Some(Self::G55),
            56 => Some(Self::G56),
            57 => Some(Self::G57),
            58 => Some(Self::G58),
            59 => Some(Self::G59),
            _ => None,
        }
    }
}

impl std::fmt::Display for WorkOffset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "G{}", self.gcode_number())
    }
}

/// A parsed G-code command.
#[derive(Clone, Debug, PartialEq)]
pub enum GCodeCommand {
    RapidMove {
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
    },
    LinearMove {
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
        feed: Option<f64>,
    },
    ArcMove {
        clockwise: bool,
        x: Option<f64>,
        y: Option<f64>,
        i: Option<f64>,
        j: Option<f64>,
        feed: Option<f64>,
    },
    SetUnits(UnitMode),
    SetDistanceMode(DistanceMode),
    SetWorkOffset(WorkOffset),
    SetSpindle(SpindleControl),
    SetCoolant(CoolantControl),
    Dwell {
        seconds: f64,
    },
    ProgramEnd,
    ProgramPause,
    Home {
        axes: AxisMask,
    },
    ProbeToward {
        z: f64,
        feed: f64,
    },
    Raw(String),
    Comment(String),
}

impl GCodeCommand {
    #[must_use]
    pub const fn is_motion(&self) -> bool {
        matches!(
            self,
            Self::RapidMove { .. } | Self::LinearMove { .. } | Self::ArcMove { .. }
        )
    }

    #[must_use]
    pub const fn is_modal(&self) -> bool {
        matches!(
            self,
            Self::SetUnits(_)
                | Self::SetDistanceMode(_)
                | Self::SetWorkOffset(_)
                | Self::SetSpindle(_)
                | Self::SetCoolant(_)
        )
    }

    #[must_use]
    pub const fn motion_mode(&self) -> Option<MotionMode> {
        match self {
            Self::RapidMove { .. } => Some(MotionMode::Rapid),
            Self::LinearMove { .. } => Some(MotionMode::Linear),
            Self::ArcMove {
                clockwise: true, ..
            } => Some(MotionMode::ClockwiseArc),
            Self::ArcMove {
                clockwise: false, ..
            } => Some(MotionMode::CounterClockwiseArc),
            _ => None,
        }
    }
}

impl std::fmt::Display for GCodeCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RapidMove { x, y, z } => {
                write!(f, "G00")?;
                if let Some(v) = x {
                    write!(f, " X{v:.4}")?;
                }
                if let Some(v) = y {
                    write!(f, " Y{v:.4}")?;
                }
                if let Some(v) = z {
                    write!(f, " Z{v:.4}")?;
                }
                Ok(())
            }
            Self::LinearMove { x, y, z, feed } => {
                write!(f, "G01")?;
                if let Some(v) = x {
                    write!(f, " X{v:.4}")?;
                }
                if let Some(v) = y {
                    write!(f, " Y{v:.4}")?;
                }
                if let Some(v) = z {
                    write!(f, " Z{v:.4}")?;
                }
                if let Some(v) = feed {
                    write!(f, " F{v:.0}")?;
                }
                Ok(())
            }
            Self::ArcMove {
                clockwise,
                x,
                y,
                i,
                j,
                feed,
            } => {
                write!(f, "{}", if *clockwise { "G02" } else { "G03" })?;
                if let Some(v) = x {
                    write!(f, " X{v:.4}")?;
                }
                if let Some(v) = y {
                    write!(f, " Y{v:.4}")?;
                }
                if let Some(v) = i {
                    write!(f, " I{v:.4}")?;
                }
                if let Some(v) = j {
                    write!(f, " J{v:.4}")?;
                }
                if let Some(v) = feed {
                    write!(f, " F{v:.0}")?;
                }
                Ok(())
            }
            Self::SetUnits(mode) => write!(f, "{mode}"),
            Self::SetDistanceMode(mode) => write!(f, "{mode}"),
            Self::SetWorkOffset(offset) => write!(f, "{offset}"),
            Self::SetSpindle(ctrl) => match ctrl {
                SpindleControl::Off => write!(f, "M05"),
                SpindleControl::Clockwise(s) => write!(f, "M03 S{}", s.rpm()),
                SpindleControl::CounterClockwise(s) => write!(f, "M04 S{}", s.rpm()),
            },
            Self::SetCoolant(ctrl) => match ctrl {
                CoolantControl::Off => write!(f, "M09"),
                CoolantControl::Mist => write!(f, "M07"),
                CoolantControl::Flood => write!(f, "M08"),
            },
            Self::Dwell { seconds } => write!(f, "G04 P{seconds:.3}"),
            Self::ProgramEnd => write!(f, "M02"),
            Self::ProgramPause => write!(f, "M00"),
            Self::Home { axes } => {
                write!(f, "G28")?;
                if axes.x {
                    write!(f, " X0")?;
                }
                if axes.y {
                    write!(f, " Y0")?;
                }
                if axes.z {
                    write!(f, " Z0")?;
                }
                Ok(())
            }
            Self::ProbeToward { z, feed } => write!(f, "G38.2 Z{z:.4} F{feed:.0}"),
            Self::Raw(s) => write!(f, "{s}"),
            Self::Comment(s) => write!(f, "({s})"),
        }
    }
}

// ── Parser ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    InvalidNumber(String),
    UnknownGCode(u8),
    UnknownMCode(u8),
    MissingParameter(char),
    EmptyLine,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidNumber(s) => write!(f, "Invalid number: {s}"),
            Self::UnknownGCode(n) => write!(f, "Unknown G-code: G{n}"),
            Self::UnknownMCode(n) => write!(f, "Unknown M-code: M{n}"),
            Self::MissingParameter(c) => write!(f, "Missing parameter: {c}"),
            Self::EmptyLine => write!(f, "Empty line"),
        }
    }
}

/// Parses a single G-code line into a command.
pub fn parse_line(line: &str) -> Result<GCodeCommand, ParseError> {
    let line = strip_comments(line);
    let line = line.trim().to_uppercase();

    if line.is_empty() {
        return Err(ParseError::EmptyLine);
    }

    if line.starts_with('(') || line.starts_with(';') {
        return Ok(GCodeCommand::Comment(line));
    }

    let words = parse_words(&line)?;

    let g_code = words.iter().find(|(c, _)| *c == 'G');
    let m_code = words.iter().find(|(c, _)| *c == 'M');

    match (g_code, m_code) {
        (Some(&(_, g)), _) => parse_g_code(g as u8, &words),
        (None, Some(&(_, m))) => parse_m_code(m as u8, &words),
        (None, None) => {
            if words.iter().any(|(c, _)| matches!(c, 'X' | 'Y' | 'Z')) {
                parse_g_code(1, &words)
            } else {
                Ok(GCodeCommand::Raw(line))
            }
        }
    }
}

fn strip_comments(line: &str) -> &str {
    let line = line.split(';').next().unwrap_or(line);
    if let Some(paren_start) = line.find('(') {
        if let Some(paren_end) = line.find(')') {
            let before = &line[..paren_start];
            let after = &line[paren_end + 1..];
            if !before.trim().is_empty() {
                return before;
            }
            if !after.trim().is_empty() {
                return after;
            }
        }
    }
    line
}

fn parse_words(line: &str) -> Result<Vec<(char, f64)>, ParseError> {
    let mut words = Vec::new();
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
        }

        if c.is_ascii_alphabetic() {
            let letter = c;
            let mut num_str = String::new();

            if chars.peek() == Some(&'-') {
                num_str.push(chars.next().unwrap());
            }

            while let Some(&ch) = chars.peek() {
                if ch.is_ascii_digit() || ch == '.' {
                    num_str.push(chars.next().unwrap());
                } else {
                    break;
                }
            }

            if num_str.is_empty() {
                words.push((letter, 0.0));
            } else {
                let value: f64 = num_str
                    .parse()
                    .map_err(|_| ParseError::InvalidNumber(num_str))?;
                words.push((letter, value));
            }
        }
    }

    Ok(words)
}

fn parse_g_code(code: u8, words: &[(char, f64)]) -> Result<GCodeCommand, ParseError> {
    let get_word =
        |letter: char| -> Option<f64> { words.iter().find(|(c, _)| *c == letter).map(|(_, v)| *v) };

    match code {
        0 => Ok(GCodeCommand::RapidMove {
            x: get_word('X'),
            y: get_word('Y'),
            z: get_word('Z'),
        }),
        1 => Ok(GCodeCommand::LinearMove {
            x: get_word('X'),
            y: get_word('Y'),
            z: get_word('Z'),
            feed: get_word('F'),
        }),
        2 => Ok(GCodeCommand::ArcMove {
            clockwise: true,
            x: get_word('X'),
            y: get_word('Y'),
            i: get_word('I'),
            j: get_word('J'),
            feed: get_word('F'),
        }),
        3 => Ok(GCodeCommand::ArcMove {
            clockwise: false,
            x: get_word('X'),
            y: get_word('Y'),
            i: get_word('I'),
            j: get_word('J'),
            feed: get_word('F'),
        }),
        4 => {
            let seconds = get_word('P').unwrap_or(0.0);
            Ok(GCodeCommand::Dwell { seconds })
        }
        20 => Ok(GCodeCommand::SetUnits(UnitMode::Inches)),
        21 => Ok(GCodeCommand::SetUnits(UnitMode::Millimeters)),
        28 => {
            let axes = AxisMask {
                x: get_word('X').is_some(),
                y: get_word('Y').is_some(),
                z: get_word('Z').is_some(),
            };
            Ok(GCodeCommand::Home { axes })
        }
        38 => {
            let z = get_word('Z').ok_or(ParseError::MissingParameter('Z'))?;
            let feed = get_word('F').ok_or(ParseError::MissingParameter('F'))?;
            Ok(GCodeCommand::ProbeToward { z, feed })
        }
        54 => Ok(GCodeCommand::SetWorkOffset(WorkOffset::G54)),
        55 => Ok(GCodeCommand::SetWorkOffset(WorkOffset::G55)),
        56 => Ok(GCodeCommand::SetWorkOffset(WorkOffset::G56)),
        57 => Ok(GCodeCommand::SetWorkOffset(WorkOffset::G57)),
        58 => Ok(GCodeCommand::SetWorkOffset(WorkOffset::G58)),
        59 => Ok(GCodeCommand::SetWorkOffset(WorkOffset::G59)),
        90 => Ok(GCodeCommand::SetDistanceMode(DistanceMode::Absolute)),
        91 => Ok(GCodeCommand::SetDistanceMode(DistanceMode::Incremental)),
        _ => Err(ParseError::UnknownGCode(code)),
    }
}

fn parse_m_code(code: u8, words: &[(char, f64)]) -> Result<GCodeCommand, ParseError> {
    let get_word =
        |letter: char| -> Option<f64> { words.iter().find(|(c, _)| *c == letter).map(|(_, v)| *v) };

    match code {
        0 => Ok(GCodeCommand::ProgramPause),
        2 | 30 => Ok(GCodeCommand::ProgramEnd),
        3 => {
            let speed = get_word('S').map(|s| s as u32).unwrap_or(0);
            Ok(GCodeCommand::SetSpindle(SpindleControl::Clockwise(
                SpindleSpeed::new(speed),
            )))
        }
        4 => {
            let speed = get_word('S').map(|s| s as u32).unwrap_or(0);
            Ok(GCodeCommand::SetSpindle(SpindleControl::CounterClockwise(
                SpindleSpeed::new(speed),
            )))
        }
        5 => Ok(GCodeCommand::SetSpindle(SpindleControl::Off)),
        7 => Ok(GCodeCommand::SetCoolant(CoolantControl::Mist)),
        8 => Ok(GCodeCommand::SetCoolant(CoolantControl::Flood)),
        9 => Ok(GCodeCommand::SetCoolant(CoolantControl::Off)),
        _ => Err(ParseError::UnknownMCode(code)),
    }
}

// ── Validator ────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum ValidationError {
    InvalidFeedRate(f64),
    InvalidDwellTime(f64),
    InvalidArcRadius,
    NoAxisSpecified,
    SpindleSpeedOutOfRange(u32),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFeedRate(v) => write!(f, "Invalid feed rate: {v}"),
            Self::InvalidDwellTime(v) => write!(f, "Invalid dwell time: {v}"),
            Self::InvalidArcRadius => write!(f, "Invalid arc radius"),
            Self::NoAxisSpecified => write!(f, "No axis specified"),
            Self::SpindleSpeedOutOfRange(v) => write!(f, "Spindle speed out of range: {v}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ValidationConfig {
    pub max_feed_rate: f64,
    pub max_spindle_speed: u32,
    pub max_travel: [f64; 3],
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            max_feed_rate: 10000.0,
            max_spindle_speed: 30000,
            max_travel: [300.0, 300.0, 100.0],
        }
    }
}

pub fn validate_command(
    cmd: &GCodeCommand,
    config: &ValidationConfig,
) -> Result<(), ValidationError> {
    match cmd {
        GCodeCommand::LinearMove { feed, .. } | GCodeCommand::ArcMove { feed, .. } => {
            if let Some(f) = feed {
                if *f <= 0.0 || *f > config.max_feed_rate {
                    return Err(ValidationError::InvalidFeedRate(*f));
                }
            }
        }
        GCodeCommand::Dwell { seconds } => {
            if *seconds < 0.0 {
                return Err(ValidationError::InvalidDwellTime(*seconds));
            }
        }
        GCodeCommand::SetSpindle(ctrl) => {
            if let Some(speed) = ctrl.speed() {
                if speed.rpm() > config.max_spindle_speed {
                    return Err(ValidationError::SpindleSpeedOutOfRange(speed.rpm()));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rapid_move() {
        let cmd = parse_line("G00 X10.5 Y20.0 Z-5.0").unwrap();
        match cmd {
            GCodeCommand::RapidMove { x, y, z } => {
                assert_eq!(x, Some(10.5));
                assert_eq!(y, Some(20.0));
                assert_eq!(z, Some(-5.0));
            }
            _ => panic!("Expected RapidMove"),
        }
    }

    #[test]
    fn parse_linear_move() {
        let cmd = parse_line("G01 X10 F1000").unwrap();
        match cmd {
            GCodeCommand::LinearMove { x, y, z, feed } => {
                assert_eq!(x, Some(10.0));
                assert_eq!(y, None);
                assert_eq!(z, None);
                assert_eq!(feed, Some(1000.0));
            }
            _ => panic!("Expected LinearMove"),
        }
    }

    #[test]
    fn parse_spindle_on() {
        let cmd = parse_line("M03 S2000").unwrap();
        match cmd {
            GCodeCommand::SetSpindle(SpindleControl::Clockwise(speed)) => {
                assert_eq!(speed.rpm(), 2000);
            }
            _ => panic!("Expected SetSpindle Clockwise"),
        }
    }

    #[test]
    fn strip_comment() {
        let cmd = parse_line("G00 X10 ; move to X").unwrap();
        assert!(matches!(cmd, GCodeCommand::RapidMove { .. }));
    }

    #[test]
    fn case_insensitive() {
        let cmd1 = parse_line("g00 x10").unwrap();
        let cmd2 = parse_line("G00 X10").unwrap();
        assert_eq!(cmd1, cmd2);
    }

    #[test]
    fn command_display() {
        let cmd = GCodeCommand::LinearMove {
            x: Some(10.0),
            y: Some(20.0),
            z: None,
            feed: Some(1000.0),
        };
        assert_eq!(format!("{cmd}"), "G01 X10.0000 Y20.0000 F1000");
    }

    #[test]
    fn validate_feed_rate() {
        let config = ValidationConfig::default();
        let valid = GCodeCommand::LinearMove {
            x: Some(10.0),
            y: None,
            z: None,
            feed: Some(1000.0),
        };
        assert!(validate_command(&valid, &config).is_ok());

        let invalid = GCodeCommand::LinearMove {
            x: Some(10.0),
            y: None,
            z: None,
            feed: Some(-100.0),
        };
        assert!(matches!(
            validate_command(&invalid, &config),
            Err(ValidationError::InvalidFeedRate(_))
        ));
    }

    #[test]
    fn validate_spindle_speed() {
        let config = ValidationConfig {
            max_spindle_speed: 3000,
            ..Default::default()
        };
        let valid = GCodeCommand::SetSpindle(SpindleControl::Clockwise(SpindleSpeed::new(2000)));
        assert!(validate_command(&valid, &config).is_ok());

        let invalid = GCodeCommand::SetSpindle(SpindleControl::Clockwise(SpindleSpeed::new(5000)));
        assert!(matches!(
            validate_command(&invalid, &config),
            Err(ValidationError::SpindleSpeedOutOfRange(5000))
        ));
    }

    #[test]
    fn work_offset_roundtrip() {
        for offset in [
            WorkOffset::G54,
            WorkOffset::G55,
            WorkOffset::G56,
            WorkOffset::G57,
            WorkOffset::G58,
            WorkOffset::G59,
        ] {
            let code = offset.gcode_number();
            let parsed = WorkOffset::from_gcode(code);
            assert_eq!(parsed, Some(offset));
        }
    }

    #[test]
    fn distance_mode_display_and_gcode_number() {
        assert_eq!(DistanceMode::Absolute.gcode_number(), 90);
        assert_eq!(DistanceMode::Incremental.gcode_number(), 91);
        assert_eq!(format!("{}", DistanceMode::Absolute), "G90");
        assert_eq!(format!("{}", DistanceMode::Incremental), "G91");
    }

    #[test]
    fn unit_mode_display_and_gcode_number() {
        assert_eq!(UnitMode::Inches.gcode_number(), 20);
        assert_eq!(UnitMode::Millimeters.gcode_number(), 21);
        assert_eq!(format!("{}", UnitMode::Inches), "G20");
        assert_eq!(format!("{}", UnitMode::Millimeters), "G21");
    }

    #[test]
    fn parse_error_display() {
        assert_eq!(format!("{}", ParseError::InvalidNumber("abc".into())), "Invalid number: abc");
        assert_eq!(format!("{}", ParseError::UnknownGCode(99)), "Unknown G-code: G99");
        assert_eq!(format!("{}", ParseError::UnknownMCode(99)), "Unknown M-code: M99");
        assert_eq!(format!("{}", ParseError::MissingParameter('Z')), "Missing parameter: Z");
        assert_eq!(format!("{}", ParseError::EmptyLine), "Empty line");
    }

    #[test]
    fn validation_error_display() {
        assert_eq!(format!("{}", ValidationError::InvalidFeedRate(-1.0)), "Invalid feed rate: -1");
        assert_eq!(format!("{}", ValidationError::InvalidDwellTime(-2.0)), "Invalid dwell time: -2");
        assert_eq!(format!("{}", ValidationError::InvalidArcRadius), "Invalid arc radius");
        assert_eq!(format!("{}", ValidationError::NoAxisSpecified), "No axis specified");
        assert_eq!(format!("{}", ValidationError::SpindleSpeedOutOfRange(99999)), "Spindle speed out of range: 99999");
    }

    #[test]
    fn work_offset_display() {
        assert_eq!(format!("{}", WorkOffset::G54), "G54");
        assert_eq!(format!("{}", WorkOffset::G59), "G59");
    }

    #[test]
    fn parse_line_empty() {
        assert_eq!(parse_line("   "), Err(ParseError::EmptyLine));
    }

    #[test]
    fn parse_line_comment() {
        let cmd = parse_line("(this is a comment)").unwrap();
        assert!(matches!(cmd, GCodeCommand::Comment(_)));
    }

    #[test]
    fn parse_line_m_code() {
        let cmd = parse_line("M02").unwrap();
        assert!(matches!(cmd, GCodeCommand::ProgramEnd));
    }

    #[test]
    fn parse_words_basic() {
        let words = parse_words("G1 X10.5 Y-3.2 F500").unwrap();
        assert_eq!(words.len(), 4);
        assert_eq!(words[0], ('G', 1.0));
        assert_eq!(words[1], ('X', 10.5));
        assert_eq!(words[2], ('Y', -3.2));
        assert_eq!(words[3], ('F', 500.0));
    }

    #[test]
    fn parse_words_letter_only() {
        // A letter with no number should default to 0.0
        let words = parse_words("G").unwrap();
        assert_eq!(words, vec![('G', 0.0)]);
    }
}
