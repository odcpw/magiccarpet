import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import * as THREE from 'three';
import {ModernWorldObjects,applySourceTints,buildingMesh,dolmenBaseHeight,reconcileBuildingTerrain,type BuildingLayout,type LiveBuilding} from '../src/modern-world-objects.ts';
const generated=new URL('../../../generated/',import.meta.url);
const layout=JSON.parse(readFileSync(new URL('browser-level1-assets/world/layout.json',generated),'utf8')) as BuildingLayout;
const source=readFileSync(new URL('level1-export-m/terrain.bin',generated)).subarray(65536,131072);
const original=Uint8Array.from(source);
const live:LiveBuilding[]=layout.buildings.map(b=>({slot:b.source_slot,slotGeneration:0,buildingIndex:b.build_index,action:52,drawing:true,...b.source_entity_position}));

test('textured imported stone retains its Modern presenter tint',()=>{
 const material=new THREE.MeshStandardMaterial({map:new THREE.Texture()});material.name='Blue grey weathered stone';const root=new THREE.Group();root.add(new THREE.Mesh(new THREE.BoxGeometry(),material));applySourceTints(root);
 assert.deepEqual(material.color.toArray(),[.42,.51,.57]);
});
test('dolmen supports use the source terrain sampling and bounded slope sink',()=>{
 assert.equal(dolmenBaseHeight(0,0,2,()=>7),7);
 assert.ok(Math.abs(dolmenBaseHeight(0,0,2,(x)=>x)-.66)<1e-9);
});

