using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Text;
using UnityEngine;

namespace MagicCarpet.Native
{
    /// <summary>
    /// Crash-resilient owner-local command and state journal for human Level 1
    /// playtests. The command stream is sufficient to drive the same native
    /// session again. Flight hashes expose flight-state divergence; the
    /// explicitly recorded mission, ownership, spell, palette, and trail
    /// fields are compared field by field for systems outside those hashes.
    /// </summary>
    internal sealed class PlaythroughRecorder : IDisposable
    {
        private const string Schema = "mc2-unity-playthrough/v1";
        private const string CommandEncoding = "mc-native-command-10-byte-le/v1";

        private readonly string startedUtc;
        private readonly string manifestPath;
        private readonly string packagePath;
        private readonly string closurePath;
        private readonly StreamWriter timeline;
        private readonly FileStream commands;
        private ulong timelineOrdinal;
        private ulong commandCount;
        private ulong eventCount;
        private ulong lastGeneration;
        private ulong lastTick;
        private ulong lastStateHash;
        private bool finished;

        private PlaythroughRecorder(
            string directory,
            string packagePath,
            string closurePath,
            NativeSim.SessionSnapshot snapshot,
            NativeSim.Level1MissionSnapshot mission)
        {
            DirectoryPath = directory;
            startedUtc = DateTime.UtcNow.ToString("O", CultureInfo.InvariantCulture);
            manifestPath = Path.Combine(directory, "manifest.json");
            this.packagePath = packagePath;
            this.closurePath = closurePath;
            timeline = new StreamWriter(
                new FileStream(
                    Path.Combine(directory, "timeline.ndjson"),
                    FileMode.CreateNew,
                    FileAccess.Write,
                    FileShare.Read),
                new UTF8Encoding(false))
            {
                AutoFlush = true
            };
            commands = new FileStream(
                Path.Combine(directory, "commands.bin"),
                FileMode.CreateNew,
                FileAccess.Write,
                FileShare.Read);
            lastGeneration = snapshot.generation;
            lastTick = snapshot.tick;
            lastStateHash = snapshot.stateHash;
            WriteManifest("running", string.Empty);
            WriteLine(
                $"{{\"kind\":\"start\",\"ordinal\":{timelineOrdinal++}," +
                $"\"utc\":\"{Escape(startedUtc)}\",\"generation\":{snapshot.generation}," +
                $"\"tick\":{snapshot.tick},\"state_hash\":\"0x{snapshot.stateHash:x16}\"," +
                $"\"conformance_hash\":\"0x{snapshot.conformanceHash:x16}\"," +
                $"\"stage\":{mission.currentStage},\"life\":{mission.playerLife}," +
                $"\"mana\":{mission.playerMana}}}");
        }

        internal string DirectoryPath { get; }

        internal static PlaythroughRecorder Start(
            string packagePath,
            string closurePath,
            NativeSim.SessionSnapshot snapshot,
            NativeSim.Level1MissionSnapshot mission)
        {
            var requested =
                Environment.GetEnvironmentVariable("MC_PLAYTHROUGH_DIR");
            var root = string.IsNullOrWhiteSpace(requested)
                ? Path.Combine(Application.persistentDataPath, "Playthroughs")
                : Path.GetFullPath(requested);
            Directory.CreateDirectory(root);
            var stamp = DateTime.UtcNow.ToString(
                "yyyyMMddTHHmmss.fffffffZ",
                CultureInfo.InvariantCulture);
            var directory = Path.Combine(
                root,
                $"{stamp}-pid{Process.GetCurrentProcess().Id}");
            Directory.CreateDirectory(directory);
            return new PlaythroughRecorder(
                directory,
                packagePath,
                closurePath,
                snapshot,
                mission);
        }

