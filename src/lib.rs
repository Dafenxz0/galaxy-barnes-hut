pub mod presets;
pub mod quadtree;
pub mod sim;

use std::cell::RefCell;

use quadtree::QUADTREE_BOX_STRIDE;
use sim::{Simulation, PARTICLE_STRIDE};

thread_local! {
    static SIM: RefCell<Simulation> = RefCell::new(Simulation::new(0, 3000, 42));
}

#[no_mangle]
pub extern "C" fn sim_init(preset_id: u32, target_n: u32, seed: u32) {
    SIM.with(|cell| {
        cell.borrow_mut().reset(preset_id, target_n, seed);
    });
}

#[no_mangle]
pub extern "C" fn sim_step(dt: f32, theta: f32, softening: f32, g_const: f32) {
    SIM.with(|cell| {
        cell.borrow_mut().step(
            f64::from(dt),
            f64::from(theta),
            f64::from(softening),
            f64::from(g_const),
        );
    });
}

#[no_mangle]
pub extern "C" fn sim_spawn_cluster(
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    count: u32,
    total_mass: f32,
    radius: f32,
) {
    SIM.with(|cell| {
        cell.borrow_mut().spawn_cluster(
            f64::from(x),
            f64::from(y),
            f64::from(vx),
            f64::from(vy),
            count,
            f64::from(total_mass),
            f64::from(radius),
        );
    });
}

#[no_mangle]
pub extern "C" fn sim_spawn_smbh(x: f32, y: f32, vx: f32, vy: f32, mass: f32) {
    SIM.with(|cell| {
        cell.borrow_mut().spawn_smbh(
            f64::from(x),
            f64::from(y),
            f64::from(vx),
            f64::from(vy),
            f64::from(mass),
        );
    });
}

#[no_mangle]
pub extern "C" fn sim_apply_pulse(x: f32, y: f32, strength: f32, radius: f32) {
    SIM.with(|cell| {
        cell.borrow_mut().apply_pulse(
            f64::from(x),
            f64::from(y),
            f64::from(strength),
            f64::from(radius),
        );
    });
}

#[no_mangle]
pub extern "C" fn sim_particles_ptr() -> *const f32 {
    SIM.with(|cell| cell.borrow().particle_f32.as_ptr())
}

#[no_mangle]
pub extern "C" fn sim_particle_count() -> u32 {
    SIM.with(|cell| cell.borrow().bodies.len() as u32)
}

#[no_mangle]
pub extern "C" fn sim_particle_stride() -> u32 {
    PARTICLE_STRIDE as u32
}

#[no_mangle]
pub extern "C" fn sim_quadtree_ptr() -> *const f32 {
    SIM.with(|cell| cell.borrow().tree.boxes_f32.as_ptr())
}

#[no_mangle]
pub extern "C" fn sim_quadtree_box_count() -> u32 {
    SIM.with(|cell| cell.borrow().tree.node_count() as u32)
}

#[no_mangle]
pub extern "C" fn sim_quadtree_stride() -> u32 {
    QUADTREE_BOX_STRIDE as u32
}

#[no_mangle]
pub extern "C" fn sim_step_count() -> u32 {
    SIM.with(|cell| cell.borrow().step_count)
}

#[no_mangle]
pub extern "C" fn sim_bh_evals() -> u32 {
    SIM.with(|cell| cell.borrow().last_bh_evals)
}

#[no_mangle]
pub extern "C" fn sim_total_kinetic_energy() -> f32 {
    SIM.with(|cell| cell.borrow().total_ke as f32)
}

#[no_mangle]
pub extern "C" fn sim_total_potential_energy() -> f32 {
    SIM.with(|cell| cell.borrow().total_pe as f32)
}

#[no_mangle]
pub extern "C" fn sim_total_momentum_x() -> f32 {
    SIM.with(|cell| cell.borrow().px as f32)
}

