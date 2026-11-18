const canvas = document.getElementById("sim-canvas");
const ctx = canvas.getContext("2d", { alpha: false });
const starCanvas = document.createElement("canvas");
const starCtx = starCanvas.getContext("2d", { alpha: false });

const errorBanner = document.getElementById("error-banner");
const statFps = document.getElementById("stat-fps");
const statN = document.getElementById("stat-n");
const statNodes = document.getElementById("stat-nodes");
const statSpeedup = document.getElementById("stat-speedup");
const statEnergy = document.getElementById("stat-energy");
const statMomentum = document.getElementById("stat-momentum");

const selectPreset = document.getElementById("select-preset");
const btnPlay = document.getElementById("btn-play");
const btnStep = document.getElementById("btn-step");
const btnReset = document.getElementById("btn-reset");
const btnCenter = document.getElementById("btn-center");
const btnCollapse = document.getElementById("btn-collapse");
const panelBody = document.getElementById("panel-body");

const sliderTheta = document.getElementById("slider-theta");
const sliderN = document.getElementById("slider-n");
const sliderEps = document.getElementById("slider-eps");
const sliderDt = document.getElementById("slider-dt");
const valTheta = document.getElementById("val-theta");
const valN = document.getElementById("val-n");
const valEps = document.getElementById("val-eps");
const valDt = document.getElementById("val-dt");

const chkQuadtree = document.getElementById("chk-quadtree");
const chkTrails = document.getElementById("chk-trails");
const selectColor = document.getElementById("select-color");
const toolHint = document.getElementById("tool-hint");
const toolButtons = Array.from(document.querySelectorAll(".tool-btn"));

const TOOL_HINTS = {
  galaxy: "Drag anywhere to aim velocity slingshot and launch a mini-galaxy.",
  smbh: "Drag anywhere to slingshot a lone Supermassive Black Hole.",
  attract: "Click or hold to pull nearby stars inward with a gravity well.",
  repulse: "Click or hold to blast nearby stars outward with a shockwave.",
  pan: "Drag to pan the camera; scroll mouse wheel to zoom.",
};

const ORIGIN_PALETTE = [
  [86, 186, 255],
  [255, 168, 76],
  [120, 232, 184],
  [222, 134, 255],
  [255, 110, 144],
  [190, 220, 255],
];

const BUCKET_COUNT = 24;
const THERMAL_COLORS = buildThermalPalette(BUCKET_COUNT);
const DENSITY_COLORS = buildDensityPalette(BUCKET_COUNT);
const ORIGIN_COLORS = ORIGIN_PALETTE.map(([r, g, b]) => `rgba(${r}, ${g}, ${b}, 0.82)`);

const bucketCoords = Array.from({ length: BUCKET_COUNT }, () => []);
const smbhList = [];
const pulseRings = [];

const state = {
  wasm: null,
  memory: null,
  running: true,
  presetId: 0,
  targetN: 3500,
  theta: 0.55,
  softening: 6.0,
  dt: 0.055,
  showQuadtree: true,
  showTrails: true,
  colorMode: "velocity",
  tool: "galaxy",
  seed: 42,
  cam: { x: 0, y: 0, zoom: 1.0 },
  width: 1280,
  height: 720,
  dpr: 1,
  spaceDown: false,
  pointer: {
    active: false,
    panning: false,
    startScreenX: 0,
    startScreenY: 0,
    currScreenX: 0,
    currScreenY: 0,
    startWorldX: 0,
    startWorldY: 0,
    currWorldX: 0,
    currWorldY: 0,
  },
  fpsFrames: 0,
  fpsAccum: 0,
  fpsDisplay: 60,
  lastTime: performance.now(),
  maxSpeedTrack: 8.0,
  maxDensityTrack: 40.0,
};