        internal void RecordTick(
            int commandIndex,
            sbyte opcode,
            sbyte argument1,
            sbyte argument2,
            sbyte roll,
            sbyte pitch,
            sbyte flags,
            short requestedSpell,
            NativeSim.SessionSnapshot snapshot,
            NativeSim.Level1MissionSnapshot mission,
            NativeSim.Level1SpellMenuSnapshot spellMenu,
            NativeSim.Level1PaletteInfo palette,
            NativeSim.Level1CastleLifecycleSnapshot castleLifecycle,
            bool spellMenuOpen,
            int speedSpellTicks,
            int goatCount,
            int manaSphereCount)
        {
            if (finished) return;
            WriteCommand(
                opcode,
                argument1,
                argument2,
                roll,
                pitch,
                flags,
                requestedSpell);
            commandCount++;
            lastGeneration = snapshot.generation;
            lastTick = snapshot.tick;
            lastStateHash = snapshot.stateHash;
            WriteLine(
                $"{{\"kind\":\"tick\",\"ordinal\":{timelineOrdinal++}," +
                $"\"command_index\":{commandIndex},\"generation\":{snapshot.generation}," +
                $"\"tick\":{snapshot.tick},\"opcode\":{opcode},\"argument1\":{argument1}," +
                $"\"argument2\":{argument2},\"roll\":{roll},\"pitch\":{pitch}," +
                $"\"flags\":{flags},\"requested_spell\":{requestedSpell}," +
                $"\"left_spell_holder\":{snapshot.leftSpellHolder}," +
                $"\"right_spell_holder\":{snapshot.rightSpellHolder}," +
                $"\"selected_spell_holder\":{snapshot.selectedSpellHolder}," +
                $"\"left_spell_family\":{snapshot.leftSpellSlot}," +
                $"\"right_spell_family\":{snapshot.rightSpellSlot}," +
                $"\"selected_spell_ui_index\":{snapshot.selectedSpellUiIndex}," +
                $"\"left_spell_tier\":{spellMenu.leftTier}," +
                $"\"right_spell_tier\":{spellMenu.rightTier}," +
                $"\"selected_spell_tier\":{spellMenu.selectedTier}," +
                $"\"spell_menu_open\":{(spellMenuOpen ? "true" : "false")}," +
                $"\"state_hash\":\"0x{snapshot.stateHash:x16}\"," +
                $"\"conformance_hash\":\"0x{snapshot.conformanceHash:x16}\"," +
                $"\"event_head\":{snapshot.eventHead},\"x\":{snapshot.x},\"y\":{snapshot.y}," +
                $"\"z\":{snapshot.z},\"yaw\":{snapshot.yaw}," +
                $"\"actual_speed\":{snapshot.actualSpeed},\"stage\":{mission.currentStage}," +
                $"\"life\":{mission.playerLife},\"mana\":{mission.playerMana}," +
                $"\"castle_exists\":{mission.castleExists}," +
                $"\"actor_count\":{mission.actorCount}," +
                $"\"projectile_count\":{mission.projectileCount}," +
                $"\"impact_count\":{mission.impactCount}," +
                $"\"possession_experience\":{mission.possessionExperience}," +
                $"\"possession_tier\":{mission.possessionTier}," +
                $"\"possessed_building_count\":{mission.possessedBuildingCount}," +
                $"\"last_possessed_building_entity\":" +
                    $"{mission.lastPossessedBuildingEntity}," +
                $"\"last_possessed_building_owner\":" +
                    $"{mission.lastPossessedBuildingOwner}," +
                $"\"speed_spell_ticks\":{speedSpellTicks}," +
                $"\"speed_trail_count\":{mission.speedTrailCount}," +
                $"\"palette_submode\":{palette.submode}," +
                $"\"palette_effect\":{palette.effect}," +
                $"\"palette_weight\":{palette.weight}," +
                $"\"palette_denominator\":{palette.denominator}," +
                $"\"castle_construction_present\":" +
                    $"{castleLifecycle.constructionPresent}," +
                $"\"castle_construction_active\":" +
                    $"{castleLifecycle.constructionActive}," +
                $"\"castle_construction_stage\":" +
                    $"{castleLifecycle.constructionStage}," +
                $"\"castle_construction_timer\":" +
                    $"{castleLifecycle.constructionTimer}," +
                $"\"goat_count\":{goatCount}," +
                $"\"mana_sphere_count\":{manaSphereCount}}}");
            if ((commandCount % 50) == 0)
            {
                commands.Flush();
            }
        }

