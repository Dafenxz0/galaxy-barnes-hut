use crate::sim::Body;

pub const NONE: u32 = u32::MAX;
pub const MAX_DEPTH: u32 = 30;
pub const MIN_HALF_SIZE: f64 = 1e-6;
pub const QUADTREE_BOX_STRIDE: usize = 4;

#[derive(Clone, Debug)]
pub struct QuadNode {
    pub cx: f64,
    pub cy: f64,
    pub half_size: f64,
    pub depth: u32,
    pub mass: f64,
    pub weighted_x: f64,
    pub weighted_y: f64,
    pub com_x: f64,
    pub com_y: f64,
    pub body_count: u32,
    pub first_body: u32,
    pub has_children: bool,
    pub children: [u32; 4],
}

impl QuadNode {
    fn new(cx: f64, cy: f64, half_size: f64, depth: u32) -> Self {
        Self {
            cx,
            cy,
            half_size,
            depth,
            mass: 0.0,
            weighted_x: 0.0,
            weighted_y: 0.0,
            com_x: cx,
            com_y: cy,
            body_count: 0,
            first_body: NONE,
            has_children: false,
            children: [NONE; 4],
        }
    }
}

#[derive(Default)]
pub struct Quadtree {
    pub nodes: Vec<QuadNode>,
    next_body: Vec<u32>,
    pub boxes_f32: Vec<f32>,
}

impl Quadtree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(&mut self, bodies: &[Body]) {
        self.nodes.clear();
        self.boxes_f32.clear();
        self.next_body.resize(bodies.len(), NONE);
        self.next_body.fill(NONE);

        if bodies.is_empty() {
            return;
        }

        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for body in bodies {
            if body.x.is_finite() && body.y.is_finite() {
                min_x = min_x.min(body.x);
                max_x = max_x.max(body.x);
                min_y = min_y.min(body.y);
                max_y = max_y.max(body.y);
            }
        }

        if !min_x.is_finite() {
            return;
        }

        let cx = 0.5 * (min_x + max_x);
        let cy = 0.5 * (min_y + max_y);
        let span = (max_x - min_x).max(max_y - min_y);
        let half_size = 0.5 * span * 1.02 + 1.0;

        self.nodes.reserve(bodies.len() * 2);
        self.nodes.push(QuadNode::new(cx, cy, half_size, 0));

        for (idx, body) in bodies.iter().enumerate() {
            if body.x.is_finite() && body.y.is_finite() {
                self.insert_body(idx, bodies);
            }
        }

