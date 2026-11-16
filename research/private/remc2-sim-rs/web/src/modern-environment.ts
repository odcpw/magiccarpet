import * as THREE from 'three';
import { wrappedDelta, type TerrainPlanes } from './original-world-data';
import { sourceWaterTypes, sourceWaterAt, modernLandform, modernSurfaceHeight } from './modern-terrain';

const SIDE = 256, CELL = 4, CHUNK = 32;
interface LandChunk { mesh: THREE.Mesh; x: number; z: number }
/** Presentation only. All collision, movement and terrain mutations remain native. */
export class ModernEnvironment {
  readonly moon: THREE.DirectionalLight;
  readonly sky: THREE.Mesh;
  private readonly chunks: LandChunk[] = [];
  revision = 0;
  private planes: TerrainPlanes | null = null;
  private visualHeights: Uint8Array | null = null;
  private surface: Float32Array | null = null;
  private readonly groundMaterial: THREE.MeshStandardMaterial;
  private readonly waterMaterial: THREE.ShaderMaterial;
  private readonly water: THREE.Mesh;
  private readonly heightTexture = new THREE.DataTexture(new Uint8Array(65536), SIDE, SIDE, THREE.RedFormat);
  private readonly waterMask = new THREE.DataTexture(new Uint8Array(65536), SIDE, SIDE, THREE.RedFormat);
  private waterTypes: Uint8Array | null = null;
  private readonly skyMaterial: THREE.ShaderMaterial;
  private readonly lights: THREE.PointLight[] = [];
  private readonly scene: THREE.Scene;
  private constructor(scene: THREE.Scene, textures: Record<string, THREE.Texture>) {
    this.scene = scene;
    this.heightTexture.minFilter = this.heightTexture.magFilter = THREE.LinearFilter;
    this.heightTexture.wrapS = this.heightTexture.wrapT = THREE.RepeatWrapping; this.heightTexture.needsUpdate = true;
    this.waterMask.minFilter = this.waterMask.magFilter = THREE.LinearFilter;
    this.waterMask.wrapS = this.waterMask.wrapT = THREE.RepeatWrapping; this.waterMask.needsUpdate = true;
    this.scene.fog = new THREE.FogExp2(0x34495e, .0028);
    this.scene.add(new THREE.HemisphereLight(0xb9d4f5, 0x363b34, 1.65));
    this.moon = new THREE.DirectionalLight(0xb8d6ff, 2.5); this.moon.castShadow = true;
    this.moon.shadow.mapSize.set(2048, 2048); this.moon.shadow.bias = -.00045; this.moon.shadow.normalBias = .1;
    const shadow = this.moon.shadow.camera; shadow.left = shadow.bottom = -65; shadow.right = shadow.top = 65; shadow.near = 1; shadow.far = 400;
    this.scene.add(this.moon, this.moon.target);
    this.groundMaterial = new THREE.MeshStandardMaterial({ color: 0xc8c2b2, roughness: .94 });
    this.groundMaterial.onBeforeCompile = program => {
      Object.assign(program.uniforms, { rockAlbedo: { value: textures.rockalbedo }, groundAlbedo: { value: textures.groundalbedo },
        rockNormal: { value: textures.rocknormal }, groundNormal: { value: textures.groundnormal },
        rockRoughness: { value: textures.rockroughness }, groundRoughness: { value: textures.groundroughness } });
      program.vertexShader = 'varying vec3 groundPosition; varying vec3 groundNormalWorld;\n' + program.vertexShader;
      program.vertexShader = program.vertexShader.replace('#include <worldpos_vertex>', `#include <worldpos_vertex>
        groundPosition=(modelMatrix*vec4(transformed,1.)).xyz;
        groundNormalWorld=normalize(mat3(modelMatrix)*objectNormal);`);
      program.fragmentShader = `uniform sampler2D rockAlbedo,groundAlbedo,rockNormal,groundNormal,rockRoughness,groundRoughness;
        varying vec3 groundPosition; varying vec3 groundNormalWorld;
        float landscapeHash(vec2 p){return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453);}
        float landscapeNoise(vec2 p){vec2 i=floor(p),f=fract(p);f=f*f*(3.-2.*f);
          return mix(mix(landscapeHash(i),landscapeHash(i+vec2(1.,0.)),f.x),mix(landscapeHash(i+vec2(0.,1.)),landscapeHash(i+1.),f.x),f.y);}
        ` + program.fragmentShader;
      program.fragmentShader = program.fragmentShader.replace('#include <map_fragment>', `
        vec3 terrainN=normalize(groundNormalWorld), p=groundPosition*.18;
        vec3 weights=max(abs(terrainN)-.2,0.);weights*=weights;weights*=weights;weights/=max(dot(weights,vec3(1.)),.001);
        vec3 rockColor=texture2D(rockAlbedo,p.yz).rgb*weights.x+texture2D(rockAlbedo,p.xz).rgb*weights.y+texture2D(rockAlbedo,p.xy).rgb*weights.z;
        vec3 earthColor=texture2D(groundAlbedo,p.xz*.82).rgb;
        float broad=landscapeNoise(groundPosition.xz*.018);
        float rockMix=clamp((.79-terrainN.y)*3.2+(broad-.5)*.3,0.,1.);
        float bed=1.-smoothstep(0.,2.8,groundPosition.y);
        diffuseColor.rgb*=mix(earthColor,rockColor,rockMix)*mix(.78,1.18,broad)*mix(1.,.58,bed);
      `);
      program.fragmentShader = program.fragmentShader.replace('#include <roughnessmap_fragment>', `
        float roughnessFactor=roughness*mix(texture2D(groundRoughness,p.xz*.82).r,texture2D(rockRoughness,p.xz).r,rockMix);
        roughnessFactor=mix(roughnessFactor,.36,bed*.65);
      `);
      // A restrained world-space detail normal avoids stretching a planar map
      // over cliffs. Source geometry and its collision surface are untouched.
      program.fragmentShader = program.fragmentShader.replace('#include <normal_fragment_maps>', `
        vec3 rnx=texture2D(rockNormal,p.yz).xyz*2.-1.,rny=texture2D(rockNormal,p.xz).xyz*2.-1.,rnz=texture2D(rockNormal,p.xy).xyz*2.-1.;
        vec3 rockDetail=normalize(vec3(rnx.z*sign(terrainN.x),rnx.x,rnx.y)*weights.x+
          vec3(rny.x,rny.z*sign(terrainN.y),rny.y)*weights.y+vec3(rnz.x,rnz.y,rnz.z*sign(terrainN.z))*weights.z);
        vec3 earthDetail=texture2D(groundNormal,p.xz*.82).xyz*2.-1.;
        vec3 worldDetail=mix(vec3(earthDetail.x,1.,earthDetail.y),rockDetail,rockMix);
        normal=normalize(normal+mat3(viewMatrix)*(worldDetail-terrainN)*.24);
      `);
    };
    this.groundMaterial.customProgramCacheKey = () => 'modern-level1-land/v1';
    const moonDirection = new THREE.Vector3(-.38, .48, .79).normalize();
    this.skyMaterial = new THREE.ShaderMaterial({ side: THREE.BackSide, depthWrite: false, uniforms: {
      time: { value: 0 }, moonDirection: { value: moonDirection } },
      vertexShader: 'varying vec3 ray; void main(){ray=position; gl_Position=projectionMatrix*modelViewMatrix*vec4(position,1.);gl_Position.z=gl_Position.w;}',
      fragmentShader: `precision highp float; uniform float time; uniform vec3 moonDirection; varying vec3 ray;
        float hash(vec2 p){return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453);}
        float noise(vec2 p){vec2 i=floor(p),f=fract(p);f=f*f*(3.-2.*f);return mix(mix(hash(i),hash(i+vec2(1.,0.)),f.x),mix(hash(i+vec2(0.,1.)),hash(i+1.),f.x),f.y);}
        void main(){vec3 d=normalize(ray);float h=max(d.y,0.);
          vec3 c=mix(vec3(.034,.067,.112),vec3(.004,.012,.033),pow(h,.42));
          vec2 wind=vec2(time*.006+sin(time*.011)*.8,time*.0015);
          vec2 p=d.xz/(h+.25)*2.8+wind;
          float cloud=noise(p)*.58+noise(p*2.03+4.7)*.28+noise(p*4.01)*.14;
          cloud=smoothstep(.46,.74,cloud)*smoothstep(-.02,.25,d.y);
          c=mix(c,vec3(.058,.082,.12),cloud*.82);
          float moon=dot(d,moonDirection);c+=vec3(.7,.82,1.)*(smoothstep(.9991,.9995,moon)*2.8+pow(max(moon,0.),140.)*.12)*(1.-cloud*.75);
          vec2 stars=d.xz/max(h,.04)*170.;vec2 cell=floor(stars),f=fract(stars)-.5;
          float star=step(.994,hash(cell))*pow(max(0.,1.-length(f)*3.),5.)*smoothstep(.08,.4,h)*(1.-cloud);
          c+=vec3(.55,.65,.8)*star;gl_FragColor=vec4(c,1.);
        }` });
    this.sky = new THREE.Mesh(new THREE.SphereGeometry(900, 32, 16), this.skyMaterial); this.sky.renderOrder = -100; this.sky.frustumCulled = false; this.scene.add(this.sky);
    this.waterMaterial = new THREE.ShaderMaterial({ transparent: true, depthWrite: false, uniforms: {
      time: { value: 0 }, ground: { value: this.heightTexture }, waterMask: { value: this.waterMask }, eye: { value: new THREE.Vector3() }, moonDirection: { value: moonDirection } },
      vertexShader: `uniform float time; varying vec3 waterPosition; varying vec3 waterNormal;
        void main(){vec3 p=position;vec2 world=(modelMatrix*vec4(p,1.)).xz;float t=time,wind=1.+sin(t*.035)*.15+sin(t*.013)*.1;
          float a=dot(world,vec2(.72,.35))*.15-t*.7,b=dot(world,vec2(-.28,.8))*.26-t*1.1;
          p.y+=.1+sin(a)*.13*wind+sin(b)*.045;
          waterNormal=normalize(vec3(-cos(a)*.13*.15*.72*wind-cos(b)*.045*.26*(-.28),1.,-cos(a)*.13*.15*.35*wind-cos(b)*.045*.26*.8));
          waterPosition=(modelMatrix*vec4(p,1.)).xyz;gl_Position=projectionMatrix*modelViewMatrix*vec4(p,1.);}`,
      fragmentShader: `precision highp float;uniform float time;uniform sampler2D ground,waterMask;uniform vec3 eye,moonDirection;
        varying vec3 waterPosition;varying vec3 waterNormal;
        float ripple(float phase){return sin(phase)*(1.-smoothstep(.4,2.,fwidth(phase)));}
        void main(){vec2 uv=fract(waterPosition.xz/1024.);
          float coverage=texture2D(waterMask,uv).r;if(coverage<.5)discard;
          // Let the actual triangle depth close the shoreline. A second,
          // bilinear height discard disagrees with the native diagonal and
          // produces holes. Filter small ripples across distant pixels.
          vec3 n=normalize(waterNormal+vec3(ripple(dot(waterPosition.xz,vec2(.47,.31))+time*.9)*.035,0.,ripple(dot(waterPosition.xz,vec2(-.23,.52))-time*.7)*.035));
          vec3 v=normalize(eye-waterPosition);float fresnel=.04+.96*pow(1.-max(dot(n,v),0.),5.);
          float reflection=pow(max(dot(reflect(-moonDirection,n),v),0.),130.);
          float nearLand=max(max(texture2D(ground,uv+vec2(.0015,0.)).r,texture2D(ground,uv-vec2(.0015,0.)).r),max(texture2D(ground,uv+vec2(0.,.0015)).r,texture2D(ground,uv-vec2(0.,.0015)).r));
          float foam=max(smoothstep(.002,.018,nearLand),1.-coverage)*(.17+.1*ripple(dot(waterPosition.xz,vec2(.51,.43))-time*.9));
          vec3 color=mix(vec3(.006,.025,.033),vec3(.034,.067,.112),fresnel)+vec3(.75,.84,1.)*reflection*1.5;
          color=mix(color,vec3(.28,.37,.4),max(0.,foam));float fog=1.-exp(-pow(length(eye-waterPosition)*.0028,2.));
          gl_FragColor=vec4(mix(color,vec3(.034,.067,.112),fog),.97);
        }` });
    const waterGeometry = new THREE.PlaneGeometry(1024, 1024, 160, 160); waterGeometry.rotateX(-Math.PI / 2);
    this.water = new THREE.Mesh(waterGeometry, this.waterMaterial); this.water.renderOrder = 3; this.scene.add(this.water);
    for (let i = 0; i < 8; i++) { const light = new THREE.PointLight(0xff9c45, 0, 22, 2); this.lights.push(light); this.scene.add(light); }
  }
  static async load(scene: THREE.Scene, base: string): Promise<ModernEnvironment> {
    const loader = new THREE.TextureLoader(), entries = await Promise.all(['groundalbedo', 'groundnormal', 'groundroughness', 'rockalbedo', 'rocknormal', 'rockroughness'].map(async key => {
      const texture = await loader.loadAsync(base + 'world/' + key + '.jpg');
      texture.wrapS = texture.wrapT = THREE.RepeatWrapping; texture.anisotropy = 4;
      texture.colorSpace = key.endsWith('albedo') ? THREE.SRGBColorSpace : THREE.NoColorSpace;
      return [key, texture] as const;
    }));
    return new ModernEnvironment(scene, Object.fromEntries(entries));
  }
  setSourceAtlas(atlas: Uint8Array, count: number): void { this.waterTypes = sourceWaterTypes(atlas, count); }
  isWater(x: number, z: number): boolean {
    return !!this.planes && !!this.waterTypes && sourceWaterAt(this.planes, ((Math.floor(z / 4) & 255) * 256) + (Math.floor(x / 4) & 255), this.waterTypes);
  }
  private geometry(x0: number, z0: number, surface: Float32Array): THREE.BufferGeometry {
    const side = CHUNK + 1, p = new Float32Array(side * side * 3), n = new Float32Array(p.length), uv = new Float32Array(side * side * 2), indices: number[] = [];
    // The wave bit is an eroded interior mask; source type/texture coverage
    // distinguishes calm sea from genuinely dry zero-height village ground.
    const h = (x: number, z: number) => surface[(z & 255) * SIDE + (x & 255)]!;
    for (let z = 0; z <= CHUNK; z++) for (let x = 0; x <= CHUNK; x++) {
      const i = z * side + x, gx = x0 + x, gz = z0 + z;
      p.set([x * CELL, h(gx, gz), z * CELL], i * 3);
      const nx = h(gx - 1, gz) - h(gx + 1, gz), nz = h(gx, gz - 1) - h(gx, gz + 1), length = Math.hypot(nx, 8, nz);
      n.set([nx / length, 8 / length, nz / length], i * 3); uv.set([gx * .5, gz * .5], i * 2);
      if (x < CHUNK && z < CHUNK) { const a = i, b = i + 1, c = i + side, d = c + 1;
        if (((gx + gz) & 1) === 0) indices.push(a, c, d, a, d, b); else indices.push(a, c, b, b, c, d); }
    }
    const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.BufferAttribute(p, 3)); g.setAttribute('normal', new THREE.BufferAttribute(n, 3));
    g.setAttribute('uv', new THREE.BufferAttribute(uv, 2)); g.setIndex(indices); g.computeBoundingSphere(); return g;
  }
  updateTerrain(planes: TerrainPlanes, visualHeights: Uint8Array): void {
    if (!this.waterTypes) throw new Error('Source shoreline atlas is missing');
    // A native publication/mutation sequence may advance without changing
    // terrain. Reconstruct and rebuild scatter only for changed inputs.
    if (this.planes && this.visualHeights) {
      let changed = false;
      for (let i = 0; i < 65536; i++) if (visualHeights[i] !== this.visualHeights[i] || planes.heights[i] !== this.planes.heights[i] ||
        planes.types[i] !== this.planes.types[i] || ((planes.angles[i]! ^ this.planes.angles[i]!) & 8)) { changed = true; break; }
      if (!changed) return;
    }
    const surface = modernLandform(planes, visualHeights, this.waterTypes);
    if (!this.planes) for (let z = 0; z < SIDE; z += CHUNK) for (let x = 0; x < SIDE; x += CHUNK) {
      const mesh = new THREE.Mesh(this.geometry(x, z, surface), this.groundMaterial); mesh.castShadow = mesh.receiveShadow = true;
      this.scene.add(mesh); this.chunks.push({ mesh, x, z });
    } else for (const chunk of this.chunks) {
      let changed = false;
      for (let z = chunk.z - 1; z <= chunk.z + CHUNK + 1 && !changed; z++) for (let x = chunk.x - 1; x <= chunk.x + CHUNK + 1; x++) {
        const at = (z & 255) * SIDE + (x & 255);
        if (surface[at] !== this.surface![at]) { changed = true; break; }
      }
      if (changed) { chunk.mesh.geometry.dispose(); chunk.mesh.geometry = this.geometry(chunk.x, chunk.z, surface); }
    }
    this.planes = planes; this.visualHeights = visualHeights; this.surface = surface;
    (this.heightTexture.image.data as Uint8Array).set(visualHeights); this.heightTexture.needsUpdate = true;
    const mask = this.waterMask.image.data as Uint8Array;
    for (let i = 0; i < mask.length; i++) mask[i] = sourceWaterAt(planes, i, this.waterTypes) ? 255 : 0;
    this.waterMask.needsUpdate = true;
    this.revision++;
  }
  height(x: number, z: number): number { return this.surface ? modernSurfaceHeight(this.surface, x, z) : 0; }
  present(camera: THREE.PerspectiveCamera, time: number): void {
    for (const chunk of this.chunks) chunk.mesh.position.set(camera.position.x + wrappedDelta(chunk.x * CELL + CHUNK * 2, camera.position.x) - CHUNK * 2,
      0, camera.position.z + wrappedDelta(chunk.z * CELL + CHUNK * 2, camera.position.z) - CHUNK * 2);
    this.sky.position.copy(camera.position); this.water.position.set(Math.round(camera.position.x / 64) * 64, 0, Math.round(camera.position.z / 64) * 64);
    this.skyMaterial.uniforms.time!.value = this.waterMaterial.uniforms.time!.value = time;
    this.waterMaterial.uniforms.eye!.value.copy(camera.position);
    this.moon.position.copy(camera.position).add(new THREE.Vector3(-76, 96, 158)); this.moon.target.position.copy(camera.position);
  }
  fireLights(positions: readonly THREE.Vector3[]): void {
    for (let i = 0; i < this.lights.length; i++) { const light = this.lights[i]!; const point = positions[i];
      light.intensity = point ? 45 : 0; if (point) light.position.copy(point); }
  }
}
