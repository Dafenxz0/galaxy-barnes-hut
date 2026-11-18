use std::f64::consts::{PI, TAU};

use crate::sim::Body;

pub const DEFAULT_G: f64 = 1.0;
pub const DEFAULT_SOFTENING: f64 = 6.0;

pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        let s = u64::from(seed).wrapping_add(0x9E37_79B9_7F4A_7C15);
        Self { state: if s == 0 { 1 } else { s } }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn next_f64(&mut self) -> f64 {
        let bits = self.next_u64() >> 11;
        (bits as f64) * (1.0 / ((1u64 << 53) as f64))
    }

    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }

    pub fn normal_approx(&mut self) -> f64 {
        let u1 = self.next_f64().max(1e-12);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()
    }
}

pub struct GalaxyParams {
    pub cx: f64,
    pub cy: f64,
    pub vx: f64,
    pub vy: f64,
    pub smbh_mass: f64,
    pub star_count: usize,
    pub star_mass: f64,
    pub radius: f64,
    pub spin: f64,
    pub origin: f32,
    pub spiral_arms: bool,
}

fn circular_speed(r: f64, enclosed_mass: f64, softening: f64, g: f64) -> f64 {
    let r2 = r * r + softening * softening;
    let accel_over_r = g * enclosed_mass / (r2 * r2.sqrt());
    r * accel_over_r.sqrt()
}

pub fn append_galaxy(bodies: &mut Vec<Body>, params: &GalaxyParams, rng: &mut Rng) {
    bodies.push(Body {
        x: params.cx,
        y: params.cy,
        vx: params.vx,
        vy: params.vy,
        ax: 0.0,
        ay: 0.0,
        mass: params.smbh_mass,
        origin: params.origin,
        density: 0.0,
    });

    let r_min = 12.0_f64.min(params.radius * 0.15);
    let total_disk_mass = (params.star_count as f64) * params.star_mass;

    for idx in 0..params.star_count {
        let u = rng.range(0.015, 1.0);
        let r = r_min + (params.radius - r_min) * u.powf(0.75);

        let angle = if params.spiral_arms {
            let arm = (idx % 2) as f64 * PI;
            let twist = params.spin * 2.8 * (r / params.radius);
            let scatter = rng.normal_approx() * 0.42;
            arm + twist + scatter
        } else {
            rng.range(0.0, TAU)
        };

        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let x = params.cx + r * cos_a;
        let y = params.cy + r * sin_a;

        let enclosed = params.smbh_mass + total_disk_mass * u;
        let v_circ = circular_speed(r, enclosed, DEFAULT_SOFTENING, DEFAULT_G);
        let v_tan = params.spin * v_circ * rng.range(0.96, 1.04);
        let v_rad = rng.normal_approx() * v_circ * 0.04;

        let vx = params.vx - v_tan * sin_a + v_rad * cos_a;
        let vy = params.vy + v_tan * cos_a + v_rad * sin_a;

        bodies.push(Body {
            x,
            y,
            vx,
            vy,
            ax: 0.0,
            ay: 0.0,
            mass: params.star_mass * rng.range(0.75, 1.25),
            origin: params.origin,
            density: 0.0,
        });
    }
}

pub fn append_smbh(bodies: &mut Vec<Body>, x: f64, y: f64, vx: f64, vy: f64, mass: f64, origin: f32) {
    bodies.push(Body {
        x,
        y,
        vx,
        vy,
        ax: 0.0,
        ay: 0.0,
        mass,
        origin,
        density: 0.0,
    });
}

pub fn build_preset(preset_id: u32, target_n: usize, seed: u32) -> Vec<Body> {
    let n = target_n.clamp(16, 20_000);
    let mut rng = Rng::new(seed);
    let mut bodies = Vec::with_capacity(n);

    match preset_id {
        1 => build_binary_smbh(&mut bodies, n, &mut rng),
        2 => build_ring_galaxy(&mut bodies, n, &mut rng),
        3 => build_globular_cluster(&mut bodies, n, &mut rng),
        _ => build_milkomeda(&mut bodies, n, &mut rng),
    }

    bodies
}

fn build_milkomeda(bodies: &mut Vec<Body>, n: usize, rng: &mut Rng) {
    let stars_total = n.saturating_sub(2);
    let stars_a = (stars_total * 56) / 100;
    let stars_b = stars_total.saturating_sub(stars_a);

    append_galaxy(
        bodies,
        &GalaxyParams {
            cx: -195.0,
            cy: -80.0,
            vx: 1.10,
            vy: 0.42,
            smbh_mass: 2600.0,
            star_count: stars_a,
            star_mass: 0.55,
            radius: 185.0,
            spin: 1.0,
            origin: 0.0,
            spiral_arms: true,
        },
        rng,
    );

    append_galaxy(
        bodies,
        &GalaxyParams {
            cx: 205.0,
            cy: 90.0,
            vx: -1.32,
            vy: -0.50,
            smbh_mass: 1950.0,
            star_count: stars_b,
            star_mass: 0.50,
            radius: 155.0,
            spin: -1.0,
            origin: 1.0,
            spiral_arms: true,
        },
        rng,
    );
}

