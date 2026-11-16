import * as THREE from 'three';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';

// Input rows must come from the same native publication. These are intentionally
// presentation observations: this module never decides collection or death.
export type WorldEntity = {slot:number; sourceId:number; generation:number; active:boolean; classId:number; model:number; action:number; x:number; y:number; z:number};
export type WorldPresentation = {slot:number; sourceId:number; generation:number; renderParameter:number; baseTmap:number; selectedTmap:number; renderMode:number; logicalHeight:number; controlFlags:number; x:number; y:number; z:number};
export type LiveBuilding = {slot:number; slotGeneration:number; buildingIndex:number; action:number; drawing:boolean; x:number; y:number; z:number};
export type NatureTerrain = {revision:string|number|bigint; height:(x:number,z:number)=>number; visualHeight:(x:number,z:number)=>number; isWater:(x:number,z:number)=>boolean; groundType:(x:number,z:number)=>number};
type Profile = {build_index:number; columns:number; rows:number; diagonal_parity:number; terrain_only?:boolean; heights:number[]};
type Stamp = {index:number; expected:number; replacement:number};
type Building = {source_slot:number; build_index:number; x:number; z:number; base_height:number; width:number; depth:number; height:number; source_entity_position:{x:number;y:number;z:number}; cells:Stamp[]};
export type BuildingLayout = {schema:string; provenance:{source_height_plane_sha256:string}; profiles:Profile[]; buildings:Building[]};
type Library = {profiles:Profile[]; castle_profiles:Profile[]};
type WindowSite = {center:THREE.Vector3; normal:THREE.Vector3; across:THREE.Vector3; up:THREE.Vector3; w:number; h:number; sector:number};
const BASE = '/original-api/modern-assets/world/';
const mod = (n:number,m:number) => ((n % m) + m) % m;
const delta = (a:number,b:number) => mod(a-b+512,1024)-512;
function hash(x:number,z:number){let h=(Math.imul(x,0x8da6b343)^Math.imul(z,0xd8163841)^0xcb1ab31f)>>>0;h^=h>>>16;h=Math.imul(h,0x7feb352d);h^=h>>>15;h=Math.imul(h,0x846ca68b);return (h^(h>>>16))>>>0;}
function random01(value:number){value^=value>>>16;value=Math.imul(value,0x7feb352d);value^=value>>>15;value=Math.imul(value,0x846ca68b);value^=value>>>16;return (value&0xffffff)/16777216;}
function patchDensity(x:number,z:number){const px=x/16,pz=z/16,ix=Math.floor(px),iz=Math.floor(pz),smooth=(v:number)=>v*v*(3-2*v),fx=smooth(px-ix),fz=smooth(pz-iz),lerp=THREE.MathUtils.lerp;return lerp(lerp(random01(hash(ix&63,iz&63)),random01(hash((ix+1)&63,iz&63)),fx),lerp(random01(hash(ix&63,(iz+1)&63)),random01(hash((ix+1)&63,(iz+1)&63)),fx),fz);}
export function dolmenBaseHeight(x:number,z:number,scale:number,height:(x:number,z:number)=>number){let low=Infinity,high=-Infinity;for(const [cx,cz] of [[-.5,-.08],[.48,-.05],[.02,.23]])for(const [dx,dz] of [[0,0],[-.1,0],[.1,0],[0,-.1],[0,.1]]){const h=height(x+(cx!+dx!)*scale,z+(cz!+dz!)*scale);low=Math.min(low,h);high=Math.max(high,h);}return Math.max(low,high-.25*scale);}

