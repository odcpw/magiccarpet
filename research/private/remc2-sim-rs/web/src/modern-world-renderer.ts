import * as THREE from 'three';
import { ModernCharacters } from './modern-characters';
import { ModernWorldObjects } from './modern-world-objects';
import { ModernEnvironment } from './modern-environment';
import type { ModernFrame } from './modern-runtime';
import type { SourceCamera } from './original-world-renderer';
import { assetBytes, wrappedDelta, signedSourceAngle, terrainHeight, type TerrainPlanes, type WorldAssetManifest, type WorldFrame } from './original-world-data';

const BASE = '/original-api/modern-assets/';
const planeVertex = 'varying vec2 textureUv; void main(){textureUv=uv;gl_Position=vec4(position.xy,0.,1.);}';
const inspection = new URLSearchParams(location.search);

/** Soft particles are effects, not replacement creatures or independent physics. */
class NativeParticles {
  readonly root: THREE.Mesh<THREE.InstancedBufferGeometry, THREE.ShaderMaterial>;
  private readonly data: { slot: number; source: number; x: number; y: number; z: number; size: number; smoke: boolean }[] = [];
  readonly firePositions: THREE.Vector3[] = [];
  constructor() {
    const g = new THREE.InstancedBufferGeometry();
    g.setAttribute('position', new THREE.Float32BufferAttribute([-.5, -.5, 0, .5, -.5, 0, -.5, .5, 0, .5, .5, 0], 3));
    g.setIndex([0, 1, 2, 2, 1, 3]);
    for (const [name, size] of [['center', 3], ['settings', 3]] as const)
      g.setAttribute(name, new THREE.InstancedBufferAttribute(new Float32Array(2048 * size), size).setUsage(THREE.DynamicDrawUsage));
    const material = new THREE.ShaderMaterial({ transparent: true, depthWrite: false, side: THREE.DoubleSide,
      uniforms: { time: { value: 0 } },
      vertexShader: `attribute vec3 center,settings;varying vec2 particleUv;varying vec3 data;
        void main(){vec4 eye=viewMatrix*vec4(center,1.);eye.xy+=position.xy*settings.x;
          particleUv=position.xy+.5;data=settings;gl_Position=projectionMatrix*eye;}`,
      fragmentShader: `uniform float time;varying vec2 particleUv;varying vec3 data;
        float hash(vec2 p){return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453);}
        float noise(vec2 p){vec2 i=floor(p),f=fract(p);f=f*f*(3.-2.*f);return mix(mix(hash(i),hash(i+vec2(1.,0.)),f.x),mix(hash(i+vec2(0.,1.)),hash(i+1.),f.x),f.y);}
        void main(){vec2 p=particleUv-.5;float density=noise(p*5.+vec2(data.z,time*.18));
          float shape=exp(-dot(p,p)*12.)*smoothstep(.52,.2,length(p));
          if(data.y>.5){float a=shape*(.1+density*.22);gl_FragColor=vec4(mix(vec3(.1,.13,.17),vec3(.25,.29,.33),density),a);}
          else {float core=exp(-dot(p,p)*45.);gl_FragColor=vec4(mix(vec3(2.2,.32,.018),vec3(5.,3.2,.7),core),shape*(.4+density*.5));}
        }` });
    this.root = new THREE.Mesh(g, material); this.root.frustumCulled = false; this.root.renderOrder = 5;
  }
  observe(frame: ModernFrame, world: WorldFrame): void {
    this.data.length = 0;
    const entities = new Map(frame.entities.map(e => [e.entityIndex, e]));
    for (const sprite of world.sprites) {
      const e = entities.get(sprite.entity);
      if (!e || e.classId !== 10 || !e.active || sprite.drawOrder === 0xffffffff || sprite.height <= 0) continue;
      this.data.push({ slot: sprite.entity, source: sprite.source, x: sprite.x / 64, y: sprite.z / 64,
        z: sprite.y / 64, size: sprite.height / 64, smoke: e.model === 14 });
    }
  }
  render(camera: THREE.Camera, time: number): number {
    const g = this.root.geometry, center = g.getAttribute('center') as THREE.InstancedBufferAttribute;
    const settings = g.getAttribute('settings') as THREE.InstancedBufferAttribute;
    this.firePositions.length = 0; let count = 0;
    const visible = this.data.filter(p => Math.hypot(wrappedDelta(p.x, camera.position.x), wrappedDelta(p.z, camera.position.z)) < 170);
    visible.sort((a, b) => Math.hypot(wrappedDelta(b.x, camera.position.x), wrappedDelta(b.z, camera.position.z)) - Math.hypot(wrappedDelta(a.x, camera.position.x), wrappedDelta(a.z, camera.position.z)));
    for (const item of visible) {
      const x = camera.position.x + wrappedDelta(item.x, camera.position.x), z = camera.position.z + wrappedDelta(item.z, camera.position.z);
      if (!item.smoke && this.firePositions.length < 8) this.firePositions.push(new THREE.Vector3(x, item.y + item.size * .4, z));
      const copies = item.smoke ? 2 : 1;
      for (let i = 0; i < copies && count < 2048; i++, count++) {
        const phase = item.slot * .17;
        center.setXYZ(count, x + (i ? Math.sin(time * .6 + phase) * item.size * .12 : 0), item.y + item.size * (.45 + i * .16), z);
        settings.setXYZ(count, item.size * (item.smoke ? 1.15 : 1), item.smoke ? 1 : 0, phase + i * 3.7);
      }
    }
    g.instanceCount = count;
    for (const a of [center, settings]) { a.clearUpdateRanges(); if (count) a.addUpdateRange(0, count * a.itemSize); a.needsUpdate = true; }
    this.root.material.uniforms.time!.value = time; return count;
  }
}

