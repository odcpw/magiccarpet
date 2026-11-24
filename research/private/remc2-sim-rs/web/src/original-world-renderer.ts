import * as THREE from 'three';
import { admitsSprite, assetBytes, signedSourceAngle, wrappedDelta, type TerrainPlanes, type WorldAssetManifest, type WorldFrame, type WorldSprite } from './original-world-data';

export interface SourceCamera { x: number; y: number; z: number; yaw: number; pitch: number; roll: number }
interface ImageFrame { x: number; y: number; width: number; height: number }
interface ImageRow { frames: ImageFrame[]; animated: boolean }
interface Chunk { mesh: THREE.Mesh; x: number; y: number }
interface SpriteRect { left: number; bottom: number; right: number; top: number; depth: number }
const UNIT = 64, CHUNK = 16, MAX_SPRITES = 4096;
const quad = new Float32Array([-.5, -.5, 0, .5, -.5, 0, -.5, .5, 0, .5, .5, 0]);
const indices = new Uint16Array([0, 1, 2, 2, 1, 3]);
const screenVertex = `precision highp float; in vec3 position; in vec2 uv; out vec2 screenUv;
  void main(){screenUv=uv; gl_Position=vec4(position.xy,0.,1.);}`;
function rawTexture(bytes: Uint8Array, width: number, height: number, format: THREE.PixelFormat = THREE.RedFormat): THREE.DataTexture {
  const texture = new THREE.DataTexture(bytes, width, height, format, THREE.UnsignedByteType);
  texture.minFilter = texture.magFilter = THREE.NearestFilter; texture.generateMipmaps = false;
  texture.colorSpace = THREE.NoColorSpace; texture.unpackAlignment = 1; texture.needsUpdate = true;
  return texture;
}
function shader(vertexShader: string, fragmentShader: string, uniforms: Record<string, THREE.IUniform>): THREE.RawShaderMaterial {
  return new THREE.RawShaderMaterial({ glslVersion: THREE.GLSL3, vertexShader, fragmentShader, uniforms,
    blending: THREE.NoBlending, toneMapped: false, side: THREE.DoubleSide });
}
const terrainVertex = `precision highp float;
  uniform mat4 projectionMatrix, modelViewMatrix, modelMatrix; uniform vec2 eyeMap; uniform float turn;
  in vec3 position; in vec2 uv; in float tile; in float shade; in vec3 sourceNode;
  out vec2 tileUv; flat out int tileId; out float brightness;
  void main(){
    vec3 p=position; float wave=floor(sin((turn*64.+sourceNode.x*128.)*6.28318530718/2048.)*256.)*
      floor(sin((turn*64.+sourceNode.y*128.)*6.28318530718/2048.)*256.);
    float water=sourceNode.z; p.y-=water*floor(wave/1024.)/64.;
    vec4 world=modelMatrix*vec4(p,1.); vec2 delta=(world.xz-eyeMap)*64.;
    float d=dot(delta,delta), fog=clamp((23658496.-d)/8912896.,0.,1.);
    brightness=(shade+.5+(shade<56.?water*wave/8192.:0.))*fog;
    tileUv=uv;tileId=int(tile+.5); gl_Position=projectionMatrix*modelViewMatrix*vec4(p,1.);
  }`;
const terrainFragment = `precision highp float; precision highp sampler2DArray;
  uniform sampler2DArray tiles; uniform sampler2D shadeLut;
  in vec2 tileUv; flat in int tileId; in float brightness; out vec4 pixel;
  void main(){
    ivec2 p=clamp(ivec2(floor(tileUv*32.)),ivec2(0),ivec2(31));
    int source=int(texelFetch(tiles,ivec3(p,tileId),0).r*255.+.5);
    int row=int(mod(floor(brightness),256.)); if(row<0)row+=256;
    pixel=vec4(texelFetch(shadeLut,ivec2(source,row),0).r,0.,0.,1.);
  }`;
const spriteVertex = `precision highp float;
  uniform mat4 projectionMatrix, viewMatrix;
  in vec3 position; in vec3 center; in vec2 spriteSize; in vec4 imageRect; in vec4 operation;
  out vec2 imageUv; flat out vec4 rect; flat out vec3 op;
  void main(){
    vec4 eye=viewMatrix*vec4(center,1.); eye.xy+=(position.xy+vec2(0.,.5))*spriteSize;
    gl_Position=projectionMatrix*eye; imageUv=vec2(position.x+.5,.5-position.y);
    if(operation.w>.5) imageUv.x=1.-imageUv.x; rect=imageRect; op=operation.xyz;
  }`;
