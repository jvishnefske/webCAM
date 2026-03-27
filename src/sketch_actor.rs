/// Async actor-based 2D sketch constraint engine.
///
/// Dataflow model: each point and constraint is an actor that receives
/// messages through a central mailbox.  The solver iterates until all
/// constraints converge or a max iteration count is reached.
///
/// Designed to run inside WASM (single-threaded) — the "async" contract
/// is a cooperative message-pump that yields control back to JS between
/// solver steps via `requestAnimationFrame` callbacks.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── IDs ──────────────────────────────────────────────────────────────

/// Opaque handle to a point in the sketch.
pub type PointId = u32;
/// Opaque handle to a constraint.
pub type ConstraintId = u32;

// ── Point ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    /// Whether this point is locked (fully fixed by the user).
    pub fixed: bool,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y, fixed: false }
    }
    pub fn fixed(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            fixed: true,
        }
    }
}

// ── Constraint kinds ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Constraint {
    /// Two points occupy the same position.
    Coincident(PointId, PointId),
    /// The segment between two points has a fixed length.
    Distance(PointId, PointId, f64),
    /// Two points share the same Y coordinate (horizontal line).
    Horizontal(PointId, PointId),
    /// Two points share the same X coordinate (vertical line).
    Vertical(PointId, PointId),
    /// A point is fixed at a specific (x, y).
    FixedPosition(PointId, f64, f64),
    /// The angle of the segment p0→p1 is fixed (radians).
    Angle(PointId, PointId, f64),
    /// A circle centre + radius-point has a fixed radius.
    Radius(PointId, PointId, f64),
    /// Two segments are perpendicular: (a0→a1) ⊥ (b0→b1).
    Perpendicular(PointId, PointId, PointId, PointId),
    /// Two segments are parallel: (a0→a1) ∥ (b0→b1).
    Parallel(PointId, PointId, PointId, PointId),
    /// Point lies on the midpoint of two other points.
    Midpoint(PointId, PointId, PointId),
    /// Two segments have equal length: |a0→a1| == |b0→b1|.
    EqualLength(PointId, PointId, PointId, PointId),
    /// Point is symmetric to another point about a mirror line (m0→m1).
    Symmetric(PointId, PointId, PointId, PointId),
}

// ── Actor messages ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Msg {
    /// Add a new point, returns its id.
    AddPoint(f64, f64),
    /// Add a fixed point.
    AddFixedPoint(f64, f64),
    /// Move a point (user drag).
    MovePoint(PointId, f64, f64),
    /// Toggle fixed flag.
    SetFixed(PointId, bool),
    /// Remove a point and all its constraints.
    RemovePoint(PointId),
    /// Add a constraint.
    AddConstraint(Constraint),
    /// Remove a constraint.
    RemoveConstraint(ConstraintId),
    /// Run the solver for up to N iterations.
    Solve(u32),
    /// Query the current state (snapshot).
    Snapshot,
}

// ── Solver result ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SolveStatus {
    /// All constraints satisfied within tolerance.
    Converged,
    /// Solver did not converge within the iteration limit.
    UnderConstrained,
    /// Contradictory constraints detected.
    OverConstrained,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolveResult {
    pub status: SolveStatus,
    pub iterations: u32,
    pub max_error: f64,
}

// ── DOF (degrees of freedom) tracker ─────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DofStatus {
    FullyConstrained,
    UnderConstrained,
    OverConstrained,
}

// ── Snapshot (full state for rendering) ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SketchSnapshot {
    pub points: Vec<(PointId, Point)>,
    pub constraints: Vec<(ConstraintId, Constraint)>,
    pub solve: SolveResult,
    pub dof: i32,
    pub dof_status: DofStatus,
    /// Per-point DOF status for coloring.
    pub point_status: HashMap<PointId, DofStatus>,
}

// ── The Actor ────────────────────────────────────────────────────────

pub struct SketchActor {
    pub points: HashMap<PointId, Point>,
    pub constraints: HashMap<ConstraintId, Constraint>,
    next_point_id: PointId,
    next_constraint_id: ConstraintId,
    /// Queued messages (cooperative mailbox).
    mailbox: Vec<Msg>,
    /// Last solve result, cached for snapshot queries.
    last_solve: SolveResult,
    /// Solver tolerance (distance units).
    pub tolerance: f64,
}

impl Default for SketchActor {
    fn default() -> Self {
        Self::new()
    }
}

impl SketchActor {
    pub fn new() -> Self {
        Self {
            points: HashMap::new(),
            constraints: HashMap::new(),
            next_point_id: 1,
            next_constraint_id: 1,
            mailbox: Vec::new(),
            last_solve: SolveResult {
                status: SolveStatus::Converged,
                iterations: 0,
                max_error: 0.0,
            },
            tolerance: 1e-6,
        }
    }

    // ── Mailbox API (queue + drain) ──────────────────────────────────

    /// Queue a message for later processing.
    pub fn send(&mut self, msg: Msg) {
        self.mailbox.push(msg);
    }

    /// Process all queued messages, returning the id of the last entity
    /// created (if any) and a snapshot after solving.
    pub fn pump(&mut self) -> (Option<u32>, SketchSnapshot) {
        let msgs: Vec<Msg> = self.mailbox.drain(..).collect();
        let mut last_id = None;
        for msg in msgs {
            match msg {
                Msg::AddPoint(x, y) => {
                    let id = self.add_point(x, y);
                    last_id = Some(id);
                }
                Msg::AddFixedPoint(x, y) => {
                    let id = self.add_point_fixed(x, y);
                    last_id = Some(id);
                }
                Msg::MovePoint(id, x, y) => self.move_point(id, x, y),
                Msg::SetFixed(id, f) => {
                    if let Some(p) = self.points.get_mut(&id) {
                        p.fixed = f;
                    }
                }
                Msg::RemovePoint(id) => self.remove_point(id),
                Msg::AddConstraint(c) => {
                    let id = self.add_constraint(c);
                    last_id = Some(id);
                }
                Msg::RemoveConstraint(id) => {
                    self.constraints.remove(&id);
                }
                Msg::Solve(max_iter) => {
                    self.last_solve = self.solve(max_iter);
                }
                Msg::Snapshot => {} // handled by return value
            }
        }
        // Always solve after processing to keep state consistent.
        self.last_solve = self.solve(200);
        let snap = self.snapshot();
        (last_id, snap)
    }

