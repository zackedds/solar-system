use macroquad::prelude::*;
use std::collections::VecDeque;

// ============================================================
// CONSTANTS
// ============================================================
const G: f64 = 0.5;
const SOFTENING_SQ: f64 = 100.0;
const TRAIL_MAX: usize = 200;
const ORBIT_PREDICTION_STEPS: usize = 200;
const MAX_BODIES: usize = 500;

// ============================================================
// BODY
// ============================================================
#[derive(Clone, Copy, PartialEq, Eq)]
enum BodyType {
    Star,
    Heavy,
    Planet,
    Asteroid,
}

#[derive(Clone)]
struct Body {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    mass: f64,
    radius: f64,
    body_type: BodyType,
    fill: Color,
    glow: Color,
    trail: VecDeque<(f64, f64)>,
    alive: bool,
}

impl Body {
    fn new(x: f64, y: f64, vx: f64, vy: f64, mass: f64, radius: f64, body_type: BodyType) -> Self {
        let (fill, glow) = match body_type {
            BodyType::Star => (
                Color::new(1.0, 0.87, 0.47, 1.0),
                Color::new(1.0, 0.67, 0.13, 1.0),
            ),
            BodyType::Heavy | BodyType::Planet => {
                let palettes: [(Color, Color); 7] = [
                    (Color::new(0.53, 0.60, 0.67, 1.0), Color::new(0.33, 0.40, 0.47, 1.0)),
                    (Color::new(0.67, 0.53, 0.40, 1.0), Color::new(0.53, 0.40, 0.27, 1.0)),
                    (Color::new(0.47, 0.67, 0.53, 1.0), Color::new(0.33, 0.53, 0.40, 1.0)),
                    (Color::new(0.67, 0.47, 0.47, 1.0), Color::new(0.53, 0.27, 0.27, 1.0)),
                    (Color::new(0.60, 0.53, 0.67, 1.0), Color::new(0.40, 0.33, 0.47, 1.0)),
                    (Color::new(0.40, 0.60, 0.67, 1.0), Color::new(0.27, 0.47, 0.53, 1.0)),
                    (Color::new(0.73, 0.67, 0.53, 1.0), Color::new(0.60, 0.53, 0.40, 1.0)),
                ];
                let idx = rand::gen_range(0, palettes.len());
                palettes[idx]
            }
            BodyType::Asteroid => (
                Color::new(0.47, 0.47, 0.47, 1.0),
                Color::new(0.33, 0.33, 0.33, 1.0),
            ),
        };

        Body {
            x, y, vx, vy, mass, radius, body_type, fill, glow,
            trail: VecDeque::with_capacity(TRAIL_MAX + 1),
            alive: true,
        }
    }
}

// ============================================================
// PLACEMENT MODE
// ============================================================
#[derive(Clone, Copy, PartialEq, Eq)]
enum PlaceMode {
    Planet,
    Heavy,
    Star,
    Asteroids,
    SolarSystem,
}

impl PlaceMode {
    fn name(&self) -> &str {
        match self {
            PlaceMode::Planet => "PLANET",
            PlaceMode::Heavy => "GIANT",
            PlaceMode::Star => "STAR",
            PlaceMode::Asteroids => "ASTEROIDS",
            PlaceMode::SolarSystem => "SOLAR SYSTEM",
        }
    }

    fn preview_radius(&self) -> f64 {
        match self {
            PlaceMode::Star => 18.0,
            PlaceMode::Heavy => 9.0,
            PlaceMode::Asteroids => 2.0,
            PlaceMode::Planet => 5.0,
            PlaceMode::SolarSystem => 22.0,
        }
    }
}

// ============================================================
// UI BUTTONS
// ============================================================
#[derive(Clone, Copy, PartialEq, Eq)]
enum BtnAction {
    SetPlanet, SetHeavy, SetStar, SetAsteroids, SetSolarSystem,
    ToggleTrails, ToggleOrbits, ToggleGrid, TogglePause,
    Recenter, Clear, Explode, NewSystem,
    TimeSlower, TimeFaster,
}

struct BtnRect {
    x: f32, y: f32, w: f32, h: f32,
    action: BtnAction,
    label: &'static str,
    active: bool,
}

// ============================================================
// CAMERA
// ============================================================
struct Camera {
    x: f64,
    y: f64,
    target_x: f64,
    target_y: f64,
    zoom: f64,
    zoom_target: f64,
    manual_offset: bool,
    offset_x: f64,
    offset_y: f64,
}

impl Camera {
    fn new(x: f64, y: f64) -> Self {
        Camera {
            x, y, target_x: x, target_y: y,
            zoom: 1.0, zoom_target: 1.0,
            manual_offset: false, offset_x: 0.0, offset_y: 0.0,
        }
    }

    fn screen_to_world(&self, sx: f64, sy: f64, sw: f64, sh: f64) -> (f64, f64) {
        (
            (sx - sw / 2.0) / self.zoom + self.x,
            (sy - sh / 2.0) / self.zoom + self.y,
        )
    }

    fn update(&mut self) {
        self.x += (self.target_x - self.x) * 0.05;
        self.y += (self.target_y - self.y) * 0.05;
        self.zoom += (self.zoom_target - self.zoom) * 0.1;
    }

    fn recenter(&mut self) {
        self.manual_offset = false;
        self.offset_x = 0.0;
        self.offset_y = 0.0;
        self.zoom_target = 1.0;
    }
}

// ============================================================
// FLASH MESSAGE
// ============================================================
struct FlashMsg {
    text: String,
    timer: f64,
}

impl FlashMsg {
    fn new() -> Self {
        FlashMsg { text: String::new(), timer: 0.0 }
    }