// Validate complete geometry/stamp correspondence before changing even one
// visual byte. Like ModernBuildings.Reconcile, changed terrain invalidates
// only the affected building; native state remains untouched.
export function reconcileBuildingTerrain(layout:BuildingLayout, original:Uint8Array, live:LiveBuilding[]) {
 if (layout.schema !== 'mc-modern-building-layout/v2' || original.length!==65536 || layout.buildings.length!==41) throw Error('invalid building layout');
 const profiles=new Map(layout.profiles.map(p=>[p.build_index,p]));
 if(profiles.size!==layout.profiles.length)throw Error('duplicate profile');
 const slots=new Map<number,LiveBuilding>(); for(const row of live){if(slots.has(row.slot))throw Error('duplicate building slot');slots.set(row.slot,row);}
 const occupied=new Set<number>(), ids=new Set<number>(), accepted=new Set<number>();
 for(const b of layout.buildings){
  const p=profiles.get(b.build_index); if(!p||ids.has(b.source_slot)||p.heights.length!==p.columns*p.rows||p.columns<3||p.rows<3||p.columns>33||p.rows>33||![0,1].includes(p.diagonal_parity))throw Error('invalid building profile');
  ids.add(b.source_slot);
  if(p.heights.some(h=>!Number.isFinite(h)||h<0||h>127.5||Math.abs(h*2-Math.round(h*2))>.001)||Math.abs((p.columns-1)*4-b.width)>.001||Math.abs((p.rows-1)*4-b.depth)>.001||Math.abs(Math.max(...p.heights)-b.height)>.001||p.heights.filter(h=>h>0).length!==b.cells.length)throw Error('invalid profile dimensions');
  let intact=true;
  for(const cell of b.cells){
   if(!Number.isInteger(cell.index)||cell.index<0||cell.index>=65536||occupied.has(cell.index)||cell.expected>255||cell.replacement<0||cell.replacement>=cell.expected||Math.abs(cell.replacement*.5-b.base_height)>.001)throw Error('invalid building stamp');
   occupied.add(cell.index);
   const px=(delta((cell.index%256)*4,b.x)+b.width*.5)/4,pz=(delta(Math.floor(cell.index/256)*4,b.z)+b.depth*.5)/4,ix=Math.round(px),iz=Math.round(pz);
   if(Math.abs(px-ix)>.001||Math.abs(pz-iz)>.001||ix<0||ix>=p.columns||iz<0||iz>=p.rows||Math.abs(p.heights[iz*p.columns+ix]!*2-(cell.expected-cell.replacement))>.001)throw Error('stamp/profile mismatch');
   if(original[cell.index]!==cell.expected)intact=false;
  }
  const row=slots.get(b.source_slot), pos=b.source_entity_position;
  if(intact&&row&&row.slotGeneration===0&&row.buildingIndex===b.build_index&&row.action===52&&row.drawing&&row.x===pos.x&&row.y===pos.y&&row.z===pos.z)accepted.add(b.source_slot);
 }
 const visual=original.slice(); for(const b of layout.buildings)if(accepted.has(b.source_slot))for(const cell of b.cells)visual[cell.index]=cell.replacement;
 return {visual, accepted};
}

function facets(p:Profile, fn:(a:THREE.Vector3,b:THREE.Vector3,c:THREE.Vector3)=>void){
 const ox=(p.columns-1)*2,oz=(p.rows-1)*2;
 for(let z=0;z<p.rows-1;z++)for(let x=0;x<p.columns-1;x++){
  const a=new THREE.Vector3(x*4-ox,p.heights[x+z*p.columns]!,z*4-oz),b=new THREE.Vector3((x+1)*4-ox,p.heights[x+1+z*p.columns]!,z*4-oz),c=new THREE.Vector3(x*4-ox,p.heights[x+(z+1)*p.columns]!, (z+1)*4-oz),d=new THREE.Vector3((x+1)*4-ox,p.heights[x+1+(z+1)*p.columns]!, (z+1)*4-oz);
  if(((x+z+p.diagonal_parity)&1)===0){fn(a,c,d);fn(a,d,b);}else{fn(a,c,b);fn(b,c,d);}
 }
}
function windows(p:Profile):WindowSite[]{
 if(p.terrain_only)return [];
 const min=Math.min(...p.heights),max=Math.max(...p.heights),candidates:WindowSite[]=[];
 facets(p,(a,b,c)=>{
  const normal=new THREE.Vector3().subVectors(b,a).cross(new THREE.Vector3().subVectors(c,a)).normalize();
  if(normal.y>.68||normal.y<.00001)return;
  const center=new THREE.Vector3().add(a).add(b).add(c).multiplyScalar(1/3);
  if(center.y<min+(max-min)*.12||center.y>min+(max-min)*.7||normal.x*center.x+normal.z*center.z<=.01)return;
  const distance=(v:THREE.Vector3,w:THREE.Vector3)=>new THREE.Vector3().subVectors(center,v).cross(new THREE.Vector3().subVectors(w,v)).length()/v.distanceTo(w);
  let clearance=Math.min(distance(a,b),distance(b,c),distance(c,a));clearance=Math.min(clearance,Math.min((p.columns-1)*2-Math.abs(center.x),(p.rows-1)*2-Math.abs(center.z))-.06);
  const w=Math.min(.6,clearance*.52),h=Math.min(.9,clearance*.72);if(w<.12||h<.12)return;
  const across=new THREE.Vector3().crossVectors(new THREE.Vector3(0,1,0),normal).normalize(),up=new THREE.Vector3().crossVectors(normal,across).normalize();
  const sector=Math.abs(normal.x)>Math.abs(normal.z)?(normal.x<0?0:1):(normal.z<0?2:3);
  candidates.push({center,normal,across,up,w,h,sector});
 });
 const result:WindowSite[]=[];
 function select(sector:number){let best=-1,score=-Infinity;for(let i=0;i<candidates.length;i++){const c=candidates[i]!;if(sector>=0&&c.sector!==sector)continue;let nearest=1000000;for(const o of result)nearest=Math.min(nearest,c.center.distanceToSquared(o.center));if(nearest<4)continue;const s=Math.min(nearest,100)-c.center.y*.01;if(s>score){score=s;best=i;}}if(best<0)return false;result.push(candidates.splice(best,1)[0]!);return true;}
 for(let sector=0;sector<4;sector++)select(sector);while(result.length<6&&select(-1)){}return result;
}

