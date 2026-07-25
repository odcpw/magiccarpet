set pagination off
set confirm off
set debuginfod enabled off
set env SDL_AUDIODRIVER dummy

if $_isvoid($trace_fire_flags)
  set $trace_fire_flags = 0x10
end
set $outer_turn = 0
set $captured_activation = 0

# Leave the first turn untouched so the loaded state reaches the preserved
# frame-zero checkpoint. Inject only the requested fire bits for turn two.
break PlayerEvents_51BB0
commands
  silent
  set $outer_turn = $outer_turn + 1
  if $outer_turn == 2
    set $slot = (int)D41A0_0.LevelIndex_0xc
    set x_D41A0_BYTEARRAY_4_struct.speedIndex = 0
    set D41A0_0.playerInputs_0x6E3E[$slot].PlayerAction_byte0 = 0
    set D41A0_0.playerInputs_0x6E3E[$slot].str_0x6E3E_byte1 = 0
    set D41A0_0.playerInputs_0x6E3E[$slot].str_0x6E3E_byte2 = 0
    set D41A0_0.playerInputs_0x6E3E[$slot].roll = 0
    set D41A0_0.playerInputs_0x6E3E[$slot].pitch = 0
    set D41A0_0.playerInputs_0x6E3E[$slot].entityIndex_0x6E3E_byte5 = $trace_fire_flags
    set D41A0_0.playerInputs_0x6E3E[$slot].nextEntity_0x6E3E_word6 = 0
    set D41A0_0.playerInputs_0x6E3E[$slot].entityIndex2_0x6E3E_word8 = 0
    set $player = Entities_EA3E4[D41A0_0.array_0x2BDE[$slot].playerIndex_0x00a_2BE4_11240]
    set $left_slot = (int)$player->dword_0xA4_164x->str_611.SpellIndexLeft_0x451_1105
    set $left_index = (int)$player->dword_0xA4_164x->str_611.SpellsEnabled_0x333_819x.SpellEnabled[$left_slot]
    set $left_holder = Entities_EA3E4[$left_index]
    printf "FIRE_BEFORE slot=%d flags=%d player=%u model=%u mana=%d control=%u mode=%d left_slot=%d holder=%d holder_model=%u level=%d active=%d reset=%u cost=%d marker=%d phase=%u\n", $slot, $trace_fire_flags, $player->id_0x1A_26, $player->model_0x40_64, $player->mana_0x90_144, $player->struct_byte_0xc_12_15.dword, $player->dword_0xA4_164x->byte_0x1BF_447, $left_slot, $left_index, $left_holder->model_0x40_64, $left_holder->byte_0x46_70, $left_holder->word_0x2E_46, $left_holder->word_0x30_48, $left_holder->maxMana_0x8C_140, $left_holder->byte_0x3C_60, $player->byte_0x3E_62
  end
  continue
end

# Cold activation (`sub_5F7B0`) writes the holder timer and caster direction
# bits before calling `sub_5F7E0`. This entry is therefore the narrowest exact
# checkpoint after the activation mutation and before any later entity update.
break *sub_5F7E0+12
commands
  silent
  if $outer_turn == 2 && $captured_activation == 0
    if a2x == $player
      set $captured_activation = 1
      set $left_slot = (int)$player->dword_0xA4_164x->str_611.SpellIndexLeft_0x451_1105
      set $left_index = (int)$player->dword_0xA4_164x->str_611.SpellsEnabled_0x333_819x.SpellEnabled[$left_slot]
      set $left_holder = Entities_EA3E4[$left_index]
      printf "FIRE_AFTER slot=%d flags=%d player=%u model=%u mana=%d control=%u mode=%d left_slot=%d holder=%d holder_model=%u level=%d active=%d reset=%u cost=%d marker=%d phase=%u\n", $slot, $trace_fire_flags, $player->id_0x1A_26, $player->model_0x40_64, $player->mana_0x90_144, $player->struct_byte_0xc_12_15.dword, $player->dword_0xA4_164x->byte_0x1BF_447, $left_slot, $left_index, $left_holder->model_0x40_64, $left_holder->byte_0x46_70, $left_holder->word_0x2E_46, $left_holder->word_0x30_48, $left_holder->maxMana_0x8C_140, $left_holder->byte_0x3C_60, $player->byte_0x3E_62
    end
  end
  continue
end

break *sub_5F660+19
commands
  silent
  if $outer_turn == 2
    printf "FIRE_GATE caster=%u holder=%u holder_model=%u direction=%d\n", a1x->id_0x1A_26, a2x->id_0x1A_26, a2x->model_0x40_64, a3
  end
  continue
end

run