    fn show(&mut self, text: &str) {
        self.text = text.to_string();
        self.timer = 0.6;
    }

    fn update(&mut self, dt: f64) {
        if self.timer > 0.0 { self.timer -= dt; }
    }

    fn draw(&self) {
        if self.timer <= 0.0 { return; }
        let alpha = (self.timer / 0.6) as f32;
        let sw = screen_width();
        let sh = screen_height();
        let size = 40.0;
        let dims = measure_text(&self.text, None, size as u16, 1.0);
        draw_text(&self.text, sw / 2.0 - dims.width / 2.0, sh / 2.0, size, Color::new(1.0, 1.0, 1.0, alpha));
    }
}

// ============================================================
// STARFIELD
// ============================================================
struct BgStar { x: f32, y: f32, size: f32, alpha: f32 }

fn make_starfield(w: f32, h: f32) -> Vec<BgStar> {
    (0..400)
        .map(|_| BgStar {
            x: rand::gen_range(0.0, w),
            y: rand::gen_range(0.0, h),
            size: if rand::gen_range(0.0f32, 1.0) < 0.1 { 1.2 } else { 0.6 },
            alpha: 0.1 + rand::gen_range(0.0f32, 0.4),
        })
        .collect()
}

fn draw_starfield(stars: &[BgStar]) {
    for s in stars {
        draw_rectangle(s.x, s.y, s.size, s.size, Color::new(0.67, 0.73, 0.8, s.alpha));
    }
}

// ============================================================
// SOLAR SYSTEM SPAWN
// ============================================================
fn spawn_system(bodies: &mut Vec<Body>, cx: f64, cy: f64, base_vx: f64, base_vy: f64) {
    let star_mass = 1200.0;
    bodies.push(Body::new(cx, cy, base_vx, base_vy, star_mass, 22.0, BodyType::Star));

    let orbits = [90.0, 140.0, 200.0, 275.0, 360.0];
    for &r in &orbits {
        let angle = rand::gen_range(0.0, std::f64::consts::TAU);
        let speed = (G * star_mass / r).sqrt();
        let btype = if r > 300.0 { BodyType::Heavy } else { BodyType::Planet };
        let (mass, rad) = if btype == BodyType::Heavy {
            (120.0 + rand::gen_range(0.0, 80.0), 8.0 + rand::gen_range(0.0, 3.0))
        } else {
            (8.0 + rand::gen_range(0.0, 15.0), 3.0 + rand::gen_range(0.0, 3.0))
        };
        bodies.push(Body::new(
            cx + angle.cos() * r, cy + angle.sin() * r,
            -angle.sin() * speed + base_vx, angle.cos() * speed + base_vy,
            mass, rad, btype,
        ));
    }

    for _ in 0..20 {
        let r = 410.0 + rand::gen_range(0.0, 60.0);
        let angle = rand::gen_range(0.0, std::f64::consts::TAU);
        let speed = (G * star_mass / r).sqrt() * (0.93 + rand::gen_range(0.0, 0.14));
        bodies.push(Body::new(
            cx + angle.cos() * r, cy + angle.sin() * r,
            -angle.sin() * speed + base_vx, angle.cos() * speed + base_vy,
            0.5 + rand::gen_range(0.0, 1.5), 1.0 + rand::gen_range(0.0, 1.0),
            BodyType::Asteroid,
        ));
    }
}

fn place_body(bodies: &mut Vec<Body>, x: f64, y: f64, vx: f64, vy: f64, mode: PlaceMode) {
    match mode {
        PlaceMode::Star => {
            bodies.push(Body::new(x, y, vx, vy, 600.0 + rand::gen_range(0.0, 400.0), 16.0 + rand::gen_range(0.0, 6.0), BodyType::Star));
        }
        PlaceMode::Heavy => {
            bodies.push(Body::new(x, y, vx, vy, 100.0 + rand::gen_range(0.0, 100.0), 7.0 + rand::gen_range(0.0, 4.0), BodyType::Heavy));
        }
        PlaceMode::Asteroids => {
            for _ in 0..15 {
                bodies.push(Body::new(
                    x + rand::gen_range(-25.0, 25.0), y + rand::gen_range(-25.0, 25.0),
                    vx + rand::gen_range(-0.2, 0.2), vy + rand::gen_range(-0.2, 0.2),
                    0.5 + rand::gen_range(0.0, 1.0), 1.0 + rand::gen_range(0.0, 1.0),
                    BodyType::Asteroid,
                ));
            }
        }
        PlaceMode::Planet => {
            bodies.push(Body::new(x, y, vx, vy, 8.0 + rand::gen_range(0.0, 15.0), 3.0 + rand::gen_range(0.0, 3.0), BodyType::Planet));
        }
        PlaceMode::SolarSystem => {
            spawn_system(bodies, x, y, vx, vy);
        }
    }
}

// ============================================================
// PHYSICS — single-threaded (wasm compatible)
// ============================================================
fn compute_accelerations(bodies: &[Body]) -> Vec<(f64, f64)> {
    let len = bodies.len();
    let mut accs = vec![(0.0f64, 0.0f64); len];
    for i in 0..len {
        if !bodies[i].alive { continue; }
        let xi = bodies[i].x;
        let yi = bodies[i].y;
        for j in (i + 1)..len {
            if !bodies[j].alive { continue; }
            let dx = bodies[j].x - xi;
            let dy = bodies[j].y - yi;
            let dist_sq = dx * dx + dy * dy + SOFTENING_SQ;
            let inv_dist = 1.0 / dist_sq.sqrt();
            let inv_dist3 = inv_dist * inv_dist * inv_dist;
            // Acceleration on i from j
            let ai = G * bodies[j].mass * inv_dist3;
            accs[i].0 += ai * dx;
            accs[i].1 += ai * dy;
            // Acceleration on j from i (Newton's 3rd law — half the work)
            let aj = G * bodies[i].mass * inv_dist3;
            accs[j].0 -= aj * dx;
            accs[j].1 -= aj * dy;
        }
    }
    accs
}