function buildThermalPalette(count) {
  const out = [];
  for (let i = 0; i < count; i += 1) {
    const t = i / (count - 1);
    let r;
    let g;
    let b;
    if (t < 0.35) {
      const k = t / 0.35;
      r = Math.round(45 + k * 45);
      g = Math.round(105 + k * 95);
      b = Math.round(220 + k * 35);
    } else if (t < 0.72) {
      const k = (t - 0.35) / 0.37;
      r = Math.round(90 + k * 165);
      g = Math.round(200 + k * 42);
      b = Math.round(255 - k * 85);
    } else {
      const k = (t - 0.72) / 0.28;
      r = 255;
      g = Math.round(242 - k * 124);
      b = Math.round(170 - k * 130);
    }
    out.push(`rgba(${r}, ${g}, ${b}, 0.78)`);
  }
  return out;
}

function buildDensityPalette(count) {
  const out = [];
  for (let i = 0; i < count; i += 1) {
    const t = i / (count - 1);
    let r;
    let g;
    let b;
    if (t < 0.45) {
      const k = t / 0.45;
      r = Math.round(62 + k * 18);
      g = Math.round(68 + k * 136);
      b = Math.round(180 + k * 55);
    } else {
      const k = (t - 0.45) / 0.55;
      r = Math.round(80 + k * 175);
      g = Math.round(204 + k * 46);
      b = Math.round(235 - k * 65);
    }
    out.push(`rgba(${r}, ${g}, ${b}, 0.80)`);
  }
  return out;
}

function showError(message) {
  errorBanner.textContent = message;
  errorBanner.hidden = false;
}

function resizeCanvases() {
  state.dpr = Math.min(window.devicePixelRatio || 1, 2);
  state.width = window.innerWidth;
  state.height = window.innerHeight;
  canvas.width = Math.floor(state.width * state.dpr);
  canvas.height = Math.floor(state.height * state.dpr);
  starCanvas.width = canvas.width;
  starCanvas.height = canvas.height;
  ctx.setTransform(state.dpr, 0, 0, state.dpr, 0, 0);
  starCtx.setTransform(state.dpr, 0, 0, state.dpr, 0, 0);
  clearStarBuffer();
}

function clearStarBuffer() {
  starCtx.globalCompositeOperation = "source-over";
  starCtx.fillStyle = "#03050b";
  starCtx.fillRect(0, 0, state.width, state.height);
}

function fitCameraToPreset(presetId) {
  state.cam.x = 0;
  state.cam.y = 0;
  const minDim = Math.min(state.width, state.height);
  const baseZoom = minDim / 780;
  state.cam.zoom = presetId === 1 ? baseZoom * 0.95 : baseZoom;
  clearStarBuffer();
}

function screenToWorld(sx, sy) {
  return {
    x: (sx - state.width * 0.5) / state.cam.zoom + state.cam.x,
    y: (sy - state.height * 0.5) / state.cam.zoom + state.cam.y,
  };
}

function worldToScreen(wx, wy) {
  return {
    x: (wx - state.cam.x) * state.cam.zoom + state.width * 0.5,
    y: (wy - state.cam.y) * state.cam.zoom + state.height * 0.5,
  };
}

function setTool(toolName) {
  state.tool = toolName;
  for (const btn of toolButtons) {
    btn.classList.toggle("active", btn.dataset.tool === toolName);
  }
  toolHint.textContent = TOOL_HINTS[toolName] || "";
  canvas.style.cursor = toolName === "pan" ? "grab" : "crosshair";
}

function resetSimulation() {
  if (!state.wasm) return;
  state.seed = (state.seed + 1) >>> 0;
  state.wasm.sim_init(state.presetId, state.targetN, state.seed);
  state.maxSpeedTrack = 8.0;
  state.maxDensityTrack = 40.0;
  clearStarBuffer();
  updateTelemetry();
}

function stepSimulation() {
  if (!state.wasm) return;
  state.wasm.sim_step(state.dt, state.theta, state.softening, 1.0);
}

function applyContinuousPulse() {
  if (!state.wasm || !state.pointer.active || state.pointer.panning) return;
  if (state.tool !== "attract" && state.tool !== "repulse") return;

  const strength = state.tool === "attract" ? 1.45 : -1.45;
  const worldRadius = 210 / Math.max(state.cam.zoom, 0.35);
  state.wasm.sim_apply_pulse(
    state.pointer.currWorldX,
    state.pointer.currWorldY,
    strength,
    worldRadius,
  );
  if (Math.random() < 0.35) {
    pulseRings.push({
      x: state.pointer.currWorldX,
      y: state.pointer.currWorldY,
      radius: state.tool === "attract" ? worldRadius * 0.75 : 8,
      maxRadius: worldRadius,
      attract: state.tool === "attract",
      alpha: 0.75,
    });
  }
}

