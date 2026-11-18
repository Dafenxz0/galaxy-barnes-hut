use crate::presets::{
    append_galaxy, append_smbh, build_preset, GalaxyParams, Rng, DEFAULT_G, DEFAULT_SOFTENING,
};
use crate::quadtree::Quadtree;

pub const PARTICLE_STRIDE: usize = 8;

pub struct ClusterSpawn {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub count: u32,
    pub total_mass: f64,
    pub radius: f64,
}

#[derive(Clone, Debug)]
pub struct Body {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub ax: f64,
    pub ay: f64,
    pub mass: f64,
    pub origin: f32,
    pub density: f32,
}

pub struct Simulation {
    pub bodies: Vec<Body>,
    pub tree: Quadtree,
    pub particle_f32: Vec<f32>,
    pub step_count: u32,
    pub last_bh_evals: u32,
    pub total_ke: f64,
    pub total_pe: f64,
    pub px: f64,
    pub py: f64,
    forces_dirty: bool,
    last_theta: f64,
    last_softening: f64,
    last_g: f64,
    next_origin: u32,
    spawn_rng: Rng,
    stack: Vec<u32>,
}

impl Simulation {
    pub fn new(preset_id: u32, target_n: u32, seed: u32) -> Self {
        let mut sim = Self {
            bodies: Vec::new(),
            tree: Quadtree::new(),
            particle_f32: Vec::new(),
            step_count: 0,
            last_bh_evals: 0,
            total_ke: 0.0,
            total_pe: 0.0,
            px: 0.0,
            py: 0.0,
            forces_dirty: true,
            last_theta: 0.55,
            last_softening: DEFAULT_SOFTENING,
            last_g: DEFAULT_G,
            next_origin: 3,
            spawn_rng: Rng::new(seed ^ 0x5A17_BEEF),
            stack: Vec::with_capacity(64),
        };
        sim.reset(preset_id, target_n, seed);
        sim
    }

    pub fn reset(&mut self, preset_id: u32, target_n: u32, seed: u32) {
        self.bodies = build_preset(preset_id, target_n as usize, seed);
        self.step_count = 0;
        self.next_origin = 3;
        self.spawn_rng = Rng::new(seed ^ 0x5A17_BEEF);
        self.forces_dirty = true;
        self.refresh_tree_and_forces(self.last_theta, self.last_softening, self.last_g);
        self.sync_particle_view();
    }

    pub fn step(&mut self, dt: f64, theta: f64, softening: f64, g_const: f64) {
        if self.bodies.is_empty() {
            return;
        }
        let dt = dt.clamp(1e-4, 0.5);
        let theta = theta.clamp(0.0, 2.5);
        let softening = softening.clamp(0.5, 100.0);
        let g_const = g_const.clamp(0.01, 50.0);

        self.last_theta = theta;
        self.last_softening = softening;
        self.last_g = g_const;

        if self.forces_dirty {
            self.refresh_tree_and_forces(theta, softening, g_const);
        }

        let half_dt = 0.5 * dt;
        for body in &mut self.bodies {
            body.vx += body.ax * half_dt;
            body.vy += body.ay * half_dt;
            body.x += body.vx * dt;
            body.y += body.vy * dt;
        }

        self.refresh_tree_and_forces(theta, softening, g_const);

        for body in &mut self.bodies {
            body.vx += body.ax * half_dt;
            body.vy += body.ay * half_dt;
        }

        self.step_count = self.step_count.wrapping_add(1);
        self.sync_particle_view();
    }