fn step_physics(bodies: &mut Vec<Body>, dt: f64) {
    let accs = compute_accelerations(bodies);

    for (i, &(ax, ay)) in accs.iter().enumerate() {
        if !bodies[i].alive { continue; }
        bodies[i].vx += ax * dt;
        bodies[i].vy += ay * dt;
        bodies[i].x += bodies[i].vx * dt;
        bodies[i].y += bodies[i].vy * dt;

        let pos = (bodies[i].x, bodies[i].y);
        bodies[i].trail.push_back(pos);
        if bodies[i].trail.len() > TRAIL_MAX {
            bodies[i].trail.pop_front();
        }
    }

    // Realistic collisions: merge, fragment, or shatter depending on impact energy
    let len = bodies.len();
    let mut new_bodies: Vec<Body> = Vec::new();
    for i in 0..len {
        if !bodies[i].alive { continue; }
        for j in (i + 1)..len {
            if !bodies[j].alive { continue; }
            let dx = bodies[j].x - bodies[i].x;
            let dy = bodies[j].y - bodies[i].y;
            let dist_sq = dx * dx + dy * dy;
            let touch_r = (bodies[i].radius + bodies[j].radius) * 0.75;
            if dist_sq < touch_r * touch_r {
                let result = compute_collision_outcome(
                    &bodies[i], &bodies[j], len + new_bodies.len(),
                );
                bodies[i].alive = false;
                bodies[j].alive = false;
                new_bodies.extend(result);
            }
        }
    }
    bodies.retain(|b| b.alive);
    bodies.extend(new_bodies);
}

// ============================================================
// COLLISION OUTCOMES
// ============================================================
fn mass_to_radius(mass: f64) -> f64 {
    mass.max(0.3).powf(0.36) * 1.8
}

fn mass_to_type(mass: f64) -> BodyType {
    if mass > 400.0 { BodyType::Star }
    else if mass > 80.0 { BodyType::Heavy }
    else if mass > 5.0 { BodyType::Planet }
    else { BodyType::Asteroid }
}

fn make_body(mass: f64, x: f64, y: f64, vx: f64, vy: f64) -> Body {
    let btype = mass_to_type(mass);
    let radius = mass_to_radius(mass);
    Body::new(x, y, vx, vy, mass, radius, btype)
}

/// Adjust velocities so total momentum exactly equals target
fn correct_momentum(frags: &mut [Body], target_px: f64, target_py: f64) {
    let total_m: f64 = frags.iter().map(|b| b.mass).sum();
    if total_m < 0.001 { return; }
    let actual_px: f64 = frags.iter().map(|b| b.vx * b.mass).sum();
    let actual_py: f64 = frags.iter().map(|b| b.vy * b.mass).sum();
    let dvx = (target_px - actual_px) / total_m;
    let dvy = (target_py - actual_py) / total_m;
    for b in frags.iter_mut() {
        b.vx += dvx;
        b.vy += dvy;
    }
}

