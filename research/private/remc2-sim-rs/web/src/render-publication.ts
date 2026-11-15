/**
 * MCPO v2 is one copied, coherent native publication. Decode before retaining
 * the result: a WebAssembly memory view can be overwritten or detached on growth.
 * No references to the supplied bytes survive this call.
 */
export type DeepReadonly<T> = T extends bigint | number | string | boolean | null | undefined
  ? T : T extends readonly (infer U)[] ? readonly DeepReadonly<U>[]
  : { readonly [K in keyof T]: DeepReadonly<T[K]> };

const MAX_PACKET = 16 * 1024 * 1024;
const MAX_STRING = MAX_PACKET;
const MAX_ITEMS = 65_536;
const utf8 = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });

class Reader {
  private readonly view: DataView;
  private readonly bytes: Uint8Array;
  private at = 0;
  constructor(bytes: Uint8Array) {
    this.bytes = bytes;
    if (bytes.byteLength > MAX_PACKET) throw new Error('render publication exceeds packet limit');
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  }
  private take(size: number): number {
    if (size > this.bytes.byteLength - this.at) throw new Error('truncated render publication');
    const start = this.at;
    this.at += size;
    return start;
  }
  u8(): number { return this.view.getUint8(this.take(1)); }
  i8(): number { return this.view.getInt8(this.take(1)); }
  u16(): number { return this.view.getUint16(this.take(2), true); }
  i16(): number { return this.view.getInt16(this.take(2), true); }
  u32(): number { return this.view.getUint32(this.take(4), true); }
  i32(): number { return this.view.getInt32(this.take(4), true); }
  u64(): bigint { return this.view.getBigUint64(this.take(8), true); }
  bool(): boolean {
    const value = this.u8();
    if (value > 1) throw new Error('invalid render publication boolean');
    return value === 1;
  }
  option<T>(read: () => T): T | null { return this.bool() ? read() : null; }
  string(): string {
    const size = this.u32();
    if (size > MAX_STRING) throw new Error('render publication string exceeds limit');
    const start = this.take(size);
    return utf8.decode(this.bytes.subarray(start, start + size));
  }
  array<T>(minimumSize: number, read: () => T): T[] {
    const count = this.u32();
    if (count > MAX_ITEMS || count > Math.floor((this.bytes.byteLength - this.at) / minimumSize)) {
      throw new Error('render publication array exceeds available bytes or item limit');
    }
    return Array.from({ length: count }, read);
  }
  enumeration<T extends string>(values: readonly T[]): T {
    const value = values[this.u8()];
    if (value === undefined) throw new Error('invalid render publication enum');
    return value;
  }
  end(): void {
    if (this.at !== this.bytes.byteLength) throw new Error('trailing render publication bytes');
  }
}

function freeze<T>(value: T): DeepReadonly<T> {
  if (value !== null && typeof value === 'object') {
    for (const child of Object.values(value)) freeze(child);
    Object.freeze(value);
  }
  return value as DeepReadonly<T>;
}

const menuActions = ['toggle_gameplay_map', 'open_abandon_confirmation', 'confirm_abandon',
  'cancel_abandon_confirmation', 'open_spell_menu', 'close_spell_menu',
  'show_player_information', 'hide_player_information', 'close_gameplay_menus'] as const;
const hands = ['none', 'left', 'right', 'both'] as const;
const markedHands = ['none', 'left', 'right'] as const;
const availability = ['ready', 'not_acquired', 'invalid_holder', 'invalid_caster', 'active', 'insufficient_mana'] as const;

/**
 * Decode one complete MCPO packet. This reader's resource limits are 16 MiB
 * per packet and per string and 65,536 items per array; these are
 * stricter than the wire format's u32 length limits. Malformed, unsupported or
 * larger publications throw before any result is exposed.
 */
