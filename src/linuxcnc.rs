/// LinuxCNC-compatible G-code interpreter running in WebAssembly.
///
/// Implements the RS274NGC dialect used by LinuxCNC:
///   - G-code parser (words, blocks, comments, line numbers)
///   - Modal group tracking per the RS274NGC spec
///   - Machine state (position, coordinate systems G54-G59, units, etc.)
///   - Trajectory planner with trapezoidal velocity profiles
///
/// Swiss-cheese layer: **Interpreter / Controller**
/// Extension point: add canned cycles, cutter compensation, probing, etc.

use serde::{Deserialize, Serialize};

// ── G-code parser ────────────────────────────────────────────────────

/// A single word in a G-code block, e.g. `G1`, `X10.5`, `F800`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Word {
    pub letter: char,
    pub value: f64,
}

impl Word {
    pub fn new(letter: char, value: f64) -> Self {
        Self { letter, value }
    }
    /// Integer code (e.g. G1 → 1, G0 → 0, M3 → 3).
    pub fn code(&self) -> i32 {
        self.value.round() as i32
    }
}

/// A parsed G-code block (one line).
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub line_number: Option<u32>,
    pub words: Vec<Word>,
    pub comment: Option<String>,
    /// True when the block has been deleted by the block-delete switch (leading `/`).
    pub deleted: bool,
}

impl Block {
    pub fn find(&self, letter: char) -> Option<f64> {
        self.words.iter().find(|w| w.letter == letter).map(|w| w.value)
    }

    pub fn find_all(&self, letter: char) -> Vec<f64> {
        self.words.iter().filter(|w| w.letter == letter).map(|w| w.value).collect()
    }

    pub fn has(&self, letter: char, code: i32) -> bool {
        self.words.iter().any(|w| w.letter == letter && w.code() == code)
    }
}

/// Parse errors.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    InvalidWord(usize, String),
    BadNumber(usize, String),
    UnexpectedChar(usize, char),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::InvalidWord(col, s) => write!(f, "col {col}: invalid word '{s}'"),
            ParseError::BadNumber(col, s) => write!(f, "col {col}: bad number '{s}'"),
            ParseError::UnexpectedChar(col, c) => write!(f, "col {col}: unexpected '{c}'"),
        }
    }
}

/// Parse a single line of RS274NGC G-code into a `Block`.
pub fn parse_line(line: &str) -> Result<Block, ParseError> {
    let line = line.trim();
    let mut words = Vec::new();
    let mut comment = None;
    let mut line_number = None;
    let mut deleted = false;

    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    // Block delete
    if i < chars.len() && chars[i] == '/' {
        deleted = true;
        i += 1;
    }

    while i < chars.len() {
        let ch = chars[i];

        // Skip whitespace
        if ch.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // Parenthesized comment
        if ch == '(' {
            let start = i + 1;
            while i < chars.len() && chars[i] != ')' {
                i += 1;
            }
            comment = Some(chars[start..i].iter().collect());
            if i < chars.len() {
                i += 1; // skip ')'
            }
            continue;
        }

        // Semicolon comment — rest of line
        if ch == ';' {
            comment = Some(chars[i + 1..].iter().collect::<String>().trim().to_string());
            break;
        }

        // Percent sign — program delimiters, skip
        if ch == '%' {
            i += 1;
            continue;
        }

        // Must be a word letter
        let letter = ch.to_ascii_uppercase();
        if !letter.is_ascii_alphabetic() {
            return Err(ParseError::UnexpectedChar(i, ch));
        }
        i += 1;

        // Collect the number (optional sign, digits, decimal point, digits)
        let num_start = i;
        if i < chars.len() && (chars[i] == '-' || chars[i] == '+') {
            i += 1;
        }
        while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
            i += 1;
        }
        if i == num_start {
            return Err(ParseError::BadNumber(num_start, String::new()));
        }
        let num_str: String = chars[num_start..i].iter().collect();
        let value: f64 = num_str
            .parse()
            .map_err(|_| ParseError::BadNumber(num_start, num_str.clone()))?;

        if letter == 'N' {
            line_number = Some(value as u32);
        } else {
            words.push(Word::new(letter, value));
        }
    }

    Ok(Block { line_number, words, comment, deleted })
}

/// Parse a complete G-code program (multi-line string) into blocks.
pub fn parse_program(text: &str) -> Result<Vec<Block>, (usize, ParseError)> {
    let mut blocks = Vec::new();
    for (line_idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == "%" {
            continue;
        }
        let block = parse_line(trimmed).map_err(|e| (line_idx + 1, e))?;
        if !block.words.is_empty() || block.comment.is_some() {
            blocks.push(block);
        }
    }
    Ok(blocks)
}

// ── Modal groups (RS274NGC) ──────────────────────────────────────────

/// Motion mode (modal group 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MotionMode {
    Rapid,       // G0
    Linear,      // G1
    ArcCW,       // G2
    ArcCCW,      // G3
    Dwell,       // G4
    None,        // G80 (cancel canned cycle)
}

/// Active plane (modal group 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Plane {
    XY, // G17
    XZ, // G18
    YZ, // G19
}

/// Distance mode (modal group 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceMode {
    Absolute,    // G90
    Incremental, // G91
}