fn compute_collision_outcome(a: &Body, b: &Body, body_count: usize) -> Vec<Body> {
    let total_mass = a.mass + b.mass;
    let reduced_mass = a.mass * b.mass / total_mass;

    // Center-of-mass position & velocity (conserved)
    let com_x = (a.x * a.mass + b.x * b.mass) / total_mass;
    let com_y = (a.y * a.mass + b.y * b.mass) / total_mass;
    let com_vx = (a.vx * a.mass + b.vx * b.mass) / total_mass;
    let com_vy = (a.vy * a.mass + b.vy * b.mass) / total_mass;

    // Relative velocity → kinetic energy in COM frame
    let rel_vx = a.vx - b.vx;
    let rel_vy = a.vy - b.vy;
    let rel_speed = (rel_vx * rel_vx + rel_vy * rel_vy).sqrt();
    let ke = 0.5 * reduced_mass * rel_speed * rel_speed;

    // Gravitational binding energy
    let sep = (a.radius + b.radius).max(1.0);
    let binding = G * a.mass * b.mass / sep;
    let eta = ke / binding.max(0.001);

    let collision_r = a.radius + b.radius;
    let target_px = com_vx * total_mass;
    let target_py = com_vy * total_mass;

    // --- Force simple merge when body count is high or masses are tiny ---
    if body_count > MAX_BODIES || total_mass < 6.0 || eta < 0.3 {
        return vec![make_body(total_mass, com_x, com_y, com_vx, com_vy)];
    }

    // --- Star absorption: star survives, sprays minor debris ---
    if (a.body_type == BodyType::Star || b.body_type == BodyType::Star)
        && (eta < 2.0 || a.mass.min(b.mass) < a.mass.max(b.mass) * 0.3)
    {
        let star_frac = 0.92 - (eta * 0.04).min(0.15);
        let star_mass = total_mass * star_frac;
        let debris_mass = total_mass - star_mass;
        let n_debris = (3.0 + eta * 2.0).min(8.0) as usize;
        let per = debris_mass / n_debris as f64;

        let mut result = vec![make_body(star_mass, com_x, com_y, com_vx, com_vy)];
        for _ in 0..n_debris {
            let angle = rand::gen_range(0.0, std::f64::consts::TAU);
            let speed = rel_speed * rand::gen_range(0.2, 0.7);
            let dist = collision_r * rand::gen_range(1.0, 2.0);
            result.push(make_body(
                per.max(0.3),
                com_x + angle.cos() * dist,
                com_y + angle.sin() * dist,
                com_vx + angle.cos() * speed,
                com_vy + angle.sin() * speed,
            ));
        }
        correct_momentum(&mut result, target_px, target_py);
        return result;
    }

    // --- Fragmentation: severity scales with energy ratio η ---
    //
    //  η < 1.0  →  Partial fragmentation   (largest ~55-70%)
    //  η < 3.0  →  Catastrophic disruption  (largest ~30-40%)
    //  η ≥ 3.0  →  Super-catastrophic       (largest ~8-20%)

    let largest_frac = if eta < 1.0 {
        0.70 - 0.15 * eta           // 70% → 55%
    } else if eta < 3.0 {
        0.55 - 0.125 * (eta - 1.0)  // 55% → 30%
    } else {
        (0.20 - 0.02 * (eta - 3.0)).max(0.08) // 20% → min 8%
    };

    let n_medium = ((1.0 + eta * 1.2).min(5.0)) as usize;
    let n_small  = ((4.0 + eta * 4.0).min(22.0)) as usize;
    let eject_speed = rel_speed * 0.25 * eta.sqrt().min(2.5);

    let mut result: Vec<Body> = Vec::new();
    let mut remaining = total_mass;

    // Largest fragment — stays near collision center
    let m_largest = total_mass * largest_frac;
    remaining -= m_largest;
    let a0 = rand::gen_range(0.0, std::f64::consts::TAU);
    result.push(make_body(
        m_largest,
        com_x + a0.cos() * collision_r * 0.2,
        com_y + a0.sin() * collision_r * 0.2,
        com_vx + a0.cos() * eject_speed * 0.3,
        com_vy + a0.sin() * eject_speed * 0.3,
    ));

    // Medium chunks — ejected outward
    for _ in 0..n_medium {
        if remaining < 2.0 { break; }
        let frac = rand::gen_range(0.06, 0.14);
        let m = (total_mass * frac).min(remaining * 0.6);
        remaining -= m;
        let angle = rand::gen_range(0.0, std::f64::consts::TAU);
        let dist = collision_r * rand::gen_range(0.8, 2.0);
        let speed = eject_speed * rand::gen_range(0.6, 1.4);
        result.push(make_body(
            m,
            com_x + angle.cos() * dist,
            com_y + angle.sin() * dist,
            com_vx + angle.cos() * speed,
            com_vy + angle.sin() * speed,
        ));
    }

    // Small debris — sprayed in all directions
    if n_small > 0 && remaining > 0.5 {
        let per = remaining / n_small as f64;
        for _ in 0..n_small {
            let m = (per * rand::gen_range(0.3, 1.7)).max(0.3);
            let angle = rand::gen_range(0.0, std::f64::consts::TAU);
            let dist = collision_r * rand::gen_range(0.5, 3.0);
            let speed = eject_speed * rand::gen_range(0.5, 2.2);
            result.push(make_body(
                m,
                com_x + angle.cos() * dist,
                com_y + angle.sin() * dist,
                com_vx + angle.cos() * speed,
                com_vy + angle.sin() * speed,
            ));
        }
    }

    // Enforce momentum conservation
    correct_momentum(&mut result, target_px, target_py);
    result
}

// ============================================================
// CENTER OF MASS
// ============================================================
fn center_of_mass(bodies: &[Body]) -> (f64, f64) {
    if bodies.is_empty() { return (0.0, 0.0); }
    let (mut tx, mut ty, mut tm) = (0.0, 0.0, 0.0);
    for b in bodies {
        tx += b.x * b.mass;
        ty += b.y * b.mass;
        tm += b.mass;
    }
    (tx / tm, ty / tm)
}

// ============================================================
// PROJECTED ORBITS
// ============================================================
fn compute_orbit_predictions(bodies: &[Body]) -> Vec<Vec<(f64, f64)>> {
    let len = bodies.len();
    if len < 2 { return vec![]; }

    let mut sx: Vec<f64> = bodies.iter().map(|b| b.x).collect();
    let mut sy: Vec<f64> = bodies.iter().map(|b| b.y).collect();
    let mut svx: Vec<f64> = bodies.iter().map(|b| b.vx).collect();
    let mut svy: Vec<f64> = bodies.iter().map(|b| b.vy).collect();
    let sm: Vec<f64> = bodies.iter().map(|b| b.mass).collect();

    let mut paths: Vec<Vec<(f64, f64)>> = (0..len)
        .map(|i| vec![(sx[i], sy[i])])
        .collect();

    for step in 0..ORBIT_PREDICTION_STEPS {
        // Symmetric force computation (half the work)
        let mut ax = vec![0.0f64; len];
        let mut ay = vec![0.0f64; len];
        for i in 0..len {
            for j in (i + 1)..len {
                let dx = sx[j] - sx[i];
                let dy = sy[j] - sy[i];
                let dist_sq = dx * dx + dy * dy + SOFTENING_SQ;
                let inv_dist = 1.0 / dist_sq.sqrt();
                let inv_dist3 = inv_dist * inv_dist * inv_dist;
                let fi = G * sm[j] * inv_dist3;
                let fj = G * sm[i] * inv_dist3;
                ax[i] += fi * dx;
                ay[i] += fi * dy;
                ax[j] -= fj * dx;
                ay[j] -= fj * dy;
            }
        }
        for i in 0..len {
            svx[i] += ax[i];
            svy[i] += ay[i];
            sx[i] += svx[i];
            sy[i] += svy[i];
            if step % 2 == 0 {
                paths[i].push((sx[i], sy[i]));
            }
        }
    }
    paths
}