    // ── Direct API (synchronous) ─────────────────────────────────────

    pub fn add_point(&mut self, x: f64, y: f64) -> PointId {
        let id = self.next_point_id;
        self.next_point_id += 1;
        self.points.insert(id, Point::new(x, y));
        id
    }

    pub fn add_point_fixed(&mut self, x: f64, y: f64) -> PointId {
        let id = self.next_point_id;
        self.next_point_id += 1;
        self.points.insert(id, Point::fixed(x, y));
        id
    }

    pub fn move_point(&mut self, id: PointId, x: f64, y: f64) {
        if let Some(p) = self.points.get_mut(&id) {
            p.x = x;
            p.y = y;
        }
    }

    pub fn remove_point(&mut self, id: PointId) {
        self.points.remove(&id);
        // Remove all constraints referencing this point.
        self.constraints.retain(|_, c| !constraint_refs_point(c, id));
    }

    pub fn add_constraint(&mut self, c: Constraint) -> ConstraintId {
        let id = self.next_constraint_id;
        self.next_constraint_id += 1;
        self.constraints.insert(id, c);
        id
    }

    pub fn point(&self, id: PointId) -> Option<&Point> {
        self.points.get(&id)
    }

    pub fn points(&self) -> &HashMap<PointId, Point> {
        &self.points
    }

    pub fn constraints(&self) -> &HashMap<ConstraintId, Constraint> {
        &self.constraints
    }

    // ── DOF calculation ──────────────────────────────────────────────

    /// Total degrees of freedom = 2 * free_points - constraint_equations.
    pub fn dof(&self) -> i32 {
        let free_pts = self.points.values().filter(|p| !p.fixed).count() as i32;
        let n_eqs: i32 = self
            .constraints
            .values()
            .map(|c| constraint_equation_count(c) as i32)
            .sum();
        2 * free_pts - n_eqs
    }

    pub fn dof_status(&self) -> DofStatus {
        let d = self.dof();
        if d == 0 {
            DofStatus::FullyConstrained
        } else if d > 0 {
            DofStatus::UnderConstrained
        } else {
            DofStatus::OverConstrained
        }
    }

    /// Per-point constraint status (how many equations reference each point).
    fn point_statuses(&self) -> HashMap<PointId, DofStatus> {
        let mut eq_count: HashMap<PointId, i32> = HashMap::new();
        for c in self.constraints.values() {
            for pid in constraint_point_ids(c) {
                *eq_count.entry(pid).or_insert(0) += constraint_equation_count(c) as i32;
            }
        }
        self.points
            .keys()
            .map(|&id| {
                let p = &self.points[&id];
                if p.fixed {
                    return (id, DofStatus::FullyConstrained);
                }
                let eqs = eq_count.get(&id).copied().unwrap_or(0);
                let status = if eqs >= 2 {
                    DofStatus::FullyConstrained
                } else {
                    DofStatus::UnderConstrained
                };
                (id, status)
            })
            .collect()
    }

    // ── Snapshot ─────────────────────────────────────────────────────

    pub fn snapshot(&self) -> SketchSnapshot {
        let mut pts: Vec<_> = self.points.iter().map(|(&id, &p)| (id, p)).collect();
        pts.sort_by_key(|(id, _)| *id);
        let mut cons: Vec<_> = self
            .constraints
            .iter()
            .map(|(&id, c)| (id, c.clone()))
            .collect();
        cons.sort_by_key(|(id, _)| *id);
        SketchSnapshot {
            points: pts,
            constraints: cons,
            solve: self.last_solve.clone(),
            dof: self.dof(),
            dof_status: self.dof_status(),
            point_status: self.point_statuses(),
        }
    }

    // ── Constraint solver (Gauss-Seidel relaxation) ──────────────────

    pub fn solve(&mut self, max_iter: u32) -> SolveResult {
        if self.constraints.is_empty() {
            return SolveResult {
                status: SolveStatus::Converged,
                iterations: 0,
                max_error: 0.0,
            };
        }

        let mut iterations = 0;
        let mut max_error;

        for _ in 0..max_iter {
            iterations += 1;
            max_error = 0.0_f64;

            // Collect constraint ids to avoid borrow issues.
            let cids: Vec<ConstraintId> = self.constraints.keys().copied().collect();

            for cid in &cids {
                let c = self.constraints[cid].clone();
                let err = self.apply_constraint(&c);
                max_error = max_error.max(err);
            }

            if max_error < self.tolerance {
                return SolveResult {
                    status: SolveStatus::Converged,
                    iterations,
                    max_error,
                };
            }
        }

        // Check if we're over-constrained (DOF < 0 and didn't converge).
        let status = if self.dof() < 0 {
            SolveStatus::OverConstrained
        } else {
            SolveStatus::UnderConstrained
        };

        SolveResult {
            status,
            iterations,
            max_error: self.max_constraint_error(),
        }
    }