        internal void RecordEvent(NativeSim.NativeEvent nativeEvent)
        {
            if (finished) return;
            eventCount++;
            WriteLine(
                $"{{\"kind\":\"event\",\"ordinal\":{timelineOrdinal++}," +
                $"\"generation\":{nativeEvent.generation},\"sequence\":{nativeEvent.sequence}," +
                $"\"state_hash\":\"0x{nativeEvent.stateHash:x16}\"," +
                $"\"event_kind\":{nativeEvent.kind},\"code\":{nativeEvent.code}," +
                $"\"entity_index\":{nativeEvent.entityIndex}," +
                $"\"player_index\":{nativeEvent.playerIndex}," +
                $"\"value0\":{nativeEvent.value0},\"value1\":{nativeEvent.value1}," +
                $"\"asset_ref_index\":{nativeEvent.assetRefIndex}}}");
        }

        internal void RecordReset(NativeSim.SessionSnapshot snapshot)
        {
            if (finished) return;
            commands.Flush();
            lastGeneration = snapshot.generation;
            lastTick = snapshot.tick;
            lastStateHash = snapshot.stateHash;
            WriteLine(
                $"{{\"kind\":\"reset\",\"ordinal\":{timelineOrdinal++}," +
                $"\"generation\":{snapshot.generation},\"tick\":{snapshot.tick}," +
                $"\"state_hash\":\"0x{snapshot.stateHash:x16}\"}}");
        }

        internal void RecordFatal(Exception exception)
        {
            if (finished) return;
            WriteLine(
                $"{{\"kind\":\"fatal\",\"ordinal\":{timelineOrdinal++}," +
                $"\"generation\":{lastGeneration},\"tick\":{lastTick}," +
                $"\"state_hash\":\"0x{lastStateHash:x16}\"," +
                $"\"exception\":\"{Escape(exception.ToString())}\"}}");
            commands.Flush(true);
        }

        internal void Finish(string fatalError)
        {
            if (finished) return;
            finished = true;
            commands.Flush(true);
            timeline.Flush();
            WriteManifest(
                string.IsNullOrEmpty(fatalError) ? "complete" : "fatal",
                fatalError ?? string.Empty);
        }

        public void Dispose()
        {
            Finish(string.Empty);
            commands.Dispose();
            timeline.Dispose();
        }

        private void WriteCommand(
            sbyte opcode,
            sbyte argument1,
            sbyte argument2,
            sbyte roll,
            sbyte pitch,
            sbyte flags,
            short requestedSpell)
        {
            commands.WriteByte(unchecked((byte)opcode));
            commands.WriteByte(unchecked((byte)argument1));
            commands.WriteByte(unchecked((byte)argument2));
            commands.WriteByte(unchecked((byte)roll));
            commands.WriteByte(unchecked((byte)pitch));
            commands.WriteByte(unchecked((byte)flags));
            commands.WriteByte(unchecked((byte)requestedSpell));
            commands.WriteByte(unchecked((byte)(requestedSpell >> 8)));
            commands.WriteByte(0);
            commands.WriteByte(0);
        }

        private void WriteLine(string value)
        {
            RequireUniqueTopLevelMembers(value);
            timeline.WriteLine(value);
        }