// ============================================================
// DRAWING HELPERS
// ============================================================
#[inline(always)]
fn world_to_screen(wx: f64, wy: f64, cam: &Camera, sw: f64, sh: f64) -> (f32, f32) {
    (
        ((wx - cam.x) * cam.zoom + sw / 2.0) as f32,
        ((wy - cam.y) * cam.zoom + sh / 2.0) as f32,
    )
}

fn draw_body_at(b: &Body, cam: &Camera, sw: f64, sh: f64) {
    let (sx, sy) = world_to_screen(b.x, b.y, cam, sw, sh);
    let r = (b.radius * cam.zoom) as f32;
    let margin = r * 3.5;
    if sx < -margin || sx > sw as f32 + margin || sy < -margin || sy > sh as f32 + margin {
        return;
    }

    match b.body_type {
        BodyType::Star => {
            draw_circle(sx, sy, r * 3.0, Color::new(1.0, 0.86, 0.47, 0.12));
            draw_circle(sx, sy, r * 2.0, Color::new(1.0, 0.75, 0.30, 0.08));
        }
        BodyType::Heavy => {
            draw_circle(sx, sy, r * 2.0, Color::new(b.glow.r, b.glow.g, b.glow.b, 0.06));
        }
        _ => {}
    }

    if b.body_type == BodyType::Star {
        draw_circle(sx, sy, r, Color::new(1.0, 0.95, 0.85, 1.0));
        draw_circle(sx, sy, r * 0.7, Color::new(1.0, 0.90, 0.55, 1.0));
    } else {
        draw_circle(sx, sy, r.max(1.0), b.fill);
    }

    if r > 3.0 {
        draw_circle(sx - r * 0.25, sy - r * 0.25, r * 0.2, Color::new(1.0, 1.0, 1.0, 0.15));
    }
}

fn draw_trail(b: &Body, cam: &Camera, sw: f64, sh: f64) {
    let tlen = b.trail.len();
    if tlen < 2 { return; }
    let max_alpha = if b.body_type == BodyType::Asteroid { 0.08 } else { 0.3 };
    let thickness = (b.radius * cam.zoom * 0.3).max(0.5) as f32;
    let swf = sw as f32;
    let shf = sh as f32;

    for i in 1..tlen {
        let t = i as f32 / tlen as f32;
        let (x1, y1) = world_to_screen(b.trail[i - 1].0, b.trail[i - 1].1, cam, sw, sh);
        let (x2, y2) = world_to_screen(b.trail[i].0, b.trail[i].1, cam, sw, sh);
        if (x1 < 0.0 && x2 < 0.0) || (x1 > swf && x2 > swf)
            || (y1 < 0.0 && y2 < 0.0) || (y1 > shf && y2 > shf)
        { continue; }
        draw_line(x1, y1, x2, y2, thickness, Color::new(b.glow.r, b.glow.g, b.glow.b, t * max_alpha));
    }
}

fn draw_orbit_predictions(bodies: &[Body], paths: &[Vec<(f64, f64)>], cam: &Camera, sw: f64, sh: f64) {
    let swf = sw as f32;
    let shf = sh as f32;
    for (i, path) in paths.iter().enumerate() {
        if i >= bodies.len() || path.len() < 2 { continue; }
        let b = &bodies[i];
        let alpha = if b.body_type == BodyType::Asteroid { 0.04 } else { 0.12 };
        for j in 1..path.len() {
            if j % 3 == 0 { continue; }
            let (x1, y1) = world_to_screen(path[j - 1].0, path[j - 1].1, cam, sw, sh);
            let (x2, y2) = world_to_screen(path[j].0, path[j].1, cam, sw, sh);
            if (x1 < 0.0 && x2 < 0.0) || (x1 > swf && x2 > swf)
                || (y1 < 0.0 && y2 < 0.0) || (y1 > shf && y2 > shf)
            { continue; }
            draw_line(x1, y1, x2, y2, 1.0, Color::new(b.glow.r, b.glow.g, b.glow.b, alpha));
        }
    }
}

fn draw_drag_preview(
    start: (f64, f64), mouse: (f64, f64),
    mode: PlaceMode, bodies: &[Body],
    cam: &Camera, sw: f64, sh: f64,
) {
    let (sx, sy) = world_to_screen(start.0, start.1, cam, sw, sh);
    let r = (mode.preview_radius() * cam.zoom) as f32;
    draw_circle_lines(sx, sy, r, 1.0, Color::new(1.0, 1.0, 1.0, 0.3));

    let dx = start.0 - mouse.0;
    let dy = start.1 - mouse.1;
    let len = (dx * dx + dy * dy).sqrt();

    if len > 5.0 / cam.zoom {
        let (ex, ey) = world_to_screen(start.0 + dx, start.1 + dy, cam, sw, sh);
        draw_line(sx, sy, ex, ey, 1.0, Color::new(1.0, 1.0, 1.0, 0.15));

        let mut px = start.0;
        let mut py = start.1;
        let mut pvx = dx * 0.05;
        let mut pvy = dy * 0.05;
        let mut prev = world_to_screen(px, py, cam, sw, sh);

        for step in 0..400 {
            for b in bodies {
                let bx = b.x - px;
                let by = b.y - py;
                let ds = bx * bx + by * by + SOFTENING_SQ;
                let inv_d = 1.0 / ds.sqrt();
                let accel = G * b.mass * inv_d * inv_d * inv_d;
                pvx += accel * bx;
                pvy += accel * by;
            }
            px += pvx;
            py += pvy;
            let cur = world_to_screen(px, py, cam, sw, sh);
            if step % 3 != 0 {
                draw_line(prev.0, prev.1, cur.0, cur.1, 1.0, Color::new(0.3, 0.5, 0.9, 0.08));
            }
            prev = cur;
        }
    }
}