const spriteFragment = `precision highp float;
  uniform sampler2D atlas, shadeLut, blendLut, destination; uniform vec2 resolution;
  in vec2 imageUv; flat in vec4 rect; flat in vec3 op; out vec4 pixel;
  int shade(int offset,int source){return int(texelFetch(shadeLut,ivec2(source,(offset>>8)&255),0).r*255.+.5);}
  int blend(int high,int low){return int(texelFetch(blendLut,ivec2(low,high),0).r*255.+.5);}
  void main(){
    ivec2 p=ivec2(rect.xy)+clamp(ivec2(floor(imageUv*rect.zw)),ivec2(0),ivec2(rect.zw)-1);
    int source=int(texelFetch(atlas,p,0).r*255.+.5); if(source==0)discard;
    int mode=int(op.x+.5), brightness=int(op.y+.5), owner=int(op.z+.5);
    int target=0; if(mode==2||mode==3||mode==6||mode==7||mode==8)
      target=int(texelFetch(destination,ivec2(gl_FragCoord.xy),0).r*255.+.5);
    int value=source;
    if(mode==2||mode==6)value=blend(source,target);
    else if(mode==3||mode==7)value=blend(target,source);
    else if(mode==4)value=blend(owner,source);
    else if(mode==5)value=blend(source,owner);
    else if(mode==8)value=shade(brightness,target);
    if(mode==1||mode==6||mode==7)value=shade(brightness,value);
    pixel=vec4(float(value)/255.,0.,0.,1.);
  }`;

/** Actual local geometry renderer. It never downloads a rendered world image. */
export class OriginalWorldRenderer {
  readonly gpu: string;
  onError: (error: unknown) => void = error => console.error(error);
  private readonly renderer: THREE.WebGLRenderer;
  private readonly scene = new THREE.Scene();
  private readonly blendScene = new THREE.Scene();
  private readonly finalScene = new THREE.Scene();
  private readonly camera = new THREE.PerspectiveCamera(60, 4 / 3, .15, 128);
  private readonly screenCamera = new THREE.Camera();
  private readonly paletteBytes = new Uint8Array(1024);
  private readonly palette = rawTexture(this.paletteBytes, 256, 1, THREE.RGBAFormat);
  private readonly hudBytes = new Uint8Array(640 * 480 * 2);
  private readonly hud = rawTexture(this.hudBytes, 640, 480, THREE.RGFormat);
  private readonly target = new THREE.WebGLRenderTarget(1, 1, { minFilter: THREE.NearestFilter, magFilter: THREE.NearestFilter,
    depthBuffer: true, stencilBuffer: false, colorSpace: THREE.NoColorSpace });
  private snapshot = new THREE.FramebufferTexture(1, 1);
  private readonly images = new Map<number, ImageRow>();
  private readonly frustum = new THREE.Frustum();
  private readonly viewProjection = new THREE.Matrix4();
  private readonly spriteBounds = new THREE.Sphere(new THREE.Vector3(), 1);
  private readonly eyePoint = new THREE.Vector3();
  private readonly copyBox = new THREE.Box2();
  private readonly unavailable: Set<number>;
  private readonly terrainMaterial: THREE.RawShaderMaterial;
  private readonly spriteMaterial: THREE.RawShaderMaterial;
  private readonly blendMaterial: THREE.RawShaderMaterial;
  private readonly finalMaterial: THREE.RawShaderMaterial;
  private readonly sprites: THREE.Mesh<THREE.InstancedBufferGeometry, THREE.RawShaderMaterial>;
  private readonly blendSprite: THREE.Mesh<THREE.InstancedBufferGeometry, THREE.RawShaderMaterial>;
  private chunks: Chunk[] = [];
  private planes: TerrainPlanes | null = null;
  private frame: WorldFrame | null = null;
  private sourceCamera: SourceCamera | null = null;
  private previousCamera: SourceCamera | null = null;
  private arrival = 0; private fov = 256; private mapOpen = false; private playing = false;
  private width = 0; private height = 0; private dirty = true; private raf = 0; private lastDraw = 0;
  private frameTimes: number[] = []; private samplesAt = 0; private framesDrawn = 0; private spriteCount = 0;
  private draws = 0; private triangles = 0; private blendCopies = 0; private rebuiltChunks = 0;
  private readonly shadeBytes: Uint8Array; private readonly mapColors: Uint8Array;
  private readonly mapSine: DataView;
  private readonly mapDivider: number;
  private readonly cave: boolean;