/// Feed rate mode (modal group 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedRateMode {
    InverseTime,  // G93
    UnitsPerMin,  // G94
    UnitsPerRev,  // G95
}

/// Units (modal group 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Units {
    Imperial, // G20 — inches
    Metric,   // G21 — mm
}

/// Cutter compensation (modal group 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CutterComp {
    Off,   // G40
    Left,  // G41
    Right, // G42
}

/// Path control mode (modal group 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathMode {
    Exact,    // G61
    Blending, // G64
}

/// Spindle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpindleState {
    Off,
    CW,  // M3
    CCW, // M4
}

/// Coolant state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoolantState {
    pub mist: bool,  // M7
    pub flood: bool, // M8
}

// ── Work coordinate systems ──────────────────────────────────────────

/// G54-G59 offset index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordSystem {
    G54 = 0,
    G55 = 1,
    G56 = 2,
    G57 = 3,
    G58 = 4,
    G59 = 5,
}

impl CoordSystem {
    fn from_code(code: i32) -> Option<Self> {
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

/// 3-axis offset for a coordinate system.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Offset {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Default for Offset {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, z: 0.0 }
    }
}

// ── Machine state ────────────────────────────────────────────────────

/// Full interpreter state, mirroring the LinuxCNC canonical machine interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineState {
    // Position (machine coordinates)
    pub pos_x: f64,
    pub pos_y: f64,
    pub pos_z: f64,

    // Modal state
    pub motion_mode: MotionMode,
    pub plane: Plane,
    pub distance_mode: DistanceMode,
    pub feed_rate_mode: FeedRateMode,
    pub units: Units,
    pub cutter_comp: CutterComp,
    pub path_mode: PathMode,
    pub coord_system: CoordSystem,

    // Rates
    pub feed_rate: f64,
    pub spindle_speed: f64,
    pub spindle: SpindleState,
    pub coolant: CoolantState,

    // Tool
    pub tool_number: u32,
    pub tool_length_offset: f64,

    // Coordinate system offsets (G54-G59)
    pub offsets: [Offset; 6],

    // G92 offset (temporary)
    pub g92_offset: Offset,

    // Program state
    pub line: usize,
    pub running: bool,
    pub error: Option<String>,
}

impl Default for MachineState {
    fn default() -> Self {
        Self {
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            motion_mode: MotionMode::Rapid,
            plane: Plane::XY,
            distance_mode: DistanceMode::Absolute,
            feed_rate_mode: FeedRateMode::UnitsPerMin,
            units: Units::Metric,
            cutter_comp: CutterComp::Off,
            path_mode: PathMode::Blending,
            coord_system: CoordSystem::G54,
            feed_rate: 0.0,
            spindle_speed: 0.0,
            spindle: SpindleState::Off,
            coolant: CoolantState { mist: false, flood: false },
            tool_number: 0,
            tool_length_offset: 0.0,
            offsets: [Offset::default(); 6],
            g92_offset: Offset::default(),
            line: 0,
            running: true,
            error: None,
        }
    }
}

impl MachineState {
    /// Work-coordinate position (machine pos minus active offsets).
    pub fn work_pos(&self) -> (f64, f64, f64) {
        let off = self.active_offset();
        (
            self.pos_x - off.x - self.g92_offset.x,
            self.pos_y - off.y - self.g92_offset.y,
            self.pos_z - off.z - self.g92_offset.z,
        )
    }

    /// Active coordinate system offset.
    pub fn active_offset(&self) -> Offset {
        self.offsets[self.coord_system as usize]
    }

    /// Convert a work-coordinate target to machine coordinates.
    fn to_machine(&self, wx: f64, wy: f64, wz: f64) -> (f64, f64, f64) {
        let off = self.active_offset();
        (
            wx + off.x + self.g92_offset.x,
            wy + off.y + self.g92_offset.y,
            wz + off.z + self.g92_offset.z,
        )
    }
}

// ── Planned motion segments ──────────────────────────────────────────

/// A single planned motion segment produced by the interpreter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionSegment {
    /// Start position (machine coords).
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    /// End position (machine coords).
    pub x1: f64,
    pub y1: f64,
    pub z1: f64,
    /// Requested feed rate (mm/min or in/min). 0 = rapid.
    pub feed: f64,
    /// True for G0 rapids.
    pub rapid: bool,
    /// Source line number in the G-code program.
    pub source_line: usize,
}

