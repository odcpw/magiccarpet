/** Native campaign observations bound to one MCW1 publication. All identity counters are exact u64. */
export interface ModernEntity {
  entityIndex: number; sourceId: number; nextRelation: number; parameterRelation: number;
  authority: number; active: number; classId: number; model: number; action: number; phase: number;
  x: number; y: number; z: number; yaw: number; pitch: number; fov: number; speed: number;
}
export interface ModernCharacter {
  slotGeneration: number; parentGeneration: number; entityIndex: number; sourceId: number;
  parentIndex: number; parentId: number; classId: number; model: number; action: number; phase: number;
  active: number; hasTarget: number; hasParent: number; stance: number; x: number; y: number; z: number;
  yaw: number; pitch: number; speed: number; life: number; targetIndex: number; targetX: number;
  targetY: number; targetZ: number; controlFlags: number; ageTicks: number; renderParameter: number;
  roll: number; sourceHalfWidth: number; renderHeight: number; previousIndex: number; nextIndex: number;
  partIndex: number; lifecycleFlags: number;
}
export interface ModernBuilding {
  slotGeneration: number; sourceSlot: number; sourceId: number; buildingIndex: number;
  action: number; active: number; drawing: number; x: number; y: number; z: number;
}
export interface ModernCastleChild {
  slotGeneration: number; castleGeneration: number; targetGeneration: number; entityIndex: number;
  sourceId: number; ownerEntityIndex: number; playerEntityIndex: number; castleIndex: number;
  targetIndex: number; x: number; y: number; z: number; yaw: number; speed: number;
  renderParameter: number; renderHeight: number; life: number; maxLife: number; mana: number;
  maxMana: number; controlFlags: number; classId: number; model: number; action: number;
  phase: number; active: number; drawing: number; ownerPlayerColor: number; relationFlags: number;
}
export interface ModernTree {
  slotGeneration: number; entityIndex: number; sourceId: number; authoredIndex: number;
  renderParameter: number; renderHeight: number; x: number; y: number; z: number;
  life: number; controlFlags: number; classId: number; model: number; action: number; drawing: number;
}
export interface ModernManaSphere {
  sphereId: number; sourceEntityIndex: number; ownerEntityIndex: number; authority: number;
  active: number; classId: number; model: number; action: number; phase: number; renderMode: number;
  x: number; y: number; z: number; yaw: number; velocityX: number; velocityY: number; verticalVelocity: number;
  renderParameterIndex: number; baseTmap: number; logicalHeight: number; mana: number;
  assignedCarrierId: number; attractionSource: number; radialWords: number; playerRelation: number;
}
export interface ModernFrame {
  runtimeGeneration: bigint; sessionGeneration: bigint; tick: bigint; sequence: bigint; epoch: bigint;
  entities: ModernEntity[]; characters: ModernCharacter[]; buildings: ModernBuilding[]; castleChildren: ModernCastleChild[]; trees: ModernTree[]; manaSpheres: ModernManaSphere[];
}