fn draw_grid(cam: &Camera, sw: f64, sh: f64) {
    let sp = 80.0;
    let start_x = ((cam.x - sw / 2.0 / cam.zoom) / sp).floor() as i32;
    let end_x = ((cam.x + sw / 2.0 / cam.zoom) / sp).ceil() as i32;
    let start_y = ((cam.y - sh / 2.0 / cam.zoom) / sp).floor() as i32;
    let end_y = ((cam.y + sh / 2.0 / cam.zoom) / sp).ceil() as i32;

    let color = Color::new(1.0, 1.0, 1.0, 0.02);
    for gx in start_x..=end_x {
        let (x1, y1) = world_to_screen(gx as f64 * sp, start_y as f64 * sp, cam, sw, sh);
        let (x2, y2) = world_to_screen(gx as f64 * sp, end_y as f64 * sp, cam, sw, sh);
        draw_line(x1, y1, x2, y2, 0.5, color);
    }
    for gy in start_y..=end_y {
        let (x1, y1) = world_to_screen(start_x as f64 * sp, gy as f64 * sp, cam, sw, sh);
        let (x2, y2) = world_to_screen(end_x as f64 * sp, gy as f64 * sp, cam, sw, sh);
        draw_line(x1, y1, x2, y2, 0.5, color);
    }
}

// ============================================================
// UI BUTTON HELPERS
// ============================================================
fn layout_buttons(
    sw: f32, sh: f32, place_mode: PlaceMode,
    show_trails: bool, show_orbits: bool, show_grid: bool, paused: bool,
) -> Vec<BtnRect> {
    let btn_h = 36.0f32;
    let gap = 5.0f32;
    let margin = 10.0f32;
    let pad_x = 12.0f32;
    let font_size = 15u16;

    let rows: [&[(&str, BtnAction, bool)]; 3] = [
        &[
            ("PLANET", BtnAction::SetPlanet, place_mode == PlaceMode::Planet),
            ("GIANT", BtnAction::SetHeavy, place_mode == PlaceMode::Heavy),
            ("STAR", BtnAction::SetStar, place_mode == PlaceMode::Star),
            ("ASTEROIDS", BtnAction::SetAsteroids, place_mode == PlaceMode::Asteroids),
            ("SOLAR SYS", BtnAction::SetSolarSystem, place_mode == PlaceMode::SolarSystem),
        ],
        &[
            ("TRAILS", BtnAction::ToggleTrails, show_trails),
            ("ORBITS", BtnAction::ToggleOrbits, show_orbits),
            ("GRID", BtnAction::ToggleGrid, show_grid),
            ("PAUSE", BtnAction::TogglePause, paused),
            ("SLOWER", BtnAction::TimeSlower, false),
            ("FASTER", BtnAction::TimeFaster, false),
        ],
        &[
            ("CENTER", BtnAction::Recenter, false),
            ("CLEAR", BtnAction::Clear, false),
            ("EXPLODE", BtnAction::Explode, false),
            ("NEW SYS", BtnAction::NewSystem, false),
        ],
    ];

    let mut buttons = Vec::new();
    for (row_idx, items) in rows.iter().enumerate() {
        let y = sh - margin - ((rows.len() - row_idx) as f32) * (btn_h + gap);
        let mut x = margin;
        for &(label, action, active) in *items {
            let dims = measure_text(label, None, font_size, 1.0);
            let w = dims.width + pad_x * 2.0;
            if x + w > sw - margin && x > margin + 1.0 {
                x = margin;
            }
            buttons.push(BtnRect { x, y, w, h: btn_h, action, label, active });
            x += w + gap;
        }
    }
    buttons
}

fn draw_buttons(buttons: &[BtnRect]) {
    let font_size = 15.0f32;
    for btn in buttons {
        let bg_alpha = if btn.active { 0.22 } else { 0.08 };
        draw_rectangle(btn.x, btn.y, btn.w, btn.h, Color::new(1.0, 1.0, 1.0, bg_alpha));
        draw_rectangle_lines(btn.x, btn.y, btn.w, btn.h, 1.0, Color::new(1.0, 1.0, 1.0, 0.15));

        let dims = measure_text(btn.label, None, font_size as u16, 1.0);
        let tx = btn.x + (btn.w - dims.width) / 2.0;
        let ty = btn.y + (btn.h + dims.height) / 2.0 - 2.0;
        let text_alpha = if btn.active { 1.0 } else { 0.85 };
        draw_text(btn.label, tx, ty, font_size, Color::new(1.0, 1.0, 1.0, text_alpha));
    }
}

fn check_button_hit(buttons: &[BtnRect], mx: f32, my: f32) -> Option<BtnAction> {
    for btn in buttons {
        if mx >= btn.x && mx <= btn.x + btn.w && my >= btn.y && my <= btn.y + btn.h {
            return Some(btn.action);
        }
    }
    None
}