function buildingSurface(material:THREE.MeshStandardMaterial, kind:number, rock:THREE.Texture){
 material.color.setRGB(1,1,1);
 material.onBeforeCompile=shader=>{
  shader.uniforms.buildingRock={value:rock};
  shader.vertexShader='varying vec3 buildingPos,buildingNormal;\n'+shader.vertexShader;
  shader.vertexShader=shader.vertexShader.replace('#include <begin_vertex>', '#include <begin_vertex>\nbuildingPos=position; buildingNormal=normal;');
  shader.fragmentShader=`uniform sampler2D buildingRock; varying vec3 buildingPos,buildingNormal;
   float buildingHash(vec2 cell){return fract(sin(dot(cell,vec2(127.1,311.7)))*43758.5453);}\n`+shader.fragmentShader;
  const albedo=kind===0?`
   vec2 surface=abs(bn.x)>abs(bn.z)?buildingPos.zy:buildingPos.xy;
   float row=floor(surface.y/1.05);vec2 bond=vec2(surface.x/2.15+fract(row*.5),surface.y/1.05);
   vec2 edge=min(fract(bond),1.-fract(bond))*vec2(2.15,1.05);float dist=min(edge.x,edge.y);
   float aa=max(fwidth(dist),.008),mortar=1.-smoothstep(.018-aa,.06+aa,dist);
   vec3 stone=mix(vec3(.075,.12,.14),vec3(.20,.285,.355),smoothstep(4.6,5.5,buildingPos.y));
   stone*=mix(.82,1.14,buildingHash(floor(bond)))*mix(.8,1.13,grain);stone=mix(stone,vec3(.035,.048,.054),mortar*.65);
  `:`
   vec2 tile=buildingPos.xz/vec2(.9,1.1);tile.x+=fract(floor(tile.y)*.5);
   vec2 edges=min(fract(tile),1.-fract(tile));float seam=1.-smoothstep(.015,.055+max(fwidth(tile.x),fwidth(tile.y)),min(edges.x,edges.y));
   vec3 stone=vec3(.13,.105,.075)*mix(.75,1.2,buildingHash(floor(tile)))*mix(.85,1.1,grain);stone*=1.-seam*.5;
  `;
  shader.fragmentShader=shader.fragmentShader.replace('#include <map_fragment>',`
   vec3 bn=normalize(buildingNormal),bw=pow(abs(bn),vec3(4.));bw/=max(.001,dot(bw,vec3(1.)));
   vec3 bp=buildingPos*.36;vec3 texel=texture2D(buildingRock,bp.yz).rgb*bw.x+texture2D(buildingRock,bp.xz).rgb*bw.y+texture2D(buildingRock,bp.xy).rgb*bw.z;
   float grain=clamp(dot(texel,vec3(.3,.5,.2))*1.7,0.,1.);${albedo}
   float damp=(1.-smoothstep(-.1,1.3,buildingPos.y))*(.10+.13*grain);diffuseColor.rgb*=stone*(1.-damp);
  `);
 };
 material.customProgramCacheKey=()=>`modern-building-${kind}-v1`;
}

export function buildingMesh(p:Profile, night:number,rock?:THREE.Texture):THREE.Group{
 const vertices:number[][]=[[],[],[],[],[],[],[]];
 const tri=(a:THREE.Vector3,b:THREE.Vector3,c:THREE.Vector3,kind:number)=>vertices[kind]!.push(...a.toArray(),...b.toArray(),...c.toArray());
 facets(p,(a,b,c)=>{const n=new THREE.Vector3().subVectors(b,a).cross(new THREE.Vector3().subVectors(c,a)).normalize();tri(a,b,c,n.y<.76||Math.max(a.y,b.y,c.y)<.1?0:2);});
 const rect=(s:WindowSite,x0:number,y0:number,x1:number,y1:number,depth:number,k:number)=>{const point=(x:number,y:number)=>s.center.clone().addScaledVector(s.across,x).addScaledVector(s.up,y).addScaledVector(s.normal,depth);const a=point(x0,y0),b=point(x1,y0),c=point(x1,y1),d=point(x0,y1);tri(a,b,c,k);tri(a,c,d,k);};
 for(const s of windows(p)){const w=s.w,h=s.h,t=Math.min(w,h)*.18;rect(s,-w,-h,w,h,.012,6);rect(s,-w*.78,-h*.82,w*.78,h*.82,.025,5);rect(s,-w,-h,-w+t,h,.035,3);rect(s,w-t,-h,w,h,.035,3);rect(s,-w,-h,w,-h+t,.035,3);rect(s,-w,h-t,w,h,.035,3);rect(s,-w*.045,-h*.8,w*.045,h*.8,.04,4);rect(s,-w*.75,-h*.035,w*.75,h*.035,.04,4);rect(s,-w,-h,w,-h+t*.55,.045,0);}
 const colors=[0x304453,0x304453,0x211b13,0x1b0c06,0x12110d,0x161006,0x070504];const group=new THREE.Group();
 for(let k=0;k<vertices.length;k++){if(!vertices[k]!.length)continue;const g=new THREE.BufferGeometry();g.setAttribute('position',new THREE.Float32BufferAttribute(vertices[k]!,3));g.computeVertexNormals();const m=new THREE.MeshStandardMaterial({color:colors[k],roughness:.85,side:THREE.DoubleSide,...(k===5?{emissive:0xffad56,emissiveIntensity:THREE.MathUtils.clamp(night,0,1)}:{})});if(rock&&(k===0||k===2))buildingSurface(m,k,rock);const mesh=new THREE.Mesh(g,m);mesh.receiveShadow=true;mesh.castShadow=k===0||k===2;group.add(mesh);}
 return group;
}

