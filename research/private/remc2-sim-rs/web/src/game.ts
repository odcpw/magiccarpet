import * as THREE from "three";
import { AudioDirector } from "./audio";
import { Hud } from "./hud";
import { FlightInput } from "./input";
import type { Simulation } from "./simulation";

const SIM_SCALE = 256;
const WORLD_CENTER = 32_768;
const FIXED_STEP = 1 / 30;
const BEACON_COUNT = 6;

interface Projectile {
  mesh: THREE.Mesh;
  velocity: THREE.Vector3;
  life: number;
}

interface Beacon {
  group: THREE.Group;
  active: boolean;
  phase: number;
}

interface Burst {
  points: THREE.Points;
  velocities: THREE.Vector3[];
  life: number;
}

const requireCanvas = (): HTMLCanvasElement => {
  const canvas = document.querySelector<HTMLCanvasElement>("#game-canvas");
  if (!canvas) throw new Error("Missing game canvas");
  return canvas;
};

const requireGameShell = (): HTMLElement => {
  const shell = document.querySelector<HTMLElement>("#game-shell");
  if (!shell) throw new Error("Missing game shell");
  return shell;
};

const signedAngle = (angle: number): number => (angle > 1_024 ? angle - 2_048 : angle);

const toWorld = (value: number): number => (value - WORLD_CENTER) / SIM_SCALE;

const toSimulation = (value: number): number =>
  Math.round(value * SIM_SCALE + WORLD_CENTER) & 0xffff;

export class CarpetGame {
  readonly #simulation: Simulation;
  readonly #shell = requireGameShell();
  readonly #canvas = requireCanvas();
  readonly #renderer: THREE.WebGLRenderer;
  readonly #scene = new THREE.Scene();
  readonly #camera = new THREE.PerspectiveCamera(72, 1, 0.05, 520);
  readonly #hud: Hud;
  readonly #audio = new AudioDirector();
  readonly #input: FlightInput;
  readonly #clock = new THREE.Clock();
  readonly #projectiles: Projectile[] = [];
  readonly #bursts: Burst[] = [];
  readonly #beacons: Beacon[] = [];
  readonly #carpet: THREE.Mesh;
  readonly #smoke: THREE.Points;
  readonly #pausePanel: HTMLElement;
  #started = false;
  #paused = false;
  #accumulator = 0;
  #lastFire = -1;
  #magic = 100;
  #lit = 0;
  #shots = 0;
  #resets = 0;