impl MotionSegment {
    pub fn length(&self) -> f64 {
        let dx = self.x1 - self.x0;
        let dy = self.y1 - self.y0;
        let dz = self.z1 - self.z0;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

/// A velocity-planned segment with trapezoidal profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedSegment {
    pub segment: MotionSegment,
    /// Entry velocity (mm/min).
    pub v_entry: f64,
    /// Cruise velocity (mm/min).
    pub v_cruise: f64,
    /// Exit velocity (mm/min).
    pub v_exit: f64,
    /// Duration of the segment in seconds.
    pub duration: f64,
}

// ── Interpreter ──────────────────────────────────────────────────────

/// Interpreter error.
#[derive(Debug, Clone, PartialEq)]
pub struct InterpError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for InterpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

/// Interpret a parsed program, producing motion segments and final machine state.
pub fn interpret(blocks: &[Block]) -> Result<(Vec<MotionSegment>, MachineState), InterpError> {
    let mut state = MachineState::default();
    let mut segments = Vec::new();

    for (idx, block) in blocks.iter().enumerate() {
        state.line = idx + 1;
        if block.deleted {
            continue;
        }
        interpret_block(block, &mut state, &mut segments)?;
        if !state.running {
            break;
        }
    }

    Ok((segments, state))
}

fn interpret_block(
    block: &Block,
    state: &mut MachineState,
    segments: &mut Vec<MotionSegment>,
) -> Result<(), InterpError> {
    let err = |msg: &str| InterpError {
        line: state.line,
        message: msg.to_string(),
    };

    let g_codes: Vec<i32> = block.words.iter().filter(|w| w.letter == 'G').map(|w| w.code()).collect();
    let m_codes: Vec<i32> = block.words.iter().filter(|w| w.letter == 'M').map(|w| w.code()).collect();

    // ── Process G codes (settings first, motion last) ────────────

    for &g in &g_codes {
        match g {
            // Units
            20 => state.units = Units::Imperial,
            21 => state.units = Units::Metric,
            // Distance mode
            90 => state.distance_mode = DistanceMode::Absolute,
            91 => state.distance_mode = DistanceMode::Incremental,
            // Plane
            17 => state.plane = Plane::XY,
            18 => state.plane = Plane::XZ,
            19 => state.plane = Plane::YZ,
            // Feed rate mode
            93 => state.feed_rate_mode = FeedRateMode::InverseTime,
            94 => state.feed_rate_mode = FeedRateMode::UnitsPerMin,
            95 => state.feed_rate_mode = FeedRateMode::UnitsPerRev,
            // Coordinate system
            54 | 55 | 56 | 57 | 58 | 59 => {
                state.coord_system = CoordSystem::from_code(g).unwrap();
            }
            // Cutter comp
            40 => state.cutter_comp = CutterComp::Off,
            41 => state.cutter_comp = CutterComp::Left,
            42 => state.cutter_comp = CutterComp::Right,
            // Path mode
            61 => state.path_mode = PathMode::Exact,
            64 => state.path_mode = PathMode::Blending,
            // Tool length offset
            43 => {
                // G43 H<n> — simplified: just mark offset active
                state.tool_length_offset = block.find('H').unwrap_or(0.0);
            }
            49 => state.tool_length_offset = 0.0,
            // Cancel canned cycle
            80 => state.motion_mode = MotionMode::None,
            // G92 — set coordinate offset
            92 => {
                let (wx, wy, wz) = state.work_pos();
                if let Some(x) = block.find('X') { state.g92_offset.x = state.pos_x - x; let _ = wx; }
                else { state.g92_offset.x = 0.0; }
                if let Some(y) = block.find('Y') { state.g92_offset.y = state.pos_y - y; let _ = wy; }
                else { state.g92_offset.y = 0.0; }
                if let Some(z) = block.find('Z') { state.g92_offset.z = state.pos_z - z; let _ = wz; }
                else { state.g92_offset.z = 0.0; }
            }
            // G10 L2 P<n> — set coordinate system offset
            10 => {
                let l = block.find('L').unwrap_or(0.0) as i32;
                if l == 2 {
                    let p = block.find('P').unwrap_or(1.0) as usize;
                    if p >= 1 && p <= 6 {
                        let idx = p - 1;
                        if let Some(x) = block.find('X') { state.offsets[idx].x = x; }
                        if let Some(y) = block.find('Y') { state.offsets[idx].y = y; }
                        if let Some(z) = block.find('Z') { state.offsets[idx].z = z; }
                    }
                }
            }
            // Motion codes — handled below
            0 | 1 | 2 | 3 | 4 | 28 | 30 | 53 => {}
            _ => {
                // Unknown G code — ignore for forward compatibility
            }
        }
    }

    // ── Process M codes ──────────────────────────────────────────

    for &m in &m_codes {
        match m {
            0 | 1 => {
                // M0 program pause, M1 optional stop — we just note it
            }
            2 | 30 => {
                state.running = false;
                state.spindle = SpindleState::Off;
                state.coolant = CoolantState { mist: false, flood: false };
            }
            3 => state.spindle = SpindleState::CW,
            4 => state.spindle = SpindleState::CCW,
            5 => state.spindle = SpindleState::Off,
            6 => {
                if let Some(t) = block.find('T') {
                    state.tool_number = t as u32;
                }
            }
            7 => state.coolant.mist = true,
            8 => state.coolant.flood = true,
            9 => {
                state.coolant.mist = false;
                state.coolant.flood = false;
            }
            _ => {}
        }
    }

    // ── Feed rate / spindle speed ────────────────────────────────

    if let Some(f) = block.find('F') {
        state.feed_rate = f;
    }
    if let Some(s) = block.find('S') {
        state.spindle_speed = s;
    }
    if let Some(t) = block.find('T') {
        state.tool_number = t as u32;
    }

    // ── Motion handling ──────────────────────────────────────────

    // Determine active motion mode for this block
    let motion_g: Option<i32> = g_codes.iter().copied().find(|&g| matches!(g, 0 | 1 | 2 | 3 | 4 | 28 | 30 | 53));

    // Update motion mode if a motion G-code was specified
    if let Some(g) = motion_g {
        match g {
            0 => state.motion_mode = MotionMode::Rapid,
            1 => state.motion_mode = MotionMode::Linear,
            2 => state.motion_mode = MotionMode::ArcCW,
            3 => state.motion_mode = MotionMode::ArcCCW,
            4 => state.motion_mode = MotionMode::Dwell,
            _ => {}
        }
    }

    // Check if any axis words are present (X, Y, Z)
    let has_axis = block.find('X').is_some() || block.find('Y').is_some() || block.find('Z').is_some();

    // G28 — return to home via optional intermediate
    if g_codes.contains(&28) {
        // Move to intermediate if axis words given, then to 0,0,0
        if has_axis {
            let (tx, ty, tz) = resolve_target(block, state);
            emit_move(state, segments, tx, ty, tz, true);
        }
        emit_move(state, segments, 0.0, 0.0, 0.0, true);
        return Ok(());
    }

    // G30 — return to second home
    if g_codes.contains(&30) {
        if has_axis {
            let (tx, ty, tz) = resolve_target(block, state);
            emit_move(state, segments, tx, ty, tz, true);
        }
        emit_move(state, segments, 0.0, 0.0, 0.0, true);
        return Ok(());
    }

    // G53 — single-block machine-coordinate move
    if g_codes.contains(&53) {
        let tx = block.find('X').unwrap_or(state.pos_x);
        let ty = block.find('Y').unwrap_or(state.pos_y);
        let tz = block.find('Z').unwrap_or(state.pos_z);
        let rapid = state.motion_mode == MotionMode::Rapid;
        emit_move(state, segments, tx, ty, tz, rapid);
        return Ok(());
    }

    // G4 — dwell (no motion, we just skip)
    if let Some(4) = motion_g {
        return Ok(());
    }

    // Handle axis motion if axis words are present or it's a modal repeat
    if has_axis {
        let (tx, ty, tz) = resolve_target(block, state);

        match state.motion_mode {
            MotionMode::Rapid => {
                emit_move(state, segments, tx, ty, tz, true);
            }
            MotionMode::Linear => {
                if state.feed_rate <= 0.0 {
                    return Err(err("G1 with zero feed rate"));
                }
                emit_move(state, segments, tx, ty, tz, false);
            }
            MotionMode::ArcCW | MotionMode::ArcCCW => {
                let cw = state.motion_mode == MotionMode::ArcCW;
                let i = block.find('I').unwrap_or(0.0);
                let j = block.find('J').unwrap_or(0.0);
                if state.feed_rate <= 0.0 {
                    return Err(err("arc with zero feed rate"));
                }
                linearize_arc(state, segments, tx, ty, tz, i, j, cw);
            }
            MotionMode::Dwell | MotionMode::None => {
                // No motion
            }
        }
    }

    Ok(())
}

/// Resolve X/Y/Z target considering absolute vs incremental and coordinate offsets.
fn resolve_target(block: &Block, state: &MachineState) -> (f64, f64, f64) {
    match state.distance_mode {
        DistanceMode::Absolute => {
            let wx = block.find('X').unwrap_or_else(|| {
                let (wx, _, _) = state.work_pos();
                wx
            });
            let wy = block.find('Y').unwrap_or_else(|| {
                let (_, wy, _) = state.work_pos();
                wy
            });
            let wz = block.find('Z').unwrap_or_else(|| {
                let (_, _, wz) = state.work_pos();
                wz
            });
            state.to_machine(wx, wy, wz)
        }
        DistanceMode::Incremental => {
            let dx = block.find('X').unwrap_or(0.0);
            let dy = block.find('Y').unwrap_or(0.0);
            let dz = block.find('Z').unwrap_or(0.0);
            (state.pos_x + dx, state.pos_y + dy, state.pos_z + dz)
        }
    }
}

/// Emit a linear motion segment and update position.
fn emit_move(
    state: &mut MachineState,
    segments: &mut Vec<MotionSegment>,
    tx: f64,
    ty: f64,
    tz: f64,
    rapid: bool,
) {
    let seg = MotionSegment {
        x0: state.pos_x,
        y0: state.pos_y,
        z0: state.pos_z,
        x1: tx,
        y1: ty,
        z1: tz,
        feed: if rapid { 0.0 } else { state.feed_rate },
        rapid,
        source_line: state.line,
    };
    // Only emit if the segment actually moves
    if seg.length() > 1e-9 {
        segments.push(seg);
    }
    state.pos_x = tx;
    state.pos_y = ty;
    state.pos_z = tz;
}

/// Linearize a circular arc (G2/G3) into short line segments.
fn linearize_arc(
    state: &mut MachineState,
    segments: &mut Vec<MotionSegment>,
    tx: f64,
    ty: f64,
    tz: f64,
    i: f64,
    j: f64,
    cw: bool,
) {
    let cx = state.pos_x + i;
    let cy = state.pos_y + j;

    let start_angle = (state.pos_y - cy).atan2(state.pos_x - cx);
    let end_angle = (ty - cy).atan2(tx - cx);
    let radius = ((state.pos_x - cx).powi(2) + (state.pos_y - cy).powi(2)).sqrt();

    if radius < 1e-9 {
        emit_move(state, segments, tx, ty, tz, false);
        return;
    }

    let mut sweep = end_angle - start_angle;
    if cw {
        if sweep >= 0.0 {
            sweep -= 2.0 * std::f64::consts::PI;
        }
    } else if sweep <= 0.0 {
        sweep += 2.0 * std::f64::consts::PI;
    }

    // Number of segments: ~1 degree per segment, min 4
    let n = (sweep.abs() / (std::f64::consts::PI / 180.0)).ceil().max(4.0) as usize;
    let dz = tz - state.pos_z;

    for step in 1..=n {
        let t = step as f64 / n as f64;
        let angle = start_angle + sweep * t;
        let sx = cx + radius * angle.cos();
        let sy = cy + radius * angle.sin();
        let sz = state.pos_z + dz * t;
        // For the last step, snap to exact target
        if step == n {
            emit_move(state, segments, tx, ty, tz, false);
        } else {
            emit_move(state, segments, sx, sy, sz, false);
        }
    }
}

// ── Trajectory planner ───────────────────────────────────────────────

/// Trajectory planner configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerConfig {
    /// Maximum acceleration in mm/s^2.
    pub max_accel: f64,
    /// Maximum velocity for rapids in mm/min.
    pub rapid_rate: f64,
    /// Junction deviation for path blending in mm.
    pub junction_deviation: f64,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        Self {
            max_accel: 500.0,
            rapid_rate: 5000.0,
            junction_deviation: 0.05,
        }
    }
}