import type {ModernFrame} from './modern-runtime';
import type {WorldFrame, WorldSprite} from './original-world-data';
type Manifest = {schema:string; files:Record<string,{sha256:string;bytes:number;source:string}>};
async function fetchChecked(base:string,name:string, info:{sha256:string;bytes:number}):Promise<ArrayBuffer>{
 const response=await fetch(base+name,{signal:AbortSignal.timeout(30000)});if(!response.ok)throw Error(`world asset ${name}: ${response.status}`);
 const data=await response.arrayBuffer();if(data.byteLength!==info.bytes)throw Error(`world asset ${name}: size mismatch`);
 // The native bridge also checks the manifest hash before serving. Browsers
 // omit SubtleCrypto on the supported plain-HTTP tailnet route, so retain the
 // bridge check there and independently verify when the API is available.
 if(globalThis.crypto?.subtle){const digest=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',data))).map(b=>b.toString(16).padStart(2,'0')).join('');
  if(digest!==info.sha256)throw Error(`world asset ${name}: hash mismatch`);}
 return data;
}
function decode<T>(bytes:ArrayBuffer):T{return JSON.parse(new TextDecoder().decode(bytes)) as T;}
function assetKind(e:ModernFrame['entities'][number],p:WorldSprite):string|null{
 if(!e.active||!e.entityIndex||!e.sourceId||e.entityIndex!==p.entity||e.sourceId!==p.source||p.height<=0||p.drawOrder===0xffffffff)return null;
 if(e.classId===15&&((e.model===2&&(e.action===7||e.action===8))||(e.model===3&&(e.action===10||e.action===11)))&&p.selector===59&&p.mode===1&&p.height===150)return 'spell-jar';
 if(e.classId===14&&e.model===5&&e.action===10&&p.selector===259&&p.mode===1&&p.height===75)return 'experience-scroll';
 if(e.classId===2&&e.model===1&&e.action===3&&p.selector===78)return 'stone-statue';
 if(e.classId===2&&e.model===2&&e.action===6&&p.selector===79)return 'dolmen';
 if(e.classId===9&&(e.model===0||e.model===28)&&p.selector===463)return 'fireball';
 if(e.classId===9&&(e.model===1||e.model===17)&&p.selector===144)return 'possession';
 if(e.classId===9&&e.model===10&&e.action===10&&p.selector===61&&p.mode===21)return 'castle-head';
 if(p.selector===461)return 'exit-portal';return null;
}

