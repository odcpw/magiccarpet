import * as THREE from 'three';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';
import { clone } from 'three/addons/utils/SkeletonUtils.js';
import type { ModernCharacter, ModernFrame } from './modern-runtime';

interface ClipInfo { name: string; duration: number; loop: boolean }
interface CharacterAsset { url: string; sha256: string; bytes: number; height: number; forward_yaw: number; walk_speed: number; run_speed: number; clips: ClipInfo[] }
interface CharacterManifest { schema: string; assets: Record<string, CharacterAsset> }
export interface ModernCharacterMetrics {
  actorCount: number; arrowCount: number;
  resourceCounts: Record<string, number>; clipCounts: Record<string, number>;
  actors: ReadonlyArray<{slot: number; sourceId: number; generation: number; resource: string; clip: string;
    objectName: string; visible: boolean; action: number; phase: number; x: number; y: number; z: number}>;
  arrows: ReadonlyArray<{slot: number; sourceId: number; generation: number; objectName: string; visible: boolean}>;
}
interface Art { definition: CharacterAsset; scene: THREE.Group; clips: Map<string, THREE.AnimationClip>; bounds: THREE.Box3; attackYaw?: number; arrow?: THREE.Group; nockIndices?: number[] }
interface Arrow { root: THREE.Group; generation: number; sourceId: number; from: THREE.Vector3; to: THREE.Vector3; offset: THREE.Vector3; birthElapsed: number; elapsed: number; interval: number; yaw: number; pitch: number; seen: boolean; active: boolean }
interface Actor {
  slot: number; generation: number; sourceId: number; resource: string; state: ModernCharacter;
  root: THREE.Group; model: THREE.Object3D; mixer: THREE.AnimationMixer; actions: Map<string, THREE.AnimationAction>;
  clip: string; from: THREE.Vector3; to: THREE.Vector3; interpolated: THREE.Vector3;
  fromYaw: number; toYaw: number; elapsed: number; interval: number; scale: number; bottom: number;
  observed: boolean; dying: boolean; deathTime: number; hit: number; attack: number; release: number;
  lastAction: number; seen: boolean; tickTravel: number; animationTravel: number; animationTime: number;
  ownerColor: number; heldArrow?: THREE.Object3D; bowstring?: THREE.Mesh; rendered: boolean;
  movingGrace: number; recentSpeed: number; releaseYaw: number;
}
const appearances = new Map<number, [number, string, number]>([
  [221, [12, 'Builder', 250]], [219, [14, 'Trader', 250]], [239, [13, 'VillagerBrown', 200]],
  [241, [13, 'Villager', 250]], [242, [13, 'VillagerStriped', 250]], [271, [13, 'VillagerCarrier', 300]],
]);
const period = 1024;
const delta = (a: number, b: number) => THREE.MathUtils.euclideanModulo(b - a + period / 2, period) - period / 2;
const yawDelta = (a: number, b: number) => THREE.MathUtils.euclideanModulo(b - a + Math.PI, Math.PI * 2) - Math.PI;

/** Source admission mirrors ModernCharacterActors and ModernVillagerAppearances. */
export function characterResource(s: ModernCharacter): string | undefined {
  if (s.classId !== 5) return undefined;
  if (s.model === 1) return 'Goat';
  if (s.model === 4) return 'Archer';
  if (s.model === 19) return 'Firefly';
  if (s.model === 3 && s.renderParameter === 88) return 'WormHead';
  if (s.model === 3 && s.renderParameter >= 89 && s.renderParameter <= 104 && s.hasParent) return 'WormSegment';
  const appearance = appearances.get(s.renderParameter);
  return appearance?.[0] === s.model ? appearance[1] : undefined;
}

