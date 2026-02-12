use macroquad::prelude::*;
use macroquad::miniquad::window::set_mouse_cursor;
use macroquad::miniquad::CursorIcon;
use rayon::prelude::*;

// ============================================================
// CONSTANTS
// ============================================================
const G: f64 = 0.5;
const SOFTENING: f64 = 10.0;
const TRAIL_MAX: usize = 200;
const ORBIT_PREDICTION_STEPS: usize = 250;

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
    trail: Vec<(f64, f64)>,
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
            trail: Vec::with_capacity(TRAIL_MAX),
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
        if self.timer > 0.0 {
            self.timer -= dt;
        }
    }

    fn draw(&self) {
        if self.timer <= 0.0 { return; }
        let alpha = (self.timer / 0.6) as f32;
        let sw = screen_width();
        let sh = screen_height();
        let size = 40.0;
        let dims = measure_text(&self.text, None, size as u16, 1.0);
        draw_text(
            &self.text,
            sw / 2.0 - dims.width / 2.0,
            sh / 2.0,
            size,
            Color::new(1.0, 1.0, 1.0, alpha),
        );
    }
}

// ============================================================
// STARFIELD
// ============================================================
struct BgStar {
    x: f32,
    y: f32,
    size: f32,
    alpha: f32,
}

fn make_starfield(w: f32, h: f32) -> Vec<BgStar> {
    (0..500)
        .map(|_| BgStar {
            x: rand::gen_range(0.0, w),
            y: rand::gen_range(0.0, h),
            size: if rand::gen_range(0.0f32, 1.0) < 0.1 { 1.5 } else { 0.7 },
            alpha: 0.1 + rand::gen_range(0.0f32, 0.4),
        })
        .collect()
}