    /// Apply a single constraint, moving free points towards satisfaction.
    /// Returns the error magnitude before correction.
    fn apply_constraint(&mut self, c: &Constraint) -> f64 {
        match c {
            Constraint::Coincident(a, b) => self.apply_coincident(*a, *b),
            Constraint::Distance(a, b, d) => self.apply_distance(*a, *b, *d),
            Constraint::Horizontal(a, b) => self.apply_horizontal(*a, *b),
            Constraint::Vertical(a, b) => self.apply_vertical(*a, *b),
            Constraint::FixedPosition(id, x, y) => self.apply_fixed(*id, *x, *y),
            Constraint::Angle(a, b, angle) => self.apply_angle(*a, *b, *angle),
            Constraint::Radius(center, edge, r) => self.apply_distance(*center, *edge, *r),
            Constraint::Perpendicular(a0, a1, b0, b1) => {
                self.apply_perpendicular(*a0, *a1, *b0, *b1)
            }
            Constraint::Parallel(a0, a1, b0, b1) => self.apply_parallel(*a0, *a1, *b0, *b1),
            Constraint::Midpoint(mid, a, b) => self.apply_midpoint(*mid, *a, *b),
            Constraint::EqualLength(a0, a1, b0, b1) => {
                self.apply_equal_length(*a0, *a1, *b0, *b1)
            }
            Constraint::Symmetric(p, q, m0, m1) => self.apply_symmetric(*p, *q, *m0, *m1),
        }
    }

    fn apply_coincident(&mut self, a: PointId, b: PointId) -> f64 {
        let (pa, pb) = match (self.points.get(&a).copied(), self.points.get(&b).copied()) {
            (Some(pa), Some(pb)) => (pa, pb),
            _ => return 0.0,
        };
        let dx = pb.x - pa.x;
        let dy = pb.y - pa.y;
        let err = (dx * dx + dy * dy).sqrt();
        let (wa, wb) = weights(pa.fixed, pb.fixed);
        if let Some(p) = self.points.get_mut(&a) {
            p.x += dx * wa;
            p.y += dy * wa;
        }
        if let Some(p) = self.points.get_mut(&b) {
            p.x -= dx * wb;
            p.y -= dy * wb;
        }
        err
    }

    fn apply_distance(&mut self, a: PointId, b: PointId, target: f64) -> f64 {
        let (pa, pb) = match (self.points.get(&a).copied(), self.points.get(&b).copied()) {
            (Some(pa), Some(pb)) => (pa, pb),
            _ => return 0.0,
        };
        let dx = pb.x - pa.x;
        let dy = pb.y - pa.y;
        let current = (dx * dx + dy * dy).sqrt();
        if current < 1e-12 {
            return target; // degenerate — points on top of each other
        }
        let err = (current - target).abs();
        let correction = (current - target) / current;
        let (wa, wb) = weights(pa.fixed, pb.fixed);
        if let Some(p) = self.points.get_mut(&a) {
            p.x += dx * correction * wa;
            p.y += dy * correction * wa;
        }
        if let Some(p) = self.points.get_mut(&b) {
            p.x -= dx * correction * wb;
            p.y -= dy * correction * wb;
        }
        err
    }

    fn apply_horizontal(&mut self, a: PointId, b: PointId) -> f64 {
        let (pa, pb) = match (self.points.get(&a).copied(), self.points.get(&b).copied()) {
            (Some(pa), Some(pb)) => (pa, pb),
            _ => return 0.0,
        };
        let dy = pb.y - pa.y;
        let err = dy.abs();
        let (wa, wb) = weights(pa.fixed, pb.fixed);
        if let Some(p) = self.points.get_mut(&a) {
            p.y += dy * wa;
        }
        if let Some(p) = self.points.get_mut(&b) {
            p.y -= dy * wb;
        }
        err
    }

    fn apply_vertical(&mut self, a: PointId, b: PointId) -> f64 {
        let (pa, pb) = match (self.points.get(&a).copied(), self.points.get(&b).copied()) {
            (Some(pa), Some(pb)) => (pa, pb),
            _ => return 0.0,
        };
        let dx = pb.x - pa.x;
        let err = dx.abs();
        let (wa, wb) = weights(pa.fixed, pb.fixed);
        if let Some(p) = self.points.get_mut(&a) {
            p.x += dx * wa;
        }
        if let Some(p) = self.points.get_mut(&b) {
            p.x -= dx * wb;
        }
        err
    }

    fn apply_fixed(&mut self, id: PointId, tx: f64, ty: f64) -> f64 {
        if let Some(p) = self.points.get_mut(&id) {
            let err = ((p.x - tx).powi(2) + (p.y - ty).powi(2)).sqrt();
            p.x = tx;
            p.y = ty;
            err
        } else {
            0.0
        }
    }

    fn apply_angle(&mut self, a: PointId, b: PointId, target: f64) -> f64 {
        let (pa, pb) = match (self.points.get(&a).copied(), self.points.get(&b).copied()) {
            (Some(pa), Some(pb)) => (pa, pb),
            _ => return 0.0,
        };
        let dx = pb.x - pa.x;
        let dy = pb.y - pa.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-12 {
            return 0.0;
        }
        let current = dy.atan2(dx);
        let mut diff = target - current;
        // Normalize angle difference to [-pi, pi].
        while diff > std::f64::consts::PI {
            diff -= 2.0 * std::f64::consts::PI;
        }
        while diff < -std::f64::consts::PI {
            diff += 2.0 * std::f64::consts::PI;
        }
        let err = diff.abs() * len;
        if err < self.tolerance {
            return err;
        }
        let new_angle = current + diff;
        let nx = pa.x + len * new_angle.cos();
        let ny = pa.y + len * new_angle.sin();
        let (wa, wb) = weights(pa.fixed, pb.fixed);
        // Move b towards target angle.
        if let Some(p) = self.points.get_mut(&b) {
            p.x += (nx - pb.x) * wb;
            p.y += (ny - pb.y) * wb;
        }
        // Move a in opposite sense.
        if !pa.fixed {
            let nx_a = pb.x - len * new_angle.cos();
            let ny_a = pb.y - len * new_angle.sin();
            if let Some(p) = self.points.get_mut(&a) {
                p.x += (nx_a - pa.x) * wa;
                p.y += (ny_a - pa.y) * wa;
            }
        }
        err
    }

