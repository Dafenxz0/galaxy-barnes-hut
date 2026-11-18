# Galaxy Barnes-Hut

[![CI & Deploy Pages](https://github.com/Dafenxz0/galaxy-barnes-hut/actions/workflows/ci.yml/badge.svg)](https://github.com/Dafenxz0/galaxy-barnes-hut/actions/workflows/ci.yml)

Real-time **2D N-Body Galaxy Collision & Live Barnes-Hut Quadtree Visualization** powered by a zero-dependency Rust physics engine compiled to WebAssembly (`wasm32-unknown-unknown`) and an interactive HTML5 Canvas observatory.

**Live Demo:** https://dafenxz0.github.io/galaxy-barnes-hut/

## Physics & Architecture

### 1. Barnes-Hut Quadtree ($O(N \log N)$)
At every timestep, the Rust engine builds a spatial **Quadtree** from scratch over all bodies, aggregating total mass $M$ and center of mass $\mathbf{R}_{\text{cm}}$ into each internal node. During force evaluation for particle $i$, a node of side width $s$ at distance $d = \|\mathbf{R}_{\text{cm}} - \mathbf{r}_i\|$ is treated as a single pseudo-particle whenever the opening criterion holds:

$$\frac{s}{d} < \theta$$

- Smaller $\theta$ ($\to 0.2$) increases force accuracy and subdivides deeply into dense galactic cores.
- Larger $\theta$ ($\to 1.5$) aggregates distant clusters aggressively for higher throughput.
- Setting $\theta = 0$ reproduces exact direct $O(N^2)$ pairwise summation.

### 2. Plummer-Softened Gravity & Symplectic Integration
Pairwise and multipole accelerations use Plummer softening $\epsilon$ to prevent numerical singularities during close encounters:

$$\mathbf{a}_i = \sum_{j} G M_j \frac{\mathbf{r}_j - \mathbf{r}_i}{\left(\|\mathbf{r}_j - \mathbf{r}_i\|^2 + \epsilon^2\right)^{3/2}}$$

Positions and velocities advance via **symplectic Kick-Drift-Kick Velocity-Verlet integration**, preserving linear momentum and bounding long-term energy drift across thousands of orbits.

### 3. Zero-Copy Rust $\leftrightarrow$ JS WASM ABI
`src/lib.rs` exports flat `f32` typed buffers directly from WebAssembly linear memory:
- **Particles (`stride = 8`):** `(x, y, vx, vy, mass, speed, origin, density)`
- **Live Quadtree Boxes (`stride = 4`):** `(cx, cy, half_size, depth)`

## Presets & Interactive Controls

- **Presets:**
  - *Milkomeda Spiral Collision* — two counter-rotating spiral disks with central supermassive black holes
  - *Binary Black Hole Merger* — orbiting SMBH pair with inner accretion rings and a circumbinary stellar disk
  - *Ring Galaxy Perturbation* — high-velocity compact intruder punching through a face-on spiral disk
  - *Globular Cluster Collapse* — cold sub-virial stellar cloud undergoing violent relaxation
- **Interactive Canvas Tools:**
  - `1` **Mini-Galaxy Slingshot** — drag to aim velocity vector and launch a rotating mini-galaxy
  - `2` **SMBH Slingshot** — drag to launch a lone Supermassive Black Hole
  - `3` **Attract Pulse** — click or hold to pull nearby stars inward
  - `4` **Repulse Pulse** — click or hold to blast stars outward
  - `5` **Pan View** (`Space + Drag` or right-drag anywhere; scroll wheel zooms toward cursor)
- **Live Overlays & Color Modes:**
  - Toggle **Quadtree Overlay** (`Q`) to inspect the live recursive spatial grid as $\theta$ changes
  - Toggle **Star Trails** (`T`) for long-exposure orbital paths
  - Color by **Velocity Thermal**, **Galaxy Origin**, or **Gravitational Density**
  - Adjust **$\theta$ (`0.20..1.50`)**, **Star Count $N$ (`500..8000`)**, **Softening $\epsilon$**, and **Timestep $dt$** in real time

## Building & Testing

```bash
# Run formatting, clippy, and Rust unit tests
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --verbose

# Build the WebAssembly module
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --release

# Bundle web/ + .wasm and serve locally
mkdir -p dist
cp -r web/* dist/
cp target/wasm32-unknown-unknown/release/galaxy_barnes_hut.wasm dist/
python3 -m http.server 8080 --directory dist
```

## License

[MIT](LICENSE)