/** Shared native controls/HUD, physically lit 3D meshes and local environment. */
export class ModernWorldRenderer {
  readonly gpu: string;
  onError: (error: unknown) => void = error => console.error(error);
  private readonly renderer: THREE.WebGLRenderer;
  private readonly scene = new THREE.Scene();
  private readonly camera = new THREE.PerspectiveCamera(65, 4 / 3, .15, 580);
  private readonly mapCamera = new THREE.OrthographicCamera(-200, 200, 200, -200, 1, 1000);
  private readonly target = new THREE.WebGLRenderTarget(1, 1, { type: THREE.HalfFloatType, samples: 2 });
  private readonly screenScene = new THREE.Scene();
  private readonly overlayScene = new THREE.Scene();
  private readonly screenCamera = new THREE.Camera();
  private readonly hudBytes = new Uint8Array(640 * 480 * 2);
  private readonly paletteBytes = new Uint8Array(1024);
  private readonly hudTexture = new THREE.DataTexture(this.hudBytes, 640, 480, THREE.RGFormat);
  private readonly paletteTexture = new THREE.DataTexture(this.paletteBytes, 256, 1, THREE.RGBAFormat);
  private readonly overlayMaterial: THREE.ShaderMaterial;
  private readonly particles = new NativeParticles();
  private environment!: ModernEnvironment;
  private characters!: ModernCharacters;
  private objects!: ModernWorldObjects;
  private planes: TerrainPlanes | null = null;
  private frame: WorldFrame | null = null;
  private modern: ModernFrame | null = null;
  private pose: SourceCamera | null = null;
  private previous: SourceCamera | null = null;
  private fov = 128; private arrival = 0; private span = 33.33; private playing = false; private dirty = true; private mapOpen = false;
  private lastTime = 0; private lastMeasured = 0; private metricsAt = 0; private width = 0; private height = 0; private frames = 0;
  private readonly intervals: number[] = [];
  private resolveReady!: () => void;
  private rejectReady!: (error: unknown) => void;
  readonly ready = new Promise<void>((resolve, reject) => { this.resolveReady = resolve; this.rejectReady = reject; });
  private shade!: Uint8Array; private mapColors!: Uint8Array;
  private constructor(private readonly canvas: HTMLCanvasElement) {
    this.renderer = new THREE.WebGLRenderer({ canvas, antialias: false, powerPreference: 'high-performance' });
    this.renderer.setPixelRatio(1); this.renderer.toneMapping = THREE.ACESFilmicToneMapping; this.renderer.toneMappingExposure = 1.35;
    this.renderer.outputColorSpace = THREE.SRGBColorSpace; this.renderer.autoClear = false; this.renderer.info.autoReset = false;
    this.renderer.shadowMap.enabled = true; this.renderer.shadowMap.type = THREE.PCFSoftShadowMap; this.renderer.shadowMap.autoUpdate = false;
    const gl = this.renderer.getContext(), extension = gl.getExtension('WEBGL_debug_renderer_info');
    this.gpu = extension ? String(gl.getParameter(extension.UNMASKED_RENDERER_WEBGL)) : String(gl.getParameter(gl.RENDERER));
    for (const t of [this.hudTexture, this.paletteTexture]) { t.colorSpace = THREE.NoColorSpace; t.minFilter = t.magFilter = THREE.NearestFilter; t.generateMipmaps = false; }
    const final = new THREE.MeshBasicMaterial({ map: this.target.texture, depthTest: false, depthWrite: false });
    this.screenScene.add(new THREE.Mesh(new THREE.PlaneGeometry(2, 2), final));
    this.overlayMaterial = new THREE.ShaderMaterial({ depthTest: false, depthWrite: false, toneMapped: false,
      uniforms: { hud: { value: this.hudTexture }, palette: { value: this.paletteTexture }, viewport: { value: new THREE.Vector2(640, 480) } },
      vertexShader: planeVertex, fragmentShader: `uniform sampler2D hud,palette;uniform vec2 viewport;varying vec2 textureUv;
        void main(){float h=viewport.y*640./viewport.x;vec2 p=vec2(textureUv.x*640.,(1.-textureUv.y)*h);bool admitted=true;
          if(p.y>=h-120.)p.y+=480.-h;else if(p.y>=160.){p.y-=(h-480.)*.5;admitted=p.y>=160.&&p.y<360.;}
          if(!admitted||p.y<0.||p.y>=480.)discard;vec2 sampleUv=(floor(p)+.5)/vec2(640.,480.);vec2 s=texture2D(hud,sampleUv).rg;
          if(s.g<.5)discard;float i=floor(s.r*255.+.5);gl_FragColor=vec4(texture2D(palette,vec2((i+.5)/256.,.5)).rgb,1.);
        }` });
    this.overlayScene.add(new THREE.Mesh(new THREE.PlaneGeometry(2, 2), this.overlayMaterial));
    this.scene.add(this.particles.root);
  }
  static async load(canvas: HTMLCanvasElement, api = '/original-api'): Promise<ModernWorldRenderer> {
    const renderer = new ModernWorldRenderer(canvas), base = api + '/modern-assets/';
    const [environment, characters, objects, manifestResponse, blobResponse] = await Promise.all([
      ModernEnvironment.load(renderer.scene, base), ModernCharacters.load(renderer.scene, base), ModernWorldObjects.load(renderer.scene, base + 'world/'),
      fetch(api + '/world-assets/manifest', { signal: AbortSignal.timeout(30000) }), fetch(api + '/world-assets/blob', { signal: AbortSignal.timeout(30000) }),
    ]);
    if (!manifestResponse.ok || !blobResponse.ok) throw new Error('Source minimap assets unavailable');
    const manifest = await manifestResponse.json() as WorldAssetManifest, blob = new Uint8Array(await blobResponse.arrayBuffer());
    environment.setSourceAtlas(assetBytes(blob, manifest.terrain.indices, 256 * 608), manifest.terrain.count);
    renderer.shade = assetBytes(blob, manifest.palette.shade_lut, 65536); renderer.mapColors = assetBytes(blob, manifest.terrain.map_colors, 256);
    renderer.environment = environment; renderer.characters = characters; renderer.objects = objects; objects.setNight(1);
    renderer.queue(); return renderer;
  }
  updateTerrain(planes: TerrainPlanes): void { this.planes = planes; this.dirty = true; }
  setPlaying(value: boolean): void { this.playing = value; this.lastMeasured = 0; this.dirty = true; }
  present(frame: WorldFrame, camera: SourceCamera, fov: number, indices: Uint8Array, coverage: Uint8Array, mapOpen: boolean, playing: boolean, modern: ModernFrame): void {
    if (!this.planes) throw new Error('Modern native terrain is missing');
    if (modern.sequence !== frame.sequence || modern.tick !== frame.tick || modern.runtimeGeneration !== frame.runtimeGeneration || modern.sessionGeneration !== frame.sessionGeneration)
      throw new Error('Modern assets and world describe different native publications');
    const now = performance.now(), same = this.modern?.epoch === modern.epoch;
    if (same && this.arrival) this.span = Math.max(16, Math.min(250, now - this.arrival));
    this.previous = same ? this.pose : camera; this.pose = camera; this.arrival = now; this.frame = frame; this.modern = modern;
    this.fov = fov; this.mapOpen = mapOpen; this.playing = playing; this.dirty = true;
    const visual = this.objects.reconcileTerrain(this.planes.heights, modern.buildings);
    if (visual.changed || this.terrainRevision !== frame.terrainRevision) { this.environment.updateTerrain(this.planes, visual.visual); this.terrainRevision = frame.terrainRevision; }
    this.characters.observe(modern); this.particles.observe(modern, frame);
    for (let i = 0; i < 640 * 480; i++) { this.hudBytes[i * 2] = indices[i]!; this.hudBytes[i * 2 + 1] = coverage[i]!; }
    for (let i = 0; i < 256; i++) { this.paletteBytes[i * 4] = frame.palette[i * 3]!; this.paletteBytes[i * 4 + 1] = frame.palette[i * 3 + 1]!;
      this.paletteBytes[i * 4 + 2] = frame.palette[i * 3 + 2]!; this.paletteBytes[i * 4 + 3] = 255; }
    this.hudTexture.needsUpdate = this.paletteTexture.needsUpdate = true; this.renderer.shadowMap.needsUpdate = true;
  }
  private terrainRevision = -1n;
  hudPoint(x: number, y: number): [number, number] {
    const r = this.canvas.getBoundingClientRect(), h = r.height * 640 / r.width; let py = (y - r.top) * 640 / r.width;
    if (py >= h - 120) py += 480 - h; else if (py >= 160) { py -= (h - 480) / 2; if (py < 160 || py >= 360) return [-1, -1]; }
    return [(x - r.left) * 640 / r.width, py];
  }
  minimap(camera: SourceCamera): number[] {
    if (!this.planes) return new Array<number>(4096).fill(0);
    const p = this.planes, a = camera.yaw * Math.PI / 1024, s = Math.floor(Math.sin(a) * 256), c = Math.floor(Math.cos(a) * 256);
    const ox = camera.x - Math.trunc(64 * (c - s)), oy = camera.y - Math.trunc(64 * (c + s)), values = new Array<number>(4096);
    for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) { const tx = ((ox - y * 2 * s + x * 2 * c) >> 8) & 255, ty = ((oy + y * 2 * c + x * 2 * s) >> 8) & 255, at = ty * 256 + tx;
      values[y * 64 + x] = this.shade[(p.shades[at]! << 8) | this.mapColors[p.types[at]!]!]!; }
    return values;
  }
  private queue(): void { requestAnimationFrame(now => { try { this.draw(now); } catch (error) { this.playing = false; this.dirty = false; this.rejectReady(error); this.onError(error); } }); }
  private draw(now: number): void {
    this.queue();
    const r = this.canvas.getBoundingClientRect(), dpr = Math.min(devicePixelRatio || 1, 1.5), width = Math.max(1, Math.round(r.width * dpr)), height = Math.max(1, Math.round(r.height * dpr));
    if (width !== this.width || height !== this.height) { this.width = width; this.height = height; this.renderer.setSize(width, height, false); this.target.setSize(width, height); this.overlayMaterial.uniforms.viewport!.value.set(width, height); this.dirty = true; }
    if (!this.modern || !this.frame || !this.pose || (!this.playing && !this.dirty)) return;
    const dt = this.lastTime ? Math.min(.1, (now - this.lastTime) / 1000) : 0; this.lastTime = now; this.dirty = false;
    const to = this.pose, from = this.previous ?? to, t = this.playing ? Math.min(1, (now - this.arrival) / this.span) : 1;
    this.camera.position.set(from.x / 64 + wrappedDelta(to.x / 64, from.x / 64) * t, (from.z + (to.z - from.z) * t) / 64,
      from.y / 64 + wrappedDelta(to.y / 64, from.y / 64) * t);
    const scale = Math.max(1, (7 * 377 * this.fov) >> 11);
    this.camera.fov = Math.min(110, Math.max(25, Math.atan(120 / scale) * 360 / Math.PI)); this.camera.aspect = width / height * (this.mapOpen ? .4 : 1);
    this.camera.rotation.set(Math.atan2((from.pitch + (to.pitch - from.pitch) * t) * 1.25, scale),
      -(from.yaw + signedSourceAngle(to.yaw - from.yaw) * t) * Math.PI / 1024, -(from.roll + (to.roll - from.roll) * t) * Math.PI / 1024, 'YXZ'); this.camera.updateProjectionMatrix();
    const inspect = inspection.get('inspect');
    if (inspect) { const actor = this.characters.getMetrics().actors.find(a => a.resource.toLowerCase() === inspect.toLowerCase());
      if (actor) { const target = new THREE.Vector3(actor.x, actor.y + 1, actor.z); this.camera.position.copy(target).add(new THREE.Vector3(-7, 3, -9)); this.camera.lookAt(target); } }
    const buildingSlot = inspection.get('inspectBuilding');
    if (buildingSlot !== null) {
      const b = this.objects.layout.buildings.find(b => b.source_slot === Number(buildingSlot) && this.modern!.buildings.some(r => r.sourceSlot === b.source_slot && r.active && r.drawing));
      if (b) { const target = new THREE.Vector3(b.x, b.base_height + b.height * .4, b.z), distance = Math.max(b.width, b.depth, b.height) * 1.1;
        this.camera.position.copy(target).add(new THREE.Vector3(-distance, distance * .55, -distance)); this.camera.lookAt(target); }
    }
    this.environment.present(this.camera, Number(this.frame.tick) / 30);
    this.objects.update(this.modern, this.frame, this.camera, (x, z) => this.environment.height(x, z));
    this.characters.render(this.camera, this.playing ? dt : 0, (x, z) => this.environment.height(x, z));
    this.objects.updateNature(this.camera, { revision: this.environment.revision, height: (x, z) => terrainHeight(this.planes!, x, z),
      visualHeight: (x, z) => this.environment.height(x, z), isWater: (x, z) => this.environment.isWater(x, z),
      groundType: (x, z) => this.planes!.types[((Math.floor(z / 4) & 255) * 256) + (Math.floor(x / 4) & 255)]! });
    const particles = this.particles.render(this.camera, Number(this.frame.tick) / 30); this.environment.fireLights(this.particles.firePositions);
    this.renderer.info.reset(); this.renderer.setRenderTarget(this.target); this.renderer.setViewport(0, 0, width, height); this.renderer.setClearColor(0x0b1422, 1); this.renderer.clear();
    if (this.mapOpen) {
      const mapWidth = Math.floor(width * .6), oldFog = this.scene.fog; this.scene.fog = null; this.environment.sky.visible = false;
      this.mapCamera.position.set(this.camera.position.x, 600, this.camera.position.z); this.mapCamera.up.set(0, 0, -1); this.mapCamera.lookAt(this.camera.position.x, 0, this.camera.position.z);
      this.mapCamera.left = -256 * mapWidth / height; this.mapCamera.right = -this.mapCamera.left; this.mapCamera.updateProjectionMatrix();
      this.renderer.setViewport(0, 0, mapWidth, height); this.renderer.render(this.scene, this.mapCamera); this.scene.fog = oldFog; this.environment.sky.visible = true;
      this.renderer.setViewport(mapWidth, 0, width - mapWidth, height);
    }
    this.renderer.render(this.scene, this.camera); this.renderer.setRenderTarget(null); this.renderer.setViewport(0, 0, width, height);
    this.renderer.render(this.screenScene, this.screenCamera); this.renderer.render(this.overlayScene, this.screenCamera); this.resolveReady(); this.frames++;
    if (this.playing && this.lastMeasured) { this.intervals.push(now - this.lastMeasured); if (this.intervals.length > 1800) this.intervals.shift(); }
    this.lastMeasured = this.playing ? now : 0;
    if (now - this.metricsAt > 500) {
      const times = [...this.intervals].sort((a, b) => a - b); const p = (q: number) => times[Math.min(times.length - 1, Math.floor(times.length * q))] ?? 0;
      Object.assign(window, { modernMetrics: { gpu: this.gpu, renderer: 'threejs-modern', camera_mode: inspect || buildingSlot !== null ? 'inspection' : 'native', frames: this.frames,
        samples: times.length, fps: times.length ? 1000 * times.length / times.reduce((a, b) => a + b, 0) : 0,
        p50_ms: p(.5), p95_ms: p(.95), p99_ms: p(.99), draw_calls: this.renderer.info.render.calls, triangles: this.renderer.info.render.triangles,
        width, height, particles, landform_rebuilds: this.environment.revision, characters: this.characters.getMetrics(), objects: this.objects.metrics, tick: this.frame.tick.toString(),
        native_entities: this.modern.entities.filter(e => e.active).length, native_characters: this.modern.characters.length,
        epoch: this.modern.epoch.toString(), sequence: this.modern.sequence.toString() } }); this.metricsAt = now;
    }
  }
}