/** Zero life remains alive. A positive-life villager entry shares action 108 with death. */
export function sourceDeath(s: ModernCharacter): boolean {
  if (s.life < 0) return true;
  switch (s.model) {
    case 1: return s.action === 12 || s.action === 13;
    case 4: return s.action === 36 || s.action === 37;
    case 3: return s.action === 28 || s.action === 29;
    case 19: return s.action === 156 || s.action === 157;
    case 12: return s.action === 100 || s.action === 101;
    case 13: return s.action === 109 || (s.action === 108 && !(s.lifecycleFlags & 1));
    case 14: return s.action === 117;
    default: return false;
  }
}
function sourceHit(s: ModernCharacter): boolean {
  return (s.model === 12 && (s.action === 102 || s.action === 110)) ||
    (s.model === 13 && s.action === 110) || (s.model === 14 && s.action === 118);
}
function sourceHeight(s: ModernCharacter, resource: string): number {
  if (resource === 'Goat') return 150 / 64;
  if (resource === 'Archer') return 250 / 64;
  return (appearances.get(s.renderParameter)?.[2] ?? s.renderHeight) / 64;
}

/** One native generation owns each instance; rendering never advances simulation. */
export class ModernCharacters {
  private readonly actors = new Map<number, Actor>();
  private readonly arrows = new Map<number, Arrow>();
  private readonly wormHeads = new Map<number, ModernCharacter>();
  private readonly ownerMaterials = new Map<THREE.Material, Map<number, THREE.Material>>();
  private readonly originalMaterials = new WeakMap<THREE.Material, THREE.Material>();
  private epoch: bigint | undefined;
  private tick: bigint | undefined;
  private readonly cameraPosition = new THREE.Vector3();
  private readonly up = new THREE.Vector3(0, 1, 0);
  private readonly terrainNormal = new THREE.Vector3();
  private readonly terrainRotation = new THREE.Quaternion();
  private readonly yawRotation = new THREE.Quaternion();
  private readonly identityRotation = new THREE.Quaternion();
  private readonly parent: THREE.Object3D;
  private readonly assets: Map<string, Art>;
  private constructor(parent: THREE.Object3D, assets: Map<string, Art>) { this.parent = parent; this.assets = assets; }