function renderFrame(now) {
  requestAnimationFrame(renderFrame);
  if (!state.wasm) return;

  const elapsedMs = Math.min(now - state.lastTime, 100);
  state.lastTime = now;
  state.fpsFrames += 1;
  state.fpsAccum += elapsedMs;
  if (state.fpsAccum >= 400) {
    state.fpsDisplay = Math.round((state.fpsFrames * 1000) / state.fpsAccum);
    state.fpsFrames = 0;
    state.fpsAccum = 0;
  }

  applyContinuousPulse();

  if (state.running) {
    stepSimulation();
  }

  drawScene(elapsedMs);
  updateTelemetry();
}

function drawScene(elapsedMs) {
  const { wasm, memory, width, height, cam } = state;
  const pPtr = wasm.sim_particles_ptr() >>> 0;
  const pCount = wasm.sim_particle_count() >>> 0;
  const pStride = wasm.sim_particle_stride() >>> 0;
  const particles = new Float32Array(memory.buffer, pPtr, pCount * pStride);

  // 1. Fade or clear star layer
  starCtx.globalCompositeOperation = "source-over";
  if (state.showTrails && state.running) {
    starCtx.fillStyle = "rgba(3, 5, 11, 0.16)";
    starCtx.fillRect(0, 0, width, height);
  } else {
    starCtx.fillStyle = "#03050b";
    starCtx.fillRect(0, 0, width, height);
  }

  for (let b = 0; b < BUCKET_COUNT; b += 1) {
    bucketCoords[b].length = 0;
  }
  smbhList.length = 0;

  let frameMaxSpeed = 2.0;
  let frameMaxDensity = 5.0;
  for (let i = 0; i < pCount; i += 1) {
    const base = i * pStride;
    const speed = particles[base + 5];
    const density = particles[base + 7];
    if (speed > frameMaxSpeed) frameMaxSpeed = speed;
    if (density > frameMaxDensity) frameMaxDensity = density;
  }
  state.maxSpeedTrack = state.maxSpeedTrack * 0.94 + frameMaxSpeed * 0.06;
  state.maxDensityTrack = state.maxDensityTrack * 0.94 + frameMaxDensity * 0.06;

  const invMaxSpeed = 1.0 / Math.max(state.maxSpeedTrack, 1.0);
  const invMaxDensity = 1.0 / Math.max(state.maxDensityTrack, 1.0);
  const halfW = width * 0.5;
  const halfH = height * 0.5;
  const zoom = cam.zoom;
  const camX = cam.x;
  const camY = cam.y;
  const colorMode = state.colorMode;

  for (let i = 0; i < pCount; i += 1) {
    const base = i * pStride;
    const wx = particles[base];
    const wy = particles[base + 1];
    const sx = (wx - camX) * zoom + halfW;
    const sy = (wy - camY) * zoom + halfH;
    if (sx < -12 || sx > width + 12 || sy < -12 || sy > height + 12) {
      continue;
    }

    const mass = particles[base + 4];
    if (mass >= 50.0) {
      smbhList.push(sx, sy, mass);
      continue;
    }

    let bucket = 0;
    if (colorMode === "origin") {
      const origin = Math.round(particles[base + 6]) % ORIGIN_COLORS.length;
      bucket = origin < 0 ? 0 : origin;
    } else if (colorMode === "density") {
      const norm = Math.sqrt(Math.min(particles[base + 7] * invMaxDensity, 1.0));
      bucket = Math.min(BUCKET_COUNT - 1, Math.floor(norm * (BUCKET_COUNT - 1)));
    } else {
      const norm = Math.min(particles[base + 5] * invMaxSpeed * 1.15, 1.0);
      bucket = Math.min(BUCKET_COUNT - 1, Math.floor(norm * (BUCKET_COUNT - 1)));
    }

    bucketCoords[bucket].push(sx, sy);
  }

  starCtx.globalCompositeOperation = "lighter";
  const palette =
    colorMode === "origin"
      ? ORIGIN_COLORS
      : colorMode === "density"
        ? DENSITY_COLORS
        : THERMAL_COLORS;
  const starSize = zoom > 1.8 ? 2.2 : 1.7;
  const paletteCount = colorMode === "origin" ? ORIGIN_COLORS.length : BUCKET_COUNT;

  for (let b = 0; b < paletteCount; b += 1) {
    const coords = bucketCoords[b];
    if (coords.length === 0) continue;
    starCtx.fillStyle = palette[b];
    for (let j = 0; j < coords.length; j += 2) {
      starCtx.fillRect(coords[j], coords[j + 1], starSize, starSize);
    }
  }

  // 2. Blit star layer onto display canvas
  ctx.globalCompositeOperation = "source-over";
  ctx.drawImage(starCanvas, 0, 0, width, height);

  // 3. Live Barnes-Hut Quadtree overlay
  if (state.showQuadtree) {
    const qPtr = wasm.sim_quadtree_ptr() >>> 0;
    const qCount = wasm.sim_quadtree_box_count() >>> 0;
    const qStride = wasm.sim_quadtree_stride() >>> 0;
    const boxes = new Float32Array(memory.buffer, qPtr, qCount * qStride);

    ctx.lineWidth = 1;
    for (let i = 0; i < qCount; i += 1) {
      const base = i * qStride;
      const cx = boxes[base];
      const cy = boxes[base + 1];
      const half = boxes[base + 2];
      const depth = boxes[base + 3];

      const sizePx = half * 2 * zoom;
      if (sizePx < 2.5) continue;

      const left = (cx - half - camX) * zoom + halfW;
      const top = (cy - half - camY) * zoom + halfH;
      if (left > width || top > height || left + sizePx < 0 || top + sizePx < 0) {
        continue;
      }

      const alpha = Math.max(0.06, Math.min(0.32, 0.28 - depth * 0.014));
      ctx.strokeStyle =
        depth > 6
          ? `rgba(245, 158, 11, ${alpha.toFixed(3)})`
          : `rgba(56, 189, 248, ${alpha.toFixed(3)})`;
      ctx.strokeRect(left, top, sizePx, sizePx);
    }
  }

  // 4. Supermassive Black Holes (lensing ring + dark core)
  for (let i = 0; i < smbhList.length; i += 3) {
    const sx = smbhList[i];
    const sy = smbhList[i + 1];
    const mass = smbhList[i + 2];
    const r = Math.max(3.5, Math.min(11, Math.sqrt(mass) * 0.12 * Math.sqrt(zoom)));

    const grad = ctx.createRadialGradient(sx, sy, r * 0.6, sx, sy, r * 3.0);
    grad.addColorStop(0, "rgba(255, 224, 160, 0.85)");
    grad.addColorStop(0.45, "rgba(249, 115, 22, 0.28)");
    grad.addColorStop(1, "rgba(249, 115, 22, 0)");

    ctx.fillStyle = grad;
    ctx.beginPath();
    ctx.arc(sx, sy, r * 3.0, 0, Math.PI * 2);
    ctx.fill();

    ctx.fillStyle = "#020308";
    ctx.beginPath();
    ctx.arc(sx, sy, r, 0, Math.PI * 2);
    ctx.fill();

    ctx.strokeStyle = "rgba(255, 232, 186, 0.92)";
    ctx.lineWidth = 1.2;
    ctx.beginPath();
    ctx.arc(sx, sy, r + 0.6, 0, Math.PI * 2);
    ctx.stroke();
  }

  // 5. Pulse shockwave rings
  for (let i = pulseRings.length - 1; i >= 0; i -= 1) {
    const p = pulseRings[i];
    const step = (elapsedMs / 16.6) * 6.5;
    p.radius += p.attract ? -step : step;
    p.alpha -= elapsedMs * 0.0018;
    if (p.alpha <= 0.02 || p.radius <= 2 || p.radius >= p.maxRadius * 1.15) {
      pulseRings.splice(i, 1);
      continue;
    }
    const center = worldToScreen(p.x, p.y);
    ctx.strokeStyle = p.attract
      ? `rgba(56, 189, 248, ${p.alpha.toFixed(3)})`
      : `rgba(248, 113, 113, ${p.alpha.toFixed(3)})`;
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.arc(center.x, center.y, p.radius * zoom, 0, Math.PI * 2);
    ctx.stroke();
  }

  // 6. Drag slingshot preview
  if (
    state.pointer.active &&
    !state.pointer.panning &&
    (state.tool === "galaxy" || state.tool === "smbh")
  ) {
    const { startScreenX, startScreenY, currScreenX, currScreenY } = state.pointer;
    const dx = currScreenX - startScreenX;
    const dy = currScreenY - startScreenY;
    const len = Math.hypot(dx, dy);

    ctx.save();
    ctx.strokeStyle = state.tool === "galaxy" ? "#38bdf8" : "#f59e0b";
    ctx.fillStyle = ctx.strokeStyle;
    ctx.lineWidth = 1.6;

    if (state.tool === "galaxy") {
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      ctx.arc(startScreenX, startScreenY, 44 * zoom, 0, Math.PI * 2);
      ctx.stroke();
      ctx.setLineDash([]);
    } else {
      ctx.beginPath();
      ctx.arc(startScreenX, startScreenY, 8, 0, Math.PI * 2);
      ctx.stroke();
    }

    if (len > 4) {
      ctx.beginPath();
      ctx.moveTo(startScreenX, startScreenY);
      ctx.lineTo(currScreenX, currScreenY);
      ctx.stroke();

      const angle = Math.atan2(dy, dx);
      const head = 9;
      ctx.beginPath();
      ctx.moveTo(currScreenX, currScreenY);
      ctx.lineTo(
        currScreenX - head * Math.cos(angle - 0.42),
        currScreenY - head * Math.sin(angle - 0.42),
      );
      ctx.lineTo(
        currScreenX - head * Math.cos(angle + 0.42),
        currScreenY - head * Math.sin(angle + 0.42),
      );
      ctx.closePath();
      ctx.fill();

      const velScale = 0.035 / Math.max(zoom, 0.2);
      const speedVal = (len * velScale).toFixed(2);
      ctx.font = "11px monospace";
      ctx.fillText(`v = ${speedVal}`, currScreenX + 10, currScreenY - 8);
    }
    ctx.restore();
  }
}