  static async load(canvas: HTMLCanvasElement, base = '/original-api'): Promise<OriginalWorldRenderer> {
    const [manifestResponse, blobResponse] = await Promise.all([
      fetch(base + '/world-assets/manifest', { signal: AbortSignal.timeout(15000) }),
      fetch(base + '/world-assets/blob', { signal: AbortSignal.timeout(15000) }),
    ]);
    if (!manifestResponse.ok || !blobResponse.ok) throw new Error('Original world assets unavailable');
    return new OriginalWorldRenderer(canvas, await manifestResponse.json() as WorldAssetManifest, new Uint8Array(await blobResponse.arrayBuffer()));
  }
  private constructor(private readonly canvas: HTMLCanvasElement, manifest: WorldAssetManifest, blob: Uint8Array) {
    if (manifest.schema !== 'mc-original-world-assets/v1' || manifest.bytes !== blob.length ||
      manifest.terrain.width !== 32 || manifest.terrain.height !== 32 || manifest.terrain.count !== 152 || manifest.terrain.uv_groups.length !== 8)
      throw new Error('Unsupported Original source world assets');
    this.manifest = manifest;
    this.unavailable = new Set(manifest.unavailable_selectors);
    this.shadeBytes = assetBytes(blob, manifest.palette.shade_lut, 65536);
    this.mapColors = assetBytes(blob, manifest.terrain.map_colors, 256);
    const sine = assetBytes(blob, manifest.terrain.sine, 2048 * 4);
    this.mapSine = new DataView(sine.buffer, sine.byteOffset, sine.byteLength);
    this.mapDivider = assetBytes(blob, manifest.palette.clrn, 4096)[0]!;
    this.cave = manifest.map_type === 2;
    const shade = rawTexture(this.shadeBytes, 256, 256), blend = rawTexture(assetBytes(blob, manifest.palette.blend_lut, 65536), 256, 256);
    const tiles = new THREE.DataArrayTexture(assetBytes(blob, manifest.terrain.indices, 152 * 1024), 32, 32, 152);
    tiles.format = THREE.RedFormat; tiles.type = THREE.UnsignedByteType; tiles.colorSpace = THREE.NoColorSpace;
    tiles.minFilter = tiles.magFilter = THREE.NearestFilter; tiles.generateMipmaps = false; tiles.needsUpdate = true;
    this.renderer = new THREE.WebGLRenderer({ canvas, antialias: false, powerPreference: 'high-performance' });
    this.renderer.autoClear = false; this.renderer.toneMapping = THREE.NoToneMapping; this.renderer.setPixelRatio(1);
    this.renderer.info.autoReset = false;
    const gl = this.renderer.getContext(), debug = gl.getExtension('WEBGL_debug_renderer_info');
    this.gpu = debug ? String(gl.getParameter(debug.UNMASKED_RENDERER_WEBGL)) : String(gl.getParameter(gl.RENDERER));
    this.terrainMaterial = shader(terrainVertex, terrainFragment, { tiles: { value: tiles }, shadeLut: { value: shade },
      turn: { value: 0 }, eyeMap: { value: new THREE.Vector2() } });
    const atlas = this.makeAtlas(manifest, blob);
    const spriteUniforms = { atlas: { value: atlas }, shadeLut: { value: shade }, blendLut: { value: blend },
      destination: { value: this.snapshot }, resolution: { value: new THREE.Vector2(1, 1) } };
    this.spriteMaterial = shader(spriteVertex, spriteFragment, spriteUniforms);
    this.blendMaterial = shader(spriteVertex, spriteFragment, spriteUniforms); this.blendMaterial.depthWrite = false;
    this.sprites = new THREE.Mesh(this.spriteGeometry(MAX_SPRITES), this.spriteMaterial);
    this.blendSprite = new THREE.Mesh(this.spriteGeometry(MAX_SPRITES), this.blendMaterial);
    this.sprites.frustumCulled = this.blendSprite.frustumCulled = false;
    this.scene.add(this.sprites); this.blendScene.add(this.blendSprite);
    this.finalMaterial = shader(screenVertex, `precision highp float;
      uniform sampler2D worldIndices, palette, hud; uniform vec2 viewport; uniform bool mapOpen;
      in vec2 screenUv; out vec4 pixel;
      void main(){
        int index=int(texture(worldIndices,screenUv).r*255.+.5);
        float logicalHeight=viewport.y*640./viewport.x;
        vec2 h=vec2(screenUv.x*640.,(1.-screenUv.y)*logicalHeight);
        bool hudRegion=true;
        if(mapOpen){h.y=(1.-screenUv.y)*480.;}
        else if(h.y>=logicalHeight-120.)h.y+=480.-logicalHeight;
        else if(h.y>=160.){h.y-=(logicalHeight-480.)*.5; hudRegion=h.y>=160.&&h.y<360.;}
        if(hudRegion&&h.x>=0.&&h.x<640.&&h.y>=0.&&h.y<480.){
          vec2 overlay=texelFetch(hud,ivec2(h),0).rg; if(overlay.g>.5)index=int(overlay.r*255.+.5);
        }
        pixel=vec4(texelFetch(palette,ivec2(index,0),0).rgb,1.);
      }`, { worldIndices: { value: this.target.texture }, palette: { value: this.palette }, hud: { value: this.hud }, viewport: { value: new THREE.Vector2(640, 480) },
        mapOpen: { value: false } });
    this.finalMaterial.depthTest = this.finalMaterial.depthWrite = false;
    this.finalScene.add(new THREE.Mesh(new THREE.PlaneGeometry(2, 2), this.finalMaterial));
    this.queueFrame();
  }
  private readonly manifest: WorldAssetManifest;

