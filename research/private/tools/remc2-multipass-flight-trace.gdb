set pagination off
set confirm off
set debuginfod enabled off
set env SDL_AUDIODRIVER dummy

if $_isvoid($trace_speed_index)
  set $trace_speed_index = 1
end
if $_isvoid($trace_flags)
  set $trace_flags = 1
end
if $_isvoid($trace_roll)
  set $trace_roll = 0
end
if $_isvoid($trace_pitch)
  set $trace_pitch = 0
end
if $_isvoid($trace_recoil_speed)
  set $trace_recoil_speed = 0
end
if $_isvoid($trace_recoil_yaw)
  set $trace_recoil_yaw = 0
end
if $_isvoid($trace_speed_modifier)
  set $trace_speed_modifier = 0
end
if $_isvoid($trace_speed_modifier_counter)
  set $trace_speed_modifier_counter = 0
end
if $_isvoid($trace_immobilize_counter)
  set $trace_immobilize_counter = 0
end
if $_isvoid($trace_immobilize_subcounter)
  set $trace_immobilize_subcounter = 0
end
if $_isvoid($trace_override_motion)
  set $trace_override_motion = 0
end
if $_isvoid($trace_x)
  set $trace_x = 0
end
if $_isvoid($trace_y)
  set $trace_y = 0
end
if $_isvoid($trace_z)
  set $trace_z = 0
end
if $_isvoid($trace_yaw)
  set $trace_yaw = 0
end
if $_isvoid($trace_target_speed)
  set $trace_target_speed = 0
end
if $_isvoid($trace_actual_speed)
  set $trace_actual_speed = 0
end
if $_isvoid($trace_tether_target)
  set $trace_tether_target = 0
end
if $_isvoid($trace_tether_holder)
  set $trace_tether_holder = 0
end
if $_isvoid($trace_tether_tier)
  set $trace_tether_tier = 0
end
if $_isvoid($trace_tether_prior_distance)
  set $trace_tether_prior_distance = 0
end
set $outer_turn = 0
set $player_pass = 0

# Leave the first outer turn untouched so its end state is the preserved
# level-1 frame zero. On the second turn, inject one forward command and select
# either the four-pass (speed index 1) or eight-pass (speed index 2) scheduler.
break PlayerEvents_51BB0
commands
  silent
  set $outer_turn = $outer_turn + 1
  if $outer_turn == 2
    set $slot = (int)D41A0_0.LevelIndex_0xc
    set x_D41A0_BYTEARRAY_4_struct.speedIndex = $trace_speed_index
    set D41A0_0.playerInputs_0x6E3E[$slot].PlayerAction_byte0 = 0
    set D41A0_0.playerInputs_0x6E3E[$slot].str_0x6E3E_byte1 = 0
    set D41A0_0.playerInputs_0x6E3E[$slot].str_0x6E3E_byte2 = 0
    set D41A0_0.playerInputs_0x6E3E[$slot].roll = $trace_roll
    set D41A0_0.playerInputs_0x6E3E[$slot].pitch = $trace_pitch
    set D41A0_0.playerInputs_0x6E3E[$slot].entityIndex_0x6E3E_byte5 = $trace_flags
    set D41A0_0.playerInputs_0x6E3E[$slot].nextEntity_0x6E3E_word6 = 0
    set D41A0_0.playerInputs_0x6E3E[$slot].entityIndex2_0x6E3E_word8 = 0
    set $player = Entities_EA3E4[D41A0_0.array_0x2BDE[$slot].playerIndex_0x00a_2BE4_11240]
    set $player->dword_0xA4_164x->moveBoost_0x1E_30 = $trace_recoil_speed
    set $player->dword_0xA4_164x->yaw_0x1E_30 = $trace_recoil_yaw
    set $player->dword_0xA4_164x->moveSpeed_0x14C_332 = $trace_speed_modifier
    set $player->dword_0xA4_164x->moveSpeedCounter_0x14D_333 = $trace_speed_modifier_counter
    set $player->dword_0xA4_164x->mobilizeCounter_0x14E_334 = $trace_immobilize_counter
    set $player->dword_0xA4_164x->mobilizeCounter2_0x150_336 = $trace_immobilize_subcounter
    if $trace_override_motion
      set $player->position_0x4C_76.x = $trace_x
      set $player->position_0x4C_76.y = $trace_y
      set $player->position_0x4C_76.z = $trace_z
      set $player->yaw_0x1C_28 = $trace_yaw
      set $player->dword_0xA4_164x->speed_0xc_12 = $trace_target_speed
      set $player->actSpeed_0x82_130 = $trace_actual_speed
    end
    if $trace_tether_target
      set $player->dword_0xA4_164x->dword_0x142_322 = $trace_tether_prior_distance
      set $player->dword_0xA4_164x->word_0x146_326 = $trace_tether_target
      set $player->dword_0xA4_164x->word_0x14A_330 = $trace_tether_tier
      set $player->dword_0xA4_164x->str_611.SpellsEnabled_0x333_819x.SpellEnabled[14] = $trace_tether_holder
      set Entities_EA3E4[$trace_tether_holder]->word_0x2E_46 = 1
    end
    printf "TRACE_BEGIN speed_index=%d flags=%d roll=%d pitch=%d recoil=%d@%d modifier=%d/%d immobilize=%d/%d override=%d pos=%u,%u,%d yaw=%d speed=%d/%d tether=%d/%d/%d/%d slot=%d\n", $trace_speed_index, $trace_flags, $trace_roll, $trace_pitch, $trace_recoil_speed, $trace_recoil_yaw, $trace_speed_modifier, $trace_speed_modifier_counter, $trace_immobilize_counter, $trace_immobilize_subcounter, $trace_override_motion, $player->position_0x4C_76.x, $player->position_0x4C_76.y, $player->position_0x4C_76.z, $player->yaw_0x1C_28, $player->dword_0xA4_164x->speed_0xc_12, $player->actSpeed_0x82_130, $player->dword_0xA4_164x->dword_0x142_322, $player->dword_0xA4_164x->word_0x146_326, $player->dword_0xA4_164x->str_611.SpellsEnabled_0x333_819x.SpellEnabled[14], $player->dword_0xA4_164x->word_0x14A_330, $slot
  end
  continue