    fn apply_perpendicular(
        &mut self,
        a0: PointId,
        a1: PointId,
        b0: PointId,
        b1: PointId,
    ) -> f64 {
        let pts = match (
            self.points.get(&a0).copied(),
            self.points.get(&a1).copied(),
            self.points.get(&b0).copied(),
            self.points.get(&b1).copied(),
        ) {
            (Some(pa0), Some(pa1), Some(pb0), Some(pb1)) => (pa0, pa1, pb0, pb1),
            _ => return 0.0,
        };
        let (pa0, pa1, pb0, pb1) = pts;
        let dax = pa1.x - pa0.x;
        let day = pa1.y - pa0.y;
        let dbx = pb1.x - pb0.x;
        let dby = pb1.y - pb0.y;
        let dot = dax * dbx + day * dby;
        let len_a = (dax * dax + day * day).sqrt();
        let len_b = (dbx * dbx + dby * dby).sqrt();
        if len_a < 1e-12 || len_b < 1e-12 {
            return 0.0;
        }
        let err = dot.abs() / (len_a * len_b);
        if err < self.tolerance {
            return err;
        }
        // Rotate segment B so dot product is zero.
        // Target direction for B: perpendicular to A.
        let target_angle = day.atan2(dax) + std::f64::consts::FRAC_PI_2;
        let (_, wb) = weights(pb0.fixed, pb1.fixed);
        let nx = pb0.x + len_b * target_angle.cos();
        let ny = pb0.y + len_b * target_angle.sin();
        if let Some(p) = self.points.get_mut(&b1) {
            p.x += (nx - pb1.x) * wb * 0.5;
            p.y += (ny - pb1.y) * wb * 0.5;
        }
        err
    }

    fn apply_parallel(&mut self, a0: PointId, a1: PointId, b0: PointId, b1: PointId) -> f64 {
        let pts = match (
            self.points.get(&a0).copied(),
            self.points.get(&a1).copied(),
            self.points.get(&b0).copied(),
            self.points.get(&b1).copied(),
        ) {
            (Some(pa0), Some(pa1), Some(pb0), Some(pb1)) => (pa0, pa1, pb0, pb1),
            _ => return 0.0,
        };
        let (pa0, pa1, pb0, pb1) = pts;
        let dax = pa1.x - pa0.x;
        let day = pa1.y - pa0.y;
        let dbx = pb1.x - pb0.x;
        let dby = pb1.y - pb0.y;
        let cross = dax * dby - day * dbx;
        let len_a = (dax * dax + day * day).sqrt();
        let len_b = (dbx * dbx + dby * dby).sqrt();
        if len_a < 1e-12 || len_b < 1e-12 {
            return 0.0;
        }
        let err = cross.abs() / (len_a * len_b);
        if err < self.tolerance {
            return err;
        }
        // Rotate B to match A's angle.
        let target_angle = day.atan2(dax);
        let (_, wb) = weights(pb0.fixed, pb1.fixed);
        let nx = pb0.x + len_b * target_angle.cos();
        let ny = pb0.y + len_b * target_angle.sin();
        if let Some(p) = self.points.get_mut(&b1) {
            p.x += (nx - pb1.x) * wb * 0.5;
            p.y += (ny - pb1.y) * wb * 0.5;
        }
        err
    }

    fn apply_midpoint(&mut self, mid: PointId, a: PointId, b: PointId) -> f64 {
        let (pm, pa, pb) = match (
            self.points.get(&mid).copied(),
            self.points.get(&a).copied(),
            self.points.get(&b).copied(),
        ) {
            (Some(pm), Some(pa), Some(pb)) => (pm, pa, pb),
            _ => return 0.0,
        };
        let mx = (pa.x + pb.x) / 2.0;
        let my = (pa.y + pb.y) / 2.0;
        let err = ((pm.x - mx).powi(2) + (pm.y - my).powi(2)).sqrt();
        if !pm.fixed {
            if let Some(p) = self.points.get_mut(&mid) {
                p.x = mx;
                p.y = my;
            }
        }
        err
    }

    fn apply_equal_length(
        &mut self,
        a0: PointId,
        a1: PointId,
        b0: PointId,
        b1: PointId,
    ) -> f64 {
        let pts = match (
            self.points.get(&a0).copied(),
            self.points.get(&a1).copied(),
            self.points.get(&b0).copied(),
            self.points.get(&b1).copied(),
        ) {
            (Some(pa0), Some(pa1), Some(pb0), Some(pb1)) => (pa0, pa1, pb0, pb1),
            _ => return 0.0,
        };
        let (pa0, pa1, pb0, pb1) = pts;
        let len_a = ((pa1.x - pa0.x).powi(2) + (pa1.y - pa0.y).powi(2)).sqrt();
        let len_b = ((pb1.x - pb0.x).powi(2) + (pb1.y - pb0.y).powi(2)).sqrt();
        let target = (len_a + len_b) / 2.0;
        // Apply distance constraint to both segments towards the average.
        let err_a = self.apply_distance(a0, a1, target);
        let err_b = self.apply_distance(b0, b1, target);
        err_a.max(err_b)
    }

    fn apply_symmetric(
        &mut self,
        p: PointId,
        q: PointId,
        m0: PointId,
        m1: PointId,
    ) -> f64 {
        let pts = match (
            self.points.get(&p).copied(),
            self.points.get(&q).copied(),
            self.points.get(&m0).copied(),
            self.points.get(&m1).copied(),
        ) {
            (Some(pp), Some(pq), Some(pm0), Some(pm1)) => (pp, pq, pm0, pm1),
            _ => return 0.0,
        };
        let (pp, pq, pm0, pm1) = pts;
        // Mirror p across line m0→m1, result should equal q.
        let mdx = pm1.x - pm0.x;
        let mdy = pm1.y - pm0.y;
        let len2 = mdx * mdx + mdy * mdy;
        if len2 < 1e-24 {
            return 0.0;
        }
        // Project p onto mirror line.
        let t = ((pp.x - pm0.x) * mdx + (pp.y - pm0.y) * mdy) / len2;
        let proj_x = pm0.x + t * mdx;
        let proj_y = pm0.y + t * mdy;
        // Reflected point.
        let rx = 2.0 * proj_x - pp.x;
        let ry = 2.0 * proj_y - pp.y;
        let err = ((pq.x - rx).powi(2) + (pq.y - ry).powi(2)).sqrt();
        if !pq.fixed {
            if let Some(pt) = self.points.get_mut(&q) {
                pt.x += (rx - pq.x) * 0.5;
                pt.y += (ry - pq.y) * 0.5;
            }
        }
        err
    }