#[no_mangle]
pub extern "C" fn sim_total_momentum_y() -> f32 {
    SIM.with(|cell| cell.borrow().py as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::build_preset;
    use crate::quadtree::{direct_force_for_body, Quadtree, QUADTREE_BOX_STRIDE};
    use crate::sim::{Body, Simulation, PARTICLE_STRIDE};

    fn make_body(x: f64, y: f64, vx: f64, vy: f64, mass: f64) -> Body {
        Body {
            x,
            y,
            vx,
            vy,
            ax: 0.0,
            ay: 0.0,
            mass,
            origin: 0.0,
            density: 0.0,
        }
    }

    #[test]
    fn quadtree_subdivision_and_com_conservation() {
        let bodies = vec![
            make_body(-120.0, -80.0, 0.0, 0.0, 2.5),
            make_body(90.0, 60.0, 0.0, 0.0, 1.5),
            make_body(90.0, 60.0, 0.0, 0.0, 3.0),
            make_body(-15.0, 140.0, 0.0, 0.0, 4.0),
            make_body(10.0, -25.0, 0.0, 0.0, 1.0),
        ];

        let mut tree = Quadtree::new();
        tree.build(&bodies);

        assert!(tree.node_count() >= 5);
        assert_eq!(
            tree.boxes_f32.len(),
            tree.node_count() * QUADTREE_BOX_STRIDE
        );

        let root = &tree.nodes[0];
        assert_eq!(root.body_count, bodies.len() as u32);
        assert_eq!(root.depth, 0);

        let expected_mass: f64 = bodies.iter().map(|b| b.mass).sum();
        let expected_cx: f64 = bodies.iter().map(|b| b.mass * b.x).sum::<f64>() / expected_mass;
        let expected_cy: f64 = bodies.iter().map(|b| b.mass * b.y).sum::<f64>() / expected_mass;

        assert!((root.mass - expected_mass).abs() < 1e-12);
        assert!((root.com_x - expected_cx).abs() < 1e-12);
        assert!((root.com_y - expected_cy).abs() < 1e-12);

        for box_chunk in tree.boxes_f32.chunks_exact(QUADTREE_BOX_STRIDE) {
            let half_size = box_chunk[2];
            let depth = box_chunk[3];
            assert!(half_size > 0.0 && half_size.is_finite());
            assert!(depth >= 0.0 && depth.is_finite());
        }
    }

    #[test]
    fn barnes_hut_converges_to_direct_n2() {
        let bodies = build_preset(0, 180, 123);
        let mut tree = Quadtree::new();
        tree.build(&bodies);

        let eps_sq = 36.0;
        let g = 1.0;
        let mut stack = Vec::with_capacity(64);

        let mut exact_forces = Vec::with_capacity(bodies.len());
        for idx in 0..bodies.len() {
            exact_forces.push(direct_force_for_body(idx, &bodies, eps_sq, g));
        }

        let measure_rel_err = |theta: f64, st: &mut Vec<u32>| -> (f64, u32) {
            let mut err_sum = 0.0;
            let mut ref_sum = 0.0;
            let mut evals = 0u32;
            for (idx, &(ex, ey)) in exact_forces.iter().enumerate() {
                let (ax, ay, _, ev) = tree.evaluate_force(idx, &bodies, theta, eps_sq, g, st);
                let dx = ax - ex;
                let dy = ay - ey;
                err_sum += (dx * dx + dy * dy).sqrt();
                ref_sum += (ex * ex + ey * ey).sqrt();
                evals = evals.saturating_add(ev);
            }
            (err_sum / ref_sum.max(1e-12), evals)
        };

        let (err_exact, evals_exact) = measure_rel_err(0.0, &mut stack);
        let (err_tight, evals_tight) = measure_rel_err(0.15, &mut stack);
        let (err_mid, evals_mid) = measure_rel_err(0.55, &mut stack);
        let (err_loose, evals_loose) = measure_rel_err(1.2, &mut stack);

        assert!(err_exact < 1e-12, "theta=0 must match direct N^2");
        assert!(err_tight < 0.015, "theta=0.15 rel err too high: {err_tight}");
        assert!(err_tight < err_mid);
        assert!(err_mid < err_loose);
        assert!(evals_loose < evals_mid);
        assert!(evals_mid < evals_tight);
        assert!(evals_tight < evals_exact);
    }

    #[test]
    fn two_body_orbit_momentum_and_energy_conservation() {
        let mut sim = Simulation::new(0, 16, 1);
        sim.bodies.clear();

        let mass = 100.0;
        let half_sep = 40.0;
        let sep = 2.0 * half_sep;
        let eps = 2.0;
        let g = 1.0;
        let r2 = sep * sep + eps * eps;
        let v_rel = sep * (g * (2.0 * mass) / (r2 * r2.sqrt())).sqrt();
        let v_orb = 0.5 * v_rel;

        sim.bodies.push(make_body(-half_sep, 0.0, 0.0, -v_orb, mass));
        sim.bodies.push(make_body(half_sep, 0.0, 0.0, v_orb, mass));
        sim.refresh_tree_and_forces(0.5, eps, g);
        sim.sync_particle_view();

        let initial_energy = sim.total_ke + sim.total_pe;
        for _ in 0..1500 {
            sim.step(0.05, 0.5, eps, g);
        }

        let final_energy = sim.total_ke + sim.total_pe;
        let rel_energy_drift = ((final_energy - initial_energy) / initial_energy.abs()).abs();

        assert!(sim.px.abs() < 1e-9, "px drift: {}", sim.px);
        assert!(sim.py.abs() < 1e-9, "py drift: {}", sim.py);
        assert!(
            rel_energy_drift < 1e-3,
            "symplectic energy drift too high: {rel_energy_drift}"
        );
        for b in &sim.bodies {
            assert!(b.x.is_finite() && b.y.is_finite());
            assert!(b.vx.is_finite() && b.vy.is_finite());
        }
    }

    #[test]
    fn all_presets_and_interactions_remain_finite() {
        for preset_id in 0..=3 {
            let mut sim = Simulation::new(preset_id, 400, 77 + preset_id);
            for _ in 0..25 {
                sim.step(0.06, 0.55, 6.0, 1.0);
            }
            sim.spawn_cluster(50.0, -40.0, 1.2, -0.8, 64, 500.0, 45.0);
            sim.spawn_smbh(-80.0, 90.0, -0.5, 0.4, 1500.0);
            sim.apply_pulse(0.0, 0.0, -4.0, 220.0);
            sim.apply_pulse(20.0, 20.0, 3.0, 180.0);
            for _ in 0..10 {
                sim.step(0.06, 0.65, 6.0, 1.0);
            }

            assert_eq!(sim.bodies.len(), 400 + 64 + 1);
            assert_eq!(sim.particle_f32.len(), sim.bodies.len() * PARTICLE_STRIDE);
            assert!(sim.particle_f32.iter().all(|v| v.is_finite()));
            assert!(sim.tree.boxes_f32.iter().all(|v| v.is_finite()));
            assert!(sim.total_ke.is_finite() && sim.total_pe.is_finite());
        }
    }

    #[test]
    fn wasm_abi_exports_smoke() {
        sim_init(0, 250, 9);
        assert_eq!(sim_particle_count(), 250);
        assert_eq!(sim_particle_stride(), 8);
        assert_eq!(sim_quadtree_stride(), 4);
        assert!(sim_quadtree_box_count() >= 1);
        assert!(!sim_particles_ptr().is_null());
        assert!(!sim_quadtree_ptr().is_null());

        sim_step(0.05, 0.55, 6.0, 1.0);
        assert_eq!(sim_step_count(), 1);
        assert!(sim_bh_evals() > 0);
        assert!(sim_total_kinetic_energy().is_finite());
        assert!(sim_total_potential_energy().is_finite());
        assert!(sim_total_momentum_x().is_finite());
        assert!(sim_total_momentum_y().is_finite());

        sim_spawn_cluster(10.0, 10.0, 0.2, -0.2, 30, 300.0, 35.0);
        sim_spawn_smbh(-20.0, -20.0, 0.1, 0.1, 1000.0);
        sim_apply_pulse(0.0, 0.0, 2.5, 150.0);
        assert_eq!(sim_particle_count(), 281);
    }
}