fn draw_starfield(stars: &[BgStar]) {
    for s in stars {
        draw_rectangle(s.x, s.y, s.size, s.size, Color::new(0.7, 0.75, 0.8, s.alpha));
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

    for _ in 0..25 {
        let r = 410.0 + rand::gen_range(0.0, 70.0);
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
// PHYSICS (parallelized gravity with rayon)
// ============================================================
fn compute_accelerations(bodies: &[Body]) -> Vec<(f64, f64)> {
    let len = bodies.len();
    (0..len)
        .into_par_iter()
        .map(|i| {
            let a = &bodies[i];
            if !a.alive { return (0.0, 0.0); }
            let mut ax = 0.0;
            let mut ay = 0.0;
            for j in 0..len {
                if i == j || !bodies[j].alive { continue; }
                let dx = bodies[j].x - a.x;
                let dy = bodies[j].y - a.y;
                let dist_sq = dx * dx + dy * dy + SOFTENING * SOFTENING;
                let dist = dist_sq.sqrt();
                let force = G * bodies[j].mass / dist_sq;
                ax += force * dx / dist;
                ay += force * dy / dist;
            }
            (ax, ay)
        })
        .collect()
}

fn step_physics(bodies: &mut Vec<Body>, dt: f64) {
    let accs = compute_accelerations(bodies);

    for (i, (ax, ay)) in accs.iter().enumerate() {
        if !bodies[i].alive { continue; }
        bodies[i].vx += ax * dt;
        bodies[i].vy += ay * dt;
        bodies[i].x += bodies[i].vx * dt;
        bodies[i].y += bodies[i].vy * dt;

        let pos = (bodies[i].x, bodies[i].y);
        bodies[i].trail.push(pos);
        if bodies[i].trail.len() > TRAIL_MAX {
            bodies[i].trail.remove(0);
        }
    }

    // Merge (sequential)
    let len = bodies.len();
    for i in 0..len {
        if !bodies[i].alive { continue; }
        for j in (i + 1)..len {
            if !bodies[j].alive { continue; }
            let dx = bodies[j].x - bodies[i].x;
            let dy = bodies[j].y - bodies[i].y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < (bodies[i].radius + bodies[j].radius) * 0.75 {
                let (big_idx, sm_idx) = if bodies[i].mass >= bodies[j].mass { (i, j) } else { (j, i) };
                let sm_mass = bodies[sm_idx].mass;
                let sm_vx = bodies[sm_idx].vx;
                let sm_vy = bodies[sm_idx].vy;
                let sm_x = bodies[sm_idx].x;
                let sm_y = bodies[sm_idx].y;
                let sm_r = bodies[sm_idx].radius;
                bodies[sm_idx].alive = false;

                let big = &mut bodies[big_idx];
                let total = big.mass + sm_mass;
                big.vx = (big.vx * big.mass + sm_vx * sm_mass) / total;
                big.vy = (big.vy * big.mass + sm_vy * sm_mass) / total;
                big.x = (big.x * big.mass + sm_x * sm_mass) / total;
                big.y = (big.y * big.mass + sm_y * sm_mass) / total;
                big.mass = total;
                big.radius = (big.radius.powi(3) + sm_r.powi(3)).cbrt();
                if big.mass > 400.0 && big.body_type != BodyType::Star {
                    big.body_type = BodyType::Star;
                    big.fill = Color::new(1.0, 0.87, 0.47, 1.0);
                    big.glow = Color::new(1.0, 0.67, 0.13, 1.0);
                } else if big.mass > 80.0 && big.body_type == BodyType::Planet {
                    big.body_type = BodyType::Heavy;
                }
            }
        }
    }

    bodies.retain(|b| b.alive);
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
    if bodies.len() < 2 { return vec![]; }

    let mut sim: Vec<(f64, f64, f64, f64, f64)> = bodies
        .iter()
        .map(|b| (b.x, b.y, b.vx, b.vy, b.mass))
        .collect();

    let mut paths: Vec<Vec<(f64, f64)>> = sim.iter().map(|s| vec![(s.0, s.1)]).collect();

    for step in 0..ORBIT_PREDICTION_STEPS {
        let len = sim.len();
        let accs: Vec<(f64, f64)> = (0..len)
            .map(|i| {
                let mut ax = 0.0;
                let mut ay = 0.0;
                for j in 0..len {
                    if i == j { continue; }
                    let dx = sim[j].0 - sim[i].0;
                    let dy = sim[j].1 - sim[i].1;
                    let dist_sq = dx * dx + dy * dy + SOFTENING * SOFTENING;
                    let dist = dist_sq.sqrt();
                    let force = G * sim[j].4 / dist_sq;
                    ax += force * dx / dist;
                    ay += force * dy / dist;
                }
                (ax, ay)
            })
            .collect();

        for i in 0..len {
            sim[i].2 += accs[i].0;
            sim[i].3 += accs[i].1;
            sim[i].0 += sim[i].2;
            sim[i].1 += sim[i].3;
            if step % 2 == 0 {
                paths[i].push((sim[i].0, sim[i].1));
            }
        }
    }

    paths
}

// ============================================================
// DRAWING HELPERS
// ============================================================
fn world_to_screen(wx: f64, wy: f64, cam: &Camera, sw: f64, sh: f64) -> (f32, f32) {
    (
        ((wx - cam.x) * cam.zoom + sw / 2.0) as f32,
        ((wy - cam.y) * cam.zoom + sh / 2.0) as f32,
    )
}

fn draw_body_at(b: &Body, cam: &Camera, sw: f64, sh: f64) {
    let (sx, sy) = world_to_screen(b.x, b.y, cam, sw, sh);
    let r = (b.radius * cam.zoom) as f32;

    if sx < -100.0 || sx > sw as f32 + 100.0 || sy < -100.0 || sy > sh as f32 + 100.0 {
        return;
    }

    // Glow
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

    // Body
    if b.body_type == BodyType::Star {
        draw_circle(sx, sy, r, Color::new(1.0, 0.95, 0.85, 1.0));
        draw_circle(sx, sy, r * 0.7, Color::new(1.0, 0.90, 0.55, 1.0));
    } else {
        draw_circle(sx, sy, r.max(1.0), b.fill);
    }

    // Specular
    if r > 3.0 {
        draw_circle(sx - r * 0.25, sy - r * 0.25, r * 0.2, Color::new(1.0, 1.0, 1.0, 0.15));
    }
}

fn draw_trail(b: &Body, cam: &Camera, sw: f64, sh: f64) {
    if b.trail.len() < 2 { return; }
    let max_alpha = if b.body_type == BodyType::Asteroid { 0.08 } else { 0.3 };
    let thickness = (b.radius * cam.zoom * 0.3).max(0.5) as f32;

    for i in 1..b.trail.len() {
        let t = i as f32 / b.trail.len() as f32;
        let (x1, y1) = world_to_screen(b.trail[i - 1].0, b.trail[i - 1].1, cam, sw, sh);
        let (x2, y2) = world_to_screen(b.trail[i].0, b.trail[i].1, cam, sw, sh);
        let alpha = t * max_alpha;
        draw_line(x1, y1, x2, y2, thickness, Color::new(b.glow.r, b.glow.g, b.glow.b, alpha));
    }
}

fn draw_orbit_predictions(bodies: &[Body], paths: &[Vec<(f64, f64)>], cam: &Camera, sw: f64, sh: f64) {
    for (i, path) in paths.iter().enumerate() {
        if path.len() < 2 { continue; }
        let b = &bodies[i];
        let alpha = if b.body_type == BodyType::Asteroid { 0.04 } else { 0.12 };

        for j in 1..path.len() {
            if j % 3 == 0 { continue; }
            let (x1, y1) = world_to_screen(path[j - 1].0, path[j - 1].1, cam, sw, sh);
            let (x2, y2) = world_to_screen(path[j].0, path[j].1, cam, sw, sh);
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
                let ds = bx * bx + by * by + SOFTENING * SOFTENING;
                let d = ds.sqrt();
                let f = G * b.mass / ds;
                pvx += f * bx / d;
                pvy += f * by / d;
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
    let mut panning = false;
    let mut pan_last = (0.0f32, 0.0f32);

    let mut orbit_paths: Vec<Vec<(f64, f64)>> = vec![];
    let mut orbit_timer = 0.0;

    let mut prev_sw = screen_width();
    let mut prev_sh = screen_height();

    set_mouse_cursor(CursorIcon::Crosshair);

    loop {
        let raw_dt = get_frame_time() as f64;
        let dt_capped = raw_dt.min(0.033);
        let sw = screen_width() as f64;
        let sh = screen_height() as f64;

        // Regenerate stars on resize
        if (screen_width() - prev_sw).abs() > 1.0 || (screen_height() - prev_sh).abs() > 1.0 {
            stars = make_starfield(screen_width(), screen_height());
            prev_sw = screen_width();
            prev_sh = screen_height();
        }

        flash.update(dt_capped);

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

        // Scroll = zoom
        let (_wheel_x, wheel_y) = mouse_wheel();
        if wheel_y != 0.0 {
            let zoom_delta = wheel_y * 0.03;
            cam.zoom_target = (cam.zoom_target * (1.0 + zoom_delta as f64)).clamp(0.15, 8.0);
        }

        // +/- keys for zoom
        if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::KpAdd) {
            cam.zoom_target = (cam.zoom_target * 1.3).min(8.0);
        }
        if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::KpSubtract) {
            cam.zoom_target = (cam.zoom_target / 1.3).max(0.15);
        }

        // Right-click drag = pan
        let (mx, my) = mouse_position();
        if is_mouse_button_pressed(MouseButton::Right) {
            panning = true;
            pan_last = (mx, my);
        }
        if panning && is_mouse_button_down(MouseButton::Right) {
            let dx = (mx - pan_last.0) as f64 / cam.zoom;
            let dy = (my - pan_last.1) as f64 / cam.zoom;
            cam.manual_offset = true;
            cam.offset_x -= dx;
            cam.offset_y -= dy;
            pan_last = (mx, my);
        }
        if is_mouse_button_released(MouseButton::Right) {
            panning = false;
        }

        // Left-click drag = place/launch body
        if is_mouse_button_pressed(MouseButton::Left) {
            dragging = true;
            drag_start = cam.screen_to_world(mx as f64, my as f64, sw, sh);
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

        // ---- ORBIT PREDICTIONS (recompute periodically) ----
        orbit_timer -= dt_capped;
        if orbit_timer <= 0.0 && show_orbits && bodies.len() >= 2 {
            orbit_paths = compute_orbit_predictions(&bodies);
            orbit_timer = 0.15;
        }

        // ---- RENDER ----
        clear_background(Color::new(0.02, 0.02, 0.03, 1.0));
        draw_starfield(&stars);

        if show_grid {
            draw_grid(&cam, sw, sh);
        }

        if show_orbits && orbit_paths.len() == bodies.len() {
            draw_orbit_predictions(&bodies, &orbit_paths, &cam, sw, sh);
        }

        if show_trails {
            for b in &bodies {
                draw_trail(b, &cam, sw, sh);
            }
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
        let hud_color = Color::new(0.75, 0.75, 0.8, 1.0);
        draw_text("VOID", 16.0, 28.0, 24.0, Color::new(0.85, 0.85, 0.9, 1.0));
        draw_text("orbital simulator", 16.0, 42.0, 12.0, Color::new(0.55, 0.55, 0.6, 0.9));

        let stats = format!(
            "{} bodies | {} mass | {:.1}x | {}% | {}",
            bodies.len(), total_mass as i64, time_scale, (cam.zoom * 100.0) as i32, place_mode.name()
        );
        draw_text(&stats, 16.0, 62.0, 13.0, hud_color);

        let help_y = screen_height() - 16.0;
        let help_color = Color::new(0.55, 0.55, 0.6, 0.85);
        draw_text(
            "click: place | drag: launch | 1-4: body type | 5: solar system | T: trails | O: orbits | G: grid | F: pause",
            16.0, help_y - 14.0, 11.0, help_color,
        );
        draw_text(
            "scroll: zoom | +/-: zoom | right-drag: pan | [ ]: time | H: recenter | C: clear | X: explode | space: new system",
            16.0, help_y, 11.0, help_color,
        );

        flash.draw();

        next_frame().await;
    }
}