/// Plan velocity profiles for a sequence of motion segments.
///
/// Uses a forward + reverse pass to compute trapezoidal velocity profiles
/// that respect acceleration limits and junction speeds.
pub fn plan_trajectory(segments: &[MotionSegment], config: &PlannerConfig) -> Vec<PlannedSegment> {
    if segments.is_empty() {
        return Vec::new();
    }

    let accel = config.max_accel;
    let mm_per_s_to_min = 60.0;

    // Compute maximum entry speeds using junction deviation
    let mut max_entry: Vec<f64> = vec![0.0; segments.len()];
    max_entry[0] = 0.0; // start from rest

    for i in 1..segments.len() {
        let prev = &segments[i - 1];
        let curr = &segments[i];

        // Direction vectors
        let prev_len = prev.length();
        let curr_len = curr.length();
        if prev_len < 1e-9 || curr_len < 1e-9 {
            max_entry[i] = 0.0;
            continue;
        }
        let pdx = (prev.x1 - prev.x0) / prev_len;
        let pdy = (prev.y1 - prev.y0) / prev_len;
        let pdz = (prev.z1 - prev.z0) / prev_len;
        let cdx = (curr.x1 - curr.x0) / curr_len;
        let cdy = (curr.y1 - curr.y0) / curr_len;
        let cdz = (curr.z1 - curr.z0) / curr_len;

        // Cosine of the angle between segments
        let cos_theta = (pdx * cdx + pdy * cdy + pdz * cdz).clamp(-1.0, 1.0);

        if cos_theta < -0.999 {
            // Nearly 180-degree turn — full stop
            max_entry[i] = 0.0;
        } else {
            // Junction speed from deviation
            let half_angle = ((1.0 - cos_theta) / 2.0).sqrt().asin();
            if half_angle < 1e-9 {
                max_entry[i] = f64::MAX; // collinear
            } else {
                let r = config.junction_deviation * half_angle.sin() / (1.0 - half_angle.sin());
                let v_junction = (accel * r).sqrt() * mm_per_s_to_min;
                max_entry[i] = v_junction;
            }
        }
    }

    // Cruise speeds
    let cruise: Vec<f64> = segments
        .iter()
        .map(|s| {
            if s.rapid {
                config.rapid_rate
            } else {
                s.feed.max(0.001)
            }
        })
        .collect();

    // Forward pass: limit entry speed by what acceleration can reach
    let mut entry: Vec<f64> = vec![0.0; segments.len()];
    entry[0] = 0.0;
    for i in 1..segments.len() {
        let dist = segments[i - 1].length();
        let v_prev = entry[i - 1];
        // v^2 = v0^2 + 2*a*d  (converted to mm/min: a in mm/s^2, d in mm)
        let v_max_from_accel = (v_prev.powi(2) + 2.0 * accel * mm_per_s_to_min.powi(2) * dist).sqrt();
        entry[i] = max_entry[i].min(cruise[i]).min(v_max_from_accel);
    }

    // Reverse pass: limit exit speed (= next entry) by deceleration
    let n = segments.len();
    let mut exit: Vec<f64> = vec![0.0; n];
    exit[n - 1] = 0.0; // come to rest at end
    for i in (0..n - 1).rev() {
        let dist = segments[i].length();
        let v_next = exit[i + 1].min(entry[i + 1]);
        let v_max_from_decel = (v_next.powi(2) + 2.0 * accel * mm_per_s_to_min.powi(2) * dist).sqrt();
        exit[i] = cruise[i].min(v_max_from_decel);
        // Also update entry of next segment
        if i + 1 < n {
            entry[i + 1] = entry[i + 1].min(exit[i]);
        }
    }

    // Build planned segments with durations
    let mut planned = Vec::with_capacity(n);
    for i in 0..n {
        // Junction speed: entry[i] was already limited by the reverse pass
        let v_entry = entry[i];
        let v_exit = if i + 1 < n { entry[i + 1] } else { 0.0 };
        let v_cruise = cruise[i].max(v_entry).max(v_exit);
        let dist = segments[i].length();

        // Approximate duration using average velocity (trapezoidal rule)
        let v_avg = ((v_entry + v_cruise + v_exit) / 3.0).max(0.001);
        let duration = dist / (v_avg / mm_per_s_to_min); // seconds

        planned.push(PlannedSegment {
            segment: segments[i].clone(),
            v_entry,
            v_cruise,
            v_exit,
            duration,
        });
    }

    planned
}