export function decodeRenderPublication(bytes: Uint8Array) {
  const r = new Reader(bytes);
  if (r.u8() !== 0x4d || r.u8() !== 0x43 || r.u8() !== 0x50 || r.u8() !== 0x4f) throw new Error('invalid render publication magic');
  if (r.u32() !== 2) throw new Error('unsupported render publication version');
  if (r.u32() !== bytes.byteLength) throw new Error('render publication length mismatch');
  const runtime_id = r.u64();
  const runtime_generation = r.u64();
  const runtime_mode = r.enumeration(['playing', 'paused'] as const);
  const identity = {
    engine_contract: r.string(), package_graph_hash: r.string(), definition_hash: r.string(),
    definition_revision: r.u64(), session_id: r.u64(), generation: r.u64(), tick: r.u64(),
  };
  const terrain_mutation_sequence = r.u64();
  const tick = r.u64();
  const effect_head = r.u64();
  if (tick !== identity.tick) throw new Error('render publication tick mismatch');
  const player_pose = { x: r.u16(), y: r.u16(), z: r.i16(), yaw: r.i16(),
    display_pitch: r.i16(), effective_pitch: r.i16(), roll: r.i16() };
  const player_motion = { target_speed: r.i16(), actual_speed: r.i16(), strafe_speed: r.i16() };
  const player_vitals = { life: r.i32(), max_life: r.i32(), mana: r.i32(), max_mana: r.i32() };
  const source_menu_admission = r.option(() => ({
    action: r.enumeration(menuActions), admitted: r.bool(), map_open: r.bool(), spells_open: r.bool(),
    player_information_open: r.bool(), tick: r.u64(),
  }));
  const return_to_title_requested = r.bool();
  const players = r.array(27, () => ({
    source_slot: r.u8(), display_name: r.string(), outline_palette_index: r.u8(),
    text_palette_index: r.u8(), max_mana: r.i32(), kills8: Array.from({ length: 8 }, () => r.i16()),
  }));
  const source_scoreboard = { players, projection_error: r.option(() => r.string()) };
  const participant_notifications = r.array(19, () => ({ participant: r.u64(), is_local: r.bool(),
    display_name: r.string(), kind: r.string(), remaining_frames: r.i16() }));
  const player_wall_contact_counter = r.i8();
  const player_immobilized_ticks = r.u16();
  const source_camera_pose = { x: r.u16(), y: r.u16(), z: r.i16(), yaw: r.i16(), pitch: r.i16(), roll: r.i16() };
  const source_camera_fov = r.i16();
  const player_spells = r.array(22, () => {
    const id = r.string(); const display_name = r.string(); const acquired = r.bool();
    const holder_actor = r.option(() => r.u64()); const selected = r.bool();
    const tier = r.i8(); const unlocked_tier = r.u8(); const menu_slot = r.u8();
    const menu_artwork = r.option(() => ({ grid_icon: r.u16(), tier_icons: Array.from({ length: 3 }, () => r.u16()) }));
    const mana_cost = r.i32(); const hand = r.enumeration(hands); const marked_hand = r.enumeration(markedHands);
    const spellAvailability = r.enumeration(availability);
    return { id, display_name, acquired, ...(holder_actor === null ? {} : { holder_actor }), selected,
      tier, unlocked_tier, menu_slot, ...(menu_artwork === null ? {} : { menu_artwork }), mana_cost,
      hand, marked_hand, availability: spellAvailability };
  });
  const objective_progress = { current_objective: r.option(() => r.string()), completed_count: r.u32(),
    total_count: r.u32(), all_objectives_satisfied: r.bool() };
  const objective_textbox_ticks = r.u8();
  const announcementRecord = r.u16();
  if (announcementRecord !== 0 && announcementRecord !== 284 && announcementRecord !== 285)
    throw new Error('invalid objective announcement record');
  const objective_secret_announcement_record = announcementRecord === 0 ? null : announcementRecord;
  const exit_sequence_active = r.bool();
  const presentation_entities = r.array(32, () => ({ actor: r.u64(), kind: r.string(),
    pose: { x: r.u16(), y: r.u16(), z: r.i16(), yaw: r.i16(), pitch: r.i16() },
    logical_height: r.u16(), asset: { id: r.string(), content_address: r.string() } }));
  r.end();
  return freeze({ runtime_id, runtime_generation, runtime_mode, identity, terrain_mutation_sequence,
    value: { tick, effect_head, player_pose, player_motion, player_vitals,
      ...(source_menu_admission === null ? {} : { source_menu_admission }), return_to_title_requested,
      source_scoreboard, participant_notifications, player_wall_contact_counter,
      player_immobilized_ticks, source_camera_pose,
      source_camera_fov, player_spells, objective_progress, objective_textbox_ticks,
      objective_secret_announcement_record, exit_sequence_active,
      presentation_entities } });
}

export type RenderPublication = ReturnType<typeof decodeRenderPublication>;