type PropInstance={kind:string;identity:string;root:THREE.Group;row:WorldSprite};
type TreeInstance={identity:string;root:THREE.Group;alive:THREE.Object3D;burned:THREE.Object3D};
type ManaInstance={identity:string;root:THREE.Group;selector:number};
function treeTemplate(source:THREE.Group, burned:boolean):THREE.Group {
 const trunk=source.getObjectByName('tree_trunk'),branches=source.getObjectByName('tree_branches'),leaves=source.getObjectByName('tree_leaves');
 if(!trunk||!branches||!leaves)throw Error('missing authored tree parts');
 const wood:THREE.Object3D[]=[trunk,branches], foliage:THREE.Object3D=leaves;
 const height=new THREE.Box3().setFromObject(source).max.y;if(!Number.isFinite(height)||height<=0)throw Error('invalid authored tree height');
 const char=new THREE.MeshStandardMaterial({color:0x3d332b,roughness:.92});
 function level(copies:number){const group=new THREE.Group();
  for(const part of wood){const child=part.clone(true);if(burned)child.traverse(o=>{if(o instanceof THREE.Mesh)o.material=char;});group.add(child);}
  if(!burned)for(let i=0;i<copies;i++){const child=foliage.clone(true);child.rotation.y+=2*Math.PI*i/copies;child.traverse(o=>{if(o instanceof THREE.Mesh&&o.material instanceof THREE.MeshStandardMaterial){o.material=o.material.clone();o.material.color.multiply(new THREE.Color(.5,.95,1.4));}});group.add(child);}
  return group;
 }
 const lod=new THREE.LOD();lod.addLevel(level(10),0);lod.addLevel(level(2),130);const root=new THREE.Group();root.scale.setScalar(1/height);root.add(lod);return root;
}
export function applySourceTints(root:THREE.Group){
 const tints:Record<string,[number,number,number]>={'Scroll parchment':[.97,.87,.72], 'Scroll parchment overlay':[1,.96,.86], 'Scroll parchment lip':[1,.98,.88],
  'Blue grey weathered stone':[.42,.51,.57], 'Weathered brown grey stone':[.84,.86,.91], 'Wrap':[.09,.28,.26]};
 root.traverse(object=>{if(!(object instanceof THREE.Mesh))return;for(const material of Array.isArray(object.material)?object.material:[object.material]){
  if(!(material instanceof THREE.MeshStandardMaterial)||!material.map)continue;const tint=tints[material.name];if(tint)material.color.setRGB(...tint);
 }});
}
export class ModernWorldObjects {
 readonly root=new THREE.Group();
 readonly layout:BuildingLayout;
 readonly manifest:Manifest;
 private readonly library:Library;
 private readonly templates:Map<string,THREE.Group>;
 private readonly buildingRock?:THREE.Texture;
 private readonly buildings=new Map<number,THREE.Group>();
 private readonly buildingTemplates=new Map<number,THREE.Group>();
 private readonly props=new Map<number,PropInstance>();
 private readonly trees=new Map<number,TreeInstance>();
 private readonly mana=new Map<number,ManaInstance>();
 private readonly nature=new THREE.Group();
 private natureKey=''; private acceptedKey='';
 private night=0; private lastTerrain:Uint8Array|null=null; private epoch:bigint|null=null;
 private constructor(layout:BuildingLayout,library:Library,templates:Map<string,THREE.Group>,manifest:Manifest,buildingRock?:THREE.Texture){this.layout=layout;this.library=library;this.templates=templates;this.manifest=manifest;this.buildingRock=buildingRock;this.root.add(this.nature);}
 static async load(scene:THREE.Scene,base=BASE):Promise<ModernWorldObjects>{
  if(!base.endsWith('/'))base+='/';
  const response=await fetch(base+'manifest.json',{signal:AbortSignal.timeout(30000)});if(!response.ok)throw Error('world manifest unavailable');const manifest=await response.json() as Manifest;if(manifest.schema!=='mc-browser-modern-world/v1')throw Error('unsupported world manifest');
  const get=(name:string)=>{const entry=manifest.files[name];if(!entry)throw Error(`world manifest missing ${name}`);return fetchChecked(base,name,entry);};
  const names=['spell-jar','experience-scroll','exit-portal','stone-statue','dolmen','castle-head','authored-tree','nature'];
  const [layoutBytes,libraryBytes,colorBytes,rockBytes,...glbs]=await Promise.all([get('layout.json'),get('building-library.json'),get('mana-colors.json'),get('rockalbedo.jpg'),...names.map(n=>get(n+'.glb'))]);
  const loader=new GLTFLoader(),templates=new Map<string,THREE.Group>();
  for(let i=0;i<names.length;i++){
   const gltf=await loader.parseAsync(glbs[i]!,base),root=gltf.scene,box=new THREE.Box3().setFromObject(root),size=box.getSize(new THREE.Vector3());
   if(!Number.isFinite(size.y)||size.y<=0)throw Error(`world model ${names[i]} has invalid bounds`);
   applySourceTints(root);
   if(names[i]==='authored-tree'){templates.set('tree-alive',treeTemplate(root,false));templates.set('tree-burned',treeTemplate(root,true));}
   else if(names[i]==='exit-portal'||names[i]==='castle-head'){const geometry=new THREE.Group();geometry.add(root);root.scale.multiplyScalar(1/size.y);root.position.y-=box.min.y/size.y;if(names[i]==='castle-head'){root.position.x-=box.getCenter(new THREE.Vector3()).x/size.y;root.position.z-=box.getCenter(new THREE.Vector3()).z/size.y;}templates.set(names[i]!,geometry);}else templates.set(names[i]!,root);
  }
  const orb=new THREE.SphereGeometry(.5,24,12);
  for(const [name,color] of [['fireball',0xff5209],['possession',0x59a8db]] as const){const group=new THREE.Group();group.add(new THREE.Mesh(orb,new THREE.MeshStandardMaterial({color,emissive:color,emissiveIntensity:1.4,roughness:.3})));templates.set(name,group);}
  const manaColors=decode<{schema:string;colors:Record<string,number[]>}>(colorBytes!);if(manaColors.schema!=='mc-modern-mana-colors/v1')throw Error('invalid mana palette');
  for(const [selector,rgb] of Object.entries(manaColors.colors)){if(rgb.length!==3||rgb.some(v=>!Number.isFinite(v)||v<0||v>1))throw Error('invalid mana colour');const color=new THREE.Color().setRGB(rgb[0]!,rgb[1]!,rgb[2]!,THREE.SRGBColorSpace),group=new THREE.Group();group.add(new THREE.Mesh(orb,new THREE.MeshStandardMaterial({color,emissive:color,emissiveIntensity:1,roughness:.3})));templates.set('mana-'+selector,group);}
  const layout=decode<BuildingLayout>(layoutBytes!),library=decode<Library>(libraryBytes!);if(library.profiles.length!==17||library.castle_profiles.length!==7)throw Error('incomplete building library');
  const rockUrl=URL.createObjectURL(new Blob([rockBytes!],{type:'image/jpeg'}));let buildingRock:THREE.Texture;
  try{buildingRock=await new THREE.TextureLoader().loadAsync(rockUrl);}finally{URL.revokeObjectURL(rockUrl);}
  buildingRock.colorSpace=THREE.SRGBColorSpace;buildingRock.wrapS=buildingRock.wrapT=THREE.RepeatWrapping;buildingRock.needsUpdate=true;
  const result=new ModernWorldObjects(layout,library,templates,manifest,buildingRock);scene.add(result.root);return result;
 }
 // Only the returned copy belongs to visual terrain. Always pass the original
 // native plane on subsequent reconciliations, including after terrain edits.
 reconcileTerrain(original:Uint8Array, live:ModernFrame['buildings']){
  const rows:LiveBuilding[]=live.map(r=>({slot:r.sourceSlot,slotGeneration:r.slotGeneration,buildingIndex:r.buildingIndex,action:r.action,drawing:!!r.active&&!!r.drawing,x:r.x,y:r.y,z:r.z}));
  const result=reconcileBuildingTerrain(this.layout,original,rows);
  this.acceptedKey=[...result.accepted].sort((a,b)=>a-b).join(',');
  for(const b of this.layout.buildings){let node=this.buildings.get(b.source_slot);if(result.accepted.has(b.source_slot)){
   if(!node){let template=this.buildingTemplates.get(b.build_index);if(!template){const p=this.library.profiles.find(q=>q.build_index===b.build_index);if(!p)throw Error('missing opening profile');template=buildingMesh(p,this.night,this.buildingRock);this.buildingTemplates.set(b.build_index,template);}node=template.clone(true);node.position.set(b.x,b.base_height+.025,b.z);this.buildings.set(b.source_slot,node);this.root.add(node);}
  }else if(node){this.root.remove(node);this.buildings.delete(b.source_slot);}}
  const changed=this.lastTerrain===null||result.visual.some((v,i)=>v!==this.lastTerrain![i]);this.lastTerrain=result.visual.slice();return {...result,changed};
 }
 setNight(amount:number){this.night=THREE.MathUtils.clamp(amount,0,1);for(const root of this.buildingTemplates.values())root.traverse(o=>{if(o instanceof THREE.Mesh&&o.material instanceof THREE.MeshStandardMaterial&&o.material.emissive.getHex()===0xffad56)o.material.emissiveIntensity=this.night;});}
 // Decorative vegetation is source-seeded presentation only. Terrain and
 // building exclusion callbacks describe the current visible terrain copy.
 updateNature(camera:THREE.Camera,terrain:NatureTerrain){
  const cx=Math.floor(camera.position.x/32),cz=Math.floor(camera.position.z/32),key=`${cx}:${cz}:${terrain.revision}:${this.acceptedKey}`;if(key===this.natureKey)return;
  const template=this.templates.get('nature');if(!template)throw Error('missing nature library');
  const names=['grass','fern','shrub','tree_trunk','rock','tree_branches','tree_leaves'];
  const meshes=names.map(name=>{const object=template.getObjectByName('nature_'+name);if(!(object instanceof THREE.Mesh))throw Error('missing nature mesh '+name);if(object.material instanceof THREE.MeshStandardMaterial&&(name==='grass'||name==='fern'||name==='shrub'||name==='tree_leaves')){object.material.alphaTest=.15;object.material.transparent=false;object.material.depthWrite=true;object.material.side=THREE.DoubleSide;}return object;});
  const matrices:THREE.Matrix4[][]=names.map(()=>[]),up=new THREE.Vector3(0,1,0),scaleVector=new THREE.Vector3(),rockHeight=new THREE.Box3().setFromObject(meshes[4]!).getSize(new THREE.Vector3()).y;
  const dry=(x:number,z:number,r:number)=>!terrain.isWater(x-r,z)&&!terrain.isWater(x+r,z)&&!terrain.isWater(x,z-r)&&!terrain.isWater(x,z+r);
  const normal=(x:number,z:number)=>new THREE.Vector3(terrain.height(x-.5,z)-terrain.height(x+.5,z),1,terrain.height(x,z-.5)-terrain.height(x,z+.5)).normalize();
  const excluded=(x:number,z:number)=>this.layout.buildings.some(b=>this.buildings.has(b.source_slot)&&Math.abs(delta(x,b.x))<b.width*.5+3&&Math.abs(delta(z,b.z))<b.depth*.5+3);
  const add=(kind:number,x:number,y:number,z:number,yaw:number,scale:number,n:THREE.Vector3)=>{if(excluded(x,z))return;const q=new THREE.Quaternion().setFromUnitVectors(up,n).multiply(new THREE.Quaternion().setFromAxisAngle(up,yaw));const matrix=new THREE.Matrix4().compose(new THREE.Vector3(x,y,z),q,scaleVector.setScalar(scale));matrices[kind]!.push(matrix);if(kind===3){matrices[5]!.push(matrix);matrices[6]!.push(matrix);}};
  const lerp=THREE.MathUtils.lerp,baseX=cx*8,baseZ=cz*8;
  for(let dz=-56;dz<=56;dz++)for(let dx=-56;dx<=56;dx++){
   const cellX=baseX+dx,cellZ=baseZ+dz,seed=hash(cellX&255,cellZ&255),x=(cellX+.12+.76*random01(seed))*4,z=(cellZ+.12+.76*random01(seed^0x9e3779b9))*4;
   const dist=Math.hypot(x-camera.position.x,z-camera.position.z);if(dist>200||terrain.isWater(x,z))continue;
   const h=terrain.height(x,z),hx=terrain.height(x+1.5,z)-terrain.height(x-1.5,z),hz=terrain.height(x,z+1.5)-terrain.height(x,z-1.5),slope=Math.hypot(hx,hz)/3,ground=terrain.groundType(x,z),density=.78+.22*random01(Math.imul(ground,7919)+31);
   let moisture=0;for(const [vx,vz] of [[1,0],[-1,0],[0,1],[0,-1]]){if(terrain.isWater(x+vx!*12,z+vz!*12))moisture+=.35;if(terrain.isWater(x+vx!*24,z+vz!*24))moisture+=.15;}moisture=Math.min(1,moisture);
   if(dist<115&&slope<1.15){const cover=Math.min(1,density*lerp(.40,.96,patchDensity(x,z))+moisture*.25);for(let tuft=0;tuft<4;tuft++){
    const s=seed^Math.imul(tuft+1,0x27d4eb2f);if(random01(s)>cover)continue;const px=(cellX+random01(s^0x68bc21eb))*4,pz=(cellZ+random01(s^0x02e5be93))*4;if(terrain.isWater(px,pz))continue;const n=normal(px,pz);if(n.y<.65)continue;
    const choice=random01(s^0x85ebca6b),kind=choice<lerp(.68,.45,moisture)?0:choice<lerp(.88,.82,moisture)?1:2,scale=.85+.75*random01(s^0x165667b1),support=(kind===0?.20:kind===1?.65:.45)*scale;if(!dry(px,pz,support))continue;
    add(kind,px,terrain.visualHeight(px,pz)-.05,pz,random01(s^0xc2b2ae35)*2*Math.PI,scale,n);
   }}
   const landmark=random01(seed^0x165667b1),yaw=random01(seed^0xc2b2ae35)*2*Math.PI;
   if(landmark<.045*lerp(.5,1.3,patchDensity(x,z))+moisture*.055&&slope<.55&&dry(x,z,.7))add(3,x,terrain.visualHeight(x,z)-.08,z,yaw,.65+.95*random01(seed^0xd3a2646c),up);
   else if(landmark>lerp(.88,.97,THREE.MathUtils.clamp((h-15)/35,0,1))&&slope<1.15){const scale=.45+1.4*random01(seed^0xfd7046c5),n=normal(x,z);add(4,x,terrain.visualHeight(x,z)-rockHeight*.48*scale,z,yaw,scale,n);}
  }
  for(const child of this.nature.children)if(child instanceof THREE.InstancedMesh)child.dispose();this.nature.clear();for(let i=0;i<meshes.length;i++){const list=matrices[i]!;if(!list.length)continue;const source=meshes[i]!,mesh=new THREE.InstancedMesh(source.geometry,source.material,list.length);for(let n=0;n<list.length;n++)mesh.setMatrixAt(n,list[n]!);mesh.instanceMatrix.needsUpdate=true;mesh.computeBoundingSphere();this.nature.add(mesh);}this.natureKey=key;
 }
 update(frame:ModernFrame,world:WorldFrame,camera:THREE.Camera,terrainHeight:(x:number,z:number)=>number){
  if(frame.sequence!==world.sequence||frame.tick!==world.tick||frame.sessionGeneration!==world.sessionGeneration||frame.runtimeGeneration!==world.runtimeGeneration)throw Error('unmatched world and modern publication');
  if(this.epoch!==frame.epoch){for(const item of this.props.values())this.root.remove(item.root);for(const item of this.trees.values())this.root.remove(item.root);for(const item of this.mana.values())this.root.remove(item.root);this.props.clear();this.trees.clear();this.mana.clear();this.epoch=frame.epoch;}
  const entityBySlot=new Map(frame.entities.map(e=>[e.entityIndex,e])),seen=new Set<number>();
  for(const row of world.sprites){const entity=entityBySlot.get(row.entity);if(!entity)continue;const kind=assetKind(entity,row);if(!kind)continue;seen.add(row.entity);const identity=`${frame.epoch}:${row.source}`;let item=this.props.get(row.entity);
   if(item&&(item.identity!==identity||item.kind!==kind)){this.root.remove(item.root);this.props.delete(row.entity);item=undefined;}
   if(!item){const template=this.templates.get(kind);if(!template)continue;const root=new THREE.Group();root.add(template.clone(true));this.root.add(root);item={kind,identity,root,row};this.props.set(row.entity,item);}item.row=row;
  }
  for(const [slot,item] of this.props)if(!seen.has(slot)){this.root.remove(item.root);this.props.delete(slot);}
  const treeSeen=new Set<number>();
  for(const row of frame.trees){const isTree=row.classId===2&&row.model===0&&row.renderHeight>0&&!!row.drawing&&((row.action===0||row.action===1)&&(row.renderParameter===83||row.renderParameter===84)||row.action===2&&(row.renderParameter===226||row.renderParameter===227));if(!isTree)continue;
   const identity=`${frame.epoch}:${row.slotGeneration}:${row.sourceId}`;treeSeen.add(row.entityIndex);let item=this.trees.get(row.entityIndex);
   if(item&&item.identity!==identity){this.root.remove(item.root);this.trees.delete(row.entityIndex);item=undefined;}
   if(!item){const alive=this.templates.get('tree-alive')!.clone(true),burned=this.templates.get('tree-burned')!.clone(true),root=new THREE.Group();root.add(alive,burned);this.root.add(root);item={identity,root,alive,burned};this.trees.set(row.entityIndex,item);}
   item.alive.visible=row.action!==2;item.burned.visible=row.action===2;
   const x=camera.position.x+delta(row.x/64,camera.position.x),z=camera.position.z+delta(row.y/64,camera.position.z);item.root.position.set(x,terrainHeight(x,z),z);item.root.rotation.y=row.entityIndex*137.50776*Math.PI/180;item.root.scale.setScalar(row.renderHeight/64);
  }
  for(const [slot,item] of this.trees)if(!treeSeen.has(slot)){this.root.remove(item.root);this.trees.delete(slot);}
  const visible=new Set(world.sprites.filter(p=>p.drawOrder!==0xffffffff).map(p=>p.entity)),manaSeen=new Set<number>();
  for(const row of frame.manaSpheres){const slot=row.sourceEntityIndex;if(!row.active||!slot||!row.logicalHeight||!visible.has(slot))continue;const template=this.templates.get('mana-'+row.baseTmap);if(!template)continue;manaSeen.add(slot);const identity=`${frame.epoch}:${row.sphereId}`;let item=this.mana.get(slot);
   if(item&&(item.identity!==identity||item.selector!==row.baseTmap)){this.root.remove(item.root);this.mana.delete(slot);item=undefined;}
   if(!item){const root=new THREE.Group();root.add(template.clone(true));this.root.add(root);item={identity,root,selector:row.baseTmap};this.mana.set(slot,item);}
   const x=camera.position.x+delta(row.x/64,camera.position.x),z=camera.position.z+delta(row.y/64,camera.position.z);item.root.position.set(x,Math.max(row.z/64,terrainHeight(x,z))+row.logicalHeight/128,z);item.root.scale.setScalar(row.logicalHeight/64);
  }
  for(const [slot,item] of this.mana)if(!manaSeen.has(slot)){this.root.remove(item.root);this.mana.delete(slot);}
  for(const item of this.props.values()){const p=item.row,x=camera.position.x+delta(p.x/64,camera.position.x),z=camera.position.z+delta(p.y/64,camera.position.z),scale=p.height/64;let y=p.z/64;
   if(item.kind==='spell-jar')y=Math.max(y,terrainHeight(x,z));else if(item.kind==='stone-statue')y=terrainHeight(x,z);
   else if(item.kind==='dolmen')y=dolmenBaseHeight(x,z,scale,terrainHeight);
   else if(item.kind==='fireball'||item.kind==='possession')y+=p.height/128;
   item.root.position.set(x,y,z);item.root.scale.setScalar(scale);if(item.kind==='spell-jar'||item.kind==='experience-scroll'||item.kind==='exit-portal'||item.kind==='castle-head')item.root.rotation.y=camera.rotation.y+(item.kind==='experience-scroll'?38*Math.PI/180:0);}
  for(const b of this.layout.buildings){const node=this.buildings.get(b.source_slot);if(node)node.position.set(camera.position.x+delta(b.x,camera.position.x),b.base_height+.025,camera.position.z+delta(b.z,camera.position.z));}
 }
 private disposeBuilding(node:THREE.Group){node.traverse(o=>{if(o instanceof THREE.Mesh){o.geometry.dispose();if(o.material instanceof THREE.Material)o.material.dispose();}});}
 get replacedSlots():ReadonlySet<number>{return new Set([...this.props.keys(),...this.trees.keys(),...this.mana.keys()]);}
 get metrics(){return {buildings:this.buildings.size,props:this.props.size,trees:this.trees.size,mana:this.mana.size};}
 dispose(){for(const node of this.buildingTemplates.values())this.disposeBuilding(node);const geometries=new Set<THREE.BufferGeometry>(),materials=new Set<THREE.Material>(),textures=new Set<THREE.Texture>();
  for(const template of this.templates.values())template.traverse(o=>{if(!(o instanceof THREE.Mesh))return;geometries.add(o.geometry);for(const material of Array.isArray(o.material)?o.material:[o.material]){materials.add(material);if(material instanceof THREE.MeshStandardMaterial&&material.map)textures.add(material.map);}});
  for(const g of geometries)g.dispose();for(const m of materials)m.dispose();for(const t of textures)t.dispose();this.buildingRock?.dispose();this.templates.clear();this.buildingTemplates.clear();this.buildings.clear();this.props.clear();this.trees.clear();this.mana.clear();this.root.removeFromParent();}
}