/// Compute position along a planned segment at time t (0..duration).
pub fn interpolate_segment(seg: &PlannedSegment, t: f64) -> (f64, f64, f64) {
    let frac = if seg.duration > 1e-9 { (t / seg.duration).clamp(0.0, 1.0) } else { 1.0 };
    let s = &seg.segment;
    (
        s.x0 + (s.x1 - s.x0) * frac,
        s.y0 + (s.y1 - s.y0) * frac,
        s.z0 + (s.z1 - s.z0) * frac,
    )
}

/// Compute the total estimated run time in seconds.
pub fn total_time(planned: &[PlannedSegment]) -> f64 {
    planned.iter().map(|p| p.duration).sum()
}

// ── Validation ───────────────────────────────────────────────────────

/// Diagnostic message from G-code validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub line: usize,
    pub severity: String, // "error", "warning", "info"
    pub message: String,
}

/// Validate a G-code program and return diagnostics.
pub fn validate(blocks: &[Block]) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut state = MachineState::default();

    for (idx, block) in blocks.iter().enumerate() {
        let line = idx + 1;

        // Check for feed rate before G1
        let has_g1 = block.has('G', 1);
        let has_axis = block.find('X').is_some() || block.find('Y').is_some() || block.find('Z').is_some();
        let sets_feed = block.find('F').is_some();

        if has_g1 || (has_axis && state.motion_mode == MotionMode::Linear) {
            if !sets_feed && state.feed_rate <= 0.0 {
                diags.push(Diagnostic {
                    line,
                    severity: "error".into(),
                    message: "Feed move with no feed rate set".into(),
                });
            }
        }

        // Check for spindle before cutting
        if has_axis && !block.has('G', 0) && state.spindle == SpindleState::Off {
            let is_rapid = block.has('G', 0) || (state.motion_mode == MotionMode::Rapid && !has_g1);
            if !is_rapid {
                diags.push(Diagnostic {
                    line,
                    severity: "warning".into(),
                    message: "Feed move with spindle off".into(),
                });
            }
        }

        // Track modal state for subsequent checks
        if sets_feed {
            state.feed_rate = block.find('F').unwrap();
        }
        if has_g1 {
            state.motion_mode = MotionMode::Linear;
        }
        if block.has('G', 0) {
            state.motion_mode = MotionMode::Rapid;
        }
        if block.has('M', 3) || block.has('M', 4) {
            state.spindle = SpindleState::CW;
        }
        if block.has('M', 5) {
            state.spindle = SpindleState::Off;
        }
    }

    diags
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Parser tests ─────────────────────────────────────────────

    #[test]
    fn parse_simple_rapid() {
        let b = parse_line("G0 X10 Y20 Z5").unwrap();
        assert!(b.has('G', 0));
        assert_eq!(b.find('X'), Some(10.0));
        assert_eq!(b.find('Y'), Some(20.0));
        assert_eq!(b.find('Z'), Some(5.0));
    }

    #[test]
    fn parse_line_number_and_comment() {
        let b = parse_line("N100 G1 X5 F800 (feed move)").unwrap();
        assert_eq!(b.line_number, Some(100));
        assert!(b.has('G', 1));
        assert_eq!(b.comment.as_deref(), Some("feed move"));
    }

    #[test]
    fn parse_semicolon_comment() {
        let b = parse_line("G0 X0 Y0 ; home").unwrap();
        assert!(b.has('G', 0));
        assert_eq!(b.comment.as_deref(), Some("home"));
    }

    #[test]
    fn parse_block_delete() {
        let b = parse_line("/G0 X10").unwrap();
        assert!(b.deleted);
        assert!(b.has('G', 0));
    }

    #[test]
    fn parse_negative_coord() {
        let b = parse_line("G1 X-5.5 Z-1.25 F600").unwrap();
        assert_eq!(b.find('X'), Some(-5.5));
        assert_eq!(b.find('Z'), Some(-1.25));
        assert_eq!(b.find('F'), Some(600.0));
    }

    #[test]
    fn parse_program_skips_blanks() {
        let text = "%\nG21\n\nG0 X0 Y0\n%\n";
        let blocks = parse_program(text).unwrap();
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn parse_arc_words() {
        let b = parse_line("G2 X10 Y0 I5 J0 F500").unwrap();
        assert!(b.has('G', 2));
        assert_eq!(b.find('I'), Some(5.0));
        assert_eq!(b.find('J'), Some(0.0));
    }

    // ── Interpreter tests ────────────────────────────────────────

    #[test]
    fn interp_rapid_move() {
        let blocks = parse_program("G0 X10 Y20 Z5").unwrap();
        let (segs, state) = interpret(&blocks).unwrap();
        assert_eq!(segs.len(), 1);
        assert!(segs[0].rapid);
        assert!((state.pos_x - 10.0).abs() < 1e-9);
        assert!((state.pos_y - 20.0).abs() < 1e-9);
        assert!((state.pos_z - 5.0).abs() < 1e-9);
    }

    #[test]
    fn interp_feed_move() {
        let blocks = parse_program("G1 X10 Y0 F800").unwrap();
        let (segs, _) = interpret(&blocks).unwrap();
        assert_eq!(segs.len(), 1);
        assert!(!segs[0].rapid);
        assert!((segs[0].feed - 800.0).abs() < 1e-9);
    }

    #[test]
    fn interp_feed_no_rate_errors() {
        let blocks = parse_program("G1 X10 Y0").unwrap();
        let result = interpret(&blocks);
        assert!(result.is_err());
    }

    #[test]
    fn interp_units_mode() {
        let blocks = parse_program("G20\nG21").unwrap();
        let (_, state) = interpret(&blocks).unwrap();
        assert_eq!(state.units, Units::Metric);
    }

    #[test]
    fn interp_distance_mode() {
        let blocks = parse_program("G91\nG0 X5\nG0 X5").unwrap();
        let (segs, state) = interpret(&blocks).unwrap();
        assert_eq!(segs.len(), 2);
        assert!((state.pos_x - 10.0).abs() < 1e-9);
    }

    #[test]
    fn interp_coord_system_offset() {
        let blocks = parse_program("G10 L2 P1 X10 Y20\nG54\nG0 X0 Y0").unwrap();
        let (_, state) = interpret(&blocks).unwrap();
        // Work coord X0 Y0 maps to machine X10 Y20
        assert!((state.pos_x - 10.0).abs() < 1e-9);
        assert!((state.pos_y - 20.0).abs() < 1e-9);
    }

    #[test]
    fn interp_g92_offset() {
        let blocks = parse_program("G0 X10 Y10\nG92 X0 Y0").unwrap();
        let (_, state) = interpret(&blocks).unwrap();
        let (wx, wy, _) = state.work_pos();
        assert!((wx - 0.0).abs() < 1e-9);
        assert!((wy - 0.0).abs() < 1e-9);
    }

    #[test]
    fn interp_spindle_and_coolant() {
        let blocks = parse_program("M3 S12000\nM8\nG0 X10\nM5\nM9").unwrap();
        let (_, state) = interpret(&blocks).unwrap();
        assert_eq!(state.spindle, SpindleState::Off);
        assert!(!state.coolant.flood);
        assert!((state.spindle_speed - 12000.0).abs() < 1e-9);
    }

    #[test]
    fn interp_program_end() {
        let blocks = parse_program("G0 X10\nM2\nG0 X20").unwrap();
        let (segs, state) = interpret(&blocks).unwrap();
        // Should stop at M2
        assert_eq!(segs.len(), 1);
        assert!(!state.running);
    }

    #[test]
    fn interp_arc_produces_segments() {
        let blocks = parse_program("G0 X10 Y0\nG2 X0 Y10 I-10 J0 F500").unwrap();
        let (segs, _) = interpret(&blocks).unwrap();
        // Arc should produce multiple line segments
        assert!(segs.len() > 2);
    }

    #[test]
    fn interp_modal_motion() {
        // G1 is modal — subsequent lines with just coordinates should use G1
        let blocks = parse_program("G1 X10 F800\nX20\nX30").unwrap();
        let (segs, state) = interpret(&blocks).unwrap();
        assert_eq!(segs.len(), 3);
        assert!(!segs[1].rapid);
        assert!(!segs[2].rapid);
        assert!((state.pos_x - 30.0).abs() < 1e-9);
    }

    #[test]
    fn interp_plane_selection() {
        let blocks = parse_program("G18\nG17").unwrap();
        let (_, state) = interpret(&blocks).unwrap();
        assert_eq!(state.plane, Plane::XY);
    }

    #[test]
    fn interp_g28_home() {
        let blocks = parse_program("G0 X10 Y10 Z10\nG28 Z0").unwrap();
        let (_, state) = interpret(&blocks).unwrap();
        assert!((state.pos_x - 0.0).abs() < 1e-9);
        assert!((state.pos_y - 0.0).abs() < 1e-9);
        assert!((state.pos_z - 0.0).abs() < 1e-9);
    }

    // ── Trajectory planner tests ─────────────────────────────────

    #[test]
    fn planner_single_segment() {
        let segs = vec![MotionSegment {
            x0: 0.0, y0: 0.0, z0: 0.0,
            x1: 100.0, y1: 0.0, z1: 0.0,
            feed: 1000.0,
            rapid: false,
            source_line: 1,
        }];
        let planned = plan_trajectory(&segs, &PlannerConfig::default());
        assert_eq!(planned.len(), 1);
        assert!(planned[0].duration > 0.0);
        assert!((planned[0].v_entry - 0.0).abs() < 1e-3); // starts from rest
        assert!((planned[0].v_exit - 0.0).abs() < 1e-3);  // ends at rest
    }

    #[test]
    fn planner_rapids_faster() {
        let slow = vec![MotionSegment {
            x0: 0.0, y0: 0.0, z0: 0.0,
            x1: 100.0, y1: 0.0, z1: 0.0,
            feed: 500.0,
            rapid: false,
            source_line: 1,
        }];
        let fast = vec![MotionSegment {
            x0: 0.0, y0: 0.0, z0: 0.0,
            x1: 100.0, y1: 0.0, z1: 0.0,
            feed: 0.0,
            rapid: true,
            source_line: 1,
        }];
        let cfg = PlannerConfig::default();
        let ps = plan_trajectory(&slow, &cfg);
        let pf = plan_trajectory(&fast, &cfg);
        assert!(pf[0].duration < ps[0].duration, "rapid should be faster than feed");
    }

    #[test]
    fn planner_collinear_maintains_speed() {
        let segs = vec![
            MotionSegment {
                x0: 0.0, y0: 0.0, z0: 0.0,
                x1: 50.0, y1: 0.0, z1: 0.0,
                feed: 1000.0,
                rapid: false,
                source_line: 1,
            },
            MotionSegment {
                x0: 50.0, y0: 0.0, z0: 0.0,
                x1: 100.0, y1: 0.0, z1: 0.0,
                feed: 1000.0,
                rapid: false,
                source_line: 2,
            },
        ];
        let planned = plan_trajectory(&segs, &PlannerConfig::default());
        // Collinear segments should have high junction speed
        assert!(planned[1].v_entry > 100.0, "collinear junction should be fast");
    }

    #[test]
    fn planner_sharp_corner_slows() {
        let segs = vec![
            MotionSegment {
                x0: 0.0, y0: 0.0, z0: 0.0,
                x1: 50.0, y1: 0.0, z1: 0.0,
                feed: 1000.0,
                rapid: false,
                source_line: 1,
            },
            MotionSegment {
                x0: 50.0, y0: 0.0, z0: 0.0,
                x1: 50.0, y1: 50.0, z1: 0.0,
                feed: 1000.0,
                rapid: false,
                source_line: 2,
            },
        ];
        let planned = plan_trajectory(&segs, &PlannerConfig::default());
        // 90-degree corner should have limited junction speed
        assert!(planned[1].v_entry < 500.0, "sharp corner should slow down");
    }

    #[test]
    fn planner_total_time() {
        let segs = vec![
            MotionSegment {
                x0: 0.0, y0: 0.0, z0: 0.0,
                x1: 100.0, y1: 0.0, z1: 0.0,
                feed: 6000.0,
                rapid: false,
                source_line: 1,
            },
        ];
        let planned = plan_trajectory(&segs, &PlannerConfig::default());
        let t = total_time(&planned);
        assert!(t > 0.0);
        // 100mm at 6000mm/min = 1 second minimum, but accel/decel adds time
        assert!(t > 0.5, "time should be reasonable");
    }

    #[test]
    fn interpolate_midpoint() {
        let seg = PlannedSegment {
            segment: MotionSegment {
                x0: 0.0, y0: 0.0, z0: 0.0,
                x1: 100.0, y1: 0.0, z1: 0.0,
                feed: 1000.0,
                rapid: false,
                source_line: 1,
            },
            v_entry: 0.0,
            v_cruise: 1000.0,
            v_exit: 0.0,
            duration: 10.0,
        };
        let (x, y, z) = interpolate_segment(&seg, 5.0);
        assert!((x - 50.0).abs() < 1e-9);
        assert!((y - 0.0).abs() < 1e-9);
        assert!((z - 0.0).abs() < 1e-9);
    }

    // ── Validation tests ─────────────────────────────────────────

    #[test]
    fn validate_missing_feed() {
        let blocks = parse_program("G1 X10").unwrap();
        let diags = validate(&blocks);
        assert!(diags.iter().any(|d| d.severity == "error" && d.message.contains("feed rate")));
    }

    #[test]
    fn validate_spindle_off_warning() {
        let blocks = parse_program("G1 X10 F800").unwrap();
        let diags = validate(&blocks);
        assert!(diags.iter().any(|d| d.severity == "warning" && d.message.contains("spindle")));
    }

    #[test]
    fn validate_clean_program() {
        let blocks = parse_program("G21\nM3 S12000\nG0 X0 Y0\nG1 X10 F800\nM5\nM2").unwrap();
        let diags = validate(&blocks);
        let errors: Vec<_> = diags.iter().filter(|d| d.severity == "error").collect();
        assert!(errors.is_empty(), "clean program should have no errors: {:?}", errors);
    }

    // ── End-to-end test ──────────────────────────────────────────

    #[test]
    fn end_to_end_parse_interpret_plan() {
        let program = "\
            G21 (metric)\n\
            G90 (absolute)\n\
            M3 S12000\n\
            G0 X0 Y0 Z5\n\
            G0 X10 Y10\n\
            G1 Z-1 F300\n\
            G1 X20 Y10 F800\n\
            G1 X20 Y20\n\
            G1 X10 Y20\n\
            G1 X10 Y10\n\
            G0 Z5\n\
            M5\n\
            M2\n";

        let blocks = parse_program(program).unwrap();
        assert!(!blocks.is_empty());

        let (segments, state) = interpret(&blocks).unwrap();
        assert!(!segments.is_empty());
        assert!(!state.running);
        assert_eq!(state.spindle, SpindleState::Off);

        let planned = plan_trajectory(&segments, &PlannerConfig::default());
        assert_eq!(planned.len(), segments.len());
        let t = total_time(&planned);
        assert!(t > 0.0);
    }
}