  static async load(parent: THREE.Object3D, assetBaseUrl: string): Promise<ModernCharacters> {
    const base = new URL(assetBaseUrl, window.location.href);
    const response = await fetch(new URL('characters/manifest.json', base));
    if (!response.ok) throw new Error(`character manifest: ${response.status}`);
    const manifest = await response.json() as CharacterManifest;
    if (manifest.schema !== 'mc-browser-modern-characters/v1') throw new Error('unsupported character manifest');
    for (const name of ['Goat','Archer','Villager','VillagerBrown','VillagerStriped','VillagerCarrier',
      'Builder','Trader','Firefly','WormHead','WormSegment','Collector']) {
      if (!manifest.assets?.[name]) throw new Error(`character manifest is missing ${name}`);
    }
    const loader = new GLTFLoader();
    const entries = await Promise.all(Object.entries(manifest.assets).map(async ([name, definition]) => {
      const gltf = await loader.loadAsync(new URL(definition.url, base).href);
      const clips = new Map(gltf.animations.map(clip => [clip.name, clip]));
      for (const expected of definition.clips) if (!clips.has(expected.name)) throw new Error(`${name}: missing ${expected.name}`);
      // GLTFLoader and SkeletonUtils share geometry/materials; skin state belongs to each clone.
      const probe = clone(gltf.scene), probeMixer = new THREE.AnimationMixer(probe);
      const idle = probeMixer.clipAction(clips.get('Idle')!); idle.play(); probeMixer.update(0);
      probe.updateMatrixWorld(true);
      const bounds = new THREE.Box3().setFromObject(probe, true);
      if (bounds.isEmpty() || !Number.isFinite(bounds.max.y - bounds.min.y) || bounds.max.y <= bounds.min.y)
        throw new Error(`${name}: invalid imported bounds`);
      let attackYaw: number | undefined, arrow: THREE.Group | undefined, nockIndices: number[] | undefined;
      if (name === 'Archer') {
        const control = probe.getObjectByName('ArrowControl');
        const string = probe.getObjectByName('Bowstring');
        if (!control || !(string instanceof THREE.Mesh)) throw new Error('Archer is missing bow or arrow geometry');
        idle.stop(); const shot = probeMixer.clipAction(clips.get('StandingShot')!); shot.play();
        const sample = (time: number) => { shot.time = time; probeMixer.update(0); probe.updateMatrixWorld(true); };
        // Use the visible shaft's actual tip-to-fletching direction. The FBX
        // control's sampled translation can point the opposite way and is not
        // a reliable proxy for the axis of the authored arrow geometry.
        sample(2.35);
        const tip = control.getObjectByName('Arrowhead'), tail = control.getObjectByName('Arrow_fletching');
        if (!tip || !tail) throw new Error('Archer arrow lacks its tip or fletching');
        const direction = new THREE.Box3().setFromObject(tip, true).getCenter(new THREE.Vector3())
          .sub(new THREE.Box3().setFromObject(tail, true).getCenter(new THREE.Vector3())).normalize();
        if (direction.lengthSq() < .99) throw new Error('Archer shot has no measurable shaft direction');
        attackYaw = Math.atan2(direction.x, direction.z);
        const positions = string.geometry.getAttribute('position');
        string.geometry.computeBoundingBox();
        const size = string.geometry.boundingBox!.getSize(new THREE.Vector3());
        const axis = size.x > size.y && size.x > size.z ? 0 : size.y > size.z ? 1 : 2;
        const middle = (string.geometry.boundingBox!.min.getComponent(axis) + string.geometry.boundingBox!.max.getComponent(axis)) / 2;
        let nearest = Infinity;
        for (let i = 0; i < positions.count; i++) nearest = Math.min(nearest, Math.abs(positions.getComponent(i, axis) - middle));
        const nock = new THREE.Vector3(), vertex = new THREE.Vector3(); let count = 0; nockIndices = [];
        for (let i = 0; i < positions.count; i++) if (Math.abs(positions.getComponent(i, axis) - middle) < nearest + .0001) {
          string.getVertexPosition(i, vertex); string.localToWorld(vertex); nock.add(vertex); count++; nockIndices.push(i);
        }
        if (!count) throw new Error('Archer bowstring has no nock vertices');
        nock.divideScalar(count);
        const align = new THREE.Quaternion().setFromUnitVectors(direction, new THREE.Vector3(0, 0, 1));
        arrow = new THREE.Group();
        const geometry = control.clone(true);
        geometry.position.copy(control.getWorldPosition(new THREE.Vector3()).sub(nock).applyQuaternion(align));
        geometry.quaternion.copy(align).multiply(control.getWorldQuaternion(new THREE.Quaternion()));
        geometry.scale.copy(control.getWorldScale(new THREE.Vector3()));
        arrow.add(geometry);
      }
      probeMixer.stopAllAction(); probeMixer.uncacheRoot(probe);
      return [name, { definition, scene:gltf.scene, clips, bounds, attackYaw, arrow, nockIndices }] as const;
    }));
    return new ModernCharacters(parent, new Map(entries));
  }

  private make(state: ModernCharacter, resource: string): Actor {
    const art = this.assets.get(resource);
    if (!art) throw new Error(`missing character art ${resource}`);
    const root = new THREE.Group(), model = clone(art.scene);
    root.name = `MCCharacter:${resource}:${state.entityIndex}:${state.sourceId}:${state.slotGeneration}`;
    root.add(model); this.parent.add(root);
    const mixer = new THREE.AnimationMixer(model), actions = new Map<string, THREE.AnimationAction>();
    for (const info of art.definition.clips) {
      const clip = art.clips.get(info.name)!;
      const action = mixer.clipAction(clip);
      action.setLoop(info.loop ? THREE.LoopRepeat : THREE.LoopOnce, info.loop ? Infinity : 1);
      action.clampWhenFinished = !info.loop;
      actions.set(info.name, action);
    }
    const scale = sourceHeight(state, resource) / (art.bounds.max.y - art.bounds.min.y);
    model.scale.setScalar(scale);
    const position = new THREE.Vector3(state.x / 64, state.z / 64, state.y / 64);
    const yaw = art.definition.forward_yaw - state.yaw * Math.PI * 2 / 2048;
    return {slot:state.entityIndex, generation:state.slotGeneration, sourceId:state.sourceId, resource, state,
      root, model, mixer, actions, clip:'', from:position.clone(), to:position.clone(), interpolated:position.clone(),
      fromYaw:yaw, toYaw:yaw, elapsed:0, interval:1/30, scale, bottom:art.bounds.min.y * scale,
      observed:false, dying:false, deathTime:0, hit:0, attack:0, release:0, lastAction:state.action,
      seen:true, tickTravel:0, animationTravel:0, animationTime:0, ownerColor:-1,
      heldArrow:resource === 'Archer' ? model.getObjectByName('ArrowControl') : undefined,
      bowstring:resource === 'Archer' ? model.getObjectByName('Bowstring') as THREE.Mesh : undefined,
      rendered:false, movingGrace:0, recentSpeed:0, releaseYaw:0};
  }