/** Reject truncation, unexpected ABI layouts, duplicate slots and stale native rows. */
export function decodeModernFrame(buffer: ArrayBuffer): ModernFrame {
  const d = new DataView(buffer), len = d.byteLength;
  if (len < 96 || d.getUint32(0, true) !== 0x3144434d || d.getUint16(4, true) !== 1 || d.getUint16(6, true) !== 96)
    throw new Error('invalid MCD1 header');
  const u8 = (o: number) => d.getUint8(o), u16 = (o: number) => d.getUint16(o, true),
    i16 = (o: number) => d.getInt16(o, true), u32 = (o: number) => d.getUint32(o, true),
    i32 = (o: number) => d.getInt32(o, true), u64 = (o: number) => d.getBigUint64(o, true);
  const runtimeGeneration = u64(8), sessionGeneration = u64(16), tick = u64(24), sequence = u64(32), epoch = u64(40);
  const counts = [u32(48), u32(56), u32(64), u32(72), u32(80), u32(88)] as const, sizes = [u32(52), u32(60), u32(68), u32(76), u32(84), u32(92)] as const;
  if (sizes.some((n, i) => n !== [52, 96, 48, 96, 56, 64][i]) || counts.some(n => n > 65536) ||
      96 + counts.reduce((sum, n, i) => sum + n * sizes[i]!, 0) !== len) throw new Error('invalid MCD1 bounds');
  let p = 96;
  function rows<T>(index: number, keyOffset: number, decode: (o: number) => T): T[] {
    const result: T[] = [], seen = new Set<number>(), size = sizes[index]!;
    for (let i = 0; i < counts[index]!; i++, p += size) {
      if (u32(p) !== 2 || u32(p + 4) !== size ||
          (index !== 0 && (u64(p + 8) !== sessionGeneration || u64(p + 16) !== tick))) throw new Error('stale MCD1 row');
      const key = u16(p + keyOffset);
      if (seen.has(key)) throw new Error('duplicate MCD1 slot');
      seen.add(key); result.push(decode(p));
    }
    return result;
  }
  const entities = rows(0, 8, o => ({entityIndex:u16(o+8),sourceId:u16(o+10),nextRelation:u16(o+12),parameterRelation:u16(o+14),
    authority:u8(o+16),active:u8(o+17),classId:u8(o+18),model:u8(o+19),action:u8(o+20),phase:u8(o+21),
    x:u16(o+22),y:u16(o+24),z:i16(o+26),yaw:i16(o+28),pitch:i16(o+30),fov:i16(o+32),speed:i16(o+34)}));
  const characters = rows(1, 32, o => ({slotGeneration:u32(o+24),parentGeneration:u32(o+28),entityIndex:u16(o+32),
    sourceId:u16(o+34),parentIndex:u16(o+36),parentId:u16(o+38),classId:u8(o+40),model:u8(o+41),action:u8(o+42),
    phase:u8(o+43),active:u8(o+44),hasTarget:u8(o+45),hasParent:u8(o+46),stance:u8(o+47),x:u16(o+48),y:u16(o+50),
    z:i16(o+52),yaw:i16(o+54),pitch:i16(o+56),speed:i16(o+58),life:i32(o+60),targetIndex:u16(o+64),targetX:u16(o+66),
    targetY:u16(o+68),targetZ:i16(o+70),controlFlags:u32(o+72),ageTicks:u16(o+76),renderParameter:u16(o+78),
    roll:i16(o+80),sourceHalfWidth:u16(o+82),renderHeight:u16(o+84),previousIndex:u16(o+86),nextIndex:u16(o+88),
    partIndex:u16(o+90),lifecycleFlags:u32(o+92)}));
  const buildings = rows(2, 28, o => ({slotGeneration:u32(o+24),sourceSlot:u16(o+28),sourceId:u16(o+30),
    buildingIndex:u8(o+32),action:u8(o+33),active:u8(o+34),drawing:u8(o+35),x:u16(o+36),y:u16(o+38),z:i16(o+40)}));
  const castleChildren = rows(3, 36, o => ({slotGeneration:u32(o+24),castleGeneration:u32(o+28),targetGeneration:u32(o+32),
    entityIndex:u16(o+36),sourceId:u16(o+38),ownerEntityIndex:u16(o+40),playerEntityIndex:u16(o+42),castleIndex:u16(o+44),
    targetIndex:u16(o+46),x:u16(o+48),y:u16(o+50),z:i16(o+52),yaw:i16(o+54),speed:i16(o+56),renderParameter:u16(o+58),
    renderHeight:u16(o+60),life:i32(o+64),maxLife:i32(o+68),mana:i32(o+72),maxMana:i32(o+76),controlFlags:u32(o+80),
    classId:u8(o+84),model:u8(o+85),action:u8(o+86),phase:u8(o+87),active:u8(o+88),drawing:u8(o+89),
    ownerPlayerColor:u8(o+90),relationFlags:u8(o+91)}));
  const trees = rows(4, 28, o => ({slotGeneration:u32(o+24),entityIndex:u16(o+28),sourceId:u16(o+30),
    authoredIndex:u16(o+32),renderParameter:u16(o+34),renderHeight:u16(o+36),x:u16(o+38),y:u16(o+40),z:i16(o+42),
    life:i32(o+44),controlFlags:u32(o+48),classId:u8(o+52),model:u8(o+53),action:u8(o+54),drawing:u8(o+55)}));
  // Mana rows are guarded by their native batch generation/tick rather than
  // per-row identity. Inactive retained rows are part of the native census.
  const manaSpheres: ModernManaSphere[] = [];
  for (let i = 0; i < counts[5]; i++, p += 64) {
    if (u32(p) !== 2 || u32(p+4) !== 64) throw new Error('invalid MCD1 mana row');
    manaSpheres.push({sphereId:u32(p+8),sourceEntityIndex:u16(p+12),ownerEntityIndex:u16(p+14),
      authority:u8(p+16),active:u8(p+17),classId:u8(p+18),model:u8(p+19),action:u8(p+20),phase:u8(p+21),
      renderMode:u8(p+22),x:u16(p+24),y:u16(p+26),z:i16(p+28),yaw:i16(p+30),velocityX:i16(p+32),
      velocityY:i16(p+34),verticalVelocity:i16(p+36),renderParameterIndex:u16(p+38),baseTmap:u16(p+40),
      logicalHeight:u16(p+42),mana:i32(p+44),assignedCarrierId:u32(p+48),attractionSource:u32(p+52),
      radialWords:u32(p+56),playerRelation:u32(p+60)});
  }
  return {runtimeGeneration, sessionGeneration, tick, sequence, epoch, entities, characters, buildings, castleChildren, trees, manaSpheres};
}