    pub fn spawn_cluster(&mut self, spec: ClusterSpawn) {
        if !spec.x.is_finite()
            || !spec.y.is_finite()
            || !spec.vx.is_finite()
            || !spec.vy.is_finite()
        {
            return;
        }
        let count = spec.count.clamp(8, 2000) as usize;
        let total_mass = spec.total_mass.clamp(10.0, 20_000.0);
        let radius = spec.radius.clamp(15.0, 300.0);
        let origin = (self.next_origin % 6) as f32;
        self.next_origin = self.next_origin.wrapping_add(1);

        let smbh_mass = total_mass * 0.65;
        let star_count = count.saturating_sub(1);
        let star_mass = if star_count > 0 {
            (total_mass * 0.35) / (star_count as f64)
        } else {
            0.5
        };

        append_galaxy(
            &mut self.bodies,
            &GalaxyParams {
                cx: spec.x,
                cy: spec.y,
                vx: spec.vx,
                vy: spec.vy,
                smbh_mass,
                star_count,
                star_mass,
                radius,
                spin: if self.next_origin.is_multiple_of(2) {
                    1.0
                } else {
                    -1.0
                },
                origin,
                spiral_arms: true,
            },
            &mut self.spawn_rng,
        );

        self.forces_dirty = true;
        self.refresh_tree_and_forces(self.last_theta, self.last_softening, self.last_g);
        self.sync_particle_view();
    }

    pub fn spawn_smbh(&mut self, x: f64, y: f64, vx: f64, vy: f64, mass: f64) {
        if !x.is_finite() || !y.is_finite() || !vx.is_finite() || !vy.is_finite() {
            return;
        }
        let mass = mass.clamp(50.0, 50_000.0);
        let origin = (self.next_origin % 6) as f32;
        self.next_origin = self.next_origin.wrapping_add(1);
        append_smbh(&mut self.bodies, x, y, vx, vy, mass, origin);

        self.forces_dirty = true;
        self.refresh_tree_and_forces(self.last_theta, self.last_softening, self.last_g);
        self.sync_particle_view();
    }

    pub fn apply_pulse(&mut self, cx: f64, cy: f64, strength: f64, radius: f64) {
        if !cx.is_finite() || !cy.is_finite() || !strength.is_finite() {
            return;
        }
        let radius = radius.clamp(10.0, 2000.0);
        let r_max_sq = radius * radius;
        let strength = strength.clamp(-25.0, 25.0);

        for body in &mut self.bodies {
            let dx = cx - body.x;
            let dy = cy - body.y;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq < r_max_sq {
                let dist = dist_sq.sqrt();
                let falloff = 1.0 - (dist / radius);
                let factor = falloff * falloff;
                let r_eff = (dist_sq + 64.0).sqrt();
                let dv = strength * factor;
                body.vx += (dx / r_eff) * dv;
                body.vy += (dy / r_eff) * dv;
            }
        }
        self.sync_particle_view();
    }

    pub fn refresh_tree_and_forces(&mut self, theta: f64, softening: f64, g_const: f64) {
        self.tree.build(&self.bodies);
        let eps_sq = softening * softening;
        let mut total_evals = 0u32;
        let mut pe_sum = 0.0;

        let len = self.bodies.len();
        for idx in 0..len {
            let (ax, ay, potential, evals) = self.tree.evaluate_force(
                idx,
                &self.bodies,
                theta,
                eps_sq,
                g_const,
                &mut self.stack,
            );
            let body = &mut self.bodies[idx];
            body.ax = ax;
            body.ay = ay;
            body.density = potential as f32;
            pe_sum += body.mass * potential;
            total_evals = total_evals.saturating_add(evals);
        }

        self.total_pe = -0.5 * pe_sum;
        self.last_bh_evals = total_evals;
        self.forces_dirty = false;
    }

    pub fn sync_particle_view(&mut self) {
        self.particle_f32.clear();
        self.particle_f32
            .reserve(self.bodies.len() * PARTICLE_STRIDE);

        let mut ke = 0.0;
        let mut px = 0.0;
        let mut py = 0.0;

        for body in &self.bodies {
            let v2 = body.vx * body.vx + body.vy * body.vy;
            let speed = v2.sqrt();
            ke += 0.5 * body.mass * v2;
            px += body.mass * body.vx;
            py += body.mass * body.vy;

            self.particle_f32.push(body.x as f32);
            self.particle_f32.push(body.y as f32);
            self.particle_f32.push(body.vx as f32);
            self.particle_f32.push(body.vy as f32);
            self.particle_f32.push(body.mass as f32);
            self.particle_f32.push(speed as f32);
            self.particle_f32.push(body.origin);
            self.particle_f32.push(body.density);
        }

        self.total_ke = ke;
        self.px = px;
        self.py = py;
    }
}