  private setCollectorColor(actor: Actor, owner: number): void {
    const colors = [[.66,.82,.95],[.73,.12,.15],[.20,.45,.18],[.16,.43,.68],
      [.46,.23,.50],[.57,.29,.31],[.12,.30,.32],[.73,.38,.09]];
    const rgb = colors[owner]; if (!rgb || actor.ownerColor === owner) return;
    actor.ownerColor = owner;
    const tint = (input: THREE.Material): THREE.Material => {
      const base = this.originalMaterials.get(input) ?? input;
      const name = base.name.replaceAll('_', ' ').toLowerCase();
      const factor = name.includes('fabric seams') ? .55 : name.includes('owner embroidery') ? .72 : 1;
      if (!(name.includes('owner fabric') || name.includes('fabric seams') || name.includes('owner embroidery') ||
        name.includes('owner crown') || name.includes('owner band'))) return base;
      let variants = this.ownerMaterials.get(base);
      if (!variants) { variants = new Map(); this.ownerMaterials.set(base, variants); }
      let material = variants.get(owner);
      if (!material) {
        material = base.clone();
        if (material instanceof THREE.MeshStandardMaterial) {
          if (name.includes('owner crown')) material.color.setRGB(.12,.38,.57);
          else if (name.includes('owner band')) material.color.setRGB(.018,.028,.044);
          else material.color.setRGB(rgb[0]! * factor, rgb[1]! * factor, rgb[2]! * factor);
          if (name.includes('owner embroidery')) material.emissive.setRGB(rgb[0]! * .08, rgb[1]! * .08, rgb[2]! * .08);
        }
        this.originalMaterials.set(material, base); variants.set(owner, material);
      }
      return material;
    };
    actor.model.traverse(object => { if (object instanceof THREE.Mesh) {
      object.material = Array.isArray(object.material) ? object.material.map(tint) : tint(object.material);
    }});
  }