        internal static void RequireUniqueTopLevelMembers(string value)
        {
            if (string.IsNullOrEmpty(value) || value[0] != '{' ||
                value[value.Length - 1] != '}')
            {
                throw new InvalidDataException("timeline row is not a JSON object");
            }
            var names = new HashSet<string>(StringComparer.Ordinal);
            var index = 1;
            while (index < value.Length - 1)
            {
                while (index < value.Length - 1 &&
                       (char.IsWhiteSpace(value[index]) || value[index] == ','))
                {
                    index++;
                }
                if (index >= value.Length - 1) break;
                if (value[index] != '"')
                {
                    throw new InvalidDataException(
                        "timeline JSON member name is not quoted");
                }
                var start = ++index;
                var escaped = false;
                while (index < value.Length - 1)
                {
                    var character = value[index];
                    if (escaped)
                    {
                        escaped = false;
                    }
                    else if (character == '\\')
                    {
                        escaped = true;
                    }
                    else if (character == '"')
                    {
                        break;
                    }
                    index++;
                }
                if (index >= value.Length - 1)
                {
                    throw new InvalidDataException(
                        "timeline JSON member name is unterminated");
                }
                var name = value.Substring(start, index - start);
                if (!names.Add(name))
                {
                    throw new InvalidDataException(
                        $"timeline JSON member is duplicated: {name}");
                }
                index++;
                while (index < value.Length - 1 &&
                       char.IsWhiteSpace(value[index]))
                {
                    index++;
                }
                if (index >= value.Length - 1 || value[index++] != ':')
                {
                    throw new InvalidDataException(
                        $"timeline JSON member has no value separator: {name}");
                }
                var nested = 0;
                var inString = false;
                escaped = false;
                while (index < value.Length - 1)
                {
                    var character = value[index];
                    if (inString)
                    {
                        if (escaped)
                        {
                            escaped = false;
                        }
                        else if (character == '\\')
                        {
                            escaped = true;
                        }
                        else if (character == '"')
                        {
                            inString = false;
                        }
                    }
                    else if (character == '"')
                    {
                        inString = true;
                    }
                    else if (character == '[' || character == '{')
                    {
                        nested++;
                    }
                    else if (character == ']' || character == '}')
                    {
                        nested--;
                    }
                    else if (character == ',' && nested == 0)
                    {
                        break;
                    }
                    index++;
                }
            }
        }

        private void WriteManifest(
            string status,
            string fatalError)
        {
            var completedUtc = status == "running"
                ? string.Empty
                : DateTime.UtcNow.ToString("O", CultureInfo.InvariantCulture);
            var json =
                "{\n" +
                $"  \"schema\": \"{Schema}\",\n" +
                $"  \"status\": \"{Escape(status)}\",\n" +
                $"  \"started_utc\": \"{Escape(startedUtc)}\",\n" +
                $"  \"completed_utc\": \"{Escape(completedUtc)}\",\n" +
                $"  \"unity_version\": \"{Escape(Application.unityVersion)}\",\n" +
                $"  \"unity_build_guid\": \"{Escape(Application.buildGUID)}\",\n" +
                $"  \"package_path\": \"{Escape(packagePath)}\",\n" +
                $"  \"closure_path\": \"{Escape(closurePath)}\",\n" +
                $"  \"command_encoding\": \"{CommandEncoding}\",\n" +
                "  \"command_bytes\": 10,\n" +
                $"  \"command_count\": {commandCount},\n" +
                $"  \"event_count\": {eventCount},\n" +
                $"  \"last_generation\": {lastGeneration},\n" +
                $"  \"last_tick\": {lastTick},\n" +
                $"  \"last_state_hash\": \"0x{lastStateHash:x16}\",\n" +
                $"  \"fatal_error\": \"{Escape(fatalError)}\"\n" +
                "}\n";
            File.WriteAllText(manifestPath, json, new UTF8Encoding(false));
        }

        private static string Escape(string value)
        {
            if (string.IsNullOrEmpty(value)) return string.Empty;
            var escaped = new StringBuilder(value.Length + 16);
            foreach (var character in value)
            {
                switch (character)
                {
                    case '\\': escaped.Append("\\\\"); break;
                    case '"': escaped.Append("\\\""); break;
                    case '\n': escaped.Append("\\n"); break;
                    case '\r': escaped.Append("\\r"); break;
                    case '\t': escaped.Append("\\t"); break;
                    default:
                        if (character < 0x20)
                        {
                            escaped.Append("\\u");
                            escaped.Append(((int)character).ToString("x4"));
                        }
                        else
                        {
                            escaped.Append(character);
                        }
                        break;
                }
            }
            return escaped.ToString();
        }
    }
}