test('opening native records and intact source stamps flatten all 41 without mutating authority',()=>{
 const snapshot=original.slice();const result=reconcileBuildingTerrain(layout,original,live);
 assert.equal(result.accepted.size,41);assert.deepEqual(original,snapshot);
 for(const b of layout.buildings)for(const c of b.cells)assert.equal(result.visual[c.index],c.replacement);
});
test('decorative scatter is bounded, deterministic and excludes admitted building footprints',()=>{
 const nature=new THREE.Group();for(const name of ['grass','fern','shrub','tree_trunk','rock','tree_branches','tree_leaves']){const m=new THREE.Mesh(new THREE.BoxGeometry(1,1,1),new THREE.MeshStandardMaterial());m.name='nature_'+name;nature.add(m);}
 const templates=new Map([['nature',nature]]),library={profiles:layout.profiles,castle_profiles:[]};
 const objects=new (ModernWorldObjects as any)(layout,library,templates,{schema:'test',files:{}}) as ModernWorldObjects;
 const selected=live.slice(0,1);objects.reconcileTerrain(original,selected.map(r=>({sourceSlot:r.slot,sourceId:1,slotGeneration:r.slotGeneration,buildingIndex:r.buildingIndex,action:r.action,drawing:1,active:1,x:r.x,y:r.y,z:r.z})));
 const house=layout.buildings[0]!,camera=new THREE.PerspectiveCamera();camera.position.set(house.x,20,house.z);
 const terrain={revision:0,height:()=>10,visualHeight:()=>10,isWater:()=>false,groundType:()=>1};
 objects.updateNature(camera,terrain);const group=objects.root.children.find(o=>o.type==='Group'&&o.children.some(c=>c instanceof THREE.InstancedMesh))!;
 const counts=group.children.map(c=>(c as THREE.InstancedMesh).count);assert.ok(counts.length>=5);assert.ok(counts.every(n=>n>0&&n<15000));
 const matrix=new THREE.Matrix4(),position=new THREE.Vector3();for(const child of group.children){const mesh=child as THREE.InstancedMesh;for(let i=0;i<mesh.count;i++){mesh.getMatrixAt(i,matrix);position.setFromMatrixPosition(matrix);assert.ok(Math.abs(position.x-house.x)>=house.width*.5+3||Math.abs(position.z-house.z)>=house.depth*.5+3);}}
 const same=[...group.children];objects.updateNature(camera,terrain);assert.deepEqual(group.children,same);
 objects.dispose();
});
test('native census controls projectile, tree and mana identity and removal',()=>{
 const model=()=>{const g=new THREE.Group();g.add(new THREE.Mesh(new THREE.SphereGeometry(.5,8,4),new THREE.MeshStandardMaterial()));return g;};
 const templates=new Map([['fireball',model()],['tree-alive',model()],['tree-burned',model()],['mana-58',model()]]);
 const objects=new (ModernWorldObjects as any)(layout,{profiles:layout.profiles,castle_profiles:[]},templates,{schema:'test',files:{}}) as ModernWorldObjects;
 const camera=new THREE.PerspectiveCamera(),sprite=(entity:number,source:number,selector:number)=>({entity,source,selector,height:64,mode:1,x:640,y:1280,z:128,drawOrder:0});
 const world:any={sequence:1n,tick:1n,sessionGeneration:1n,runtimeGeneration:1n,sprites:[sprite(10,7,463),sprite(20,8,58)]};
 const frame:any={sequence:1n,tick:1n,sessionGeneration:1n,runtimeGeneration:1n,epoch:1n,entities:[{entityIndex:10,sourceId:7,active:1,classId:9,model:0,action:0}],
  trees:[{entityIndex:30,sourceId:9,slotGeneration:0,classId:2,model:0,action:0,renderParameter:83,renderHeight:256,drawing:1,x:100,y:200,z:0}],
  manaSpheres:[{sphereId:44,sourceEntityIndex:20,active:1,logicalHeight:64,baseTmap:58,x:640,y:1280,z:128}],buildings:[],characters:[],castleChildren:[]};
 objects.update(frame,world,camera,()=>3);assert.deepEqual(objects.metrics,{buildings:0,props:1,trees:1,mana:1});assert.deepEqual([...objects.replacedSlots].sort((a,b)=>a-b),[10,20,30]);
 frame.trees[0].action=2;frame.trees[0].renderParameter=226;objects.update(frame,world,camera,()=>3);assert.equal(objects.metrics.trees,1);
 frame.manaSpheres[0].active=0;frame.entities[0].sourceId=99;frame.trees=[];objects.update(frame,world,camera,()=>3);assert.deepEqual(objects.metrics,{buildings:0,props:0,trees:0,mana:0});assert.equal(objects.replacedSlots.size,0);
 assert.throws(()=>objects.update({...frame,tick:2n},world,camera,()=>3),/unmatched/);objects.dispose();
});
test('changed stamp or reused slot remains terrain and removes only that model',()=>{
 const changed=original.slice(),first=layout.buildings[0]!;changed[first.cells[0]!.index]=changed[first.cells[0]!.index]!-1;
 const rows=live.map((r,i)=>i===1?{...r,slotGeneration:1}:r);
 const result=reconcileBuildingTerrain(layout,changed,rows);
 assert.equal(result.accepted.size,39);assert.equal(result.accepted.has(first.source_slot),false);
 for(const b of layout.buildings.slice(0,2))for(const c of b.cells)assert.equal(result.visual[c.index],changed[c.index]);
 assert.equal(result.accepted.has(layout.buildings[2]!.source_slot),true);
});
test('inconsistent descriptor fails before returning a partial visual plane',()=>{
 const bad=structuredClone(layout);bad.buildings[0]!.cells[0]!.replacement++;
 assert.throws(()=>reconcileBuildingTerrain(bad,original,live),/stamp|mismatch/);
});
test('opening building facets and emissive windows are finite and remain within source envelope',()=>{
 for(const profile of layout.profiles){const group=buildingMesh(profile,1);let points=0,glazing=0;
  group.traverse((obj:any)=>{if(!obj.isMesh)return;const pos=obj.geometry.getAttribute('position');points+=pos.count;
   if(obj.material.emissive?.getHex()===0xffad56){glazing+=pos.count;assert.equal(obj.material.emissiveIntensity,1);
    const normal=obj.geometry.getAttribute('normal');assert.ok(normal.count===pos.count&&Math.hypot(normal.getX(0),normal.getY(0),normal.getZ(0))>.99);
    if([26,29,30].includes(profile.build_index)){const center=[0,1,2].map(axis=>{let sum=0;for(let i=0;i<6;i++)sum+=axis===0?pos.getX(i):axis===1?pos.getY(i):pos.getZ(i);return +(sum/6).toFixed(3);});console.log('window sample',profile.build_index,center,'normal',[normal.getX(0),normal.getY(0),normal.getZ(0)].map(v=>+v.toFixed(3)));}}
   for(let i=0;i<pos.count;i++){assert.ok(Number.isFinite(pos.getX(i))&&Number.isFinite(pos.getY(i))&&Number.isFinite(pos.getZ(i)));assert.ok(Math.abs(pos.getX(i))<=(profile.columns-1)*2+.06);assert.ok(Math.abs(pos.getZ(i))<=(profile.rows-1)*2+.06);}
  });assert.ok(points>0);assert.ok(glazing>0,`missing glazing for ${profile.build_index}`);
 }
});