  constructor(simulation: Simulation) {
    this.#simulation = simulation;
    this.#renderer = new THREE.WebGLRenderer({
      canvas: this.#canvas,
      antialias: true,
      powerPreference: "high-performance"
    });
    this.#renderer.outputColorSpace = THREE.SRGBColorSpace;
    this.#renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.#renderer.toneMappingExposure = 1.12;
    this.#renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.35));

    this.#scene.fog = new THREE.FogExp2(0x789b91, 0.0075);
    this.#scene.add(this.#camera);
    this.#buildSkyAndLight();
    this.#scene.add(this.#buildTerrain());
    this.#scene.add(this.#buildWater());
    this.#buildTrees();
    this.#buildClouds();
    this.#smoke = this.#buildSmoke();
    this.#scene.add(this.#smoke);
    this.#buildBeacons();
    this.#carpet = this.#buildCarpet();
    this.#camera.add(this.#carpet);

    this.#hud = new Hud(simulation);
    const pausePanel = document.querySelector<HTMLElement>("#pause-panel");
    if (!pausePanel) throw new Error("Missing pause panel");
    this.#pausePanel = pausePanel;
    this.#input = new FlightInput(
      this.#canvas,
      () => this.fire(),
      () => this.reset(),
      () => this.togglePause()
    );

    window.addEventListener("resize", () => this.#resize());
    document.addEventListener("visibilitychange", () => {
      if (document.hidden && this.#started) this.#setPaused(true);
    });
    this.#resize();
    this.#syncCamera(true);
    this.#renderer.setAnimationLoop(() => this.#frame());
  }

  async start(): Promise<void> {
    this.#started = true;
    this.#paused = false;
    this.#shell.dataset.started = "true";
    this.#input.enable();
    await this.#audio.start();
    this.#hud.toast("Follow the brass lights north.");
  }

  fire(): void {
    if (!this.#started || this.#paused || this.#magic < 8) return;
    const now = performance.now() / 1_000;
    if (now - this.#lastFire < 0.22) return;
    this.#lastFire = now;
    this.#magic -= 8;
    this.#shots += 1;
    this.#shell.dataset.shots = String(this.#shots);

    const direction = new THREE.Vector3();
    this.#camera.getWorldDirection(direction);
    const position = this.#camera.position
      .clone()
      .add(direction.clone().multiplyScalar(1.8))
      .add(new THREE.Vector3(0, -0.15, 0));
    const geometry = new THREE.SphereGeometry(0.28, 12, 10);
    const material = new THREE.MeshBasicMaterial({ color: 0xffb04d });
    const mesh = new THREE.Mesh(geometry, material);
    mesh.position.copy(position);
    const light = new THREE.PointLight(0xff7a24, 3.5, 8, 2);
    mesh.add(light);
    this.#scene.add(mesh);
    this.#projectiles.push({
      mesh,
      velocity: direction.multiplyScalar(44),
      life: 3.8
    });
    this.#audio.fire();
  }

  reset(): void {
    this.#simulation.reset();
    this.#resets += 1;
    this.#shell.dataset.resets = String(this.#resets);
    this.#magic = 100;
    this.#lit = 0;
    this.#shots = 0;
    this.#shell.dataset.shots = "0";
    for (const projectile of this.#projectiles) {
      this.#scene.remove(projectile.mesh);
    }
    this.#projectiles.length = 0;
    for (const burst of this.#bursts) this.#scene.remove(burst.points);
    this.#bursts.length = 0;
    for (const beacon of this.#beacons) {
      beacon.active = true;
      beacon.group.visible = true;
    }
    this.#hud.reset();
    this.#syncCamera(true);
  }

  togglePause(): void {
    if (!this.#started) return;
    this.#setPaused(!this.#paused);
  }

  #setPaused(paused: boolean): void {
    this.#paused = paused;
    this.#pausePanel.hidden = !paused;
    if (!paused) {
      this.#clock.getDelta();
      this.#input.enable();
    }
  }

  #frame(): void {
    const delta = Math.min(this.#clock.getDelta(), 0.1);
    if (this.#started && !this.#paused) {
      this.#accumulator += delta;
      let steps = 0;
      while (this.#accumulator >= FIXED_STEP && steps < 4) {
        const command = this.#input.sample();
        this.#simulation.advance(command.flags, command.roll, command.pitch, 1);
        this.#accumulator -= FIXED_STEP;
        steps += 1;
      }
      this.#magic = Math.min(100, this.#magic + delta * 7);
      this.#updateProjectiles(delta);
    }
    this.#syncCamera(false);
    this.#animateWorld(delta);
    this.#hud.update(
      this.#simulation,
      this.#lit,
      BEACON_COUNT,
      this.#magic
    );
    this.#shell.dataset.tick = String(this.#simulation.tick());
    this.#audio.update(this.#simulation.actual_speed());
    this.#renderer.render(this.#scene, this.#camera);
  }

  #syncCamera(immediate: boolean): void {
    const target = new THREE.Vector3(
      toWorld(this.#simulation.x()),
      this.#simulation.z() / SIM_SCALE + 0.5,
      toWorld(this.#simulation.y())
    );
    if (immediate) this.#camera.position.copy(target);
    else this.#camera.position.lerp(target, 0.23);

    const yaw = (this.#simulation.yaw() / 2_048) * Math.PI * 2;
    const pitch = (signedAngle(this.#simulation.pitch()) / 2_048) * Math.PI * 2;
    const roll = (signedAngle(this.#simulation.roll()) / 2_048) * Math.PI * 2;
    // sim-core yaw 0 subtracts from Y, matching Three's -Z camera direction;
    // positive sim yaw turns toward +X, hence the negative Three rotation.
    this.#camera.rotation.set(-pitch, -yaw, -roll * 0.5, "YXZ");
    const speedRatio = Math.min(1, Math.abs(this.#simulation.actual_speed()) / 80);
    this.#camera.fov += (72 + speedRatio * 11 - this.#camera.fov) * 0.08;
    this.#camera.updateProjectionMatrix();
  }

  #updateProjectiles(delta: number): void {
    for (let index = this.#projectiles.length - 1; index >= 0; index -= 1) {
      const projectile = this.#projectiles[index];
      if (!projectile) continue;
      projectile.mesh.position.addScaledVector(projectile.velocity, delta);
      projectile.life -= delta;
      const x = toSimulation(projectile.mesh.position.x);
      const y = toSimulation(projectile.mesh.position.z);
      const terrain = this.#simulation.terrain_height(x, y) / SIM_SCALE;
      let impact = projectile.mesh.position.y <= terrain + 0.25 || projectile.life <= 0;

      for (const beacon of this.#beacons) {
        if (!beacon.active) continue;
        if (projectile.mesh.position.distanceTo(beacon.group.position) < 2.1) {
          beacon.active = false;
          beacon.group.visible = false;
          this.#lit += 1;
          impact = true;
          this.#hud.toast(`Beacon ${this.#lit} answers.`);
          if (this.#lit === BEACON_COUNT) this.#hud.complete();
          break;
        }
      }

      if (impact) {
        this.#createBurst(projectile.mesh.position);
        this.#audio.impact();
        this.#scene.remove(projectile.mesh);
        projectile.mesh.geometry.dispose();
        (projectile.mesh.material as THREE.Material).dispose();
        this.#projectiles.splice(index, 1);
      }
    }
  }

  #createBurst(position: THREE.Vector3): void {
    const count = 34;
    const positions = new Float32Array(count * 3);
    const velocities: THREE.Vector3[] = [];
    for (let index = 0; index < count; index += 1) {
      const direction = new THREE.Vector3(
        Math.random() - 0.5,
        Math.random() * 0.8,
        Math.random() - 0.5
      )
        .normalize()
        .multiplyScalar(3 + Math.random() * 7);
      velocities.push(direction);
    }
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
    const material = new THREE.PointsMaterial({
      color: 0xffa443,
      size: 0.28,
      transparent: true,
      opacity: 1,
      depthWrite: false
    });
    const points = new THREE.Points(geometry, material);
    points.position.copy(position);
    this.#scene.add(points);
    this.#bursts.push({ points, velocities, life: 0.8 });
  }

  #animateWorld(delta: number): void {
    const time = performance.now() / 1_000;
    this.#smoke.rotation.y += delta * 0.035;
    for (const beacon of this.#beacons) {
      if (!beacon.active) continue;
      beacon.group.rotation.y += delta * 0.8;
      beacon.group.position.y += Math.sin(time * 1.7 + beacon.phase) * 0.004;
    }

    const carpetPositions = this.#carpet.geometry.getAttribute("position");
    for (let index = 0; index < carpetPositions.count; index += 1) {
      const x = carpetPositions.getX(index);
      const y = carpetPositions.getY(index);
      carpetPositions.setZ(index, Math.sin(time * 4 + x * 2.4 + y) * 0.045);
    }
    carpetPositions.needsUpdate = true;

    for (let burstIndex = this.#bursts.length - 1; burstIndex >= 0; burstIndex -= 1) {
      const burst = this.#bursts[burstIndex];
      if (!burst) continue;
      burst.life -= delta;
      const positions = burst.points.geometry.getAttribute("position");
      for (let index = 0; index < positions.count; index += 1) {
        const velocity = burst.velocities[index];
        if (!velocity) continue;
        positions.setXYZ(
          index,
          positions.getX(index) + velocity.x * delta,
          positions.getY(index) + velocity.y * delta,
          positions.getZ(index) + velocity.z * delta
        );
        velocity.y -= 8 * delta;
      }
      positions.needsUpdate = true;
      const material = burst.points.material as THREE.PointsMaterial;
      material.opacity = Math.max(0, burst.life / 0.8);
      if (burst.life <= 0) {
        this.#scene.remove(burst.points);
        burst.points.geometry.dispose();
        material.dispose();
        this.#bursts.splice(burstIndex, 1);
      }
    }
  }

  #buildSkyAndLight(): void {
    const skyGeometry = new THREE.SphereGeometry(420, 32, 18);
    const skyMaterial = new THREE.ShaderMaterial({
      side: THREE.BackSide,
      depthWrite: false,
      uniforms: {
        topColor: { value: new THREE.Color(0x071f38) },
        horizonColor: { value: new THREE.Color(0xf09a46) },
        lowerColor: { value: new THREE.Color(0x496b63) }
      },
      vertexShader: `
        varying vec3 vPosition;
        void main() {
          vPosition = position;
          gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
        }
      `,
      fragmentShader: `
        varying vec3 vPosition;
        uniform vec3 topColor;
        uniform vec3 horizonColor;
        uniform vec3 lowerColor;
        void main() {
          float h = normalize(vPosition).y;
          vec3 color = h > 0.0
            ? mix(horizonColor, topColor, pow(h, 0.55))
            : mix(horizonColor, lowerColor, min(1.0, -h * 3.0));
          gl_FragColor = vec4(color, 1.0);
        }
      `
    });
    this.#scene.add(new THREE.Mesh(skyGeometry, skyMaterial));

    const hemi = new THREE.HemisphereLight(0xffd9a1, 0x17342f, 2.4);
    this.#scene.add(hemi);
    const sun = new THREE.DirectionalLight(0xffc477, 4.2);
    sun.position.set(-90, 120, -40);
    this.#scene.add(sun);

    const sunDisk = new THREE.Mesh(
      new THREE.SphereGeometry(5, 16, 12),
      new THREE.MeshBasicMaterial({ color: 0xffe1a1, fog: false })
    );
    sunDisk.position.set(-160, 62, -190);
    this.#scene.add(sunDisk);
  }

  #buildTerrain(): THREE.Mesh {
    const resolution = 104;
    const size = 256;
    const positions: number[] = [];
    const colors: number[] = [];
    const indices: number[] = [];
    const color = new THREE.Color();
    for (let z = 0; z <= resolution; z += 1) {
      for (let x = 0; x <= resolution; x += 1) {
        const worldX = -size / 2 + (x / resolution) * size;
        const worldZ = -size / 2 + (z / resolution) * size;
        const simX = toSimulation(worldX);
        const simY = toSimulation(worldZ);
        const height = this.#simulation.terrain_height(simX, simY);
        const volcanoDistance = Math.hypot(
          worldX - toWorld(34_000),
          worldZ - toWorld(23_000)
        );
        positions.push(worldX, height / SIM_SCALE, worldZ);
        if (height < 0) color.setRGB(0.08, 0.23, 0.22);
        else if (height < 170) color.setRGB(0.6, 0.43, 0.22);
        else if (volcanoDistance < 25 && height > 850) color.setRGB(0.19, 0.15, 0.13);
        else if (height < 2_450) {
          color.setRGB(0.12, 0.34 + Math.min(0.14, height / 12_000), 0.17);
        } else if (height < 3_500) color.setRGB(0.31, 0.29, 0.22);
        else color.setRGB(0.65, 0.61, 0.52);
        colors.push(color.r, color.g, color.b);
      }
    }
    for (let z = 0; z < resolution; z += 1) {
      for (let x = 0; x < resolution; x += 1) {
        const a = z * (resolution + 1) + x;
        const b = a + 1;
        const c = a + resolution + 1;
        const d = c + 1;
        indices.push(a, c, b, b, c, d);
      }
    }
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
    geometry.setAttribute("color", new THREE.Float32BufferAttribute(colors, 3));
    geometry.setIndex(indices);
    geometry.computeVertexNormals();
    const material = new THREE.MeshStandardMaterial({
      vertexColors: true,
      roughness: 0.94,
      metalness: 0,
      flatShading: false
    });
    const terrain = new THREE.Mesh(geometry, material);
    return terrain;
  }

  #buildWater(): THREE.Mesh {
    const geometry = new THREE.PlaneGeometry(320, 320, 1, 1);
    const material = new THREE.MeshPhysicalMaterial({
      color: 0x147a83,
      roughness: 0.22,
      metalness: 0.08,
      transmission: 0.12,
      transparent: true,
      opacity: 0.84,
      depthWrite: true
    });
    const water = new THREE.Mesh(geometry, material);
    water.rotation.x = -Math.PI / 2;
    water.position.y = 0.015;
    return water;
  }

  #buildTrees(): void {
    const canopyGeometry = new THREE.ConeGeometry(0.42, 1.8, 6);
    const canopyMaterial = new THREE.MeshStandardMaterial({
      color: 0x183f28,
      roughness: 0.96
    });
    const canopies = new THREE.InstancedMesh(canopyGeometry, canopyMaterial, 420);
    const dummy = new THREE.Object3D();
    let seed = 0x4f19a2b3;
    const random = (): number => {
      seed = (Math.imul(seed, 1_664_525) + 1_013_904_223) >>> 0;
      return seed / 0xffff_ffff;
    };
    let placed = 0;
    for (let attempt = 0; attempt < 3_000 && placed < 420; attempt += 1) {
      const x = (random() - 0.5) * 220;
      const z = (random() - 0.5) * 220;
      const height = this.#simulation.terrain_height(toSimulation(x), toSimulation(z));
      const volcanoDistance = Math.hypot(x - toWorld(34_000), z - toWorld(23_000));
      if (height < 260 || height > 1_300 || volcanoDistance < 20) continue;
      const scale = 0.7 + random() * 1.2;
      dummy.position.set(x, height / SIM_SCALE + 0.75 * scale, z);
      dummy.rotation.y = random() * Math.PI * 2;
      dummy.scale.set(scale, scale, scale);
      dummy.updateMatrix();
      canopies.setMatrixAt(placed, dummy.matrix);
      placed += 1;
    }
    canopies.count = placed;
    this.#scene.add(canopies);
  }

  #buildClouds(): void {
    const cloudGeometry = new THREE.SphereGeometry(1, 8, 6);
    const material = new THREE.MeshStandardMaterial({
      color: 0xd8d4c5,
      transparent: true,
      opacity: 0.36,
      roughness: 1,
      depthWrite: false
    });
    for (let cloudIndex = 0; cloudIndex < 12; cloudIndex += 1) {
      const cloud = new THREE.Group();
      for (let puff = 0; puff < 4; puff += 1) {
        const mesh = new THREE.Mesh(cloudGeometry, material);
        const radius = 3 + Math.random() * 3;
        mesh.position.set(puff * 3 - 6, Math.random() * 2, Math.random() * 3);
        mesh.scale.set(radius, radius * 0.5, radius);
        cloud.add(mesh);
      }
      cloud.position.set(
        (Math.random() - 0.5) * 230,
        13 + Math.random() * 15,
        (Math.random() - 0.5) * 230
      );
      this.#scene.add(cloud);
    }
  }

  #buildSmoke(): THREE.Points {
    const count = 360;
    const positions = new Float32Array(count * 3);
    for (let index = 0; index < count; index += 1) {
      const radius = Math.random() * (1 + index / count * 7);
      const angle = Math.random() * Math.PI * 2;
      positions[index * 3] = toWorld(34_000) + Math.cos(angle) * radius;
      positions[index * 3 + 1] = 8 + (index / count) * 36 + Math.random() * 4;
      positions[index * 3 + 2] = toWorld(23_000) + Math.sin(angle) * radius;
    }
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
    const material = new THREE.PointsMaterial({
      color: 0x403d39,
      size: 1.4,
      transparent: true,
      opacity: 0.42,
      depthWrite: false
    });
    return new THREE.Points(geometry, material);
  }

  #buildBeacons(): void {
    const positions = [
      [0, -18],
      [-18, -36],
      [16, -54],
      [-9, -70],
      [14, -84],
      [toWorld(34_000), toWorld(23_900)]
    ] as const;
    for (const [x, z] of positions) {
      const height = this.#simulation.terrain_height(toSimulation(x), toSimulation(z));
      const group = new THREE.Group();
      const ring = new THREE.Mesh(
        new THREE.TorusGeometry(1.15, 0.12, 8, 28),
        new THREE.MeshStandardMaterial({
          color: 0xd6a64f,
          emissive: 0x7f4b16,
          emissiveIntensity: 2.4,
          metalness: 0.55,
          roughness: 0.3
        })
      );
      const core = new THREE.Mesh(
        new THREE.OctahedronGeometry(0.42, 0),
        new THREE.MeshBasicMaterial({ color: 0xffd27a })
      );
      group.add(ring, core);
      group.position.set(x, height / SIM_SCALE + 4.5, z);
      this.#scene.add(group);
      this.#beacons.push({
        group,
        active: true,
        phase: Math.random() * Math.PI * 2
      });
    }
  }

  #buildCarpet(): THREE.Mesh {
    const geometry = new THREE.PlaneGeometry(3.25, 1.65, 20, 10);
    const texture = new THREE.TextureLoader().load("/assets/carpet-texture.png");
    texture.colorSpace = THREE.SRGBColorSpace;
    texture.anisotropy = this.#renderer.capabilities.getMaxAnisotropy();
    const material = new THREE.MeshStandardMaterial({
      map: texture,
      roughness: 0.86,
      metalness: 0.04,
      side: THREE.DoubleSide
    });
    const carpet = new THREE.Mesh(geometry, material);
    carpet.position.set(0, -1.35, -2.3);
    carpet.rotation.x = -0.92;
    return carpet;
  }

  #resize(): void {
    const width = window.innerWidth;
    const height = window.innerHeight;
    this.#renderer.setSize(width, height, false);
    this.#camera.aspect = width / height;
    this.#camera.updateProjectionMatrix();
  }
}