function updateTelemetry() {
  const { wasm } = state;
  const n = wasm.sim_particle_count() >>> 0;
  const nodes = wasm.sim_quadtree_box_count() >>> 0;
  const evals = wasm.sim_bh_evals() >>> 0;
  const n2 = n > 1 ? n * (n - 1) : 1;
  const speedup = evals > 0 ? n2 / evals : 1;
  const totalE = wasm.sim_total_kinetic_energy() + wasm.sim_total_potential_energy();
  const px = wasm.sim_total_momentum_x();
  const py = wasm.sim_total_momentum_y();
  const pMag = Math.hypot(px, py);

  statFps.textContent = `${state.fpsDisplay}`;
  statN.textContent = n.toLocaleString();
  statNodes.textContent = nodes.toLocaleString();
  statSpeedup.textContent = `${speedup.toFixed(1)}x`;
  statEnergy.textContent = totalE.toFixed(0);
  statMomentum.textContent = pMag.toFixed(2);
}

function attachEvents() {
  window.addEventListener("resize", resizeCanvases);

  selectPreset.addEventListener("change", () => {
    state.presetId = Number(selectPreset.value);
    fitCameraToPreset(state.presetId);
    resetSimulation();
  });

  btnPlay.addEventListener("click", () => {
    state.running = !state.running;
    btnPlay.textContent = state.running ? "Pause" : "Play";
    btnPlay.classList.toggle("primary", state.running);
  });

  btnStep.addEventListener("click", () => {
    state.running = false;
    btnPlay.textContent = "Play";
    btnPlay.classList.remove("primary");
    stepSimulation();
    drawScene(16);
    updateTelemetry();
  });

  btnReset.addEventListener("click", resetSimulation);
  btnCenter.addEventListener("click", () => fitCameraToPreset(state.presetId));

  btnCollapse.addEventListener("click", () => {
    const collapsed = panelBody.classList.toggle("collapsed");
    btnCollapse.textContent = collapsed ? "+" : "−";
  });

  for (const btn of toolButtons) {
    btn.addEventListener("click", () => setTool(btn.dataset.tool));
  }

  sliderTheta.addEventListener("input", () => {
    state.theta = Number(sliderTheta.value);
    valTheta.textContent = state.theta.toFixed(2);
  });

  sliderN.addEventListener("input", () => {
    state.targetN = Number(sliderN.value);
    valN.textContent = String(state.targetN);
  });
  sliderN.addEventListener("change", resetSimulation);

  sliderEps.addEventListener("input", () => {
    state.softening = Number(sliderEps.value);
    valEps.textContent = state.softening.toFixed(1);
  });

  sliderDt.addEventListener("input", () => {
    state.dt = Number(sliderDt.value);
    valDt.textContent = state.dt.toFixed(3);
  });

  chkQuadtree.addEventListener("change", () => {
    state.showQuadtree = chkQuadtree.checked;
  });

  chkTrails.addEventListener("change", () => {
    state.showTrails = chkTrails.checked;
    if (!state.showTrails) clearStarBuffer();
  });

  selectColor.addEventListener("change", () => {
    state.colorMode = selectColor.value;
  });

  canvas.addEventListener("contextmenu", (e) => e.preventDefault());

  canvas.addEventListener("pointerdown", (e) => {
    canvas.setPointerCapture(e.pointerId);
    const panning =
      e.button === 1 || e.button === 2 || state.spaceDown || state.tool === "pan";
    const world = screenToWorld(e.clientX, e.clientY);

    state.pointer.active = true;
    state.pointer.panning = panning;
    state.pointer.startScreenX = e.clientX;
    state.pointer.startScreenY = e.clientY;
    state.pointer.currScreenX = e.clientX;
    state.pointer.currScreenY = e.clientY;
    state.pointer.startWorldX = world.x;
    state.pointer.startWorldY = world.y;
    state.pointer.currWorldX = world.x;
    state.pointer.currWorldY = world.y;

    if (panning) {
      canvas.style.cursor = "grabbing";
    } else if (state.tool === "attract" || state.tool === "repulse") {
      const worldRadius = 210 / Math.max(state.cam.zoom, 0.35);
      const strength = state.tool === "attract" ? 3.5 : -3.5;
      state.wasm.sim_apply_pulse(world.x, world.y, strength, worldRadius);
      pulseRings.push({
        x: world.x,
        y: world.y,
        radius: state.tool === "attract" ? worldRadius * 0.85 : 8,
        maxRadius: worldRadius,
        attract: state.tool === "attract",
        alpha: 0.9,
      });
    }
  });

  canvas.addEventListener("pointermove", (e) => {
    if (!state.pointer.active) return;

    if (state.pointer.panning) {
      const dx = e.clientX - state.pointer.currScreenX;
      const dy = e.clientY - state.pointer.currScreenY;
      state.cam.x -= dx / state.cam.zoom;
      state.cam.y -= dy / state.cam.zoom;
      if (state.showTrails) clearStarBuffer();
    }

    state.pointer.currScreenX = e.clientX;
    state.pointer.currScreenY = e.clientY;
    const world = screenToWorld(e.clientX, e.clientY);
    state.pointer.currWorldX = world.x;
    state.pointer.currWorldY = world.y;
  });

  canvas.addEventListener("pointerup", (e) => {
    if (!state.pointer.active) return;
    if (canvas.hasPointerCapture(e.pointerId)) {
      canvas.releasePointerCapture(e.pointerId);
    }

    if (!state.pointer.panning && state.wasm) {
      if (state.tool === "galaxy" || state.tool === "smbh") {
        const velScale = 0.035 / Math.max(state.cam.zoom, 0.2);
        const vx = (state.pointer.currScreenX - state.pointer.startScreenX) * velScale;
        const vy = (state.pointer.currScreenY - state.pointer.startScreenY) * velScale;

        if (state.tool === "galaxy") {
          state.wasm.sim_spawn_cluster(
            state.pointer.startWorldX,
            state.pointer.startWorldY,
            vx,
            vy,
            180,
            1400,
            64,
          );
        } else {
          state.wasm.sim_spawn_smbh(
            state.pointer.startWorldX,
            state.pointer.startWorldY,
            vx,
            vy,
            3200,
          );
        }
        updateTelemetry();
      }
    }

    state.pointer.active = false;
    state.pointer.panning = false;
    canvas.style.cursor = state.tool === "pan" ? "grab" : "crosshair";
  });

  canvas.addEventListener(
    "wheel",
    (e) => {
      e.preventDefault();
      const before = screenToWorld(e.clientX, e.clientY);
      const factor = e.deltaY < 0 ? 1.12 : 1 / 1.12;
      state.cam.zoom = Math.max(0.08, Math.min(8.0, state.cam.zoom * factor));
      const after = screenToWorld(e.clientX, e.clientY);
      state.cam.x += before.x - after.x;
      state.cam.y += before.y - after.y;
      if (state.showTrails) clearStarBuffer();
    },
    { passive: false },
  );

  window.addEventListener("keydown", (e) => {
    if (e.target && (e.target.tagName === "INPUT" || e.target.tagName === "SELECT")) {
      return;
    }
    if (e.code === "Space") {
      state.spaceDown = true;
      canvas.style.cursor = "grab";
    } else if (e.key === "1") {
      setTool("galaxy");
    } else if (e.key === "2") {
      setTool("smbh");
    } else if (e.key === "3") {
      setTool("attract");
    } else if (e.key === "4") {
      setTool("repulse");
    } else if (e.key === "5") {
      setTool("pan");
    } else if (e.key === "p" || e.key === "P") {
      btnPlay.click();
    } else if (e.key === "s" || e.key === "S") {
      btnStep.click();
    } else if (e.key === "r" || e.key === "R") {
      resetSimulation();
    } else if (e.key === "c" || e.key === "C") {
      fitCameraToPreset(state.presetId);
    } else if (e.key === "q" || e.key === "Q") {
      chkQuadtree.checked = !chkQuadtree.checked;
      state.showQuadtree = chkQuadtree.checked;
    } else if (e.key === "t" || e.key === "T") {
      chkTrails.checked = !chkTrails.checked;
      state.showTrails = chkTrails.checked;
      if (!state.showTrails) clearStarBuffer();
    }
  });

  window.addEventListener("keyup", (e) => {
    if (e.code === "Space") {
      state.spaceDown = false;
      canvas.style.cursor = state.tool === "pan" ? "grab" : "crosshair";
    }
  });
}

