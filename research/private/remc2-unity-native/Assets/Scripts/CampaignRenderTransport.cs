using System;
using System.Globalization;
using System.IO;
using System.Text;

namespace MagicCarpet.Native
{
    // MCPO v2 is shared with sim-session and the browser DataView reader.
    // This adapter owns its reusable copy buffer; no native pointer escapes.
    internal sealed class CampaignRenderTransport
    {
        internal const int MaximumBytes = 16 * 1024 * 1024;
        private byte[] buffer = new byte[64 * 1024];
        private static readonly string[] Modes = { "playing", "paused" };
        private static readonly string[] MenuActions = {
            "toggle_gameplay_map", "open_abandon_confirmation", "confirm_abandon",
            "cancel_abandon_confirmation", "open_spell_menu", "close_spell_menu",
            "show_player_information", "hide_player_information", "close_gameplay_menus"
        };
        private static readonly string[] Hands = { "none", "left", "right", "both" };
        private static readonly string[] MarkHands = { "none", "left", "right" };
        private static readonly string[] Availability = {
            "ready", "not_acquired", "invalid_holder", "invalid_caster", "active", "insufficient_mana"
        };

        internal sealed class Publication
        {
            internal ulong RuntimeId, RuntimeGeneration;
            internal string Mode;
            internal EngineSessionTransport.CampaignRenderObservation Observation;
        }

        internal Publication Read(ulong handle, ulong runtimeGeneration,
            ulong sessionGeneration, ulong tick)
        {
            var nativeStart = ModernCapture.BeginMeasurement();
            var status = NativeSim.mc_game_runtime_render_observation_copy_v1(handle,
                runtimeGeneration, sessionGeneration, tick, buffer, (uint)buffer.Length, out var length);
            if (status == NativeSim.BufferTooSmall)
            {
                if (length <= buffer.Length || length > MaximumBytes)
                    throw new InvalidDataException("Invalid render publication capacity");
                buffer = new byte[checked((int)length)];
                status = NativeSim.mc_game_runtime_render_observation_copy_v1(handle,
                    runtimeGeneration, sessionGeneration, tick, buffer, (uint)buffer.Length, out length);
            }
            EngineSessionTransport.Check(status, "copy render publication");
            ModernCapture.EndMeasurement("engine_native_RenderPublication", nativeStart);
            var decodeStart = ModernCapture.BeginMeasurement();
            var publication = Decode(buffer, checked((int)length));
            ModernCapture.EndMeasurement("engine_decode_RenderPublication", decodeStart);
            return publication;
        }