  private makeAtlas(manifest: WorldAssetManifest, blob: Uint8Array): THREE.DataTexture {
    const width = Math.min(4096, this.renderer.capabilities.maxTextureSize);
    let x = 1, y = 1, line = 0;
    for (const row of manifest.tmaps) {
      if (row.width <= 0 || row.height <= 0 || row.width > width - 2 || row.height > 1024 || !row.frames.length) throw new Error('Invalid source sprite extent');
      const frames: ImageFrame[] = [];
      for (const bytes of row.frames) {
        assetBytes(blob, bytes, row.width * row.height);
        if (x + row.width + 1 > width) { x = 1; y += line + 2; line = 0; }
        frames.push({ x, y, width: row.width, height: row.height }); x += row.width + 2; line = Math.max(line, row.height);
      }
      this.images.set(row.selector, { frames, animated: row.animated });
    }
    let height = 1; while (height < y + line + 1) height *= 2;
    if (height > width) throw new Error('Source atlas exceeds bounded browser allocation');
    const pixels = new Uint8Array(width * height);
    for (const row of manifest.tmaps) row.frames.forEach((range, index) => {
      const frame = this.images.get(row.selector)!.frames[index]!, source = assetBytes(blob, range);
      for (let line = 0; line < row.height; line++) pixels.set(source.subarray(line * row.width, (line + 1) * row.width), (frame.y + line) * width + frame.x);
    });
    return rawTexture(pixels, width, height);
  }
  private spriteGeometry(capacity: number): THREE.InstancedBufferGeometry {
    const geometry = new THREE.InstancedBufferGeometry();
    geometry.setAttribute('position', new THREE.BufferAttribute(quad, 3)); geometry.setIndex(new THREE.BufferAttribute(indices, 1));
    for (const [key, size] of [['center', 3], ['spriteSize', 2], ['imageRect', 4], ['operation', 4]] as const)
      geometry.setAttribute(key, new THREE.InstancedBufferAttribute(new Float32Array(capacity * size), size).setUsage(THREE.DynamicDrawUsage));
    geometry.instanceCount = 0; return geometry;
  }
  private chunkGeometry(x0: number, y0: number, planes: TerrainPlanes): THREE.BufferGeometry {
    const positions: number[] = [], uv: number[] = [], tile: number[] = [], shade: number[] = [], nodes: number[] = [];
    for (let y = y0; y < y0 + CHUNK; y++) for (let x = x0; x < x0 + CHUNK; x++) {
      const cell = y * 256 + x, group = this.manifest.terrain.uv_groups[(planes.angles[cell]! >> 4) & 7]!;
      const corners = [[x, y], [x + 1, y], [x, y + 1], [x + 1, y + 1]];
      const order = ((x + y) & 1) === 0 ? [0, 2, 3, 0, 3, 1] : [0, 2, 1, 1, 2, 3];
      const selector = planes.types[cell]!;
      if (selector >= this.manifest.terrain.count) throw new Error(`Source terrain tile ${selector} unavailable`);
      for (const i of order) {
        const [cx, cy] = corners[i]!, at = (cy! & 255) * 256 + (cx! & 255), tc = group[i]!;
        positions.push((cx! - x0) * 4, planes.heights[at]! / 2, (cy! - y0) * 4);
        uv.push(tc[0]!, tc[1]!); tile.push(selector); shade.push(planes.shades[at]!);
        nodes.push(cx!, cy!, (planes.angles[at]! & 8) ? 1 : 0);
      }
    }
    const g = new THREE.BufferGeometry();
    for (const [name, values, size] of [['position', positions, 3], ['uv', uv, 2], ['tile', tile, 1], ['shade', shade, 1], ['sourceNode', nodes, 3]] as const)
      g.setAttribute(name, new THREE.Float32BufferAttribute(values, size));
    g.computeBoundingSphere(); return g;
  }
  updateTerrain(planes: TerrainPlanes): void {
    if (!this.planes) for (let y = 0; y < 256; y += CHUNK) for (let x = 0; x < 256; x += CHUNK) {
      const mesh = new THREE.Mesh(this.chunkGeometry(x, y, planes), this.terrainMaterial); this.scene.add(mesh); this.chunks.push({ mesh, x, y });
    }
    else for (const chunk of this.chunks) {
      let changed = false;
      for (let y = chunk.y; y <= chunk.y + CHUNK && !changed; y++) for (let x = chunk.x; x <= chunk.x + CHUNK; x++) {
        const at = (y & 255) * 256 + (x & 255);
        if (planes.heights[at] !== this.planes.heights[at] || planes.types[at] !== this.planes.types[at] ||
          planes.shades[at] !== this.planes.shades[at] || planes.angles[at] !== this.planes.angles[at]) { changed = true; break; }
      }
      if (changed) { const prior = chunk.mesh.geometry; chunk.mesh.geometry = this.chunkGeometry(chunk.x, chunk.y, planes); prior.dispose(); this.rebuiltChunks++; }
    }
    this.planes = planes; this.dirty = true;
  }
  present(frame: WorldFrame, camera: SourceCamera, fov: number, hudIndices: Uint8Array, coverage: Uint8Array, mapOpen: boolean, playing: boolean): void {
    if (!this.planes) throw new Error('Native terrain must be installed before its world frame');
    const sameEpoch = this.frame?.runtimeGeneration === frame.runtimeGeneration && this.frame.sessionGeneration === frame.sessionGeneration;
    this.previousCamera = sameEpoch ? this.sourceCamera : camera; this.sourceCamera = camera; this.frame = frame; this.fov = fov;
    this.arrival = performance.now(); this.mapOpen = mapOpen; this.playing = playing;
    for (let i = 0; i < 256; i++) { this.paletteBytes[i * 4] = frame.palette[i * 3]!; this.paletteBytes[i * 4 + 1] = frame.palette[i * 3 + 1]!;
      this.paletteBytes[i * 4 + 2] = frame.palette[i * 3 + 2]!; this.paletteBytes[i * 4 + 3] = 255; }
    for (let i = 0; i < 640 * 480; i++) {
      this.hudBytes[i * 2] = hudIndices[i]!;
      this.hudBytes[i * 2 + 1] = mapOpen && i % 640 < 386 ? 255 : coverage[i]!;
    }
    this.palette.needsUpdate = this.hud.needsUpdate = true; this.dirty = true;
  }
  setPlaying(value: boolean): void { this.playing = value; this.lastDraw = 0; if (!value) this.dirty = true; }
  hudPoint(x: number, y: number): [number, number] {
    const rect = this.canvas.getBoundingClientRect(), h = rect.height * 640 / rect.width;
    if (this.mapOpen) return [(x - rect.left) * 640 / rect.width, (y - rect.top) * 480 / rect.height];
    let py = (y - rect.top) * 640 / rect.width;
    if (py >= h - 120) py += 480 - h;
    else if (py >= 160) { py -= (h - 480) / 2; if (py < 160 || py >= 360) return [-1, -1]; }
    return [(x - rect.left) * 640 / rect.width, py];
  }
  /** Source split map at VGA1 scale 408. The narrow right viewport stays in Three.js. */
  fullMapTerrain(camera: SourceCamera): Uint8Array {
    if (!this.planes) throw new Error('Full map requires the published terrain planes');
    const planes = this.planes;
    const sine = (yaw: number) => (408 * this.mapSine.getInt32((yaw & 2047) * 4, true)) >> 16;
    const sin = sine(camera.yaw), cos = sine(camera.yaw + 512);
    const stepX = Math.trunc(200 * cos / 191), stepY = Math.trunc(200 * sin / 191);
    let rowX = camera.x - Math.trunc((191 * stepX - 200 * sin) / 2);
    let rowY = camera.y - Math.trunc((200 * cos + 191 * stepY) / 2);
    const frame = new Uint8Array(320 * 200);
    for (let row = 0; row < 200; row++, rowX -= sin, rowY += cos) {
      let x = rowX, y = rowY;
      const start = row * 320;
      for (let col = 0; col < 191; col++, x += stepX, y += stepY) {
        const tile = (((y >> 8) & 255) << 8) | ((x >> 8) & 255);
        if (!this.cave || (planes.angles[tile]! & 8) === 0) {
          frame[start + col] = this.shadeBytes[(planes.shades[tile]! << 8) | this.mapColors[planes.types[tile]!]!]!;
        }
      }
      frame[start + 191] = frame[start + 192] = this.mapDivider;
    }
    return frame;
  }
  minimap(camera: SourceCamera): number[] {
    if (!this.planes) return new Array<number>(4096).fill(0);
    const planes = this.planes, angle = camera.yaw * Math.PI / 1024;
    const sin = Math.floor(Math.sin(angle) * 256), cos = Math.floor(Math.cos(angle) * 256);
    const ox = camera.x - Math.trunc(64 * (cos - sin)), oy = camera.y - Math.trunc(64 * (cos + sin));
    const result = new Array<number>(4096);
    for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) {
      const tx = ((ox - y * 2 * sin + x * 2 * cos) >> 8) & 255, ty = ((oy + y * 2 * cos + x * 2 * sin) >> 8) & 255;
      const at = ty * 256 + tx; result[y * 64 + x] = this.shadeBytes[(planes.shades[at]! << 8) | this.mapColors[planes.types[at]!]!]!;
    }
    return result;
  }
  private writeSprite(geometry: THREE.InstancedBufferGeometry, index: number, row: WorldSprite, eye: THREE.Vector3): boolean {
    if (!admitsSprite(row, { x: eye.x * 64, y: eye.z * 64, yaw: -this.camera.rotation.y * 1024 / Math.PI })) return false;
    const image = this.images.get(row.selector);
    if (!image) { if (this.unavailable.has(row.selector)) return false; throw new Error(`Unbound native TMAP ${row.selector}`); }
    if (row.pixelMode > 8) throw new Error(`Unsupported native sprite pixel mode ${row.pixelMode}`);
    // Native selectors own creature animation. SS2 texture delta phase is a
    // presentation approximation until its source-global clock has an ABI.
    const phase = image.animated && image.frames.length > 1 ? Math.floor(Number(this.frame!.tick % 100000n) / 4) % (image.frames.length - 1) : 0;
    const rect = image.frames[phase]!, h = row.height / UNIT;
    const x = eye.x + wrappedDelta(row.x / UNIT, eye.x), z = eye.z + wrappedDelta(row.y / UNIT, eye.z);
    this.spriteBounds.center.set(x, row.z / UNIT + h / 2, z);
    this.spriteBounds.radius = Math.hypot(h * rect.width / rect.height, h) / 2;
    if (!this.frustum.intersectsSphere(this.spriteBounds)) return false;
    (geometry.getAttribute('center') as THREE.InstancedBufferAttribute).setXYZ(index, x, row.z / UNIT, z);
    (geometry.getAttribute('spriteSize') as THREE.InstancedBufferAttribute).setXY(index, h * rect.width / rect.height, h);
    (geometry.getAttribute('imageRect') as THREE.InstancedBufferAttribute).setXYZW(index, rect.x, rect.y, rect.width, rect.height);
    (geometry.getAttribute('operation') as THREE.InstancedBufferAttribute).setXYZW(index, row.pixelMode, row.brightness, row.ownerColor, row.mirrored ? 1 : 0);
    return true;
  }
  private upload(geometry: THREE.InstancedBufferGeometry): void {
    for (const name of ['center', 'spriteSize', 'imageRect', 'operation']) {
      const attribute = geometry.getAttribute(name) as THREE.InstancedBufferAttribute;
      attribute.clearUpdateRanges();
      if (geometry.instanceCount) attribute.addUpdateRange(0, geometry.instanceCount * attribute.itemSize);
      attribute.needsUpdate = true;
    }
  }
  private projectedRect(): SpriteRect | null {
    const g = this.blendSprite.geometry, center = g.getAttribute('center'), size = g.getAttribute('spriteSize');
    const p = this.eyePoint.set(center.getX(0), center.getY(0), center.getZ(0)).applyMatrix4(this.camera.matrixWorldInverse);
    const depth = -p.z; if (depth <= 0) return null;
    const e = this.camera.projectionMatrix.elements, vw = this.width * (this.mapOpen ? .4 : 1), vx = this.mapOpen ? this.width - vw : 0;
    const x = vx + (p.x * e[0]! / depth * .5 + .5) * vw, y = (p.y * e[5]! / depth * .5 + .5) * this.height;
    const w = size.getX(0) * e[0]! / depth * .5 * vw, h = size.getY(0) * e[5]! / depth * .5 * this.height;
    const left = Math.max(0, Math.floor(x - w / 2) - 1), right = Math.min(this.width, Math.ceil(x + w / 2) + 1);
    const bottom = Math.max(0, Math.floor(y) - 1), top = Math.min(this.height, Math.ceil(y + h) + 1);
    return left < right && bottom < top ? { left, right, bottom, top, depth } : null;
  }
  private resize(): void {
    const rect = this.canvas.getBoundingClientRect(), scale = Math.min(devicePixelRatio || 1, 1.5);
    const width = Math.max(1, Math.round(rect.width * scale)), height = Math.max(1, Math.round(rect.height * scale));
    if (width === this.width && height === this.height) return;
    this.width = width; this.height = height; this.renderer.setSize(width, height, false); this.target.setSize(width, height);
    this.snapshot.dispose(); this.snapshot = new THREE.FramebufferTexture(width, height); this.snapshot.minFilter = this.snapshot.magFilter = THREE.NearestFilter;
    this.spriteMaterial.uniforms.destination!.value = this.blendMaterial.uniforms.destination!.value = this.snapshot;
    this.finalMaterial.uniforms.viewport!.value.set(width, height); this.dirty = true;
  }
  private queueFrame(): void {
    this.raf = requestAnimationFrame(time => {
      try { this.loop(time); }
      catch (error) { this.playing = false; this.dirty = false; this.onError(error); }
    });
  }
  private loop(now: number): void {
    this.queueFrame(); this.resize();
    if (!this.frame || !this.sourceCamera || (!this.playing && !this.dirty)) return;
    this.dirty = false; const to = this.sourceCamera, from = this.previousCamera ?? to;
    const t = this.playing ? Math.min(1, Math.max(0, (now - this.arrival) / (1000 / 30))) : 1;
    const lerp = (a: number, b: number) => a + (b - a) * t;
    this.camera.position.set(from.x / UNIT + wrappedDelta(to.x / UNIT, from.x / UNIT) * t,
      lerp(from.z, to.z) / UNIT, from.y / UNIT + wrappedDelta(to.y / UNIT, from.y / UNIT) * t);
    const yaw = from.yaw + signedSourceAngle(to.yaw - from.yaw) * t, scale = Math.max(1, (7 * 377 * this.fov) >> 11);
    this.camera.fov = Math.min(110, Math.max(25, 2 * Math.atan(120 / scale) * 180 / Math.PI));
    this.camera.aspect = this.width / this.height * (this.mapOpen ? .4 : 1);
    this.camera.rotation.set(Math.atan2(lerp(from.pitch, to.pitch) * 1.25, scale), -yaw * Math.PI / 1024, -lerp(from.roll, to.roll) * Math.PI / 1024, 'YXZ');
    this.camera.updateProjectionMatrix();
    this.camera.updateMatrixWorld();
    this.frustum.setFromProjectionMatrix(this.viewProjection.multiplyMatrices(this.camera.projectionMatrix, this.camera.matrixWorldInverse));
    for (const chunk of this.chunks) {
      chunk.mesh.position.set(this.camera.position.x + wrappedDelta(chunk.x * 4 + CHUNK * 2, this.camera.position.x) - CHUNK * 2,
        0, this.camera.position.z + wrappedDelta(chunk.y * 4 + CHUNK * 2, this.camera.position.z) - CHUNK * 2);
    }
    this.terrainMaterial.uniforms.eyeMap!.value.set(this.camera.position.x, this.camera.position.z);
    this.terrainMaterial.uniforms.turn!.value = Number(this.frame.tick % 2048n);
    let opaqueCount = 0; const transparent: WorldSprite[] = [];
    for (const row of this.frame.sprites) {
      if ([2, 3, 6, 7, 8].includes(row.pixelMode)) { transparent.push(row); continue; }
      if (this.writeSprite(this.sprites.geometry, opaqueCount, row, this.camera.position)) opaqueCount++;
    }
    this.sprites.geometry.instanceCount = opaqueCount; this.upload(this.sprites.geometry);
    this.renderer.info.reset(); this.renderer.setRenderTarget(this.target); this.renderer.setClearColor(0x000000, 1); this.renderer.clear();
    this.renderer.setViewport(0, 0, this.width, this.height);
    if (this.mapOpen) {
      const mapWidth = Math.floor(this.width * .6);
      this.renderer.setViewport(mapWidth, 0, this.width - mapWidth, this.height);
    }
    this.renderer.render(this.scene, this.camera); this.blendCopies = 0;
    const visible: { row: WorldSprite; rect: SpriteRect; layer: number }[] = [];
    for (const row of transparent) {
      if (!this.writeSprite(this.blendSprite.geometry, 0, row, this.camera.position)) continue;
      const rect = this.projectedRect(); if (rect) visible.push({ row, rect, layer: 0 });
    }
    // Disjoint sprites commute: batch them while preserving the painter order
    // of every overlapping pair. Copy only their screen rectangle, entirely on
    // GPU. No full-screen per-particle copy and no CPU framebuffer readback.
    visible.sort((a, b) => b.rect.depth - a.rect.depth || a.row.drawOrder - b.row.drawOrder);
    const layers: typeof visible[] = [];
    for (let i = 0; i < visible.length; i++) {
      const item = visible[i]!, a = item.rect;
      for (let j = 0; j < i; j++) {
        const prior = visible[j]!, b = prior.rect;
        if (a.left < b.right && a.right > b.left && a.bottom < b.top && a.top > b.bottom) item.layer = Math.max(item.layer, prior.layer + 1);
      }
      (layers[item.layer] ??= []).push(item);
    }
    for (const layer of layers) {
      let count = 0; this.copyBox.makeEmpty();
      for (const { row, rect } of layer) {
        if (this.writeSprite(this.blendSprite.geometry, count, row, this.camera.position)) count++;
        this.copyBox.min.x = Math.min(this.copyBox.min.x, rect.left); this.copyBox.min.y = Math.min(this.copyBox.min.y, rect.bottom);
        this.copyBox.max.x = Math.max(this.copyBox.max.x, rect.right); this.copyBox.max.y = Math.max(this.copyBox.max.y, rect.top);
      }
      this.blendSprite.geometry.instanceCount = count; this.upload(this.blendSprite.geometry);
      this.renderer.copyTextureToTexture(this.target.texture, this.snapshot, this.copyBox, this.copyBox.min);
      this.renderer.render(this.blendScene, this.camera); this.blendCopies++;
    }
    this.spriteCount = opaqueCount + visible.length;
    this.finalMaterial.uniforms.mapOpen!.value = this.mapOpen;
    this.renderer.setRenderTarget(null); this.renderer.setViewport(0, 0, this.width, this.height); this.renderer.render(this.finalScene, this.screenCamera);
    this.draws = this.renderer.info.render.calls; this.triangles = this.renderer.info.render.triangles; this.framesDrawn++;
    if (this.playing && this.lastDraw) { this.frameTimes.push(now - this.lastDraw); if (this.frameTimes.length > 1800) this.frameTimes.shift(); }
    this.lastDraw = this.playing ? now : 0;
    if (now - this.samplesAt > 500) {
      const sorted = [...this.frameTimes].sort((a, b) => a - b), percentile = (p: number) => sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * p))] ?? 0;
      Object.assign(window, { originalWorldMetrics: { renderer: 'threejs-local-geometry', gpu: this.gpu,
        frames: this.framesDrawn, samples: sorted.length, fps: sorted.length ? 1000 * sorted.length / sorted.reduce((a, b) => a + b, 0) : 0,
        p50_ms: percentile(.5), p95_ms: percentile(.95), p99_ms: percentile(.99), draw_calls: this.draws, triangles: this.triangles,
        sprites: this.spriteCount, blend_copies: this.blendCopies, terrain_chunks: this.chunks.length, rebuilt_chunks: this.rebuiltChunks,
        width: this.width, height: this.height, tick: this.frame.tick.toString(), terrain_revision: this.frame.terrainRevision.toString(),
        static_palette: this.frame.staticPalette } }); this.samplesAt = now;
    }
  }
}