        self.boxes_f32
            .reserve(self.nodes.len() * QUADTREE_BOX_STRIDE);
        for node in &mut self.nodes {
            if node.mass > 0.0 {
                node.com_x = node.weighted_x / node.mass;
                node.com_y = node.weighted_y / node.mass;
            }
            self.boxes_f32.push(node.cx as f32);
            self.boxes_f32.push(node.cy as f32);
            self.boxes_f32.push(node.half_size as f32);
            self.boxes_f32.push(node.depth as f32);
        }
    }

    fn insert_body(&mut self, body_idx: usize, bodies: &[Body]) {
        let bx = bodies[body_idx].x;
        let by = bodies[body_idx].y;
        let bm = bodies[body_idx].mass;
        let mut curr = 0usize;

        loop {
            if self.nodes[curr].body_count == 0 {
                let node = &mut self.nodes[curr];
                node.mass = bm;
                node.weighted_x = bm * bx;
                node.weighted_y = bm * by;
                node.body_count = 1;
                node.first_body = body_idx as u32;
                return;
            }

            if self.nodes[curr].has_children {
                let cx = self.nodes[curr].cx;
                let cy = self.nodes[curr].cy;
                self.nodes[curr].mass += bm;
                self.nodes[curr].weighted_x += bm * bx;
                self.nodes[curr].weighted_y += bm * by;
                self.nodes[curr].body_count += 1;

                let q = quadrant(cx, cy, bx, by);
                let child = self.ensure_child(curr, q);
                curr = child;
                continue;
            }

            let at_limit = self.nodes[curr].depth >= MAX_DEPTH
                || self.nodes[curr].half_size <= MIN_HALF_SIZE;
            if at_limit {
                let prev_first = self.nodes[curr].first_body;
                self.next_body[body_idx] = prev_first;
                let node = &mut self.nodes[curr];
                node.first_body = body_idx as u32;
                node.mass += bm;
                node.weighted_x += bm * bx;
                node.weighted_y += bm * by;
                node.body_count += 1;
                return;
            }

            let existing = self.nodes[curr].first_body as usize;
            let ex_x = bodies[existing].x;
            let ex_y = bodies[existing].y;
            let ex_m = bodies[existing].mass;
            let cx = self.nodes[curr].cx;
            let cy = self.nodes[curr].cy;

            self.nodes[curr].first_body = NONE;
            self.nodes[curr].has_children = true;
            self.nodes[curr].mass += bm;
            self.nodes[curr].weighted_x += bm * bx;
            self.nodes[curr].weighted_y += bm * by;
            self.nodes[curr].body_count += 1;

            let q_old = quadrant(cx, cy, ex_x, ex_y);
            let child_old = self.ensure_child(curr, q_old);
            let old_node = &mut self.nodes[child_old];
            old_node.mass = ex_m;
            old_node.weighted_x = ex_m * ex_x;
            old_node.weighted_y = ex_m * ex_y;
            old_node.body_count = 1;
            old_node.first_body = existing as u32;

            let q_new = quadrant(cx, cy, bx, by);
            curr = self.ensure_child(curr, q_new);
        }
    }

    fn ensure_child(&mut self, parent_idx: usize, q: usize) -> usize {
        let existing = self.nodes[parent_idx].children[q];
        if existing != NONE {
            return existing as usize;
        }
        let p = &self.nodes[parent_idx];
        let child_half = p.half_size * 0.5;
        let child_cx = if (q & 1) != 0 {
            p.cx + child_half
        } else {
            p.cx - child_half
        };
        let child_cy = if (q & 2) != 0 {
            p.cy + child_half
        } else {
            p.cy - child_half
        };
        let child_depth = p.depth + 1;
        let child_idx = self.nodes.len();
        self.nodes
            .push(QuadNode::new(child_cx, child_cy, child_half, child_depth));
        self.nodes[parent_idx].children[q] = child_idx as u32;
        child_idx
    }

    pub fn evaluate_force(
        &self,
        body_idx: usize,
        bodies: &[Body],
        theta: f64,
        eps_sq: f64,
        g: f64,
        stack: &mut Vec<u32>,
    ) -> (f64, f64, f64, u32) {
        if self.nodes.is_empty() {
            return (0.0, 0.0, 0.0, 0);
        }

        let x = bodies[body_idx].x;
        let y = bodies[body_idx].y;
        let theta_sq = theta * theta;

        let mut ax = 0.0;
        let mut ay = 0.0;
        let mut potential = 0.0;
        let mut evals = 0u32;

        stack.clear();
        stack.push(0);

        while let Some(node_idx) = stack.pop() {
            let node = &self.nodes[node_idx as usize];
            if node.body_count == 0 {
                continue;
            }

            if !node.has_children {
                let mut cur = node.first_body;
                while cur != NONE {
                    let other_idx = cur as usize;
                    if other_idx != body_idx {
                        let other = &bodies[other_idx];
                        let dx = other.x - x;
                        let dy = other.y - y;
                        let r2 = dx * dx + dy * dy + eps_sq;
                        let inv_r = 1.0 / r2.sqrt();
                        let inv_r3 = inv_r * inv_r * inv_r;
                        let factor = g * other.mass * inv_r3;
                        ax += dx * factor;
                        ay += dy * factor;
                        potential += g * other.mass * inv_r;
                        evals += 1;
                    }
                    cur = self.next_body[other_idx];
                }
                continue;
            }

            let dx = node.com_x - x;
            let dy = node.com_y - y;
            let dist_sq = dx * dx + dy * dy;
            let width = node.half_size * 2.0;

            if width * width < theta_sq * dist_sq {
                let r2 = dist_sq + eps_sq;
                let inv_r = 1.0 / r2.sqrt();
                let inv_r3 = inv_r * inv_r * inv_r;
                let factor = g * node.mass * inv_r3;
                ax += dx * factor;
                ay += dy * factor;
                potential += g * node.mass * inv_r;
                evals += 1;
            } else {
                for &child in &node.children {
                    if child != NONE {
                        stack.push(child);
                    }
                }
            }
        }

        (ax, ay, potential, evals)
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

fn quadrant(cx: f64, cy: f64, x: f64, y: f64) -> usize {
    let right = usize::from(x >= cx);
    let top = usize::from(y >= cy);
    (top << 1) | right
}

pub fn direct_force_for_body(
    body_idx: usize,
    bodies: &[Body],
    eps_sq: f64,
    g: f64,
) -> (f64, f64) {
    let x = bodies[body_idx].x;
    let y = bodies[body_idx].y;
    let mut ax = 0.0;
    let mut ay = 0.0;
    for (idx, other) in bodies.iter().enumerate() {
        if idx == body_idx {
            continue;
        }
        let dx = other.x - x;
        let dy = other.y - y;
        let r2 = dx * dx + dy * dy + eps_sq;
        let inv_r = 1.0 / r2.sqrt();
        let inv_r3 = inv_r * inv_r * inv_r;
        let factor = g * other.mass * inv_r3;
        ax += dx * factor;
        ay += dy * factor;
    }
    (ax, ay)
}