  observe(frame: ModernFrame): void {
    if (this.epoch !== frame.epoch || (this.tick !== undefined && frame.tick < this.tick)) this.clear();
    this.epoch = frame.epoch;
    if (this.tick === frame.tick) return;
    const interval = this.tick === undefined ? 1/30 : Math.max(1/30, Math.min(1, Number(frame.tick - this.tick) / 30));
    this.tick = frame.tick;
    for (const actor of this.actors.values()) actor.seen = false;
    for (const arrow of this.arrows.values()) arrow.seen = false;
    this.wormHeads.clear();
    for (const s of frame.characters) if (s.classId === 5 && s.model === 3 && s.renderParameter === 88)
      this.wormHeads.set(s.entityIndex, s);
    for (const s of frame.characters) {
      const resource = characterResource(s);
      if (!resource) continue;
      if (resource === 'WormSegment') {
        const head = this.wormHeads.get(s.parentIndex);
        if (!head || head.sourceId !== s.parentId || head.slotGeneration !== s.parentGeneration || sourceDeath(head)) continue;
      }
      let actor = this.actors.get(s.entityIndex);
      if (actor && (actor.generation !== s.slotGeneration || actor.sourceId !== s.sourceId || actor.resource !== resource)) {
        this.destroy(actor); this.actors.delete(s.entityIndex); actor = undefined;
      }
      if (!actor) { actor = this.make(s, resource); this.actors.set(s.entityIndex, actor); }
      actor.seen = true;
      const art = this.assets.get(resource)!;
      actor.from.copy(actor.interpolated);
      actor.to.set(s.x / 64, s.z / 64, s.y / 64);
      actor.tickTravel = Math.hypot(delta(actor.from.x, actor.to.x), delta(actor.from.z, actor.to.z));
      actor.fromYaw += yawDelta(actor.fromYaw, actor.toYaw) * Math.min(1, actor.elapsed / actor.interval);
      actor.toYaw = art.definition.forward_yaw - s.yaw * Math.PI * 2 / 2048;
      actor.elapsed = 0; actor.interval = interval;
      if (sourceDeath(s)) actor.dying = true;
      else if (!actor.dying && ((actor.observed && s.life < actor.state.life) || (sourceHit(s) && (!actor.observed || s.action !== actor.lastAction)))) {
        actor.hit = actor.actions.get('Hit')?.getClip().duration ?? 0;
      }
      if ((resource === 'Firefly' && s.action === 154 || resource === 'WormHead' && s.action === 26) && (!actor.observed || actor.lastAction !== s.action))
        actor.attack = actor.actions.get('Attack')?.getClip().duration ?? 0;
      actor.lastAction = s.action; actor.state = s; actor.observed = true;
    }
    for (const child of frame.castleChildren) {
      let actor = this.actors.get(child.entityIndex);
      if (actor && (actor.generation !== child.slotGeneration || actor.sourceId !== child.sourceId || actor.resource !== 'Collector')) {
        this.destroy(actor); this.actors.delete(child.entityIndex); actor = undefined;
      }
      if (!actor) {
        const state: ModernCharacter = {slotGeneration:child.slotGeneration, parentGeneration:child.castleGeneration,
          entityIndex:child.entityIndex, sourceId:child.sourceId, parentIndex:child.ownerEntityIndex, parentId:0,
          classId:child.classId, model:child.model, action:child.action, phase:child.phase, active:child.active,
          hasTarget:0, hasParent:1, stance:0, x:child.x, y:child.y, z:child.z, yaw:child.yaw, pitch:0,
          speed:child.speed, life:child.life, targetIndex:child.targetIndex, targetX:0, targetY:0, targetZ:0,
          controlFlags:child.controlFlags, ageTicks:0, renderParameter:child.renderParameter, roll:0,
          sourceHalfWidth:0, renderHeight:child.renderHeight, previousIndex:0, nextIndex:0, partIndex:0, lifecycleFlags:0};
        actor = this.make(state, 'Collector'); this.actors.set(child.entityIndex, actor);
      }
      actor.seen = true; actor.from.copy(actor.interpolated);
      actor.to.set(child.x / 64, child.z / 64, child.y / 64);
      actor.tickTravel = Math.hypot(delta(actor.from.x, actor.to.x), delta(actor.from.z, actor.to.z));
      actor.fromYaw += yawDelta(actor.fromYaw, actor.toYaw) * Math.min(1, actor.elapsed / actor.interval);
      actor.toYaw = this.assets.get('Collector')!.definition.forward_yaw - child.yaw * Math.PI * 2 / 2048;
      actor.elapsed = 0; actor.interval = interval;
      actor.state.active = Number(child.drawing !== 0 && (child.relationFlags & 1) !== 0 && child.ownerPlayerColor < 8);
      actor.state.controlFlags = child.controlFlags;
      this.setCollectorColor(actor, child.ownerPlayerColor);
    }
    // A release exists only when native allocated a fresh child arrow.
    for (const s of frame.characters) if (s.classId === 9 && s.model === 13) {
      let arrow = this.arrows.get(s.entityIndex);
      if (arrow && (arrow.generation !== s.slotGeneration || arrow.sourceId !== s.sourceId)) {
        arrow.root.removeFromParent(); this.arrows.delete(s.entityIndex); arrow = undefined;
      }
      const fresh = !arrow;
      if (!arrow) {
        const art = this.assets.get('Archer')!, root = art.arrow!.clone(true);
        root.name = `MCArrow:${s.entityIndex}:${s.sourceId}:${s.slotGeneration}`;
        root.scale.setScalar((250 / 64) / (art.bounds.max.y - art.bounds.min.y));
        this.parent.add(root);
        const position = new THREE.Vector3(s.x / 64, s.z / 64, s.y / 64);
        arrow = {root, generation:s.slotGeneration, sourceId:s.sourceId, from:position.clone(), to:position,
          offset:new THREE.Vector3(), birthElapsed:0, elapsed:0, interval, yaw:s.yaw, pitch:s.pitch, seen:true, active:!!s.active};
        this.arrows.set(s.entityIndex, arrow);
      }
      arrow.seen = true;
      const t = Math.min(1, arrow.elapsed / arrow.interval);
      arrow.from.set(arrow.from.x + delta(arrow.from.x, arrow.to.x) * t,
        THREE.MathUtils.lerp(arrow.from.y, arrow.to.y, t), arrow.from.z + delta(arrow.from.z, arrow.to.z) * t);
      arrow.to.set(s.x / 64, s.z / 64, s.y / 64); arrow.elapsed = 0; arrow.interval = interval;
      arrow.yaw = s.yaw; arrow.pitch = s.pitch; arrow.active = !!s.active;
      if (fresh && s.hasParent && s.ageTicks <= 1) {
        const parent = this.actors.get(s.parentIndex);
        if (parent?.resource === 'Archer' && parent.generation === s.parentGeneration && parent.sourceId === s.parentId) {
          parent.release = .2; parent.releaseYaw = s.yaw;
          const indices = this.assets.get('Archer')?.nockIndices;
          if (parent.rendered && parent.bowstring && indices?.length) {
            const nock = new THREE.Vector3(), vertex = new THREE.Vector3();
            parent.model.updateWorldMatrix(true, true);
            for (const index of indices) { parent.bowstring.getVertexPosition(index, vertex); parent.bowstring.localToWorld(vertex); nock.add(vertex); }
            nock.divideScalar(indices.length);
            arrow.offset.set(delta(arrow.from.x, nock.x), nock.y - arrow.from.y, delta(arrow.from.z, nock.z));
          }
        }
      }
    }
    for (const [slot, arrow] of this.arrows) if (!arrow.seen) { arrow.root.removeFromParent(); this.arrows.delete(slot); }
    for (const [slot, actor] of this.actors) if (!actor.seen && !actor.dying) { this.destroy(actor); this.actors.delete(slot); }
  }