async function init() {
  resizeCanvases();
  attachEvents();

  try {
    const response = await fetch("./galaxy_barnes_hut.wasm");
    if (!response.ok) {
      throw new Error(`HTTP ${response.status} fetching galaxy_barnes_hut.wasm`);
    }
    let instantiated;
    if (typeof WebAssembly.instantiateStreaming === "function") {
      try {
        instantiated = await WebAssembly.instantiateStreaming(response, {});
      } catch {
        const fallbackRes = await fetch("./galaxy_barnes_hut.wasm");
        const bytes = await fallbackRes.arrayBuffer();
        instantiated = await WebAssembly.instantiate(bytes, {});
      }
    } else {
      const bytes = await response.arrayBuffer();
      instantiated = await WebAssembly.instantiate(bytes, {});
    }

    const { exports } = instantiated.instance;
    state.wasm = exports;
    state.memory = exports.memory;

    fitCameraToPreset(state.presetId);
    state.wasm.sim_init(state.presetId, state.targetN, state.seed);
    updateTelemetry();
    requestAnimationFrame(renderFrame);
  } catch (err) {
    showError(
      `Failed to load WebAssembly physics module (${err.message}). Serve this directory over HTTP/HTTPS (e.g. GitHub Pages or python -m http.server).`,
    );
  }
}

init();