    /// Compute the maximum error across all constraints.
    fn max_constraint_error(&self) -> f64 {
        let mut max_err = 0.0_f64;
        for c in self.constraints.values() {
            let err = self.constraint_error(c);
            max_err = max_err.max(err);
        }
        max_err
    }

    /// Read-only error measurement for a constraint (does not mutate points).
    fn constraint_error(&self, c: &Constraint) -> f64 {
        match c {
            Constraint::Coincident(a, b) | Constraint::Radius(a, b, _) => {
                let target = match c {
                    Constraint::Radius(_, _, r) => *r,
                    _ => 0.0,
                };
                match (self.points.get(a), self.points.get(b)) {
                    (Some(pa), Some(pb)) => {
                        let d = ((pb.x - pa.x).powi(2) + (pb.y - pa.y).powi(2)).sqrt();
                        if target == 0.0 {
                            d
                        } else {
                            (d - target).abs()
                        }
                    }
                    _ => 0.0,
                }
            }
            Constraint::Distance(a, b, d) => match (self.points.get(a), self.points.get(b)) {
                (Some(pa), Some(pb)) => {
                    let cur = ((pb.x - pa.x).powi(2) + (pb.y - pa.y).powi(2)).sqrt();
                    (cur - d).abs()
                }
                _ => 0.0,
            },
            Constraint::Horizontal(a, b) => match (self.points.get(a), self.points.get(b)) {
                (Some(pa), Some(pb)) => (pb.y - pa.y).abs(),
                _ => 0.0,
            },
            Constraint::Vertical(a, b) => match (self.points.get(a), self.points.get(b)) {
                (Some(pa), Some(pb)) => (pb.x - pa.x).abs(),
                _ => 0.0,
            },
            Constraint::FixedPosition(id, x, y) => match self.points.get(id) {
                Some(p) => ((p.x - x).powi(2) + (p.y - y).powi(2)).sqrt(),
                None => 0.0,
            },
            Constraint::Angle(a, b, target) => {
                match (self.points.get(a), self.points.get(b)) {
                    (Some(pa), Some(pb)) => {
                        let dx = pb.x - pa.x;
                        let dy = pb.y - pa.y;
                        let len = (dx * dx + dy * dy).sqrt();
                        if len < 1e-12 {
                            return 0.0;
                        }
                        let current = dy.atan2(dx);
                        let mut diff = target - current;
                        while diff > std::f64::consts::PI {
                            diff -= 2.0 * std::f64::consts::PI;
                        }
                        while diff < -std::f64::consts::PI {
                            diff += 2.0 * std::f64::consts::PI;
                        }
                        diff.abs() * len
                    }
                    _ => 0.0,
                }
            }
            Constraint::Perpendicular(a0, a1, b0, b1) => {
                match (
                    self.points.get(a0),
                    self.points.get(a1),
                    self.points.get(b0),
                    self.points.get(b1),
                ) {
                    (Some(pa0), Some(pa1), Some(pb0), Some(pb1)) => {
                        let dax = pa1.x - pa0.x;
                        let day = pa1.y - pa0.y;
                        let dbx = pb1.x - pb0.x;
                        let dby = pb1.y - pb0.y;
                        let len_a = (dax * dax + day * day).sqrt();
                        let len_b = (dbx * dbx + dby * dby).sqrt();
                        if len_a < 1e-12 || len_b < 1e-12 {
                            return 0.0;
                        }
                        (dax * dbx + day * dby).abs() / (len_a * len_b)
                    }
                    _ => 0.0,
                }
            }
            Constraint::Parallel(a0, a1, b0, b1) => {
                match (
                    self.points.get(a0),
                    self.points.get(a1),
                    self.points.get(b0),
                    self.points.get(b1),
                ) {
                    (Some(pa0), Some(pa1), Some(pb0), Some(pb1)) => {
                        let dax = pa1.x - pa0.x;
                        let day = pa1.y - pa0.y;
                        let dbx = pb1.x - pb0.x;
                        let dby = pb1.y - pb0.y;
                        let len_a = (dax * dax + day * day).sqrt();
                        let len_b = (dbx * dbx + dby * dby).sqrt();
                        if len_a < 1e-12 || len_b < 1e-12 {
                            return 0.0;
                        }
                        (dax * dby - day * dbx).abs() / (len_a * len_b)
                    }
                    _ => 0.0,
                }
            }
            Constraint::Midpoint(mid, a, b) => {
                match (self.points.get(mid), self.points.get(a), self.points.get(b)) {
                    (Some(pm), Some(pa), Some(pb)) => {
                        let mx = (pa.x + pb.x) / 2.0;
                        let my = (pa.y + pb.y) / 2.0;
                        ((pm.x - mx).powi(2) + (pm.y - my).powi(2)).sqrt()
                    }
                    _ => 0.0,
                }
            }
            Constraint::EqualLength(a0, a1, b0, b1) => {
                match (
                    self.points.get(a0),
                    self.points.get(a1),
                    self.points.get(b0),
                    self.points.get(b1),
                ) {
                    (Some(pa0), Some(pa1), Some(pb0), Some(pb1)) => {
                        let la = ((pa1.x - pa0.x).powi(2) + (pa1.y - pa0.y).powi(2)).sqrt();
                        let lb = ((pb1.x - pb0.x).powi(2) + (pb1.y - pb0.y).powi(2)).sqrt();
                        (la - lb).abs()
                    }
                    _ => 0.0,
                }
            }
            Constraint::Symmetric(p, q, m0, m1) => {
                match (
                    self.points.get(p),
                    self.points.get(q),
                    self.points.get(m0),
                    self.points.get(m1),
                ) {
                    (Some(pp), Some(pq), Some(pm0), Some(pm1)) => {
                        let mdx = pm1.x - pm0.x;
                        let mdy = pm1.y - pm0.y;
                        let len2 = mdx * mdx + mdy * mdy;
                        if len2 < 1e-24 {
                            return 0.0;
                        }
                        let t = ((pp.x - pm0.x) * mdx + (pp.y - pm0.y) * mdy) / len2;
                        let rx = 2.0 * (pm0.x + t * mdx) - pp.x;
                        let ry = 2.0 * (pm0.y + t * mdy) - pp.y;
                        ((pq.x - rx).powi(2) + (pq.y - ry).powi(2)).sqrt()
                    }
                    _ => 0.0,
                }
            }
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Weight distribution: if one point is fixed, the other gets all the
/// correction.  If both free, split 50/50.
fn weights(a_fixed: bool, b_fixed: bool) -> (f64, f64) {
    match (a_fixed, b_fixed) {
        (true, true) => (0.0, 0.0),
        (true, false) => (0.0, 1.0),
        (false, true) => (1.0, 0.0),
        (false, false) => (0.5, 0.5),
    }
}

/// How many scalar equations a constraint contributes.
fn constraint_equation_count(c: &Constraint) -> u32 {
    match c {
        Constraint::Coincident(..) => 2,  // x and y
        Constraint::Distance(..) => 1,
        Constraint::Horizontal(..) => 1,
        Constraint::Vertical(..) => 1,
        Constraint::FixedPosition(..) => 2,
        Constraint::Angle(..) => 1,
        Constraint::Radius(..) => 1,
        Constraint::Perpendicular(..) => 1,
        Constraint::Parallel(..) => 1,
        Constraint::Midpoint(..) => 2,
        Constraint::EqualLength(..) => 1,
        Constraint::Symmetric(..) => 2,
    }
}

/// Does this constraint reference a given point?
fn constraint_refs_point(c: &Constraint, id: PointId) -> bool {
    constraint_point_ids(c).contains(&id)
}

/// All point ids referenced by a constraint.
fn constraint_point_ids(c: &Constraint) -> Vec<PointId> {
    match c {
        Constraint::Coincident(a, b)
        | Constraint::Distance(a, b, _)
        | Constraint::Horizontal(a, b)
        | Constraint::Vertical(a, b)
        | Constraint::Angle(a, b, _)
        | Constraint::Radius(a, b, _) => vec![*a, *b],
        Constraint::FixedPosition(a, _, _) => vec![*a],
        Constraint::Perpendicular(a, b, c, d)
        | Constraint::Parallel(a, b, c, d)
        | Constraint::EqualLength(a, b, c, d)
        | Constraint::Symmetric(a, b, c, d) => vec![*a, *b, *c, *d],
        Constraint::Midpoint(m, a, b) => vec![*m, *a, *b],
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_points_and_snapshot() {
        let mut actor = SketchActor::new();
        let p1 = actor.add_point(0.0, 0.0);
        let p2 = actor.add_point(10.0, 0.0);
        let snap = actor.snapshot();
        assert_eq!(snap.points.len(), 2);
        assert_eq!(p1, 1);
        assert_eq!(p2, 2);
    }

    #[test]
    fn test_coincident_constraint() {
        let mut actor = SketchActor::new();
        let p1 = actor.add_point(0.0, 0.0);
        let p2 = actor.add_point(10.0, 5.0);
        actor.add_constraint(Constraint::Coincident(p1, p2));
        let result = actor.solve(200);
        assert_eq!(result.status, SolveStatus::Converged);
        let a = actor.point(p1).unwrap();
        let b = actor.point(p2).unwrap();
        assert!((a.x - b.x).abs() < 1e-6);
        assert!((a.y - b.y).abs() < 1e-6);
    }

    #[test]
    fn test_distance_constraint() {
        let mut actor = SketchActor::new();
        let p1 = actor.add_point_fixed(0.0, 0.0);
        let p2 = actor.add_point(3.0, 0.0);
        actor.add_constraint(Constraint::Distance(p1, p2, 5.0));
        let result = actor.solve(200);
        assert_eq!(result.status, SolveStatus::Converged);
        let b = actor.point(p2).unwrap();
        let dist = (b.x * b.x + b.y * b.y).sqrt();
        assert!((dist - 5.0).abs() < 1e-6, "dist={dist}");
    }

    #[test]
    fn test_horizontal_constraint() {
        let mut actor = SketchActor::new();
        let p1 = actor.add_point(0.0, 0.0);
        let p2 = actor.add_point(10.0, 7.0);
        actor.add_constraint(Constraint::Horizontal(p1, p2));
        let result = actor.solve(200);
        assert_eq!(result.status, SolveStatus::Converged);
        let a = actor.point(p1).unwrap();
        let b = actor.point(p2).unwrap();
        assert!((a.y - b.y).abs() < 1e-6);
    }

    #[test]
    fn test_vertical_constraint() {
        let mut actor = SketchActor::new();
        let p1 = actor.add_point(0.0, 0.0);
        let p2 = actor.add_point(5.0, 10.0);
        actor.add_constraint(Constraint::Vertical(p1, p2));
        let result = actor.solve(200);
        assert_eq!(result.status, SolveStatus::Converged);
        let a = actor.point(p1).unwrap();
        let b = actor.point(p2).unwrap();
        assert!((a.x - b.x).abs() < 1e-6);
    }

    #[test]
    fn test_fixed_position() {
        let mut actor = SketchActor::new();
        let p1 = actor.add_point(5.0, 5.0);
        actor.add_constraint(Constraint::FixedPosition(p1, 10.0, 20.0));
        actor.solve(200);
        let a = actor.point(p1).unwrap();
        assert!((a.x - 10.0).abs() < 1e-6);
        assert!((a.y - 20.0).abs() < 1e-6);
    }

    #[test]
    fn test_dof_tracking() {
        let mut actor = SketchActor::new();
        // 1 free point = 2 DOF
        let p1 = actor.add_point(0.0, 0.0);
        assert_eq!(actor.dof(), 2);

        // Fix it = 0 DOF
        actor.add_constraint(Constraint::FixedPosition(p1, 0.0, 0.0));
        assert_eq!(actor.dof(), 0);
        assert_eq!(actor.dof_status(), DofStatus::FullyConstrained);
    }

    #[test]
    fn test_dof_two_points_with_distance() {
        let mut actor = SketchActor::new();
        let p1 = actor.add_point_fixed(0.0, 0.0);
        let _p2 = actor.add_point(5.0, 0.0);
        // p1 fixed (0 DOF) + p2 free (2 DOF) - distance (1 eq) = 1 DOF
        actor.add_constraint(Constraint::Distance(p1, _p2, 5.0));
        assert_eq!(actor.dof(), 1);
    }

    #[test]
    fn test_fully_constrained_triangle() {
        let mut actor = SketchActor::new();
        let p1 = actor.add_point_fixed(0.0, 0.0);
        let p2 = actor.add_point(10.0, 0.0);
        let p3 = actor.add_point(5.0, 8.66);

        // Fix p2 horizontal + distance from p1
        actor.add_constraint(Constraint::Horizontal(p1, p2));
        actor.add_constraint(Constraint::Distance(p1, p2, 10.0));
        // Fix p3 with distances from both
        actor.add_constraint(Constraint::Distance(p1, p3, 10.0));
        actor.add_constraint(Constraint::Distance(p2, p3, 10.0));

        // p1 fixed (0) + p2 free (2) + p3 free (2) = 4 DOF
        // Constraints: horiz(1) + dist(1) + dist(1) + dist(1) = 4 equations
        // DOF = 4 - 4 = 0
        assert_eq!(actor.dof(), 0);
        assert_eq!(actor.dof_status(), DofStatus::FullyConstrained);

        let result = actor.solve(200);
        assert_eq!(result.status, SolveStatus::Converged);

        // p2 should be at (10, 0) — horizontal + distance 10
        let b = actor.point(p2).unwrap();
        assert!((b.x - 10.0).abs() < 1e-4, "p2.x={}", b.x);
        assert!(b.y.abs() < 1e-4, "p2.y={}", b.y);
    }

    #[test]
    fn test_mailbox_pump() {
        let mut actor = SketchActor::new();
        actor.send(Msg::AddPoint(0.0, 0.0));
        actor.send(Msg::AddPoint(10.0, 0.0));
        actor.send(Msg::AddConstraint(Constraint::Distance(1, 2, 5.0)));
        actor.send(Msg::Solve(100));

        let (last_id, snap) = actor.pump();
        assert!(last_id.is_some());
        assert_eq!(snap.points.len(), 2);
        assert_eq!(snap.constraints.len(), 1);
    }

    #[test]
    fn test_remove_point_cascades() {
        let mut actor = SketchActor::new();
        let p1 = actor.add_point(0.0, 0.0);
        let p2 = actor.add_point(10.0, 0.0);
        let p3 = actor.add_point(5.0, 5.0);
        actor.add_constraint(Constraint::Distance(p1, p2, 10.0));
        actor.add_constraint(Constraint::Distance(p2, p3, 7.0));
        assert_eq!(actor.constraints().len(), 2);

        actor.remove_point(p2);
        // Both constraints reference p2, so both should be removed.
        assert_eq!(actor.constraints().len(), 0);
        assert_eq!(actor.points().len(), 2);
    }

    #[test]
    fn test_over_constrained() {
        let mut actor = SketchActor::new();
        let p1 = actor.add_point_fixed(0.0, 0.0);
        let p2 = actor.add_point(10.0, 0.0);
        // 1 free point = 2 DOF, but 3 equations = over-constrained
        actor.add_constraint(Constraint::FixedPosition(p2, 5.0, 0.0));
        actor.add_constraint(Constraint::Distance(p1, p2, 10.0));
        assert!(actor.dof() < 0);
        assert_eq!(actor.dof_status(), DofStatus::OverConstrained);
    }

    #[test]
    fn test_perpendicular() {
        let mut actor = SketchActor::new();
        let a0 = actor.add_point_fixed(0.0, 0.0);
        let a1 = actor.add_point_fixed(10.0, 0.0);
        let b0 = actor.add_point_fixed(5.0, 0.0);
        let b1 = actor.add_point(8.0, 3.0);
        actor.add_constraint(Constraint::Perpendicular(a0, a1, b0, b1));
        let result = actor.solve(200);
        assert_eq!(result.status, SolveStatus::Converged);
        // b0→b1 should be vertical (perpendicular to horizontal a0→a1)
        let p_b0 = actor.point(b0).unwrap();
        let p_b1 = actor.point(b1).unwrap();
        assert!(
            (p_b1.x - p_b0.x).abs() < 0.1,
            "dx={}",
            (p_b1.x - p_b0.x).abs()
        );
    }

    #[test]
    fn test_parallel() {
        let mut actor = SketchActor::new();
        let a0 = actor.add_point_fixed(0.0, 0.0);
        let a1 = actor.add_point_fixed(10.0, 5.0);
        let b0 = actor.add_point_fixed(0.0, 10.0);
        let b1 = actor.add_point(7.0, 2.0);
        actor.add_constraint(Constraint::Parallel(a0, a1, b0, b1));
        let result = actor.solve(200);
        assert_eq!(result.status, SolveStatus::Converged);
        // Verify cross product is ~0
        let pa0 = actor.point(a0).unwrap();
        let pa1 = actor.point(a1).unwrap();
        let pb0 = actor.point(b0).unwrap();
        let pb1 = actor.point(b1).unwrap();
        let dax = pa1.x - pa0.x;
        let day = pa1.y - pa0.y;
        let dbx = pb1.x - pb0.x;
        let dby = pb1.y - pb0.y;
        let cross = (dax * dby - day * dbx).abs();
        let len = (dax * dax + day * day).sqrt() * (dbx * dbx + dby * dby).sqrt();
        assert!(cross / len < 0.01, "cross/len={}", cross / len);
    }

    #[test]
    fn test_midpoint() {
        let mut actor = SketchActor::new();
        let a = actor.add_point_fixed(0.0, 0.0);
        let b = actor.add_point_fixed(10.0, 6.0);
        let m = actor.add_point(3.0, 1.0);
        actor.add_constraint(Constraint::Midpoint(m, a, b));
        actor.solve(200);
        let pm = actor.point(m).unwrap();
        assert!((pm.x - 5.0).abs() < 1e-6);
        assert!((pm.y - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_equal_length() {
        let mut actor = SketchActor::new();
        let a0 = actor.add_point_fixed(0.0, 0.0);
        let a1 = actor.add_point_fixed(10.0, 0.0);
        let b0 = actor.add_point_fixed(0.0, 5.0);
        let b1 = actor.add_point(3.0, 5.0);
        actor.add_constraint(Constraint::EqualLength(a0, a1, b0, b1));
        actor.solve(200);
        let pb0 = actor.point(b0).unwrap();
        let pb1 = actor.point(b1).unwrap();
        let len_b = ((pb1.x - pb0.x).powi(2) + (pb1.y - pb0.y).powi(2)).sqrt();
        assert!((len_b - 10.0).abs() < 1e-4, "len_b={len_b}");
    }

    #[test]
    fn test_symmetric() {
        let mut actor = SketchActor::new();
        // Mirror line: vertical at x=5
        let m0 = actor.add_point_fixed(5.0, 0.0);
        let m1 = actor.add_point_fixed(5.0, 10.0);
        let p = actor.add_point_fixed(2.0, 3.0);
        let q = actor.add_point(4.0, 3.0);
        actor.add_constraint(Constraint::Symmetric(p, q, m0, m1));
        actor.solve(200);
        let pq = actor.point(q).unwrap();
        assert!((pq.x - 8.0).abs() < 1e-4, "q.x={}", pq.x);
        assert!((pq.y - 3.0).abs() < 1e-4, "q.y={}", pq.y);
    }

    #[test]
    fn test_move_point() {
        let mut actor = SketchActor::new();
        let p1 = actor.add_point(0.0, 0.0);
        actor.move_point(p1, 5.0, 7.0);
        let pt = actor.point(p1).unwrap();
        assert!((pt.x - 5.0).abs() < 1e-9);
        assert!((pt.y - 7.0).abs() < 1e-9);
        // Moving nonexistent point should not panic
        actor.move_point(999, 1.0, 1.0);
    }

    #[test]
    fn test_apply_angle() {
        let mut actor = SketchActor::new();
        let p1 = actor.add_point_fixed(0.0, 0.0);
        let p2 = actor.add_point(10.0, 0.0);
        // Constrain angle to 90 degrees (pi/2)
        actor.add_constraint(Constraint::Angle(p1, p2, std::f64::consts::FRAC_PI_2));
        let result = actor.solve(500);
        assert_eq!(result.status, SolveStatus::Converged);
        let b = actor.point(p2).unwrap();
        // p2 should be roughly above p1 (angle = pi/2 means pointing up)
        assert!(b.x.abs() < 0.5, "p2.x should be near 0, got {}", b.x);
        assert!(b.y > 5.0, "p2.y should be positive, got {}", b.y);
    }

    #[test]
    fn test_constraint_error_and_max() {
        let mut actor = SketchActor::new();
        let p1 = actor.add_point(0.0, 0.0);
        let p2 = actor.add_point(3.0, 4.0);
        // Distance is 5.0, constrain to 10.0
        actor.add_constraint(Constraint::Distance(p1, p2, 10.0));
        // Before solving, error should be |5 - 10| = 5
        let max_err = actor.max_constraint_error();
        assert!((max_err - 5.0).abs() < 1e-6, "max_err={max_err}");
    }

    #[test]
    fn test_rectangle_fully_constrained() {
        // A rectangle: 4 points, fixed origin, width=20, height=10
        let mut actor = SketchActor::new();
        let p0 = actor.add_point_fixed(0.0, 0.0);  // bottom-left
        let p1 = actor.add_point(20.0, 1.0);        // bottom-right
        let p2 = actor.add_point(21.0, 11.0);       // top-right
        let p3 = actor.add_point(1.0, 9.0);         // top-left

        // Bottom edge: horizontal, length 20
        actor.add_constraint(Constraint::Horizontal(p0, p1));
        actor.add_constraint(Constraint::Distance(p0, p1, 20.0));
        // Right edge: vertical, length 10
        actor.add_constraint(Constraint::Vertical(p1, p2));
        actor.add_constraint(Constraint::Distance(p1, p2, 10.0));
        // Top edge: horizontal
        actor.add_constraint(Constraint::Horizontal(p2, p3));
        // Left edge: vertical
        actor.add_constraint(Constraint::Vertical(p3, p0));

        // 3 free points × 2 = 6 DOF, 6 equations → fully constrained
        assert_eq!(actor.dof(), 0);

        let result = actor.solve(500);
        assert_eq!(result.status, SolveStatus::Converged);

        let r0 = actor.point(p0).unwrap();
        let r1 = actor.point(p1).unwrap();
        let r2 = actor.point(p2).unwrap();
        let r3 = actor.point(p3).unwrap();
        assert!((r1.x - 20.0).abs() < 1e-3, "p1.x={}", r1.x);
        assert!(r1.y.abs() < 1e-3, "p1.y={}", r1.y);
        assert!((r2.x - 20.0).abs() < 1e-3, "p2.x={}", r2.x);
        assert!((r2.y - 10.0).abs() < 1e-3, "p2.y={}", r2.y);
        assert!(r3.x.abs() < 1e-3, "p3.x={}", r3.x);
        assert!((r3.y - 10.0).abs() < 1e-3, "p3.y={}", r3.y);
        let _ = r0; // used as fixed anchor
    }
}