end

# sub_585D0 is called exactly once near the end of every UpdateEntities_57730
# pass, after the entity dispatch loop has updated the local player. Reading
# the player through the global slot avoids relying on the optimized lifetime
# of UpdateEntities' local `mx` variable.
break sub_585D0
commands
  silent
  if $outer_turn == 2
    set $player = Entities_EA3E4[D41A0_0.array_0x2BDE[D41A0_0.LevelIndex_0xc].playerIndex_0x00a_2BE4_11240]
    set $player_pass = $player_pass + 1
    printf "TRACE pass=%d pos=%u,%u,%d yaw=%d display_pitch=%d effective_pitch=%d roll_acc=%d pitch_acc=%d target=%d changed=%d actual=%d strafe=%d recoil=%d@%d modifier=%d/%d immobilize=%d/%d water=%d recovery=%d tether=%d/%d/%d/%d phase=%u\n", $player_pass, $player->position_0x4C_76.x, $player->position_0x4C_76.y, $player->position_0x4C_76.z, $player->yaw_0x1C_28, $player->pitch_0x1E_30, $player->dword_0xA4_164x->pitch_0x24_36, $player->dword_0xA4_164x->roll_0x155_341, $player->dword_0xA4_164x->pitch_0x157_343, $player->dword_0xA4_164x->speed_0xc_12, $player->dword_0xA4_164x->word_0xe_14, $player->actSpeed_0x82_130, $player->dword_0xA4_164x->strafeSpeed_0x10_16, $player->dword_0xA4_164x->moveBoost_0x1E_30, $player->dword_0xA4_164x->yaw_0x1E_30, $player->dword_0xA4_164x->moveSpeed_0x14C_332, $player->dword_0xA4_164x->moveSpeedCounter_0x14D_333, $player->dword_0xA4_164x->mobilizeCounter_0x14E_334, $player->dword_0xA4_164x->mobilizeCounter2_0x150_336, $player->dword_0xA4_164x->waterCounter_0x262_610, $player->dword_0xA4_164x->byte_0x261_609, $player->dword_0xA4_164x->dword_0x142_322, $player->dword_0xA4_164x->word_0x146_326, $player->dword_0xA4_164x->str_611.SpellsEnabled_0x333_819x.SpellEnabled[14], $player->dword_0xA4_164x->word_0x14A_330, $player->byte_0x3E_62
  end
  continue
end

run