  /** Render relative to camera using the 1024-unit torus. terrainHeight is the visual ground sampler. */
  render(camera: THREE.Camera, dt: number, terrainHeight?: (x: number, z: number) => number): void {
    if (!Number.isFinite(dt) || dt < 0) throw new Error('invalid character frame interval');
    camera.getWorldPosition(this.cameraPosition);
    for (const [slot, a] of this.actors) {
      const previous = Math.min(1, a.elapsed / a.interval);
      a.elapsed += dt;
      const t = Math.min(1, a.elapsed / a.interval);
      const frameTravel = a.tickTravel * (t - previous);
      a.animationTravel += frameTravel; a.animationTime += dt;
      if (frameTravel > .00001) { a.movingGrace = .08; if (dt > .000001) a.recentSpeed = frameTravel / dt / a.scale; }
      else a.movingGrace = Math.max(0, a.movingGrace - dt);
      a.interpolated.set(a.from.x + delta(a.from.x, a.to.x) * t, THREE.MathUtils.lerp(a.from.y, a.to.y, t),
        a.from.z + delta(a.from.z, a.to.z) * t);
      const x = this.cameraPosition.x + delta(this.cameraPosition.x, a.interpolated.x);
      const z = this.cameraPosition.z + delta(this.cameraPosition.z, a.interpolated.z);
      const grounded = a.resource !== 'Firefly' && a.resource !== 'Collector' && !a.resource.startsWith('Worm');
      const y = grounded && terrainHeight ? terrainHeight(x, z) : a.interpolated.y;
      a.root.position.set(x, y - a.bottom, z);
      const yaw = a.fromYaw + yawDelta(a.fromYaw, a.toYaw) * t;
      const dx = x - this.cameraPosition.x, dz = z - this.cameraPosition.z;
      a.root.visible = dx * dx + dz * dz < 350 * 350 && (a.dying || !!a.state.active &&
        (a.resource === 'Collector' || !(a.state.controlFlags & 0x421)));
      if (grounded && terrainHeight && a.root.visible) {
        // Mirror ModernCharacterActors.Place: fit the local ground normal,
        // retaining its separate Goat and human maximum terrain slopes.
        const e = .65;
        this.terrainNormal.set(terrainHeight(x - e, z) - terrainHeight(x + e, z), 2 * e,
          terrainHeight(x, z - e) - terrainHeight(x, z + e)).normalize();
        const angle = Math.acos(THREE.MathUtils.clamp(this.terrainNormal.y, -1, 1));
        const limit = (a.resource === 'Goat' ? 65 : 35) * Math.PI / 180;
        this.terrainRotation.setFromUnitVectors(this.up, this.terrainNormal);
        if (angle > limit) this.terrainRotation.slerp(this.identityRotation, 1 - limit / angle);
        this.yawRotation.setFromAxisAngle(this.up, yaw);
        a.root.quaternion.copy(this.terrainRotation).multiply(this.yawRotation);
      } else a.root.rotation.set(0, yaw, 0);
      if (a.dying) {
        a.deathTime += dt;
        if (!a.seen && a.deathTime > (a.actions.get('Death')?.getClip().duration ?? 0) + .5) {
          this.destroy(a); this.actors.delete(slot); continue;
        }
      }
      a.hit = Math.max(0, a.hit - dt); a.attack = Math.max(0, a.attack - dt); a.release = Math.max(0, a.release - dt);
      if (!a.root.visible) { a.animationTravel = 0; a.animationTime = 0; continue; }
      if (dx * dx + dz * dz > 120 * 120 && a.animationTime < .12) continue;
      const animationTime = a.animationTime, animationTravel = a.animationTravel;
      a.animationTime = 0; a.animationTravel = 0;
      a.rendered = true;
      const moving = a.movingGrace > 0;
      let name = a.dying ? 'Death' : a.hit > 0 && !moving ? 'Hit' : a.attack > 0 ? 'Attack' :
        a.resource === 'Firefly' ? (moving ? 'Flight' : 'Idle') :
        a.resource === 'Archer' && a.state.action === 34 ? (a.state.stance === 1 ? 'KneelingShot' : 'StandingShot') :
        moving ? (a.resource === 'Goat' && a.state.action === 14 ? 'Sprint' :
          a.actions.has('Run') && a.recentSpeed > (this.assets.get(a.resource)!.definition.walk_speed * 1.35) ? 'Run' : 'Walk') : 'Idle';
      if (!a.actions.has(name)) name = 'Idle';
      if (name !== a.clip) { if (a.clip) a.actions.get(a.clip)?.stop(); a.actions.get(name)?.reset().play(); a.clip = name; }
      const action = a.actions.get(name);
      if (!action) continue;
      const art = this.assets.get(a.resource)!;
      if ((name === 'StandingShot' || name === 'KneelingShot') && !a.dying) {
        const phase = a.state.phase, remaining = phase <= 240 ? (30 - phase % 30) % 30 : 256 - phase;
        const interval = phase > 240 || phase === 0 ? 16 : 30;
        const progress = THREE.MathUtils.clamp(1 - (remaining - Math.min(1, a.elapsed * 30)) / interval, 0, 1);
        const release = name === 'KneelingShot' ? 1.05 : 3.15;
        action.time = a.release > 0 ? Math.min(action.getClip().duration, release + (.2 - a.release) * 3) : Math.min(release - 1/96, release * progress);
        a.mixer.update(0);
        if (art.attackYaw !== undefined) {
          const heading = Math.PI - (a.release > 0 ? a.releaseYaw : a.state.yaw) * Math.PI * 2 / 2048;
          const tx = a.state.hasTarget && !a.release ? delta(a.interpolated.x, a.state.targetX / 64) : Math.sin(heading);
          const tz = a.state.hasTarget && !a.release ? delta(a.interpolated.z, a.state.targetY / 64) : Math.cos(heading);
          if (tx * tx + tz * tz > .000001) a.model.rotation.y = Math.atan2(tx, tz) - yaw - art.attackYaw;
        }
        if (a.heldArrow) a.heldArrow.visible = a.release <= 0;
      } else if (moving && ['Walk', 'Run', 'Sprint'].includes(name)) {
        a.model.rotation.y = 0;
        const distance = animationTravel;
        const speed = name === 'Run' ? art.definition.run_speed : art.definition.walk_speed;
        const advance = a.resource === 'Goat' ? distance / ((.16 / (name === 'Sprint' ? .075 : .18)) * a.scale) * action.getClip().duration :
          speed > 0 ? distance / (speed * a.scale) : 0;
        a.mixer.update(advance);
      } else { a.model.rotation.y = 0; a.mixer.update(animationTime); }
      if (a.resource === 'Archer' && name !== 'StandingShot' && name !== 'KneelingShot') {
        if (a.heldArrow) a.heldArrow.visible = true;
      }
    }
    for (const arrow of this.arrows.values()) {
      arrow.elapsed += dt; arrow.birthElapsed += dt; const t = Math.min(1, arrow.elapsed / arrow.interval);
      const x = arrow.from.x + delta(arrow.from.x, arrow.to.x) * t;
      const z = arrow.from.z + delta(arrow.from.z, arrow.to.z) * t;
      const offset = Math.max(0, 1 - arrow.birthElapsed * 30);
      arrow.root.position.set(this.cameraPosition.x + delta(this.cameraPosition.x, x) + arrow.offset.x * offset,
        THREE.MathUtils.lerp(arrow.from.y, arrow.to.y, t) + arrow.offset.y * offset,
        this.cameraPosition.z + delta(this.cameraPosition.z, z) + arrow.offset.z * offset);
      arrow.root.rotation.set(arrow.pitch * Math.PI * 2 / 2048, Math.PI - arrow.yaw * Math.PI * 2 / 2048, 0, 'YXZ');
      arrow.root.visible = arrow.active;
    }
  }
  private destroy(actor: Actor): void { actor.mixer.stopAllAction(); actor.mixer.uncacheRoot(actor.model); actor.root.removeFromParent(); }
  /** True only for an observed occupant whose source sprite this presenter owns. */
  replaces(slot: number, sourceId: number): boolean {
    const actor = this.actors.get(slot);
    if (actor?.sourceId === sourceId) return true;
    return this.arrows.get(slot)?.sourceId === sourceId;
  }
  /** Allocate an inspection snapshot only when requested; this never changes actors. */
  getMetrics(): ModernCharacterMetrics {
    const resourceCounts: Record<string, number> = {}, clipCounts: Record<string, number> = {};
    const actors: ModernCharacterMetrics['actors'][number][] = [], arrows: ModernCharacterMetrics['arrows'][number][] = [];
    for (const a of this.actors.values()) {
      resourceCounts[a.resource] = (resourceCounts[a.resource] ?? 0) + 1;
      clipCounts[a.clip] = (clipCounts[a.clip] ?? 0) + 1;
      actors.push({slot:a.slot, sourceId:a.sourceId, generation:a.generation, resource:a.resource, clip:a.clip,
        objectName:a.root.name, visible:a.root.visible, action:a.state.action, phase:a.state.phase,
        x:a.root.position.x, y:a.root.position.y, z:a.root.position.z});
    }
    for (const [slot, arrow] of this.arrows) arrows.push({slot, sourceId:arrow.sourceId, generation:arrow.generation,
      objectName:arrow.root.name, visible:arrow.root.visible});
    return {actorCount:actors.length, arrowCount:arrows.length, resourceCounts, clipCounts, actors, arrows};
  }
  clear(): void { for (const actor of this.actors.values()) this.destroy(actor); this.actors.clear();
    for (const arrow of this.arrows.values()) arrow.root.removeFromParent(); this.arrows.clear(); this.wormHeads.clear(); this.tick = undefined; }
  dispose(): void { this.clear(); for (const variants of this.ownerMaterials.values()) for (const material of variants.values()) material.dispose(); this.ownerMaterials.clear(); }
}
