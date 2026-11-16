import test from 'node:test';
import assert from 'node:assert/strict';
import { characterResource, sourceDeath, ModernCharacters } from '../src/modern-characters.ts';
import type { ModernCharacter, ModernFrame } from '../src/modern-runtime.ts';
import * as THREE from 'three';

function row(patch: Partial<ModernCharacter> = {}): ModernCharacter {
  return {slotGeneration:0,parentGeneration:0,entityIndex:7,sourceId:9,parentIndex:0,parentId:0,classId:5,model:1,
    action:0,phase:0,active:1,hasTarget:0,hasParent:0,stance:0,x:0,y:0,z:0,yaw:0,pitch:0,speed:0,life:0,
    targetIndex:0,targetX:0,targetY:0,targetZ:0,controlFlags:0,ageTicks:0,renderParameter:0,roll:0,
    sourceHalfWidth:0,renderHeight:150,previousIndex:0,nextIndex:0,partIndex:0,lifecycleFlags:0,...patch};
}
function frame(tick: bigint, characters: ModernCharacter[], epoch = 1n): ModernFrame {
  return {runtimeGeneration:1n,sessionGeneration:1n,epoch,tick,sequence:tick,characters,entities:[],buildings:[],castleChildren:[]};
}
function fixture(): { presenter: ModernCharacters; scene: THREE.Group; camera: THREE.Camera } {
  const clip = new THREE.AnimationClip('Idle', 1, []), death = new THREE.AnimationClip('Death', 1, []);
  const asset = {definition:{url:'',sha256:'',bytes:0,height:1,forward_yaw:Math.PI,walk_speed:1,run_speed:1,
    clips:[{name:'Idle',duration:1,loop:true},{name:'Death',duration:1,loop:false}]},
    scene:new THREE.Group(),clips:new Map([['Idle',clip],['Death',death]]),bounds:new THREE.Box3(new THREE.Vector3(0,0,0),new THREE.Vector3(1,1,1))};
  const scene = new THREE.Group();
  const presenter = new (ModernCharacters as any)(scene, new Map([['Goat',asset], ['Villager',asset], ['Archer',{...asset,arrow:new THREE.Group()}]])) as ModernCharacters;
  return {presenter,scene,camera:new THREE.PerspectiveCamera()};
}

test('source bindings distinguish appearances and worm parts', () => {
  assert.equal(characterResource(row()), 'Goat');
  assert.equal(characterResource(row({model:13,renderParameter:239})), 'VillagerBrown');
  assert.equal(characterResource(row({model:12,renderParameter:239})), undefined);
  assert.equal(characterResource(row({model:3,renderParameter:89,hasParent:0})), undefined);
  assert.equal(characterResource(row({model:3,renderParameter:89,hasParent:1})), 'WormSegment');
});
test('source deaths distinguish zero life and positive-life house entry', () => {
  assert.equal(sourceDeath(row({life:0})), false);
  assert.equal(sourceDeath(row({model:13, action:108,life:100,lifecycleFlags:1})), false);
  assert.equal(sourceDeath(row({model:13, action:108,life:100,lifecycleFlags:0})), true);
  assert.equal(sourceDeath(row({model:19,action:156,life:0})), true);
  assert.equal(sourceDeath(row({life:-1})), true);
});
test('native incarnations, removal and epoch reset reclaim models', () => {
  const {presenter,scene,camera} = fixture();
  presenter.observe(frame(1n,[row()])); presenter.render(camera,1/30); assert.equal(scene.children.length,1);
  const first = scene.children[0];
  presenter.observe(frame(2n,[row({slotGeneration:1})])); assert.equal(scene.children.length,1);
  assert.notEqual(scene.children[0],first);
  presenter.observe(frame(3n,[])); assert.equal(scene.children.length,0);
  presenter.observe(frame(4n,[row()])); assert.equal(scene.children.length,1);
  presenter.observe(frame(1n,[],2n)); assert.equal(scene.children.length,0);
  presenter.dispose();
});
test('death tail is bounded and living entry is hidden rather than dying', () => {
  const {presenter,scene,camera} = fixture();
  presenter.observe(frame(1n,[row({life:-1})])); presenter.render(camera,.5); assert.equal(scene.children.length,1);
  presenter.observe(frame(2n,[])); presenter.render(camera,1.01); assert.equal(scene.children.length,0);
  presenter.observe(frame(3n,[row({model:13,renderParameter:241,action:108,life:100,lifecycleFlags:1,controlFlags:0x20})]));
  presenter.render(camera,.01); assert.equal(scene.children[0]?.visible,false);
  presenter.observe(frame(4n,[])); assert.equal(scene.children.length,0);
});
test('only live native arrows allocate a visual and a reused slot replaces it', () => {
  const {presenter,scene,camera} = fixture();
  const arrow = row({classId:9,model:13,entityIndex:20,sourceId:44,hasParent:1,parentIndex:7,parentId:9});
  presenter.observe(frame(1n,[row({model:4}),arrow])); presenter.render(camera,1/30);
  assert.equal(scene.children.length,2);
  const original = scene.children[1];
  presenter.observe(frame(2n,[row({model:4}),{...arrow,slotGeneration:1}]));
  assert.equal(scene.children.length,2); assert.notEqual(scene.children[1],original);
  presenter.observe(frame(3n,[row({model:4})])); assert.equal(scene.children.length,1);
  presenter.dispose(); assert.equal(scene.children.length,0);
});