fn build_binary_smbh(bodies: &mut Vec<Body>, n: usize, rng: &mut Rng) {
    let m1 = 3200.0;
    let m2 = 2600.0;
    let m_tot = m1 + m2;
    let sep = 210.0;
    let x1 = -sep * (m2 / m_tot);
    let x2 = sep * (m1 / m_tot);

    let v_rel = circular_speed(sep, m_tot, DEFAULT_SOFTENING, DEFAULT_G);
    let vy1 = -v_rel * (m2 / m_tot);
    let vy2 = v_rel * (m1 / m_tot);

    let stars_total = n.saturating_sub(2);
    let inner1 = (stars_total * 20) / 100;
    let inner2 = (stars_total * 18) / 100;
    let outer = stars_total.saturating_sub(inner1 + inner2);

    append_galaxy(
        bodies,
        &GalaxyParams {
            cx: x1,
            cy: 0.0,
            vx: 0.0,
            vy: vy1,
            smbh_mass: m1,
            star_count: inner1,
            star_mass: 0.40,
            radius: 62.0,
            spin: 1.0,
            origin: 0.0,
            spiral_arms: false,
        },
        rng,
    );

    append_galaxy(
        bodies,
        &GalaxyParams {
            cx: x2,
            cy: 0.0,
            vx: 0.0,
            vy: vy2,
            smbh_mass: m2,
            star_count: inner2,
            star_mass: 0.40,
            radius: 54.0,
            spin: 1.0,
            origin: 1.0,
            spiral_arms: false,
        },
        rng,
    );

    let circ_mass = (outer as f64) * 0.45;
    for _ in 0..outer {
        let u = rng.range(0.0, 1.0);
        let r = 175.0 + 185.0 * u.powf(0.85);
        let angle = rng.range(0.0, TAU);
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let enclosed = m_tot + circ_mass * u;
        let v_circ = circular_speed(r, enclosed, DEFAULT_SOFTENING, DEFAULT_G);
        let v_tan = v_circ * rng.range(0.95, 1.05);
        let v_rad = rng.normal_approx() * v_circ * 0.04;

        bodies.push(Body {
            x: r * cos_a,
            y: r * sin_a,
            vx: -v_tan * sin_a + v_rad * cos_a,
            vy: v_tan * cos_a + v_rad * sin_a,
            ax: 0.0,
            ay: 0.0,
            mass: 0.45 * rng.range(0.8, 1.2),
            origin: 2.0,
            density: 0.0,
        });
    }
}

fn build_ring_galaxy(bodies: &mut Vec<Body>, n: usize, rng: &mut Rng) {
    let stars_total = n.saturating_sub(2);
    let target_stars = (stars_total * 88) / 100;
    let intruder_stars = stars_total.saturating_sub(target_stars);

    append_galaxy(
        bodies,
        &GalaxyParams {
            cx: 40.0,
            cy: 0.0,
            vx: -0.20,
            vy: 0.0,
            smbh_mass: 2400.0,
            star_count: target_stars,
            star_mass: 0.55,
            radius: 215.0,
            spin: 1.0,
            origin: 0.0,
            spiral_arms: true,
        },
        rng,
    );

    append_galaxy(
        bodies,
        &GalaxyParams {
            cx: -310.0,
            cy: -24.0,
            vx: 5.8,
            vy: 0.35,
            smbh_mass: 3300.0,
            star_count: intruder_stars,
            star_mass: 0.65,
            radius: 56.0,
            spin: -1.0,
            origin: 1.0,
            spiral_arms: false,
        },
        rng,
    );
}

fn build_globular_cluster(bodies: &mut Vec<Body>, n: usize, rng: &mut Rng) {
    let total_mass = (n as f64) * 1.6;
    let scale_r = 210.0;

    for idx in 0..n {
        let u = rng.range(0.01, 0.99);
        let r = scale_r * u.powf(0.65);
        let angle = rng.range(0.0, TAU);
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        let enclosed = total_mass * u;
        let v_virial = circular_speed(r, enclosed, DEFAULT_SOFTENING, DEFAULT_G);
        let cold_factor = 0.28;
        let vx = rng.normal_approx() * v_virial * cold_factor - 0.12 * v_virial * sin_a;
        let vy = rng.normal_approx() * v_virial * cold_factor + 0.12 * v_virial * cos_a;

        let origin = (idx % 3) as f32;
        let mass = if idx == 0 {
            180.0
        } else {
            1.6 * rng.range(0.6, 1.5)
        };

        bodies.push(Body {
            x: r * cos_a,
            y: r * sin_a,
            vx,
            vy,
            ax: 0.0,
            ay: 0.0,
            mass,
            origin,
            density: 0.0,
        });
    }
}