        internal static Publication Decode(byte[] bytes, int length)
        {
            var r = new Reader(bytes, length);
            if (r.U32() != 0x4f50434d || r.U32() != 2 || r.U32() != length)
                throw new InvalidDataException("Unsupported render publication header");
            var result = new Publication { RuntimeId = r.U64(), RuntimeGeneration = r.U64() };
            result.Mode = r.Enum(Modes);
            var identity = new EngineSessionTransport.Identity {
                engine_contract = r.Text(), package_graph_hash = r.Text(), definition_hash = r.Text(),
                definition_revision = r.Counter(), session_id = r.Counter(),
                generation = r.Counter(), tick = r.Counter()
            };
            var observation = new EngineSessionTransport.CampaignRenderObservation {
                contract = "carpet-flight-campaign-render-observation/v1",
                identity = identity, terrain_mutation_sequence = r.Counter()
            };
            result.Observation = observation;
            var value = new EngineSessionTransport.CampaignRenderValue {
                tick = r.Counter(), effect_head = r.Counter(),
                player_pose = new EngineSessionTransport.Pose {
                    x = r.U16(), y = r.U16(), z = r.I16(), yaw = r.I16(),
                    display_pitch = r.I16(), effective_pitch = r.I16(), roll = r.I16()
                },
                player_motion = new EngineSessionTransport.Motion {
                    target_speed = r.I16(), actual_speed = r.I16(), strafe_speed = r.I16()
                },
                player_vitals = new EngineSessionTransport.PlayerVitals {
                    life = r.I32(), max_life = r.I32(), mana = r.I32(), max_mana = r.I32()
                }
            };
            observation.value = value;
            if (r.Bool())
                value.source_menu_admission = new EngineSessionTransport.SourceMenuAdmission {
                    action = r.Enum(MenuActions),
                    admitted = r.Bool(), map_open = r.Bool(), spells_open = r.Bool(),
                    player_information_open = r.Bool(), tick = r.Counter()
                };
            value.return_to_title_requested = r.Bool();
            value.source_scoreboard = new EngineSessionTransport.SourceScoreboard {
                players = new EngineSessionTransport.SourceSorcererScore[r.Count(27)]
            };
            for (var i = 0; i < value.source_scoreboard.players.Length; i++)
            {
                var row = new EngineSessionTransport.SourceSorcererScore {
                    source_slot = r.Byte(), display_name = r.Text(),
                    outline_palette_index = r.Byte(), text_palette_index = r.Byte(),
                    max_mana = r.I32(), kills8 = new short[8]
                };
                for (var k = 0; k < row.kills8.Length; k++) row.kills8[k] = r.I16();
                value.source_scoreboard.players[i] = row;
            }
            value.source_scoreboard.projection_error = r.OptionalText();
            value.participant_notifications = new EngineSessionTransport.ParticipantNotification[r.Count(19)];
            for (var i = 0; i < value.participant_notifications.Length; i++)
                value.participant_notifications[i] = new EngineSessionTransport.ParticipantNotification {
                    participant = r.Counter(), is_local = r.Bool(), display_name = r.Text(),
                    kind = r.Text(), remaining_frames = r.I16()
                };
            value.player_wall_contact_counter = unchecked((sbyte)r.Byte());
            value.player_immobilized_ticks = r.U16();
            value.source_camera_pose = new EngineSessionTransport.CameraPose {
                x = r.U16(), y = r.U16(), z = r.I16(), yaw = r.I16(), pitch = r.I16(), roll = r.I16()
            };
            value.source_camera_fov = r.I16();
            value.player_spells = new EngineSessionTransport.SpellObservation[r.Count(22)];
            for (var i = 0; i < value.player_spells.Length; i++)
            {
                var row = new EngineSessionTransport.SpellObservation {
                    id = r.Text(), display_name = r.Text(), acquired = r.Bool(),
                    holder_actor = r.Bool() ? r.Counter() : null,
                    selected = r.Bool(), tier = unchecked((sbyte)r.Byte()),
                    unlocked_tier = r.Byte(), menu_slot = r.Byte()
                };
                if (r.Bool())
                    row.menu_artwork = new EngineSessionTransport.SpellMenuArtwork {
                        grid_icon = r.U16(), tier_icons = new[] { r.U16(), r.U16(), r.U16() }
                    };
                row.mana_cost = r.I32();
                row.hand = r.Enum(Hands);
                row.marked_hand = r.Enum(MarkHands);
                row.availability = r.Enum(Availability);
                value.player_spells[i] = row;
            }
            value.objective_progress = new EngineSessionTransport.ObjectiveProgress {
                current_objective = r.OptionalText(), completed_count = r.U32(),
                total_count = r.U32(), all_objectives_satisfied = r.Bool()
            };
            value.objective_textbox_ticks = r.Byte();
            value.objective_secret_announcement_record = r.U16();
            value.exit_sequence_active = r.Bool();
            value.presentation_entities = new EngineSessionTransport.PresentationEntity[r.Count(32)];
            for (var i = 0; i < value.presentation_entities.Length; i++)
                value.presentation_entities[i] = new EngineSessionTransport.PresentationEntity {
                    actor = r.Counter(), kind = r.Text(),
                    pose = new EngineSessionTransport.PresentationPose {
                        x = r.U16(), y = r.U16(), z = r.I16(), yaw = r.I16(), pitch = r.I16()
                    },
                    logical_height = r.U16(),
                    asset = new EngineSessionTransport.AssetReference { id = r.Text(), content_address = r.Text() }
                };
            r.Finish();
            EngineSessionTransport.ValidateCampaignRenderObservation(observation);
            if (result.RuntimeId == 0 || result.RuntimeGeneration == 0)
                throw new InvalidDataException("Unbound render publication");
            return result;
        }

        private sealed class Reader
        {
            private static readonly Encoding Utf8 = new UTF8Encoding(false, true);
            private readonly byte[] bytes;
            private readonly int length;
            private int offset;

            internal Reader(byte[] bytes, int length)
            {
                if (bytes == null || length < 29 || length > bytes.Length || length > MaximumBytes)
                    throw new InvalidDataException("Invalid render publication length");
                this.bytes = bytes;
                this.length = length;
            }
            private void Need(int count)
            {
                if (count < 0 || count > length - offset)
                    throw new InvalidDataException("Truncated render publication");
            }
            internal byte Byte() { Need(1); return bytes[offset++]; }
            internal bool Bool()
            {
                var flag = Byte();
                if (flag > 1) throw new InvalidDataException("Invalid render publication flag");
                return flag != 0;
            }
            internal ushort U16()
            {
                Need(2);
                var value = (ushort)(bytes[offset] | (bytes[offset + 1] << 8));
                offset += 2; return value;
            }
            internal short I16() => unchecked((short)U16());
            internal uint U32()
            {
                Need(4);
                var value = (uint)bytes[offset] | ((uint)bytes[offset + 1] << 8) |
                    ((uint)bytes[offset + 2] << 16) | ((uint)bytes[offset + 3] << 24);
                offset += 4; return value;
            }
            internal int I32() => unchecked((int)U32());
            internal ulong U64() { var low = U32(); return low | ((ulong)U32() << 32); }
            internal string Counter() => U64().ToString(CultureInfo.InvariantCulture);
            internal string Text()
            {
                var size = U32();
                if (size > MaximumBytes || size > length - offset)
                    throw new InvalidDataException("Invalid render publication string length");
                var value = Utf8.GetString(bytes, offset, (int)size);
                offset += (int)size; return value;
            }
            internal string OptionalText() => Bool() ? Text() : null;
            internal int Count(int minimumBytes)
            {
                var count = U32();
                if (count > 65536 || count > (length - offset) / minimumBytes)
                    throw new InvalidDataException("Invalid render publication row count");
                return (int)count;
            }
            internal string Enum(string[] values)
            {
                var index = Byte();
                if (index >= values.Length) throw new InvalidDataException("Unknown render publication enum");
                return values[index];
            }
            internal void Finish()
            {
                if (offset != length) throw new InvalidDataException("Trailing render publication bytes");
            }
        }
    }
}