// ============================================================
// MAIN
// ============================================================
fn window_conf() -> Conf {
    Conf {
        window_title: "VOID — N-Body Orbital Simulator".to_string(),
        window_width: 1280,
        window_height: 800,
        window_resizable: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut bodies: Vec<Body> = Vec::new();
    let sw = screen_width() as f64;
    let sh = screen_height() as f64;
    let mut cam = Camera::new(sw / 2.0, sh / 2.0);
    let mut stars = make_starfield(screen_width(), screen_height());

    spawn_system(&mut bodies, sw / 2.0, sh / 2.0, 0.0, 0.0);

    let mut place_mode = PlaceMode::Planet;
    let mut paused = false;
    let mut time_scale: f64 = 1.0;
    let mut show_trails = true;
    let mut show_grid = false;
    let mut show_orbits = true;
    let mut flash = FlashMsg::new();

    let mut dragging = false;
    let mut drag_start = (0.0f64, 0.0f64);

    let mut orbit_paths: Vec<Vec<(f64, f64)>> = vec![];
    let mut orbit_body_count: usize = 0;
    let mut orbit_timer = 0.0;

    let mut prev_sw = screen_width();
    let mut prev_sh = screen_height();

    let mut fps_accum = 0.0;
    let mut fps_frames = 0u32;
    let mut fps_display = 0u32;

    loop {
        let raw_dt = get_frame_time() as f64;
        let dt_capped = raw_dt.min(0.033);
        let sw = screen_width() as f64;
        let sh = screen_height() as f64;

        fps_accum += raw_dt;
        fps_frames += 1;
        if fps_accum >= 0.5 {
            fps_display = (fps_frames as f64 / fps_accum) as u32;
            fps_accum = 0.0;
            fps_frames = 0;
        }

        if (screen_width() - prev_sw).abs() > 1.0 || (screen_height() - prev_sh).abs() > 1.0 {
            stars = make_starfield(screen_width(), screen_height());
            prev_sw = screen_width();
            prev_sh = screen_height();
        }

        flash.update(dt_capped);

        let buttons = layout_buttons(
            screen_width(), screen_height(), place_mode,
            show_trails, show_orbits, show_grid, paused,
        );

        // ---- INPUT ----
        if is_key_pressed(KeyCode::Key1) { place_mode = PlaceMode::Planet; flash.show("PLANET"); }
        if is_key_pressed(KeyCode::Key2) { place_mode = PlaceMode::Heavy; flash.show("GIANT"); }
        if is_key_pressed(KeyCode::Key3) { place_mode = PlaceMode::Star; flash.show("STAR"); }
        if is_key_pressed(KeyCode::Key4) { place_mode = PlaceMode::Asteroids; flash.show("ASTEROIDS"); }
        if is_key_pressed(KeyCode::Key5) { place_mode = PlaceMode::SolarSystem; flash.show("SOLAR SYSTEM"); }
        if is_key_pressed(KeyCode::T) { show_trails = !show_trails; flash.show(if show_trails { "TRAILS ON" } else { "TRAILS OFF" }); }
        if is_key_pressed(KeyCode::G) { show_grid = !show_grid; flash.show(if show_grid { "GRID ON" } else { "GRID OFF" }); }
        if is_key_pressed(KeyCode::O) { show_orbits = !show_orbits; flash.show(if show_orbits { "ORBITS ON" } else { "ORBITS OFF" }); }
        if is_key_pressed(KeyCode::F) { paused = !paused; flash.show(if paused { "PAUSED" } else { "RUNNING" }); }
        if is_key_pressed(KeyCode::H) { cam.recenter(); flash.show("CENTERED"); }
        if is_key_pressed(KeyCode::C) { bodies.clear(); flash.show("CLEARED"); }

        if is_key_pressed(KeyCode::LeftBracket) {
            time_scale = (time_scale / 1.5).max(0.1);
            flash.show(&format!("{:.1}X", time_scale));
        }
        if is_key_pressed(KeyCode::RightBracket) {
            time_scale = (time_scale * 1.5).min(10.0);
            flash.show(&format!("{:.1}X", time_scale));
        }

        if is_key_pressed(KeyCode::X) {
            let (cx, cy) = center_of_mass(&bodies);
            for b in &mut bodies {
                let dx = b.x - cx;
                let dy = b.y - cy;
                let d = (dx * dx + dy * dy).sqrt().max(1.0);
                b.vx += dx / d * 10.0;
                b.vy += dy / d * 10.0;
            }
            flash.show("BOOM");
        }

        if is_key_pressed(KeyCode::Space) {
            let (cx, cy) = if bodies.is_empty() { (cam.x, cam.y) } else { center_of_mass(&bodies) };
            spawn_system(&mut bodies, cx, cy, 0.0, 0.0);
            flash.show("NEW SYSTEM");
        }

        // Scroll: pinch (ctrl+wheel in browser) = zoom, plain scroll = pan
        let (wheel_x, wheel_y) = mouse_wheel();
        if wheel_x != 0.0 || wheel_y != 0.0 {
            if is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl)
                || is_key_down(KeyCode::LeftSuper) || is_key_down(KeyCode::RightSuper)
            {
                let zoom_delta = wheel_y * 0.01;
                cam.zoom_target = (cam.zoom_target * (1.0 + zoom_delta as f64)).clamp(0.15, 8.0);
            } else {
                // Mac natural scrolling: content follows finger direction
                cam.manual_offset = true;
                cam.offset_x -= wheel_x as f64 / cam.zoom;
                cam.offset_y -= wheel_y as f64 / cam.zoom;
            }
        }

        // +/- keys for zoom
        if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::KpAdd) {
            cam.zoom_target = (cam.zoom_target * 1.3).min(8.0);
        }
        if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::KpSubtract) {
            cam.zoom_target = (cam.zoom_target / 1.3).max(0.15);
        }

        // Mouse / touch: check buttons first, then drag-to-place
        let (mx, my) = mouse_position();
        if is_mouse_button_pressed(MouseButton::Left) {
            if let Some(action) = check_button_hit(&buttons, mx, my) {
                match action {
                    BtnAction::SetPlanet => { place_mode = PlaceMode::Planet; flash.show("PLANET"); }
                    BtnAction::SetHeavy => { place_mode = PlaceMode::Heavy; flash.show("GIANT"); }
                    BtnAction::SetStar => { place_mode = PlaceMode::Star; flash.show("STAR"); }
                    BtnAction::SetAsteroids => { place_mode = PlaceMode::Asteroids; flash.show("ASTEROIDS"); }
                    BtnAction::SetSolarSystem => { place_mode = PlaceMode::SolarSystem; flash.show("SOLAR SYSTEM"); }
                    BtnAction::ToggleTrails => { show_trails = !show_trails; flash.show(if show_trails { "TRAILS ON" } else { "TRAILS OFF" }); }
                    BtnAction::ToggleOrbits => { show_orbits = !show_orbits; flash.show(if show_orbits { "ORBITS ON" } else { "ORBITS OFF" }); }
                    BtnAction::ToggleGrid => { show_grid = !show_grid; flash.show(if show_grid { "GRID ON" } else { "GRID OFF" }); }
                    BtnAction::TogglePause => { paused = !paused; flash.show(if paused { "PAUSED" } else { "RUNNING" }); }
                    BtnAction::Recenter => { cam.recenter(); flash.show("CENTERED"); }
                    BtnAction::Clear => { bodies.clear(); flash.show("CLEARED"); }
                    BtnAction::Explode => {
                        let (cx, cy) = center_of_mass(&bodies);
                        for b in &mut bodies {
                            let bx = b.x - cx;
                            let by = b.y - cy;
                            let d = (bx * bx + by * by).sqrt().max(1.0);
                            b.vx += bx / d * 10.0;
                            b.vy += by / d * 10.0;
                        }
                        flash.show("BOOM");
                    }
                    BtnAction::NewSystem => {
                        let (cx, cy) = if bodies.is_empty() { (cam.x, cam.y) } else { center_of_mass(&bodies) };
                        spawn_system(&mut bodies, cx, cy, 0.0, 0.0);
                        flash.show("NEW SYSTEM");
                    }
                    BtnAction::TimeSlower => {
                        time_scale = (time_scale / 1.5).max(0.1);
                        flash.show(&format!("{:.1}X", time_scale));
                    }
                    BtnAction::TimeFaster => {
                        time_scale = (time_scale * 1.5).min(10.0);
                        flash.show(&format!("{:.1}X", time_scale));
                    }
                }
            } else {
                dragging = true;
                drag_start = cam.screen_to_world(mx as f64, my as f64, sw, sh);
            }
        }
        if is_mouse_button_released(MouseButton::Left) && dragging {
            dragging = false;
            let mouse_world = cam.screen_to_world(mx as f64, my as f64, sw, sh);
            let dx = drag_start.0 - mouse_world.0;
            let dy = drag_start.1 - mouse_world.1;
            place_body(&mut bodies, drag_start.0, drag_start.1, dx * 0.05, dy * 0.05, place_mode);
        }

        // ---- PHYSICS ----
        if !paused {
            let sim_dt = dt_capped * time_scale * 60.0;
            let steps = (sim_dt / 1.5).ceil().max(1.0).min(8.0) as usize;
            let sub_dt = sim_dt / steps as f64;
            for _ in 0..steps {
                step_physics(&mut bodies, sub_dt);
            }
        }

        // ---- CAMERA ----
        let (com_x, com_y) = center_of_mass(&bodies);
        if cam.manual_offset {
            cam.target_x = com_x + cam.offset_x;
            cam.target_y = com_y + cam.offset_y;
        } else {
            cam.target_x = com_x;
            cam.target_y = com_y;
        }
        cam.update();

        // ---- ORBIT PREDICTIONS ----
        orbit_timer -= dt_capped;
        if orbit_timer <= 0.0 && show_orbits && bodies.len() >= 2 {
            orbit_paths = compute_orbit_predictions(&bodies);
            orbit_body_count = bodies.len();
            orbit_timer = 0.2;
        }

        // ---- RENDER ----
        clear_background(Color::new(0.02, 0.02, 0.03, 1.0));
        draw_starfield(&stars);

        if show_grid { draw_grid(&cam, sw, sh); }

        if show_orbits && orbit_body_count == bodies.len() && !orbit_paths.is_empty() {
            draw_orbit_predictions(&bodies, &orbit_paths, &cam, sw, sh);
        }

        if show_trails {
            for b in &bodies { draw_trail(b, &cam, sw, sh); }
        }

        let mut sorted_indices: Vec<usize> = (0..bodies.len()).collect();
        sorted_indices.sort_by(|&a, &b| bodies[a].mass.partial_cmp(&bodies[b].mass).unwrap());
        for &i in &sorted_indices {
            draw_body_at(&bodies[i], &cam, sw, sh);
        }

        if dragging {
            let mouse_world = cam.screen_to_world(mx as f64, my as f64, sw, sh);
            draw_drag_preview(drag_start, mouse_world, place_mode, &bodies, &cam, sw, sh);
        }

        // ---- HUD ----
        let total_mass: f64 = bodies.iter().map(|b| b.mass).sum();
        draw_text("VOID", 16.0, 38.0, 32.0, Color::new(1.0, 1.0, 1.0, 1.0));
        draw_text("orbital simulator", 16.0, 58.0, 16.0, Color::new(0.8, 0.8, 0.85, 1.0));

        let stats = format!(
            "{} bodies  |  {} mass  |  {:.1}x  |  {}%  |  {} fps",
            bodies.len(), total_mass as i64, time_scale, (cam.zoom * 100.0) as i32, fps_display
        );
        draw_text(&stats, 16.0, 82.0, 18.0, Color::new(0.95, 0.95, 1.0, 1.0));

        draw_buttons(&buttons);

        flash.draw();
        next_frame().await;
    }
}
