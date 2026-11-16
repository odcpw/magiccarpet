using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Runtime.Serialization.Json;
using System.Text;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class GameRuntimeEditModeTests
    {
        private sealed class CampaignGuiWindow : UnityEditor.EditorWindow
        {
            internal Action DrawRuntime;
            internal int DrawCount;
            private void OnGUI()
            {
                if (DrawRuntime == null) return;
                DrawRuntime();
                DrawCount++;
            }
        }

        [Serializable]
        private sealed class WallHelpFrame
        {
            public EngineSessionTransport.CampaignSnapshotObservation snapshot;
            // The render JSON shares the full DTO's presentation fields but
            // deliberately omits its three expensive receipt/hash fields.
            public EngineSessionTransport.CampaignSnapshotValue render;
        }

        [Serializable]
        private sealed class WallHelpReceipt
        {
            public WallHelpFrame contact, dry, reset;
        }

        private const string SecretOriginLevel = "level.magic_carpet_2.t_klom";
        private const string SecretDestinationLevel = "level.magic_carpet_2.karakir";

        private static string[] SecretOrdinaryLevels() => new[] {
            "level.magic_carpet_2.night_spire",
            "level.magic_carpet_2.payahandra_ruins",
            "level.magic_carpet_2.revealed_way",
            "level.magic_carpet_2.spider_cave",
            SecretOriginLevel
        };

        private static GameRuntimeTransport.RuntimeFlowDefinition SecretRuntimeFlow(string boundLevelId)
        {
            var levels = SecretOrdinaryLevels();
            var entries = new GameRuntimeTransport.RuntimeFlowEntry[levels.Length];
            for (var index = 0; index < levels.Length; index++)
            {
                var destination = index + 1 < levels.Length
                    ? new GameRuntimeTransport.RuntimeFlowDestination {
                        kind = "level", level_id = levels[index + 1]
                    }
                    : new GameRuntimeTransport.RuntimeFlowDestination {
                        kind = "title", node_id = "flow.magic_carpet_2.return_to_title"
                    };
                entries[index] = new GameRuntimeTransport.RuntimeFlowEntry {
                    level_id = levels[index], completion_destination = destination
                };
            }
            return new GameRuntimeTransport.RuntimeFlowDefinition {
                game_id = "game.magic_carpet_2",
                bound_level_id = boundLevelId,
                entry_level_id = levels[0],
                entries = entries,
                secret_entries = new[] {
                    new GameRuntimeTransport.RuntimeFlowSecretEntry {
                        origin_level_id = SecretOriginLevel,
                        secret_level_id = SecretDestinationLevel
                    }
                }
            };
        }

        private static GameRuntimeTransport.SecretCampaignSnapshot SecretProgress(
            string returnOrigin,
            bool completed)
        {
            return new GameRuntimeTransport.SecretCampaignSnapshot {
                discovered_levels = new[] { SecretDestinationLevel },
                completed_levels = completed
                    ? new[] { SecretDestinationLevel }
                    : Array.Empty<string>(),
                return_origin = returnOrigin
            };
        }

        private static EngineSessionTransport.Identity SecretSession(
            string generation,
            string tick,
            char definitionHash)
        {
            return new EngineSessionTransport.Identity {
                engine_contract = EngineSessionTransport.EngineContract,
                package_graph_hash = "sha256:" + new string('a', 64),
                definition_hash = "sha256:" + new string(definitionHash, 64),
                definition_revision = "1",
                session_id = "771",
                generation = generation,
                tick = tick
            };
        }

        private static GameRuntimeTransport.Identity SecretPlayingIdentity(
            string mode,
            string runtimeGeneration,
            string boundLevelId,
            EngineSessionTransport.Identity session,
            GameRuntimeTransport.SecretCampaignSnapshot secretCampaign)
        {
            var suffix = boundLevelId.Substring(boundLevelId.LastIndexOf('.') + 1);
            return new GameRuntimeTransport.Identity {
                runtime_contract = GameRuntimeTransport.RuntimeContract,
                display_settings = new GameRuntimeTransport.DisplaySettings { minimap = true, top_bar = true },
                runtime_id = "771",
                generation = runtimeGeneration,
                mode = mode,
                active_runtime_flow = SecretRuntimeFlow(boundLevelId),
                session = session,
                secret_campaign = secretCampaign,
                presentation = new GameRuntimeTransport.RuntimePresentationCue {
                    cue_id = "presentation.magic_carpet_2." + suffix + ".entry",
                    phase = GameRuntimeTransport.LevelEntryPresentationPhase,
                    level_id = boundLevelId
                }
            };
        }

        private static GameRuntimeTransport.Identity SecretLoadingIdentity(string returnOrigin)
        {
            return new GameRuntimeTransport.Identity {
                runtime_contract = GameRuntimeTransport.RuntimeContract,
                display_settings = new GameRuntimeTransport.DisplaySettings { minimap = true, top_bar = true },
                runtime_id = "771",
                generation = "8",
                mode = GameRuntimeTransport.LoadingMode,
                pending_definition = new GameRuntimeTransport.DefinitionIdentity {
                    package_graph_hash = "sha256:" + new string('a', 64),
                    definition_hash = "sha256:" + new string('b', 64),
                    definition_revision = "1",
                    runtime_flow = SecretRuntimeFlow(SecretDestinationLevel)
                },
                secret_campaign = SecretProgress(returnOrigin, completed: true),
                presentation = new GameRuntimeTransport.RuntimePresentationCue {
                    cue_id = "presentation.magic_carpet_2.karakir.loading",
                    phase = GameRuntimeTransport.LoadingPresentationPhase,
                    level_id = SecretDestinationLevel
                }
            };
        }

        private static GameRuntimeTransport.Identity SecretCampaignMapIdentity(
            string runtimeGeneration = "31")
        {
            var levels = SecretOrdinaryLevels();
            var available = new string[levels.Length + 1];
            Array.Copy(levels, available, levels.Length);
            available[levels.Length] = SecretDestinationLevel;
            var boundOrdinary = levels[1];
            return new GameRuntimeTransport.Identity {
                runtime_contract = GameRuntimeTransport.RuntimeContract,
                display_settings = new GameRuntimeTransport.DisplaySettings { minimap = true, top_bar = true },
                runtime_id = "771",
                generation = runtimeGeneration,
                mode = GameRuntimeTransport.CampaignMode,
                active_runtime_flow = SecretRuntimeFlow(boundOrdinary),
                secret_campaign = SecretProgress(null, completed: true),
                presentation = new GameRuntimeTransport.RuntimePresentationCue {
                    cue_id = "presentation.campaign.map",
                    phase = GameRuntimeTransport.CampaignMapPresentationPhase,
                    level_id = SecretOriginLevel
                },
                campaign_menu = new GameRuntimeTransport.CampaignMenuSnapshot {
                    phase = "mission_select",
                    completed_level_id = boundOrdinary,
                    completed_levels = new[] { levels[0], boundOrdinary },
                    available_levels = available,
                    selected_level_id = SecretOriginLevel,
                    briefing_level_id = SecretOriginLevel
                }
            };
        }

        private static SecretTransitionTransport.Receipt SecretEntryReceipt()
        {
            var source = SecretSession("40", "71", 'b');
            var destination = SecretSession("41", "0", 'c');
            var before = SecretPlayingIdentity(
                GameRuntimeTransport.PlayingMode, "22", SecretOriginLevel, source, null);
            var after = SecretPlayingIdentity(
                GameRuntimeTransport.PlayingMode, "23", SecretDestinationLevel,
                destination, SecretProgress(SecretOriginLevel, completed: false));
            return new SecretTransitionTransport.Receipt {
                receipt_contract = SecretTransitionTransport.ReceiptContract,
                kind = SecretTransitionTransport.EntryKind,
                before = before,
                after = after,
                source_session = source,
                origin_level_id = SecretOriginLevel,
                secret_level_id = SecretDestinationLevel,
                receipt_sha256 = new string('0', 64)
            };
        }

        private static SecretTransitionTransport.Receipt SecretCompletedReturnReceipt()
        {
            var source = SecretSession("41", "88", 'c');
            var before = SecretPlayingIdentity(
                GameRuntimeTransport.PlayingMode, "23", SecretDestinationLevel,
                source, SecretProgress(SecretOriginLevel, completed: false));
            // The Rust receipt contract advances runtime generation by exactly
            // one; this return starts at 23 and therefore lands at 24.
            var after = SecretCampaignMapIdentity("24");
            return new SecretTransitionTransport.Receipt {
                receipt_contract = SecretTransitionTransport.ReceiptContract,
                kind = SecretTransitionTransport.CompletedReturnKind,
                before = before,
                after = after,
                source_session = source,
                origin_level_id = SecretOriginLevel,
                secret_level_id = SecretDestinationLevel,
                receipt_sha256 = new string('0', 64)
            };
        }

        private static SecretTransitionTransport.Receipt SecretAbandonedReturnReceipt()
        {
            var receipt = SecretCompletedReturnReceipt();
            receipt.kind = SecretTransitionTransport.AbandonedReturnKind;
            receipt.after.secret_campaign.completed_levels = Array.Empty<string>();
            return receipt;
        }

        private static string SerializeDataContractJson<T>(T value)
        {
            using var stream = new MemoryStream();
            new DataContractJsonSerializer(typeof(T)).WriteObject(stream, value);
            return Encoding.UTF8.GetString(stream.ToArray());
        }

        [TestCase("title")]
        [TestCase("loading")]
        [TestCase("playing")]
        [TestCase("paused")]
        public void RuntimeJsonDecoderPreservesCompleteIdentityGraphAndNulls(string mode)
        {
            var identity = mode == "loading" ? SecretLoadingIdentity(null)
                : mode == "title" ? new GameRuntimeTransport.Identity { mode = mode }
                : SecretPlayingIdentity(mode, "22", SecretOriginLevel,
                    SecretSession("7", "18446744073709551615", 'd'), null);
            var bytes = Encoding.UTF8.GetBytes(SerializeDataContractJson(identity));
            var expected = GameRuntimeTransport.DecodeRuntimeJson<GameRuntimeTransport.Identity>(bytes, true);
            var actual = GameRuntimeTransport.DecodeRuntimeJson<GameRuntimeTransport.Identity>(bytes, false);
            Assert.That(SerializeDataContractJson(actual), Is.EqualTo(SerializeDataContractJson(expected)));
            Assert.That(actual, Is.Not.SameAs(expected));
            actual.mode = "mutated";
            if (actual.session != null) actual.session.generation = "mutated";
            var fresh = GameRuntimeTransport.DecodeRuntimeJson<GameRuntimeTransport.Identity>(bytes, false);
            Assert.That(SerializeDataContractJson(fresh), Is.EqualTo(SerializeDataContractJson(expected)));
            Assert.That(fresh, Is.Not.SameAs(actual));
            if (actual.session != null) Assert.That(fresh.session, Is.Not.SameAs(actual.session));
        }

        [Test]
        public void RuntimeJsonDecoderPreservesUnicodeAndIntegerBoundariesConcurrently()
        {
            var value = SecretPlayingIdentity("playing", "22", SecretOriginLevel,
                SecretSession("7", "18446744073709551615", 'd'), null);
            value.player_preferences = new PlayerPreferencesSnapshot {
                assistance_sequence = ulong.MaxValue,
                preferences = new PlayerPreferences {
                    player_name = "Oliver 🌙 \"\\\n\t ä",
                    brightness = new[] { int.MinValue, 0, int.MaxValue },
                    reflections = false, sky = false, shadows = false, dynamic_lighting = false
                }
            };
            var json = Encoding.UTF8.GetBytes(JObject.Parse(SerializeDataContractJson(value)).ToString(Formatting.None));
            var expected = GameRuntimeTransport.DecodeRuntimeJson<GameRuntimeTransport.Identity>(json, true);
            System.Threading.Tasks.Parallel.For(0, 64, _ => {
                var actual = GameRuntimeTransport.DecodeRuntimeJson<GameRuntimeTransport.Identity>(json, false);
                Assert.That(SerializeDataContractJson(actual), Is.EqualTo(SerializeDataContractJson(expected)));
                Assert.That(actual.player_preferences, Is.Not.SameAs(expected.player_preferences));
                Assert.That(actual.player_preferences.preferences.brightness, Is.Not.SameAs(expected.player_preferences.preferences.brightness));
            });
        }

        [Test]
        public void RuntimeJsonDecoderPreservesLegacyAdmissionForNoncanonicalInputs()
        {
            var value = SecretPlayingIdentity("playing", "22", SecretOriginLevel,
                SecretSession("7", "9", 'd'), null);
            value.player_preferences = new PlayerPreferencesSnapshot { preferences = new PlayerPreferences { player_name = "~NAME" } };
            var original = JObject.Parse(SerializeDataContractJson(value));
            void Compare(string name, byte[] bytes)
            {
                GameRuntimeTransport.Identity legacy = null, actual = null;
                Exception legacyError = null, actualError = null;
                try { legacy = GameRuntimeTransport.DecodeRuntimeJson<GameRuntimeTransport.Identity>(bytes, true); }
                catch (Exception exception) { legacyError = exception; }
                try { actual = GameRuntimeTransport.DecodeRuntimeJson<GameRuntimeTransport.Identity>(bytes, false); }
                catch (Exception exception) { actualError = exception; }
                Assert.That(actualError?.GetType(), Is.EqualTo(legacyError?.GetType()), name);
                if (legacyError == null) Assert.That(SerializeDataContractJson(actual), Is.EqualTo(SerializeDataContractJson(legacy)), name);
            }
            void Mutate(string name, Action<JObject> change)
            {
                var json = (JObject)original.DeepClone(); change(json);
                Compare(name, Encoding.UTF8.GetBytes(json.ToString(Formatting.None)));
            }
            Mutate("required string", json => json.Remove("mode"));
            Mutate("required boolean", json => ((JObject)json["player_preferences"]["preferences"]).Remove("reflections"));
            Mutate("required numeric", json => ((JObject)json["player_preferences"]["preferences"]).Remove("mouse_sensitivity"));
            Mutate("wrong case", json => { json["MODE"] = json["mode"]; json.Remove("mode"); });
            Mutate("optional wrong case", json => json["FRONTEND_MEDIA"] = new JObject { ["active"] = true });
            Mutate("null boolean", json => json["player_preferences"]["preferences"]["reflections"] = null);
            Mutate("null numeric", json => json["player_preferences"]["preferences"]["mouse_sensitivity"] = null);
            Mutate("legacy boolean coercion", json => json["player_preferences"]["preferences"]["reflections"] = "1");
            Mutate("legacy numeric coercion", json => json["player_preferences"]["preferences"]["mouse_sensitivity"] = 100.0);
            Mutate("ignored metadata", json => json["$ref"] = "missing");
            Mutate("DCS metadata", json => json["__type"] = "unknown");
            Mutate("unknown extension", json => json["future_extension"] = new JObject { ["value"] = 1 });
            Mutate("raw XML noncharacter FFFE", json => json["player_preferences"]["preferences"]["player_name"] = "\ufffe");
            Mutate("raw XML noncharacter FFFF", json => json["player_preferences"]["preferences"]["player_name"] = "\uffff");
            var compact = original.ToString(Formatting.None);
            Compare("duplicate", Encoding.UTF8.GetBytes("{\"mode\":\"other\"," + compact.Substring(1)));
            Compare("trailing comma", Encoding.UTF8.GetBytes(compact.Substring(0, compact.Length - 1) + ",}"));
            Compare("trailing content", Encoding.UTF8.GetBytes(compact + " false"));
            Compare("whitespace", Encoding.UTF8.GetBytes(" \n" + compact + "\t"));
            var invalid = Encoding.UTF8.GetBytes(compact);
            invalid[Array.IndexOf(invalid, (byte)'~')] = 255;
            Compare("invalid UTF8", invalid);
        }

        private static T DataContractJsonRoundTrip<T>(T value) where T : class
        {
            using var stream = new MemoryStream();
            var serializer = new DataContractJsonSerializer(typeof(T));
            serializer.WriteObject(stream, value);
            stream.Position = 0;
            return serializer.ReadObject(stream) as T;
        }

        private static byte[] SerializeBoundaryEnvelope(
            SecretTransitionTransport.AcceptedBoundaryReceipt envelope)
        {
            // Rust serializes this as a tagged enum: the kind and only the
            // selected payload are present. Build that wire shape explicitly
            // so the fixture does not acquire CLR-only envelope members.
            var json = new StringBuilder("{\"kind\":\"")
                .Append(envelope.kind)
                .Append('"');
            if (envelope.HasOrdinaryCompletionField)
            {
                json.Append(",\"ordinary_completion\":")
                    .Append(envelope.ordinary_completion == null
                        ? "null"
                        : SerializeDataContractJson(envelope.ordinary_completion));
            }
            if (envelope.HasSecretTransitionField)
            {
                json.Append(",\"secret_transition\":")
                    .Append(envelope.secret_transition == null
                        ? "null"
                        : SerializeDataContractJson(envelope.secret_transition));
            }
            return Encoding.UTF8.GetBytes(json.Append('}').ToString());
        }

        private static SecretTransitionTransport.AcceptedBoundaryReceipt DecodeBoundaryEnvelope(
            SecretTransitionTransport.Receipt receipt,
            string kind = SecretTransitionTransport.SecretTransitionBoundaryKind)
        {
            var envelope = new SecretTransitionTransport.AcceptedBoundaryReceipt { kind = kind };
            envelope.secret_transition = receipt;
            return SecretTransitionTransport.Decode(
                SerializeBoundaryEnvelope(envelope), 1024 * 1024);
        }

        private static string[] InstalledOpeningArguments()
        {
            var generated = Path.Combine(PrivateRoot, "generated");
            return new[] {
                "-mc-game-runtime", "-mc-runtime-loading-cache", Path.Combine(generated, "mc2-level1-night-cache-v2"),
                "-mc-runtime-package", Path.Combine(generated, "mc-bhq8-level1-collision-v5-night"),
                "-mc-runtime-owner-cache", Path.Combine(generated, "mc-oi62-environment-level1-v2"),
                "-mc-runtime-presentation-cache", Path.Combine(generated, "mc2-level1-night-cache-v2"),
                "-mc-runtime-package", Path.Combine(generated, "mc-level2-vault-successor-closure-20261025"),
                "-mc-runtime-owner-cache", Path.Combine(generated, "mc-level2-vault-successor-environment-20261025"),
                "-mc-runtime-presentation-cache", Path.Combine(generated, "level2-vault-successor-owner-cache-20261025"),
                "-mc-runtime-package", Path.Combine(generated, "level3-day-block32-campaign-recipe-v2"),
                "-mc-runtime-owner-cache", Path.Combine(generated, "mc-level3-environment-exit-20261025"),
                "-mc-runtime-presentation-cache", Path.Combine(generated, "level3-day-spell-owner-cache-audio-v4"),
                "-mc-runtime-package", Path.Combine(generated, "mc-bhq6-ordinary-20261025-zclu5fjj/level4"),
                "-mc-runtime-owner-cache", Path.Combine(generated, "mc-oi62-environment-level4-audio-v3"),
                "-mc-runtime-presentation-cache", Path.Combine(generated, "mc-oi62-environment-level4-audio-v3")
            };
        }

        [Test]
        public void InstalledNightSpawnAndSteeringUseProductionFramePath()
        {
            var root = new GameObject("installed Night spawn frame path");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(InstalledOpeningArguments());
                Assert.That(runtime.GameRuntimeNightPresentationReady, Is.True);
                const System.Reflection.BindingFlags privateInstance =
                    System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
                var borrowed = (NativeCampaignSession)typeof(MagicCarpetRuntime)
                    .GetField("campaignSession", privateInstance).GetValue(runtime);
                var surface = (CampaignPresentationSurface)typeof(MagicCarpetRuntime)
                    .GetField("gameRuntimeNightSurface", privateInstance).GetValue(runtime);
                var environment = (OwnerAssetCache)typeof(MagicCarpetRuntime)
                    .GetField("gameRuntimeEnvironmentCache", privateInstance).GetValue(runtime);
                var labels = (SourceMinimapLabels)typeof(CampaignPresentationSurface)
                    .GetField("sourceMinimapLabels", privateInstance).GetValue(surface);
                Assert.That(borrowed.Handle.ToString(), Is.EqualTo(runtime.GameRuntimeProductionIdentity.session.session_id));
                Assert.That(surface.Generation, Is.EqualTo(borrowed.Generation));
                Assert.That(labels.Coverage, Is.Null, "the opening player has no active Beyond Sight label");
                surface.PresentEnvironment(borrowed, environment, 0, true);
                Assert.That(labels.Coverage, Is.Null, "the full map uses the same inactive native plan");
                surface.PresentEnvironment(borrowed, environment);
                var total = System.Diagnostics.Stopwatch.StartNew();
                for (var i = 0; i < 32; i++)
                {
                    var previousPose = runtime.GameRuntimeProductionCampaignSnapshot.value.player_pose;
                    runtime.QualificationApplyGameRuntimeProductionSteering(i < 8 ? 0 : 16, i < 16 ? 0 : -8);
                    runtime.QualificationDriveGameRuntimeFrames(1);
                    var snapshot = runtime.GameRuntimeProductionCampaignSnapshot.value;
                    var pose = snapshot.player_pose;
                    Assert.That(snapshot.source_camera_pose.pitch, Is.EqualTo(SignedAngle11(previousPose.display_pitch) / 2),
                        "source view captures signed pitch/2 before the entity update");
                    Assert.That(snapshot.source_camera_pose.yaw, Is.EqualTo(previousPose.yaw));
                    Assert.That(snapshot.source_camera_pose.z, Is.EqualTo(previousPose.z + 128));
                    var native = (NativeCampaignSession)typeof(MagicCarpetRuntime).GetField("campaignSession",
                        System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic).GetValue(runtime);
                    var litPixels = 0;
                    foreach (var index in native.SceneFrameIndices)
                    {
                        var palette = native.DayIndexedFramePalette;
                        if (palette[index * 3] != 0 || palette[index * 3 + 1] != 0 || palette[index * 3 + 2] != 0) litPixels++;
                    }
                    Debug.Log($"MC_SPAWN_POSE frame={i} yaw={pose.yaw} pitch={pose.display_pitch} source_pitch={snapshot.source_camera_pose.pitch} z={pose.z} lit_pixels={litPixels}");
                    Assert.That(runtime.GameRuntimeNightPresentationHash, Is.Not.Zero);
                }
                Debug.Log($"MC_SPAWN_PROFILE frames=32 elapsed_ms={total.Elapsed.TotalMilliseconds:F3}");
                var previousGeneration = borrowed.Generation;
                var previousTick = borrowed.Tick;
                runtime.QualificationResetGameRuntimeProduction();
                Assert.That(borrowed.Generation, Is.Not.EqualTo(previousGeneration));
                Assert.That(surface.Generation, Is.EqualTo(borrowed.Generation));
                Assert.That(labels.Coverage, Is.Null);
                surface.PresentEnvironment(borrowed, environment, 0, true);
                Assert.That(labels.Coverage, Is.Null);
                var staleStatus = NativeSim.mc_native_campaign_world_map_labels_json(
                    borrowed.Handle, previousGeneration, previousTick, IntPtr.Zero, 0, out var required);
                Assert.That(staleStatus, Is.EqualTo(NativeSim.GenerationMismatch));
                Assert.That(required, Is.Zero);
            }
            finally
            {
                runtime.QualificationDisposeGameRuntimeProduction();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void InstalledOpeningAssignsSpellHandsAndRejectsLockedTierBeforeCasting()
        {
            var root = new GameObject("installed spell hand and tier producer");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(InstalledOpeningArguments());
                var openingSpells = runtime.GameRuntimeProductionCampaignSnapshot.value.player_spells;
                var textures = (System.Collections.Generic.Dictionary<int, Texture2D>)
                    typeof(MagicCarpetRuntime).GetField("hudBitmapTextures",
                        System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic).GetValue(runtime);
                Assert.That(SourceSpellMenu.HasRequiredBitmaps(openingSpells,
                    selector => textures.TryGetValue(selector, out var texture) && texture != null), Is.True,
                    "the ordinary Night HUD materializes the original acquired-spell menu artwork");
                Assert.That(SourceSpellMenu.HasRequiredBitmaps(openingSpells,
                    selector => selector != 87 && textures.TryGetValue(selector, out var texture) && texture != null), Is.False,
                    "a missing source mana-bar sprite cannot silently become an opaque fallback");
                var changed = openingSpells[0];
                var retainedHelp = changed.menu_help;
                changed.menu_help = null; // mutate only this detached observation copy
                Assert.That(SourceSpellMenu.HasRequiredBitmaps(openingSpells,
                    selector => textures.TryGetValue(selector, out var texture) && texture != null), Is.False,
                    "missing source metadata cannot be interpreted as a usable spell or absent Castle");
                changed.menu_help = retainedHelp;
                Assert.That(CampaignSpellPresenter.FindSpell(runtime.GameRuntimeProductionCampaignSnapshot.value,
                    "spell.possession").hand, Is.EqualTo("right"));
                runtime.QualificationAssignGameRuntimeProductionSpell("spell.possession", 0, false);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(CampaignSpellPresenter.FindSpell(runtime.GameRuntimeProductionCampaignSnapshot.value,
                    "spell.possession").hand, Is.EqualTo("both"),
                    "source left-hand assignment preserves the existing right hand");
                runtime.QualificationAssignGameRuntimeProductionSpell("spell.fireball", 0, true);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(CampaignSpellPresenter.FindSpell(runtime.GameRuntimeProductionCampaignSnapshot.value,
                    "spell.fireball").hand, Is.EqualTo("right"));
                Assert.That(CampaignSpellPresenter.FindSpell(runtime.GameRuntimeProductionCampaignSnapshot.value,
                    "spell.possession").hand, Is.EqualTo("left"));
                runtime.QualificationAssignGameRuntimeProductionSpell("spell.possession", 2, false);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(CampaignSpellPresenter.FindSpell(runtime.GameRuntimeProductionCampaignSnapshot.value,
                    "spell.possession").tier, Is.Zero, "a locked choice never mutates its source holder");
                runtime.ApplyGameRuntimeHeldSpellInput(true, false);
                runtime.QualificationDriveGameRuntimeFrames(2);
                runtime.ApplyGameRuntimeHeldSpellInput(false, false);
                var native = (NativeCampaignSession)typeof(MagicCarpetRuntime).GetField("campaignSession",
                    System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic).GetValue(runtime);
                Assert.That(Array.Exists(native.Entities(), row => row.active != 0 &&
                    (row.classId == 9 && row.model == 1 || row.classId == 10 && row.model == 12)), Is.True,
                    "the assigned left hand reaches the actual Possession projectile/effect producer");
            }
            finally
            {
                runtime.QualificationDisposeGameRuntimeProduction();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void NightSteeringKeepsSourceTerrainThroughGpuComposition()
        {
            Assert.That(SystemInfo.graphicsDeviceType, Is.Not.EqualTo(UnityEngine.Rendering.GraphicsDeviceType.Null));
            var generated = Path.Combine(PrivateRoot, "generated");
            var package = Path.Combine(generated, "mc-bhq8-level1-collision-v5-night");
            var cachePath = Path.Combine(generated, "mc-oi62-environment-level1-v2");
            var cache = OwnerAssetCache.OpenAndValidateEnvironment(cachePath, package);
            using var native = NativeCampaignSession.Open(package, NativeSim.CampaignSelection.Create(1, 0, 1), cachePath);
            using var surface = new CampaignPresentationSurface();
            var output = new RenderTexture(320, 200, 0, RenderTextureFormat.ARGB32, RenderTextureReadWrite.Linear);
            var pixels = new Texture2D(320, 200, TextureFormat.RGBA32, false, true);
            var previous = RenderTexture.active;
            try
            {
                for (var frame = 0; frame < 64; frame++)
                {
                    var player = native.Entities()[native.Players()[0].entityIndex];
                    if (frame != 0) native.Step(player.x, player.y, player.z, roll: 127, pitch: (sbyte)(frame < 32 ? 8 : -8));
                    player = native.Entities()[native.Players()[0].entityIndex];
                    surface.PresentEnvironment(native, cache, player.yaw);
                    surface.PresentView(new EngineSessionTransport.Pose {
                        x = player.x, y = player.y, z = player.z, yaw = player.yaw, display_pitch = player.pitch
                    });
                    var cameras = surface.FrameCamera.transform.parent.GetComponentsInChildren<Camera>();
                    Array.Sort(cameras, (left, right) => left.depth.CompareTo(right.depth));
                    foreach (var camera in cameras)
                    {
                        if (camera.targetTexture == null) camera.targetTexture = output;
                        camera.Render();
                        if (camera.depth != -100f) continue;
                        RenderTexture.active = camera.targetTexture;
                        pixels.ReadPixels(new Rect(0, 0, 320, 200), 0, 0); pixels.Apply();
                        var raw = pixels.GetPixels32();
                        var different = 0;
                        for (var y = 0; y < 200; y++)
                            for (var x = 0; x < 320; x++)
                                if (raw[y * 320 + x].r != native.SceneFrameIndices[(199-y)*320+x]) different++;
                        Assert.That(different, Is.Zero, $"source terrain frame={frame} yaw={player.yaw}");
                    }
                    RenderTexture.active = output;
                    pixels.ReadPixels(new Rect(0, 0, 320, 200), 0, 0); pixels.Apply();
                    var lit = Array.FindAll(pixels.GetPixels32(), pixel => pixel.r != 0 || pixel.g != 0 || pixel.b != 0).Length;
                    var sourceLit = Array.FindAll(native.SceneFrameIndices, index =>
                        native.DayIndexedFramePalette[index * 3] != 0 || native.DayIndexedFramePalette[index * 3 + 1] != 0 ||
                        native.DayIndexedFramePalette[index * 3 + 2] != 0).Length;
                    Debug.Log($"MC_NIGHT_GPU frame={frame} yaw={player.yaw} pitch={player.pitch} lit_pixels={lit} source_lit_pixels={sourceLit}");
                    var captureDir = Environment.GetEnvironmentVariable("MC_RENDER_CAPTURE_DIR");
                    if (!string.IsNullOrEmpty(captureDir) && (frame == 0 || frame == 32 || frame == 34 || frame == 40 || frame == 48))
                    {
                        Directory.CreateDirectory(captureDir);
                        File.WriteAllBytes(Path.Combine(captureDir, $"night-{frame:D2}.png"), pixels.EncodeToPNG());
                    }
                    Assert.That(lit, Is.GreaterThan(0), "composed steering view must retain visible world pixels");
                }
            }
            finally
            {
                RenderTexture.active = previous;
                output.Release();
                UnityEngine.Object.DestroyImmediate(output);
                UnityEngine.Object.DestroyImmediate(pixels);
            }
        }

        [Test]
        public void CaveOwnerHudAndWorldUseActualPackagePaletteAndSprites()
        {
            var generated = Path.Combine(PrivateRoot, "generated");
            var package = Path.Combine(generated, "mc-bhq6-ordinary-20261025-zclu5fjj/level4");
            var cachePath = Path.Combine(generated, "mc-oi62-environment-level4-audio-v3");
            var cache = OwnerAssetCache.OpenAndValidateEnvironment(cachePath, package);
            var root = new GameObject("actual Level4 presentation consumer");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            const System.Reflection.BindingFlags flags = System.Reflection.BindingFlags.Instance |
                System.Reflection.BindingFlags.NonPublic;
            try
            {
                typeof(MagicCarpetRuntime).GetField("ownerAssetCache", flags).SetValue(runtime, cache);
                typeof(MagicCarpetRuntime).GetMethod("MaterializeCampaignEnvironmentHud", flags).Invoke(runtime, null);
                foreach (var selector in new[] { 1, 2, 40, 41, 42, 43, 54, 123, 132 })
                    Assert.That(runtime.QualificationHasMaterializedLevel2DayHudSprite(selector), Is.True);
                var palette = (byte[])typeof(MagicCarpetRuntime).GetField("ownerHudPaletteRgb8", flags).GetValue(runtime);
                CollectionAssert.AreEqual(cache.ReadOutput(cache.EnvironmentMaterials.palette.rgb8_path), palette);
                Assert.That(cache.EnvironmentMaterials.profile.sprites, Is.EqualTo("MSPRC0-0"));
                Assert.That(cache.EnvironmentMaterials.profile.palette, Is.EqualTo("PALC-0"));
                var admits = typeof(NativeCampaignSession).GetMethod("IsPresentationSelection",
                    System.Reflection.BindingFlags.Static | System.Reflection.BindingFlags.NonPublic);
                Assert.That(admits.Invoke(null, new object[] { NativeSim.CampaignSelection.Create(4, 3, 2) }), Is.True);
                Assert.That(admits.Invoke(null, new object[] { NativeSim.CampaignSelection.Create(4, 3, 0) }), Is.False);
                Assert.That(admits.Invoke(null, new object[] { NativeSim.CampaignSelection.Create(5, 4, 2) }), Is.False);
                using var session = NativeCampaignSession.Open(package, NativeSim.CampaignSelection.Create(4, 3, 2), cachePath);
                using var surface = new CampaignPresentationSurface();
                surface.PresentEnvironment(session, cache);
                Assert.That(surface.WorldReady && surface.LiveMinimapReady, Is.True);
                Assert.That(surface.FrameHash, Is.Not.Zero);
                Assert.That(surface.Tick, Is.Zero, "isolated entry diagnostic performs no gameplay");
                Assert.That(session.CaveTerrainOrders.Length, Is.EqualTo(320 * 200));
                Assert.That(session.CaveCellOrders.Length, Is.EqualTo(256 * 256));
                Assert.That(Array.Exists(session.CaveTerrainOrders, rank => rank != 0), Is.True);
                var shortPlane = new ushort[] { 1234 };
                var shortCells = new ushort[] { 5678 };
                var shortStatus = NativeSim.mc_native_campaign_cave_occlusion_copy(
                    session.Handle, session.Generation, session.Tick, shortPlane, 1, shortCells, 1,
                    out var terrainWritten, out var cellWritten);
                Assert.That(shortStatus, Is.Not.EqualTo(NativeSim.Ok));
                Assert.That(terrainWritten, Is.Zero);
                Assert.That(cellWritten, Is.Zero);
                Assert.That(shortPlane[0], Is.EqualTo(1234));
                Assert.That(shortCells[0], Is.EqualTo(5678));
                var staleStatus = NativeSim.mc_native_campaign_cave_occlusion_copy(
                    session.Handle, session.Generation + 1, session.Tick, session.CaveTerrainOrders,
                    (uint)session.CaveTerrainOrders.Length, session.CaveCellOrders,
                    (uint)session.CaveCellOrders.Length, out terrainWritten, out cellWritten);
                Assert.That(staleStatus, Is.EqualTo(NativeSim.GenerationMismatch));
                Assert.That(terrainWritten, Is.Zero);
                Assert.That(cellWritten, Is.Zero);
                session.Reset();
                surface.PresentEnvironment(session, cache);
                Assert.That(surface.Generation, Is.EqualTo(session.Generation),
                    "same owner adopts its current generation without stale palette or asset replacement");
                using var another = NativeCampaignSession.Open(package, NativeSim.CampaignSelection.Create(4, 3, 2), cachePath);
                Assert.Throws<InvalidOperationException>(() => surface.PresentEnvironment(another, cache),
                    "the same package and cache do not authorize a different session owner");
            }
            finally { UnityEngine.Object.DestroyImmediate(root); }
        }

        [Test]
        public void AbilityImprovedUsesNamedSourceTextAndTwoHundredOuterDraws()
        {
            var path = Path.Combine(LevelOneOwnerCache, "font/language-l1.records-nul.bin");
            var bytes = File.ReadAllBytes(path);
            var records = Encoding.GetEncoding(28591).GetString(bytes).Split('\0');
            var notification = new MagicCarpetRuntime.AbilityImprovedNotification(bytes);
            notification.Improve(0);
            var expected = records[159].Replace("%s", records[160]);
            Assert.That(notification.Text, Is.EqualTo(expected.Length > 48 ? expected.Substring(0, 48) : expected));
            for (var i = 0; i < 200; i++)
            {
                notification.Advance(false);
                Assert.That(notification.Visible, Is.True, "source draw " + i);
            }
            notification.Advance(false);
            Assert.That(notification.Visible, Is.False);
            notification.Improve(1);
            for (var i = 0; i < 201; i++) notification.Advance(true);
            Assert.That(notification.Visible, Is.False, "source menu suppression freezes the carrier");
            notification.Advance(false);
            Assert.That(notification.Visible, Is.True);
            Assert.That(notification.Text, Does.Contain(records[161]));
            Assert.Throws<InvalidDataException>(() => notification.Improve(26));
        }

        [Test]
        public void ManaHelpHintKeepsSourceEligibilityPauseAndDrawCadence()
        {
            using var cache = new OwnerCampaignMenuCache(CampaignMenuCache);
            var fresh = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
            fresh.Promote(41, 6);
            fresh.Advance(true, false, true, false, false);
            Assert.That(fresh.Visible, Is.False, "the zero initial poll still requires age above 800");
            fresh.Advance(true, false, true, false, false);
            Assert.That(fresh.Visible, Is.False, "the second source pass selects without drawing");
            fresh.Advance(true, false, true, false, false);
            Assert.That(fresh.Visible, Is.True, "fresh construction does not invent a 20-frame delay");
            var suppressed = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
            suppressed.Promote(41, 6);
            suppressed.Advance(false, false, true, false, true);
            for (var pass = 0; pass < 19; pass++)
            {
                suppressed.Advance(true, false, true, false, false);
                Assert.That(suppressed.Visible, Is.False);
            }
            suppressed.Advance(true, false, true, false, false);
            Assert.That(suppressed.Visible, Is.False, "suppression installs the source 20-pass poll delay");
            suppressed.Advance(true, false, true, false, false);
            Assert.That(suppressed.Visible, Is.True);
            var hint = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
            hint.Promote(41, 6);
            for (var i = 0; i < 20; i++) hint.Advance(false, false, true, false, false);
            Assert.That(hint.VisibleRow, Is.EqualTo(1), "unavailable mana cannot displace the ordinary objective-key hint");
            hint.CloseCurrent(true);
            for (var i = 0; i < 20; i++) hint.Advance(true, false, true, false, false);
            Assert.That(hint.Visible, Is.False, "selection precedes the first source draw");
            hint.Advance(true, false, true, true, false);
            Assert.That(hint.Visible, Is.False, "pause freezes and hides the selected popup");
            for (var i = 0; i < 50; i++)
            {
                hint.Advance(false, false, true, false, false);
                Assert.That(hint.Visible, Is.True, "mana regeneration does not shorten the retained popup");
            }
            hint.Advance(true, false, true, false, false);
            Assert.That(hint.Visible, Is.False);
            Assert.That(hint.Priority, Is.EqualTo(2));
            hint.Advance(true, false, true, false, true);
            Assert.That(hint.Visible, Is.False, "objective/menu suppression closes the popup");

            var context = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
            context.RequestObjectiveContext(76, true);
            context.RequestObjectiveContext(0, true);
            context.RequestObjectiveContext(73, false);
            for (var pass = 0; pass < 8; pass++)
            {
                context.Advance(false, false, true, false, false);
                Assert.That(context.Visible, Is.False, "eight source passes select without drawing");
            }
            context.Advance(false, false, true, true, false);
            Assert.That(context.Visible, Is.False, "pause retains the selected context without drawing");
            for (var pass = 0; pass < 50; pass++)
            {
                context.Advance(false, false, true, false, false);
                Assert.That(context.Visible, Is.True);
                Assert.That(context.VisibleRow, Is.EqualTo(76), "KeepPending and disabled requests preserve the actual pending selector");
            }
            context.Advance(false, false, true, false, false);
            Assert.That(context.Visible, Is.False);
            context.RequestObjectiveContext(0, true);
            for (var pass = 0; pass < 21; pass++) context.Advance(false, false, true, false, false);
            Assert.That(context.VisibleRow, Is.EqualTo(1), "consumed word88 allows the ordinary objective-key row, not the previous context");
        }

        [Test]
        public void ObjectiveKeyHelpUsesTheSourcePollAgePrecedenceAndLifetime()
        {
            using var cache = new OwnerCampaignMenuCache(CampaignMenuCache);
            var hint = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
            hint.Advance(false, false, true, false, false);
            Assert.That(hint.Visible, Is.False, "age must exceed the source threshold before selection");
            hint.Advance(false, false, true, false, false);
            Assert.That(hint.Visible, Is.False, "selection precedes its first draw");
            hint.Advance(false, false, true, false, false);
            Assert.That(hint.VisibleRow, Is.EqualTo(1));
            hint.Advance(false, false, true, true, false);
            Assert.That(hint.Visible, Is.False, "pause hides the retained row without consuming it");
            for (var i = 1; i < 50; i++)
            {
                hint.Advance(false, false, true, false, false);
                Assert.That(hint.VisibleRow, Is.EqualTo(1));
            }
            hint.Advance(false, false, true, false, false);
            Assert.That(hint.Visible, Is.False, "the following pass clears the expired overlay");
            var ended = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
            for (var i = 0; i < 25; i++) ended.Advance(false, false, true, false, false, 0, true);
            Assert.That(ended.Visible, Is.False, "IsLevelEnd masks ordinary row1");
            var hiddenMap = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
            for (var i = 0; i < 25; i++) hiddenMap.Advance(true, true, true, false, false, 1, false, false);
            Assert.That(hiddenMap.Visible, Is.False, "source flags 0x2ff mask ordinary rows while the minimap is hidden");
            hiddenMap.RequestObjectiveContext(76, true);
            for (var i = 0; i < 9; i++) hiddenMap.Advance(true, true, true, false, false, 1, false, false);
            Assert.That(hiddenMap.VisibleRow, Is.EqualTo(76), "pending objective context bypasses the ordinary minimap mask");
            var closed = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
            for (var i = 0; i < 3; i++) closed.Advance(false, false, true, false, false);
            Assert.That(closed.VisibleRow, Is.EqualTo(1));
            closed.CloseCurrent(true);
            for (var i = 0; i < 20; i++)
            {
                closed.Advance(false, false, true, false, false, 1);
                Assert.That(closed.Visible, Is.False, "disabling icons closes the popup and retains the source 20-pass poll delay");
            }
            closed.Advance(false, false, true, false, false, 0);
            Assert.That(closed.VisibleRow, Is.EqualTo(3), "the source poll selects before the next draw");
            var helpDisabled = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
            for (var i = 0; i < 3; i++) helpDisabled.Advance(false, false, true, false, false);
            helpDisabled.CloseCurrent(false);
            helpDisabled.Advance(false, false, false, false, false, 0, false, false);
            Assert.That(helpDisabled.Visible, Is.False, "disabled help hides and freezes the carrier");
            helpDisabled.Advance(false, false, true, false, false, 0, false, false);
            Assert.That(helpDisabled.VisibleRow, Is.EqualTo(1), "source sub_87C10 does not clear while help is disabled");
            var context = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
            context.RequestObjectiveContext(76, true);
            for (var i = 0; i < 9; i++) context.Advance(false, false, true, false, false);
            Assert.That(context.VisibleRow, Is.EqualTo(76), "the source pending objective row precedes ordinary row1");
            context.Advance(false, false, true, false, true);
            Assert.That(context.Visible, Is.False, "an objective or menu suppresses the active popup");
            var equalPriority = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
            for (var i = 0; i < 3; i++) equalPriority.Advance(true, false, true, false, false);
            Assert.That(equalPriority.VisibleRow, Is.EqualTo(1), "equal overdue priority keeps the earlier source row");
            var wallPriority = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
            for (var i = 0; i < 3; i++) wallPriority.Advance(false, false, true, false, false, 1);
            Assert.That(wallPriority.VisibleRow, Is.EqualTo(3), "eligible source row3 has higher priority than row1");
            var reset = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
            reset.Advance(false, false, true, false, false);
            Assert.That(reset.Visible, Is.False, "a fresh owner has no retained popup");
        }

        [Test]
        public void OrdinaryHelpScannerKeepsPerRowPriorityAgeAndPendingContextOrder()
        {
            using var cache = new OwnerCampaignMenuCache(CampaignMenuCache);
            var tied = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
            tied.Promote(41, 6);
            tied.Advance(true, false, true, false, false, 1);
            tied.Advance(true, false, true, false, false, 1);
            Assert.That(tied.Visible, Is.False, "both rows start at their own threshold; the second pass selects");
            tied.Advance(false, false, true, false, false, 0);
            Assert.That(tied.VisibleRow, Is.EqualTo(3), "equal priority and overdue age retain earlier source row3");
            Assert.That(tied.Visible, Is.True, "current contact is not reevaluated while drawing a selected popup");

            foreach (var promoted in new[] { false, true })
            {
                var scan = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
                if (promoted) scan.Promote(41, 6);
                scan.Advance(false, false, true, false, false, 1);
                scan.Advance(false, false, true, false, false, 1);
                scan.Advance(false, false, true, false, false, 1);
                Assert.That(scan.VisibleRow, Is.EqualTo(3));
                scan.CloseCurrent(true);
                // Reset only selected row3, then let its real 600-pass cooldown
                // expire. Row41 continues aging even while neither is eligible.
                for (var pass = 0; pass < 601; pass++)
                    scan.Advance(false, false, true, false, false, 0);
                scan.Advance(true, false, true, false, false, -1);
                Assert.That(scan.Visible, Is.False, "selection does not draw in the same pass");
                scan.Advance(true, false, true, false, false, 0);
                Assert.That(scan.VisibleRow, Is.EqualTo(promoted ? 41 : 3),
                    "priority precedes age; at equal priority the largest positive overdue age wins");
                if (promoted)
                {
                    scan.CloseCurrent(true);
                    for (var pass = 0; pass < 20; pass++)
                        scan.Advance(true, false, true, false, false, 1);
                    scan.Advance(false, false, true, false, false, 0);
                    Assert.That(scan.VisibleRow, Is.EqualTo(3), "closing row41 cannot reset row3's retained age");
                }
            }

            var pending = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
            pending.Promote(41, 6);
            pending.RequestObjectiveContext(76, true);
            for (var pass = 0; pass < 8; pass++)
                pending.Advance(true, true, true, false, false, 1);
            pending.Advance(true, true, true, false, false, 1);
            Assert.That(pending.VisibleRow, Is.EqualTo(76), "word88 bypasses even eligible priority6 ordinary rows");
            pending.Advance(true, true, true, true, false, 1);
            Assert.That(pending.Visible, Is.False, "pause hides without consuming the retained context");
            pending.Advance(true, true, false, false, false, 1);
            Assert.That(pending.Visible, Is.False, "disabled help cannot select or advance any row");
            pending.Advance(true, true, true, false, true, 1);
            Assert.That(pending.Visible, Is.False, "objective/menu suppression closes the selected context");
            for (var pass = 0; pass < 20; pass++)
                pending.Advance(false, false, true, false, false, 1);
            pending.Advance(false, false, true, false, false, 0);
            Assert.That(pending.VisibleRow, Is.EqualTo(3));

            var ended = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
            ended.Promote(41, 6);
            for (var pass = 0; pass < 3; pass++)
                ended.Advance(true, true, true, false, false, 1, true);
            Assert.That(ended.Visible, Is.False,
                "source IsLevelEnd removes both ordinary rows3/41 before an exit sequence begins");
            ended.RequestObjectiveContext(80, true);
            for (var pass = 0; pass < 8; pass++)
                ended.Advance(true, true, true, false, false, 1, true);
            ended.Advance(true, true, true, false, false, 1, true);
            Assert.That(ended.VisibleRow, Is.EqualTo(80), "forced pending80 bypasses the level-end ordinary mask");

            var retained = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
            retained.Advance(false, false, true, false, false, 1);
            retained.Advance(false, false, true, false, false, 1);
            retained.Advance(false, false, true, false, false, 1, true);
            Assert.That(retained.VisibleRow, Is.EqualTo(3),
                "the mask changes polling eligibility; it does not erase an already selected popup");
        }

        [Test]
        public void WallContactReceiptReachesSelectedSourceGlyphsWithoutConsumingTheOwner()
        {
            var path = Environment.GetEnvironmentVariable("MC_WALL_HELP_RECEIPT") ??
                Path.Combine(PrivateRoot, "generated/help-20261115/wall-contact-observation.json");
            var receipt = JsonUtility.FromJson<WallHelpReceipt>(File.ReadAllText(path));
            Assert.That(receipt, Is.Not.Null);
            foreach (var frame in new[] { receipt.contact, receipt.dry, receipt.reset })
            {
                Assert.That(frame, Is.Not.Null);
                EngineSessionTransport.ValidateIdentity(frame.snapshot.identity);
                EngineSessionTransport.ValidateCampaignSnapshot(frame.snapshot.value, frame.snapshot.identity);
                Assert.That(frame.render.tick, Is.EqualTo(frame.snapshot.identity.tick));
                Assert.That(frame.render.state_hash, Is.Null.Or.Empty);
                Assert.That(frame.render.terrain_hash, Is.Null.Or.Empty);
                Assert.That(frame.render.effect_journal_sha256, Is.Null.Or.Empty);
                Assert.That(frame.render.player_wall_contact_counter,
                    Is.EqualTo(frame.snapshot.value.player_wall_contact_counter));
                Assert.That(frame.render.objective_progress.all_objectives_satisfied,
                    Is.EqualTo(frame.snapshot.value.objective_progress.all_objectives_satisfied));
                Assert.That(JsonUtility.ToJson(frame.render.player_pose),
                    Is.EqualTo(JsonUtility.ToJson(frame.snapshot.value.player_pose)));
            }
            Assert.That(typeof(EngineSessionTransport.CampaignSnapshotValue).GetField("player_wall_contact_counter").FieldType,
                Is.EqualTo(typeof(sbyte)), "the source signed byte must not become an unsigned duration");
            Assert.That(receipt.contact.render.player_wall_contact_counter, Is.EqualTo(1));
            Assert.That(receipt.contact.render.objective_textbox_ticks, Is.Zero,
                "the actual source producer receipt must allow ordinary help with no visible objective");
            Assert.That(receipt.contact.render.objective_progress.all_objectives_satisfied, Is.False);
            Assert.That(receipt.dry.render.player_wall_contact_counter, Is.Zero);
            Assert.That(ulong.Parse(receipt.dry.render.tick), Is.EqualTo(ulong.Parse(receipt.contact.render.tick) + 1));
            Assert.That(receipt.reset.render.player_wall_contact_counter, Is.Zero);
            Assert.That(receipt.reset.render.tick, Is.EqualTo("0"));

            var activeBefore = NativeSession.ActiveSessionCount;
            var temporary = Path.Combine(Path.GetTempPath(), "mc-wall-help-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(temporary);
            Texture2D scene = null;
            try
            {
                // The receipt above is the real controlled Level3 collision.
                // This separate native owner and retained Night reset frame
                // check the managed component, not same-session earned travel.
                using var native = NativeSession.OpenGameRuntime(ResolverPackages());
                native.GameRuntimePause();
                using var cache = new OwnerCampaignMenuCache(CampaignMenuCache);
                var preferences = new PlayerPreferencesPresenter(json => GameRuntimeTransport.PreferencesCommand(
                    native.Handle, native.GameRuntimeIdentity(), json), Path.Combine(temporary, "profile"));
                preferences.BindLanguages(cache);
                Assert.That(preferences.Current.preferences.show_help, Is.True);
                var identity = native.GameRuntimeIdentity();
                var generation = ulong.Parse(identity.session.generation);
                var before = JsonUtility.ToJson(identity);
                var replay = native.GameRuntimeSaveReplay();
                var observed = native.GameRuntimeCampaignSnapshot();
                Assert.That(observed.value.player_wall_contact_counter, Is.Zero);
                void Unchanged()
                {
                    Assert.That(JsonUtility.ToJson(native.GameRuntimeIdentity()), Is.EqualTo(before));
                    Assert.That(native.GameRuntimeCampaignSnapshot().value.state_hash, Is.EqualTo(observed.value.state_hash));
                    Assert.That(native.GameRuntimeCampaignSnapshot().value.player_wall_contact_counter, Is.Zero);
                    CollectionAssert.AreEqual(replay, native.GameRuntimeSaveReplay(),
                        "queries and rendering never dispatch a native frame or consume the source counter");
                }

                var popup = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
                var contact = receipt.contact.render.player_wall_contact_counter;
                var dry = receipt.dry.render.player_wall_contact_counter;
                // Hold the genuine sampled predicate for the two ordinary
                // selection passes of this component; no claim that the
                // captured native collision repeated on a second gameplay tick.
                popup.Advance(false, false, true, false, false, contact,
                    receipt.contact.render.objective_progress.all_objectives_satisfied);
                popup.Advance(false, false, true, false, false, contact,
                    receipt.contact.render.objective_progress.all_objectives_satisfied);
                Assert.That(popup.Visible, Is.False);
                popup.Advance(false, false, true, false, false, dry,
                    receipt.dry.render.objective_progress.all_objectives_satisfied);
                Assert.That(popup.Visible, Is.True);
                Assert.That(popup.VisibleRow, Is.EqualTo(3));
                var binding = cache.GameplayHelpForRow(popup.VisibleRow);
                var text = cache.LanguageText(binding.TextRecord);
                Assert.That((binding.TextRecord, binding.Duration, binding.X, binding.Y), Is.EqualTo((288, 50, 320, 80)));
                Assert.That(text, Is.EqualTo("Walls cannot be passed"));

                var ownerRoot = Path.Combine(PrivateRoot, "generated/settlement-20261105-integration/owner-closed-v6");
                var owner = OwnerAssetCache.OpenAndValidate(Path.Combine(ownerRoot, "night-presentation-v6"),
                    MagicCarpetRuntime.ExpectedPackageHash, MagicCarpetRuntime.ExpectedOwnerAssetClosureHash,
                    Path.Combine(ownerRoot, "level1"));
                var retained = owner.ReadOutput(CampaignTransitionPresenter.IndicesPath);
                Assert.That(retained.Length, Is.EqualTo(320 * 200));
                var palette = Array.ConvertAll(owner.ReadOutput(CampaignTransitionPresenter.PalettePath),
                    value => checked((byte)(value * 4)));
                scene = new Texture2D(320, 200, TextureFormat.R8, false, true)
                    { filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp };
                scene.LoadRawTextureData(retained); scene.Apply(false, false);
                using var presenter = new SourceObjectiveTextboxPresenter(cache.ObjectiveForProfile("night"));
                presenter.PresentContextHint(text, generation, binding.X, binding.Y);
                var output = presenter.Render(1, scene, generation, 0, palette, true);
                var pixels = ReadObjectivePixels(output);
                const System.Reflection.BindingFlags flags = System.Reflection.BindingFlags.Instance |
                    System.Reflection.BindingFlags.NonPublic;
                var raster = (ObjectiveTextboxCompositor.Raster)typeof(SourceObjectiveTextboxPresenter)
                    .GetField("raster", flags).GetValue(presenter);
                Assert.That((raster.X, raster.Y, raster.Width, raster.Height), Is.EqualTo((208, 80, 224, 38)));
                Assert.That((raster.BlendX, raster.BlendY, raster.BlendWidth, raster.BlendHeight),
                    Is.EqualTo((224, 90, 192, 18)), "source 22-byte text, 8x14 layout and owner frame extents");
                Color32 Colour(int index) => new(palette[index * 3], palette[index * 3 + 1], palette[index * 3 + 2], 255);
                Color32 At(int x, int y) => pixels[(raster.Height - 1 - (y - raster.Y)) * raster.Width + x - raster.X];
                var artwork = cache.ObjectiveForProfile("night");
                Assert.That(artwork.HelpTextColor, Is.EqualTo(176));
                Assert.That(At(232, 94), Is.EqualTo(Colour(176)),
                    "source W byte+1 glyph88 first set pixel is text232/92 +(0,2)");
                var blendPixel = (90 - raster.Y) * raster.Width + 224 - raster.X;
                Assert.That(raster.IndicesThenMask[raster.Width * raster.Height + blendPixel], Is.Zero);
                // The GPU samples this 320x200 retained source via normalized
                // 640x480 pixel centers; do not read it as a full-size texture.
                var underlying = retained[37 * 320 + 112];
                var blended = artwork.Tables[artwork.TableOffset + artwork.BlendSelector * 256 + underlying];
                Assert.That(At(224, 90), Is.EqualTo(Colour(blended)),
                    "the unmasked textbox interior blends the actual retained source index");
                for (var repaint = 0; repaint < 3; repaint++)
                {
                    CollectionAssert.AreEqual(pixels, ReadObjectivePixels(presenter.Render(1, scene,
                        generation, 0, palette, true)));
                    Assert.That(popup.VisibleRow, Is.EqualTo(3));
                    Unchanged();
                }
                Assert.That(presenter.Render(1, scene, generation + 1, 0, palette, true), Is.Null);
                Assert.That(presenter.Render(0, scene, generation, 0, palette, true), Is.Null);
                for (var sourceDraw = 1; sourceDraw < binding.Duration; sourceDraw++)
                {
                    popup.Advance(false, false, true, false, false, dry);
                    Assert.That(popup.Visible, Is.True, "all 50 selected source draws survive contact clearing");
                    Assert.That(popup.VisibleRow, Is.EqualTo(3));
                }
                popup.Advance(false, false, true, false, false, dry);
                Assert.That(popup.Visible, Is.False, "the following ordinary pass clears the expired overlay");
                Assert.That(presenter.Render(popup.Visible ? 1 : 0, scene, generation, 0, palette, true), Is.Null);
                var off = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
                for (var pass = 0; pass < 3; pass++)
                    off.Advance(false, false, false, false, false, contact);
                Assert.That(off.Visible, Is.False);
                off.Advance(false, false, true, true, false, contact);
                Assert.That(off.Visible, Is.False);
                off.Advance(false, false, true, false, false, receipt.reset.render.player_wall_contact_counter);
                Assert.That(off.Visible, Is.False, "fresh/reset state plus a cleared native counter cannot select wall help");

                var objectiveKey = new MagicCarpetRuntime.ManaHelpHint(cache.GameplayHelpForRow);
                objectiveKey.Advance(false, false, true, false, false);
                objectiveKey.Advance(false, false, true, false, false);
                objectiveKey.Advance(false, false, true, false, false);
                Assert.That(objectiveKey.VisibleRow, Is.EqualTo(1));
                var objectiveKeyBinding = cache.GameplayHelpForRow(objectiveKey.VisibleRow);
                var objectiveKeyText = cache.LanguageText(objectiveKeyBinding.TextRecord);
                Assert.That(objectiveKeyText, Is.EqualTo("Press 'O' to show current objective"));
                presenter.PresentContextHint(objectiveKeyText, generation, objectiveKeyBinding.X, objectiveKeyBinding.Y);
                var keyOutput = presenter.Render(1, scene, generation, 0, palette, true);
                var keyPixels = ReadObjectivePixels(keyOutput);
                var keyRaster = (ObjectiveTextboxCompositor.Raster)typeof(SourceObjectiveTextboxPresenter)
                    .GetField("raster", flags).GetValue(presenter);
                Assert.That((keyRaster.X, keyRaster.Y, keyRaster.Width, keyRaster.Height), Is.EqualTo((160, 80, 320, 38)),
                    "source 35-byte width and actual frame sprites");
                Assert.That(keyPixels[(keyRaster.Height - 1 - (94 - keyRaster.Y)) * keyRaster.Width + 180 - keyRaster.X],
                    Is.EqualTo(Colour(176)), "source HFONT3 P glyph mask(0,2) at text origin180/92");
                CollectionAssert.AreEqual(keyPixels, ReadObjectivePixels(presenter.Render(1, scene, generation, 0, palette, true)));
                Assert.That(objectiveKey.VisibleRow, Is.EqualTo(1), "rendering does not advance the source help clock");
                Unchanged();
            }
            finally
            {
                if (scene != null) UnityEngine.Object.DestroyImmediate(scene);
                Directory.Delete(temporary, true);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
        }

        [Test]
        public void SourceContextHelpBindsOwnerRowsAndRejectsRehashedReplacement()
        {
            using (var cache = new OwnerCampaignMenuCache(CampaignMenuCache))
            {
                var objectiveKey = cache.GameplayHelpForRow(1);
                Assert.That((objectiveKey.Row, objectiveKey.TextRecord, objectiveKey.Duration, objectiveKey.Flags,
                    objectiveKey.X, objectiveKey.Y, objectiveKey.AgeThreshold, objectiveKey.Priority),
                    Is.EqualTo((1, 286, 50, 4, 320, 80, 600, 2)));
                Assert.That(cache.LanguageText(objectiveKey.TextRecord), Is.EqualTo("Press 'O' to show current objective"));
                var wall = cache.GameplayHelpForRow(3);
                Assert.That((wall.Row, wall.TextRecord, wall.Duration, wall.Flags, wall.X, wall.Y,
                    wall.AgeThreshold, wall.Priority), Is.EqualTo((3, 288, 50, 4, 320, 80, 600, 6)));
                Assert.That(cache.LanguageText(wall.TextRecord), Is.EqualTo("Walls cannot be passed"));
                for (var row = 72; row <= 80; row++)
                {
                    var binding = cache.GameplayHelpForRow(row);
                    Assert.That((binding.Row, binding.TextRecord, binding.Duration, binding.Flags, binding.X, binding.Y),
                        Is.EqualTo((row, 334 + row - 72, 50, 4, 320, 80)));
                    Assert.That(binding.AgeThreshold, Is.EqualTo(row == 80 ? 140 : 600));
                    Assert.That(binding.Priority, Is.EqualTo(row == 80 ? 4 : 3));
                    Assert.That(cache.LanguageText(binding.TextRecord), Is.Not.Empty);
                }
            }
            var legacyPath = Path.Combine(PrivateRoot, "generated/locale-source-menu-20261115-candidate2");
            Assert.Throws<InvalidDataException>(() => new OwnerCampaignMenuCache(legacyPath),
                "the unmodified historical three-sprite settings binding lacks source Default");
            var legacyFixture = Path.Combine(Path.GetTempPath(), "mc-legacy-help-negative-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(legacyFixture);
            try
            {
                // Complete only the missing settings sprite with the current
                // actual owner bytes and receipt; the historical help and
                // pause bindings remain absent for their independent gates.
                CopyOwnerMenuFixture(legacyPath, legacyFixture);
                var oldManifestPath = Path.Combine(legacyFixture, "manifest.json");
                // Keep absent optional fields absent using the existing .NET
                // JSON reader; typed JsonUtility round trips turn null strings
                // into empty strings and would change unrelated admission.
                var oldManifest = ReadReplayDocument(File.ReadAllBytes(oldManifestPath));
                var currentManifest = ReadReplayDocument(File.ReadAllBytes(Path.Combine(CampaignMenuCache, "manifest.json")));
                var currentDefault = currentManifest.SelectSingleNode("/root/settings/bitmaps/item[selector='109']");
                Assert.That(currentDefault, Is.Not.Null);
                oldManifest.SelectSingleNode("/root/settings/bitmaps").AppendChild(oldManifest.ImportNode(currentDefault, true));
                var receipt = currentManifest.SelectSingleNode("/root/outputs/item[path='settings-109.rgba8']");
                Assert.That(receipt, Is.Not.Null);
                oldManifest.SelectSingleNode("/root/outputs").AppendChild(oldManifest.ImportNode(receipt, true));
                File.Copy(Path.Combine(CampaignMenuCache, "settings-109.rgba8"), Path.Combine(legacyFixture, "settings-109.rgba8"));
                using (var bytes = new MemoryStream())
                {
                    using (var writer = JsonReaderWriterFactory.CreateJsonWriter(bytes, Encoding.UTF8, false))
                        oldManifest.WriteTo(writer);
                    File.WriteAllBytes(oldManifestPath, bytes.ToArray());
                }
                using var legacy = new OwnerCampaignMenuCache(legacyFixture);
                Assert.Throws<InvalidDataException>(() => legacy.GameplayHelpForRow(72));
                Assert.Throws<InvalidDataException>(() => legacy.GameplayHelpForRow(3));
                Assert.Throws<InvalidDataException>(() => legacy.GameplayHelpForRow(1));
                Assert.Throws<InvalidDataException>(() => { var ignored = legacy.ObjectiveForProfile("night").PauseSprites; });
            }
            finally { Directory.Delete(legacyFixture, true); }

            var fixture = Path.Combine(Path.GetTempPath(), "mc-context-help-negative-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(fixture);
            try
            {
                CopyOwnerMenuFixture(CampaignMenuCache, fixture);
                var privateType = System.Reflection.BindingFlags.NonPublic;
                var manifestType = typeof(OwnerCampaignMenuCache).GetNestedType("Manifest", privateType);
                var documentType = typeof(OwnerCampaignMenuCache).GetNestedType("GameplayHelpDocument", privateType);
                var manifestPath = Path.Combine(fixture, "manifest.json");
                var originalManifest = File.ReadAllText(manifestPath);
                var manifest = JsonUtility.FromJson(originalManifest, manifestType);
                manifestType.GetField("gameplay_help_path").SetValue(manifest, "unbound-help.json");
                File.WriteAllText(manifestPath, JsonUtility.ToJson(manifest) + "\n");
                Assert.That(Assert.Throws<InvalidDataException>(() => new OwnerCampaignMenuCache(fixture)).Message,
                    Does.Contain("source gameplay help path"));

                manifest = JsonUtility.FromJson(originalManifest, manifestType);
                var tablePath = Path.Combine(fixture, "gameplay-help.json");
                var originalTable = File.ReadAllBytes(tablePath);
                foreach (var changedRow in new[] { 3, 76 })
                {
                    var table = JsonUtility.FromJson(Encoding.ASCII.GetString(originalTable), documentType);
                    var rows = (Array)documentType.GetField("rows").GetValue(table);
                    object changed = null;
                    foreach (var row in rows)
                        if ((int)row.GetType().GetField("row").GetValue(row) == changedRow) changed = row;
                    Assert.That(changed, Is.Not.Null);
                    changed.GetType().GetField(changedRow == 3 ? "text_record" : "duration")
                        .SetValue(changed, changedRow == 3 ? 289 : 51);
                    var altered = Encoding.ASCII.GetBytes(JsonUtility.ToJson(table) + "\n");
                    Assert.That(altered.Length, Is.EqualTo(originalTable.Length));
                    File.WriteAllBytes(tablePath, altered);
                    manifest = JsonUtility.FromJson(originalManifest, manifestType);
                    var outputs = (Array)manifestType.GetField("outputs").GetValue(manifest);
                    foreach (var output in outputs)
                    {
                        var type = output.GetType();
                        if ((string)type.GetField("path").GetValue(output) != "gameplay-help.json") continue;
                        using var sha = System.Security.Cryptography.SHA256.Create();
                        type.GetField("sha256").SetValue(output,
                            BitConverter.ToString(sha.ComputeHash(altered)).Replace("-", "").ToLowerInvariant());
                    }
                    File.WriteAllText(manifestPath, JsonUtility.ToJson(manifest) + "\n");
                    Assert.That(Assert.Throws<InvalidDataException>(() => new OwnerCampaignMenuCache(fixture)).Message,
                        Does.Contain("source gameplay help table identity"),
                        "a coherent output receipt cannot replace pinned source row" + changedRow);
                }
                File.WriteAllText(manifestPath, originalManifest);
                File.WriteAllBytes(tablePath, originalTable);
                var pauseMaskPath = Path.Combine(fixture, "pause-night-177.mask.u8");
                var pauseMask = File.ReadAllBytes(pauseMaskPath);
                pauseMask[0] ^= 1;
                File.WriteAllBytes(pauseMaskPath, pauseMask);
                Assert.That(Assert.Throws<InvalidDataException>(() => new OwnerCampaignMenuCache(fixture)).Message,
                    Does.Contain("campaign cache output receipt mismatch"), "indexed pause planes retain their exact owner receipts");
            }
            finally { Directory.Delete(fixture, true); }
        }

        [UnityEngine.TestTools.UnityTest]
        public System.Collections.IEnumerator PauseHoverUsesSourceRowsAndBlendsThePriorMenuBeforeItsTextbox()
        {
            var activeBefore = NativeSession.ActiveSessionCount;
            var temporary = Path.Combine(Path.GetTempPath(), "mc-pause-hover-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(temporary);
            var gui = ScriptableObject.CreateInstance<CampaignGuiWindow>();
            var shown = false;
            Texture2D scene = null;
            NativeSession native = null;
            CampaignMenuPresenter menu = null;
            try
            {
                // This component check uses the supported headless resource
                // binding. The retained, receipt-qualified source frame below
                // is explicit input, not a claimed fresh native world capture.
                var resourcePath = Environment.GetEnvironmentVariable("MC_CAMPAIGN_SAVE_RESOURCES") ??
                    Path.Combine(PrivateRoot, "generated/continuity-20261115/installed-full-resources.json");
                GameRuntimeTransport.InstalledPackageResource[] installed;
                using (var resources = File.OpenRead(resourcePath))
                    installed = (GameRuntimeTransport.InstalledPackageResource[])new DataContractJsonSerializer(
                        typeof(GameRuntimeTransport.InstalledPackageResource[])).ReadObject(resources);
                Assert.That(installed, Is.Not.Empty);
                native = NativeSession.OpenGameRuntime(installed);
                native.GameRuntimePause();
                var preferences = new PlayerPreferencesPresenter(json => GameRuntimeTransport.PreferencesCommand(
                    native.Handle, native.GameRuntimeIdentity(), json), Path.Combine(temporary, "profile"));
                var owner = OwnerAssetCache.OpenAndValidate(installed[0].presentation_cache_path,
                    MagicCarpetRuntime.ExpectedPackageHash, MagicCarpetRuntime.ExpectedOwnerAssetClosureHash,
                    installed[0].package_dir);
                menu = new CampaignMenuPresenter(owner, CampaignMenuCache);
                preferences.BindLanguages(menu.LocaleCache);
                menu.AttachPreferences(preferences);
                Assert.That(preferences.ApplySource(next => next.music_enabled = false), Is.True);
                Assert.That(preferences.Current.preferences.show_help, Is.True);
                var identity = native.GameRuntimeIdentity();
                Assert.That(identity.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                Assert.That(identity.active_runtime_flow.bound_level_id, Is.EqualTo("level.magic_carpet_2.night_spire"));
                var generation = ulong.Parse(identity.session.generation);
                var frame = owner.ReadOutput(CampaignTransitionPresenter.IndicesPath);
                var rgb6 = owner.ReadOutput(CampaignTransitionPresenter.PalettePath);
                var palette = Array.ConvertAll(rgb6, value => checked((byte)(value * 4)));
                scene = new Texture2D(320, 200, TextureFormat.R8, false, true)
                    { filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp };
                scene.LoadRawTextureData(frame); scene.Apply(false, false);
                var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
                var background = (Texture2D)typeof(CampaignMenuPresenter).GetField("background", flags).GetValue(menu);
                var view = new Rect();
                var origin = Vector2.zero;
                var received = Vector2.zero;
                var controlledCursor = new Vector2(10, 10);
                CampaignMenuPresenter.Action action = CampaignMenuPresenter.Action.None;
                SourceObjectiveTextboxPresenter Presenter() => (SourceObjectiveTextboxPresenter)typeof(SourcePauseMenu)
                    .GetField("pauseHelp", flags).GetValue(menu.LocaleCache.PauseMenu);
                var stateBefore = JsonUtility.ToJson(identity);
                var replayBefore = native.GameRuntimeSaveReplay();
                void Unchanged()
                {
                    Assert.That(JsonUtility.ToJson(native.GameRuntimeIdentity()), Is.EqualTo(stateBefore));
                    CollectionAssert.AreEqual(replayBefore, native.GameRuntimeSaveReplay(),
                        "actual hover/layout/repaint cannot advance the paused owner or its replay");
                }
                gui.DrawRuntime = () => {
                    var scale = Mathf.Min(Screen.width / 640f, Screen.height / 480f);
                    view = new Rect((Screen.width - 640 * scale) / 2, (Screen.height - 480 * scale) / 2,
                        640 * scale, 480 * scale);
                    origin = GUIUtility.GUIToScreenPoint(Vector2.zero) - gui.position.position;
                    // EditorWindow.SendEvent forwards injected mouse positions
                    // for input events but Repaint reads the OS cursor. This
                    // component check owns one logical cursor across all GUI
                    // callbacks; it does not claim physical mouse delivery.
                    var input = Event.current;
                    var hostCursor = input.mousePosition;
                    try
                    {
                        input.mousePosition = view.position + controlledCursor * scale;
                        received = input.mousePosition;
                        GUI.DrawTexture(view, background);
                        identity = native.GameRuntimeIdentity();
                        action = menu.Draw(identity, true, false, false, null);
                        menu.DrawPauseHoverHelp(identity, scene, generation, 0, view, palette, true);
                    }
                    finally { input.mousePosition = hostCursor; }
                };
                gui.wantsMouseMove = true;
                gui.minSize = new Vector2(640, 480); gui.position = new Rect(0, 0, 640, 480);
                gui.Show(); shown = true; gui.position = new Rect(0, 0, 640, 480);
                var deadline = Time.realtimeSinceStartupAsDouble + 10;
                while (view.width <= 0)
                {
                    Assert.That(Time.realtimeSinceStartupAsDouble, Is.LessThan(deadline), "the real GUI host must draw its fitted viewport");
                    gui.Repaint();
                    UnityEditor.EditorApplication.QueuePlayerLoopUpdate();
                    yield return null;
                }
                void Send(EventType type, Vector2 source, int button = 0)
                {
                    controlledCursor = source;
                    var local = view.position + source * (view.width / 640f);
                    var beforeDraw = gui.DrawCount;
                    gui.SendEvent(new Event { type = type, mousePosition = local + origin, button = button });
                    Assert.That(gui.DrawCount, Is.GreaterThan(beforeDraw), "the actual GUI host must dispatch the component event");
                }
                void Hover(Vector2 source)
                {
                    Send(EventType.MouseMove, source);
                    Send(EventType.Layout, source);
                    Send(EventType.Repaint, source);
                    Assert.That(received, Is.EqualTo(view.position + source * (view.width / 640f)),
                        "every GUI callback passes the declared controlled cursor to real source hit testing");
                }
                var texts = new[] { "Save Level", "Load Level", "Sound Volume", "Music Volume", "Options" };
                var anchors = new[] { new Vector2Int(360, 82), new Vector2Int(360, 114),
                    new Vector2Int(308, 146), new Vector2Int(360, 146), new Vector2Int(360, 178) };
                for (var slot = 0; slot < 5; slot++)
                {
                    var hit = SourcePauseMenu.IconRect(174 + slot);
                    Hover(hit.center);
                    Assert.That(menu.PauseHoverHelpRow, Is.EqualTo(86 + slot));
                    Assert.That(menu.PauseHoverHelpAnchor, Is.EqualTo(anchors[slot]));
                    var binding = menu.GameplayHelpForRow(86 + slot);
                    Assert.That(menu.GameplayLanguageText(binding.TextRecord), Is.EqualTo(texts[slot]));
                    Assert.That(typeof(SourceObjectiveTextboxPresenter).GetField("currentText", flags).GetValue(Presenter()),
                        Is.EqualTo(texts[slot]), "the actual fitted source hover reaches selected owner glyphs");
                    Assert.That(action, Is.EqualTo(CampaignMenuPresenter.Action.None));
                    Unchanged();
                }
                Hover(SourcePauseMenu.IconRect(174).center);
                var presenter = Presenter();
                var raster = (ObjectiveTextboxCompositor.Raster)typeof(SourceObjectiveTextboxPresenter)
                    .GetField("raster", flags).GetValue(presenter);
                Assert.That((raster.X, raster.Y, raster.Width, raster.Height), Is.EqualTo((321, 82, 128, 85)));
                Assert.That((raster.BlendX, raster.BlendY, raster.BlendWidth, raster.BlendHeight), Is.EqualTo((337, 139, 96, 18)));
                var output = (RenderTexture)typeof(SourceObjectiveTextboxPresenter).GetField("output", flags).GetValue(presenter);
                var pixels = ReadObjectivePixels(output);
                Color32 Colour(int index) => new(palette[index * 3], palette[index * 3 + 1], palette[index * 3 + 2], 255);
                Color32 At(int x, int y) => pixels[(raster.Height - 1 - (y - raster.Y)) * raster.Width + x - raster.X];
                var assets = menu.ObjectiveForProfile("night");
                Assert.That(assets.HelpTextColor, Is.EqualTo(176));
                Assert.That(At(346, 143), Is.EqualTo(Colour(176)), "HFONT3 S first mask pixel at source text345/141 +(1,2)");
                var sourcePixel = (144 - raster.Y) * raster.Width + 338 - raster.X;
                Assert.That(raster.IndicesThenMask[raster.Width * raster.Height + sourcePixel], Is.Zero);
                Assert.That(assets.PauseSprites[3].Mask[4 * 51 + 19], Is.EqualTo(1));
                Assert.That(assets.PauseSprites[3].Indices[4 * 51 + 19], Is.EqualTo(150));
                Assert.That(At(338, 144), Is.EqualTo(Colour(57)),
                    "real Music177 index150 is shaded twice before the popup's third source LUT application");
                Assert.That(frame[60 * 320 + 169], Is.EqualTo(28));
                Assert.That(At(338, 144), Is.Not.EqualTo(Colour(223)), "world-only sampling of this real retained frame must fail");
                Send(EventType.Repaint, SourcePauseMenu.IconRect(174).center);
                CollectionAssert.AreEqual(pixels, ReadObjectivePixels(output));
                Unchanged();

                // Declared shader boundary control: shipped owner masks are
                // opaque. A synthetic mask0/shade1 cell must shade the actual
                // underlying frame, independently of sprite opacity.
                using (var synthetic = new SourceObjectiveTextboxPresenter(assets))
                {
                    synthetic.SetPausePrior(new RectInt(338, 144, 1, 1),
                        new[] { new Color32(0, 255, 0, 0) }, generation);
                    var shade = assets.TableOffset + assets.BlendSelector * 256;
                    var twice = assets.Tables[shade + assets.Tables[shade + 28]];
                    var actual = ReadObjectivePixels(synthetic.RenderPausePrior(scene, generation, palette, true));
                    Assert.That(actual[0], Is.EqualTo(Colour(twice)));
                    Assert.That(synthetic.RenderPausePrior(scene, generation + 1, palette, true), Is.Null);
                }

                Hover(new Vector2(10, 10));
                Assert.That(menu.PauseHoverHelpRow, Is.Zero, "pointer leave clears the source byte_aa selection");
                Assert.That(preferences.ApplySource(next => next.show_help = false), Is.True);
                stateBefore = JsonUtility.ToJson(native.GameRuntimeIdentity()); replayBefore = native.GameRuntimeSaveReplay();
                Hover(SourcePauseMenu.IconRect(175).center);
                Assert.That(menu.PauseHoverHelpRow, Is.EqualTo(87), "help-off changes draw admission, not source cursor hit testing");
                Assert.That(typeof(SourceObjectiveTextboxPresenter).GetField("currentText", flags).GetValue(Presenter()),
                    Is.EqualTo("Save Level"), "disabled help does not present the newly hovered Load text");
                Unchanged();
                Assert.That(preferences.ApplySource(next => next.show_help = true), Is.True);
                stateBefore = JsonUtility.ToJson(native.GameRuntimeIdentity()); replayBefore = native.GameRuntimeSaveReplay();
                Send(EventType.MouseDown, SourcePauseMenu.IconRect(174).center);
                Send(EventType.Repaint, SourcePauseMenu.IconRect(174).center);
                Assert.That(menu.PauseHoverHelpRow, Is.Zero, "the actual save confirmation owns the cursor");
                Unchanged();
                menu.ResetPauseHoverHelp();
                Hover(SourcePauseMenu.IconRect(178).center);
                Send(EventType.MouseDown, SourcePauseMenu.IconRect(178).center);
                Send(EventType.Repaint, SourcePauseMenu.IconRect(178).center);
                Assert.That(menu.PauseHoverHelpRow, Is.Zero, "the actual options page clears pause-icon hover");
                Unchanged();
                Send(EventType.MouseDown, new Vector2(320, 67 + 12 * 18 + 9));
                Assert.That(action, Is.EqualTo(CampaignMenuPresenter.Action.ToggleIconsAndMap),
                    "the real source options row produces the same display action");
                native.GameRuntimeToggleIconsAndMap();
                Assert.That(native.GameRuntimeIdentity().display_settings.minimap, Is.False);
                Send(EventType.MouseDown, new Vector2(320, 67 + 12 * 18 + 9));
                Assert.That(action, Is.EqualTo(CampaignMenuPresenter.Action.ToggleIconsAndMap),
                    "the options submenu remains open after the first toggle");
                native.GameRuntimeToggleIconsAndMap();
                Assert.That(native.GameRuntimeIdentity().display_settings.minimap, Is.True);
                var beforePanel = native.GameRuntimeIdentity();
                Assert.That(beforePanel.display_settings.panels_opaque, Is.False, "Night LevelInit starts with transparent panels");
                Send(EventType.MouseDown, new Vector2(320, 67 + 13 * 18 + 9));
                Assert.That(action, Is.EqualTo(CampaignMenuPresenter.Action.TogglePanelTransparency),
                    "source Options row13 requests the same native transparency action");
                var opaquePanel = native.GameRuntimeTogglePanelTransparency();
                Assert.That(opaquePanel.display_settings.panels_opaque, Is.True);
                Assert.That((opaquePanel.generation, opaquePanel.session.generation, opaquePanel.session.tick),
                    Is.EqualTo((beforePanel.generation, beforePanel.session.generation, beforePanel.session.tick)));
                var panelNotice = native.GameRuntimeParticipantNotifications().value;
                Assert.That(panelNotice, Has.Length.EqualTo(1));
                Assert.That((panelNotice[0].kind, panelNotice[0].remaining_frames),
                    Is.EqualTo(("participant.panel_transparency_off", (short)50)));
                var panelRows = SourcePlayerNotificationsPresenter.Rows(panelNotice, false, false, false, 14, menu.GameplayLanguageText);
                Assert.That(panelRows[0].Text, Is.EqualTo(menu.GameplayLanguageText(441)));
                Send(EventType.MouseDown, new Vector2(320, 67 + 13 * 18 + 9));
                Assert.That(action, Is.EqualTo(CampaignMenuPresenter.Action.TogglePanelTransparency));
                Assert.That(native.GameRuntimeTogglePanelTransparency().display_settings.panels_opaque, Is.False,
                    "the Options submenu remains live and toggles the real owner back");
                Send(EventType.MouseDown, new Vector2(320, 67 + 14 * 18 + 9));
                Assert.That(action, Is.EqualTo(CampaignMenuPresenter.Action.ToggleFlatShading),
                    "source Options row14 requests the same native flat shading action");
                var flat = native.GameRuntimeToggleFlatShading();
                Assert.That(flat.display_settings.flat_shading, Is.True);
                var flatNotice = native.GameRuntimeParticipantNotifications().value;
                Assert.That((flatNotice[0].kind, flatNotice[0].remaining_frames),
                    Is.EqualTo(("participant.flat_shading_on", (short)50)));
                var flatRows = SourcePlayerNotificationsPresenter.Rows(flatNotice, false, false, false, 14, menu.GameplayLanguageText);
                Assert.That(flatRows[0].Text, Is.EqualTo(menu.GameplayLanguageText(442)));
                Send(EventType.MouseDown, new Vector2(320, 67 + 14 * 18 + 9));
                Assert.That(action, Is.EqualTo(CampaignMenuPresenter.Action.ToggleFlatShading));
                Assert.That(native.GameRuntimeToggleFlatShading().display_settings.flat_shading, Is.False);
                var plainRows = SourcePlayerNotificationsPresenter.Rows(native.GameRuntimeParticipantNotifications().value,
                    false, false, false, 14, menu.GameplayLanguageText);
                Assert.That(plainRows[0].Text, Is.EqualTo(menu.GameplayLanguageText(443)));
                stateBefore = JsonUtility.ToJson(native.GameRuntimeIdentity()); replayBefore = native.GameRuntimeSaveReplay();
                menu.ResetPauseHoverHelp();
                Hover(SourcePauseMenu.IconRect(174).center);
                menu.ResetPauseHoverHelp();
                Assert.That(menu.PauseHoverHelpRow, Is.Zero);
                Assert.That(Presenter(), Is.Null, "reset disposes the old-generation indexed popup owner");
                native.GameRuntimeResume();
                stateBefore = JsonUtility.ToJson(native.GameRuntimeIdentity()); replayBefore = native.GameRuntimeSaveReplay();
                Hover(SourcePauseMenu.IconRect(174).center);
                Assert.That(menu.PauseHoverHelpRow, Is.Zero, "normal resume no longer admits pause hover");
                Unchanged();
            }
            finally
            {
                gui.DrawRuntime = null;
                try { if (shown) gui.Close(); else UnityEngine.Object.DestroyImmediate(gui); }
                finally
                {
                    try { menu?.Dispose(); }
                    finally
                    {
                        native?.Dispose();
                        if (scene != null) UnityEngine.Object.DestroyImmediate(scene);
                        Directory.Delete(temporary, true);
                    }
                }
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
        }

        [TestCase((ushort)5, (ushort)4, (byte)0, "dragon-bee-20261105-integration/level5-owner-current/level5",
            "secret-lifecycle-20261111-source4-environment", 0x45)]
        [TestCase((ushort)31, (ushort)30, (byte)2, "mc-paj2-source30-campaign-package",
            "mc-paj2-environment-source30-audio-20261026", 0x33)]
        public void RegisteredLaterAndSecretObjectivesResolveLiveNativeSourceText(
            ushort external, ushort source, byte mapType, string packagePath, string ownerPath, int firstRecord)
        {
            var generated = Path.Combine(PrivateRoot, "generated");
            var night = Path.Combine(generated, "settlement-20261105-integration/owner-closed-v6");
            var cache = OwnerAssetCache.OpenAndValidate(Path.Combine(night, "night-presentation-v6"),
                MagicCarpetRuntime.ExpectedPackageHash, MagicCarpetRuntime.ExpectedOwnerAssetClosureHash,
                Path.Combine(night, "level1"));
            var records = Encoding.GetEncoding(28591).GetString(
                cache.ReadOutput("font/language-l1.records-nul.bin")).Split('\0');
            using var native = NativeCampaignSession.Open(Path.Combine(generated, packagePath),
                NativeSim.CampaignSelection.Create(external, source, mapType), Path.Combine(generated, ownerPath));
            var objective = native.ObjectiveStageState(0, 0);
            Assert.That(objective.isLevelEnd, Is.Zero, "the actual initial objective is not a staged completion");
            Assert.That(objective.textSelector, Is.Zero, "the actual initial source selector starts at zero");
            Assert.That(records[firstRecord], Is.Not.Empty, "the qualified original language record exists");
            Assert.That(new Mc2ObjectiveText(cache).Resolve(source, objective), Is.EqualTo(records[firstRecord]));

            var root = new GameObject("actual later objective glyph consumer");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var environment = OwnerAssetCache.OpenAndValidateEnvironment(
                Path.Combine(generated, ownerPath), Path.Combine(generated, packagePath));
            var surface = new CampaignPresentationSurface();
            var display = new RenderTexture(320, 200, 0, RenderTextureFormat.ARGB32,
                RenderTextureReadWrite.Linear);
            const System.Reflection.BindingFlags flags = System.Reflection.BindingFlags.Instance |
                System.Reflection.BindingFlags.NonPublic;
            try
            {
                typeof(MagicCarpetRuntime).GetField("campaignSession", flags).SetValue(runtime, native);
                typeof(MagicCarpetRuntime).GetField("gameRuntimeSecretSurface", flags).SetValue(runtime, surface);
                typeof(MagicCarpetRuntime).GetField("gameRuntimeEnvironmentCache", flags).SetValue(runtime, environment);
                typeof(MagicCarpetRuntime).GetField("gameRuntimeObjectiveText", flags).SetValue(runtime, new Mc2ObjectiveText(cache));
                typeof(MagicCarpetRuntime).GetField("gameRuntimeLoadingCachePath", flags).SetValue(runtime,
                    Path.Combine(generated, "mc2-level1-night-cache-v2"));
                typeof(MagicCarpetRuntime).GetField("gameRuntimeMenuCachePath", flags).SetValue(runtime,
                    CampaignMenuCache);
                typeof(MagicCarpetRuntime).GetMethod("UpdateGameRuntimeObjectiveTextbox", flags)
                    .Invoke(runtime, new object[] { surface });
                Assert.That(runtime.GameRuntimeObjectiveInstruction, Is.EqualTo(records[firstRecord]));
                var presenter = (SourceObjectiveTextboxPresenter)typeof(MagicCarpetRuntime)
                    .GetField("gameRuntimeObjectiveTextbox", flags).GetValue(runtime);
                Assert.That(presenter, Is.Not.Null, "the canonical runtime binds the actual source objective consumer");
                surface.PresentEnvironment(native, environment);
                var cameras = surface.FrameCamera.transform.parent.GetComponentsInChildren<Camera>();
                Array.Sort(cameras, (left, right) => left.depth.CompareTo(right.depth));
                foreach (var camera in cameras)
                {
                    if (camera.targetTexture == null) camera.targetTexture = display;
                    camera.Render();
                }
                Assert.That(surface.SceneHistoryGeneration, Is.EqualTo(native.Generation));
                Assert.That(surface.SceneHistoryRevision, Is.EqualTo(native.SceneFrameRevision));
                // These direct campaign owners prove real text/world glyph composition;
                // the normal GameRuntime check separately supplies its authoritative clock.
                var rendered = presenter.Render(1, surface.ComposedSceneIndices,
                    surface.SceneHistoryGeneration, surface.SceneHistoryRevision, native.DayIndexedFramePalette);
                Assert.That(rendered, Is.Not.Null);
                var pixels = ReadObjectivePixels(rendered);
                var menu = (CampaignMenuPresenter)typeof(MagicCarpetRuntime)
                    .GetField("gameRuntimeMenu", flags).GetValue(runtime);
                var artwork = menu.ObjectiveForProfile(mapType == 0 ? "day" : mapType == 1 ? "night" : "cave");
                var raster = ObjectiveTextboxCompositor.BuildSourceForeground(
                    Encoding.GetEncoding(28591).GetBytes(records[firstRecord]), artwork.Glyphs,
                    artwork.GlyphMasks, artwork.Corner, artwork.Side, artwork.Horizontal, artwork.TextColor);
                Assert.That(rendered.width, Is.EqualTo(raster.Width));
                Assert.That(rendered.height, Is.EqualTo(raster.Height));
                var textPixels = 0;
                for (var y = 0; y < raster.Height; y++)
                for (var x = 0; x < raster.Width; x++)
                {
                    var pixel = y * raster.Width + x;
                    if (raster.IndicesThenMask[raster.Width * raster.Height + pixel] == 0 ||
                        raster.IndicesThenMask[pixel] != artwork.TextColor) continue;
                    var color = artwork.TextColor * 3;
                    Assert.That(pixels[(raster.Height - 1 - y) * raster.Width + x], Is.EqualTo(new Color32(
                        native.DayIndexedFramePalette[color], native.DayIndexedFramePalette[color + 1],
                        native.DayIndexedFramePalette[color + 2], 255)),
                        "the selected source profile's original glyph pixels reach the rendered region");
                    textPixels++;
                }
                Assert.That(textPixels, Is.GreaterThan(0));
                var tick = native.TerrainState().tick;
                var stage = native.ObjectiveStageState(0, 0);
                CollectionAssert.AreEqual(pixels, ReadObjectivePixels(presenter.Render(1,
                    surface.ComposedSceneIndices, native.Generation, surface.SceneHistoryRevision,
                    native.DayIndexedFramePalette)), "repeated presentation samples the same composed scene");
                Assert.That(native.TerrainState().tick, Is.EqualTo(tick));
                var repeatedStage = native.ObjectiveStageState(0, 0);
                Assert.That((repeatedStage.generation, repeatedStage.textSelector, repeatedStage.stageState,
                    repeatedStage.done, repeatedStage.isLevelEnd), Is.EqualTo((stage.generation,
                    stage.textSelector, stage.stageState, stage.done, stage.isLevelEnd)));
                Assert.That(presenter.Render(0, surface.ComposedSceneIndices, native.Generation,
                    surface.SceneHistoryRevision, native.DayIndexedFramePalette), Is.Null);
                Assert.That(presenter.Render(1, surface.ComposedSceneIndices, native.Generation + 1,
                    surface.SceneHistoryRevision, native.DayIndexedFramePalette), Is.Null,
                    "an old text owner cannot shade a replacement generation");
            }
            finally
            {
                runtime.QualificationDisposeGameRuntimeProduction();
                display.Release();
                UnityEngine.Object.DestroyImmediate(display);
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        private static Color32[] ReadObjectivePixels(RenderTexture texture)
        {
            var previous = RenderTexture.active;
            var pixels = new Texture2D(texture.width, texture.height, TextureFormat.RGBA32, false, true);
            try
            {
                RenderTexture.active = texture;
                pixels.ReadPixels(new Rect(0, 0, texture.width, texture.height), 0, 0);
                pixels.Apply();
                return pixels.GetPixels32();
            }
            finally
            {
                RenderTexture.active = previous;
                UnityEngine.Object.DestroyImmediate(pixels);
            }
        }

        [Test]
        public void FirstFourObjectivesUseAuthoredLanguageStageAndEndRecords()
        {
            var generated = Path.Combine(PrivateRoot, "generated");
            var cache = OwnerAssetCache.OpenAndValidate(
                Path.Combine(generated, "mc2-level1-night-cache-v2"),
                "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8",
                "878123908f6de05f684f4cd1d73cff8d2ff08701cb6fdd12b7165bfcacc75719",
                Path.Combine(generated, "mc-bhq8-level1-collision-v5-night"));
            var text = new Mc2ObjectiveText(cache);
            var records = Encoding.GetEncoding(28591).GetString(
                cache.ReadOutput("font/language-l1.records-nul.bin")).Split('\0');
            var starts = new[] { 48, 54, 60, 66 };
            var ends = new[] { 53, 59, 65, 68 };
            for (ushort level = 0; level < 4; level++)
            {
                Assert.That(text.Resolve(level, new NativeSim.CampaignObjectiveStageSnapshot { textSelector = 1 }),
                    Is.EqualTo(records[starts[level] + 1]));
                Assert.That(text.Resolve(level, new NativeSim.CampaignObjectiveStageSnapshot { isLevelEnd = 1 }),
                    Is.EqualTo(records[ends[level]]));
            }
            Assert.That(text.Resolve(4, new NativeSim.CampaignObjectiveStageSnapshot { textSelector = 0 }),
                Is.EqualTo(records[0x45]));
            Assert.That(text.Resolve(4, new NativeSim.CampaignObjectiveStageSnapshot { textSelector = 1 }),
                Is.EqualTo(records[0x46]));
            Assert.That(text.Resolve(4, new NativeSim.CampaignObjectiveStageSnapshot { isLevelEnd = 1 }),
                Is.EqualTo(records[0x47]));
            Assert.That(Mc2ObjectiveText.SourceRecordIndex(24, 0, false), Is.EqualTo(0x9c));
            Assert.That(Mc2ObjectiveText.SourceRecordIndex(24, 0, true), Is.EqualTo(0x9e));
            var liveStage = new NativeSim.CampaignObjectiveStageSnapshot { textSelector = 1 };
            foreach (var announcement in new[] { 284, 285 })
            {
                Assert.That(records[announcement], Is.Not.Empty);
                Assert.That(text.Resolve(11, liveStage, announcement), Is.EqualTo(records[announcement]),
                    "the native-selected secret announcement replaces the ordinary text while active");
                var observed = JsonUtility.FromJson<EngineSessionTransport.CampaignRenderValue>(
                    "{\"objective_secret_announcement_record\":" + announcement + "}");
                Assert.That(observed.objective_secret_announcement_record, Is.EqualTo(announcement));
            }
            Assert.That(text.Resolve(11, liveStage), Is.EqualTo(records[0x67]),
                "normal objective text returns when the announcement clears");
            var noAnnouncement = JsonUtility.FromJson<EngineSessionTransport.CampaignRenderValue>(
                "{\"objective_secret_announcement_record\":null}");
            Assert.That(noAnnouncement.objective_secret_announcement_record, Is.Zero);
            Assert.Throws<ArgumentOutOfRangeException>(() => text.Resolve(11, liveStage, 283));
            Assert.Throws<ArgumentOutOfRangeException>(() => text.Resolve(29, liveStage, 284));
            for (ushort level = 30; level <= 34; level++)
            {
                Assert.That(text.Resolve(level, new NativeSim.CampaignObjectiveStageSnapshot { textSelector = 7 }),
                    Is.EqualTo(records[0x33]), "source secret objective ignores its local selector");
                Assert.That(text.Resolve(level, new NativeSim.CampaignObjectiveStageSnapshot { isLevelEnd = 1 }),
                    Is.EqualTo(records[0x65]), "source secret completion uses the common end record");
            }
            foreach (ushort invalid in new ushort[] { 25, 29, 35, ushort.MaxValue })
                Assert.Throws<ArgumentOutOfRangeException>(() => Mc2ObjectiveText.SourceRecordIndex(invalid, 0, false));
        }

        [Test]
        public void OwnerCampaignMapUsesSourceMissionNamesAndRejectsAlteredPixels()
        {
            var source = Path.Combine(PrivateRoot, "generated/mc-shared-source-map-motion-20261026");
            using (var cache = new OwnerCampaignMenuCache(source))
            {
                Assert.That(cache.World.width, Is.EqualTo(1280));
                Assert.That(cache.World.height, Is.EqualTo(960));
                Assert.That(cache.ForMission("level.magic_carpet_2.night_spire").name, Is.EqualTo("1. Jahwl"));
                Assert.That(cache.ForMission("level.magic_carpet_2.payahandra_ruins").name, Is.EqualTo("2. Kopahk"));
                Assert.That(cache.ForMission("level.magic_carpet_2.revealed_way").name, Is.EqualTo("3. Myrnan Gor"));
                var cave = cache.ForMission("level.magic_carpet_2.spider_cave");
                Assert.That(cave.name, Is.EqualTo("4. Arachnium"));
                Assert.That(cave.briefing, Is.EqualTo(
                    "\"A gift from the gods! Alas, it falls towards the netherworld of the spiders...\""));
                Assert.That(cave.portal_x, Is.EqualTo(549));
                Assert.That(cave.portal_y, Is.EqualTo(626));
                Assert.That(cache.ForMission("level.magic_carpet_2.payahandra_ruins").briefing, Is.Not.Empty);
            }
            var fixture = Path.Combine(Path.GetTempPath(), "mc-campaign-menu-negative-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(fixture);
            CopyOwnerMenuFixture(source, fixture);
            var pixels = Path.Combine(fixture, "world-map.indices.u8");
            using (var file = File.Open(pixels, FileMode.Open, FileAccess.ReadWrite))
            {
                var value = file.ReadByte();
                file.Position = 0;
                file.WriteByte((byte)(value ^ 1));
            }
            try { Assert.Throws<InvalidDataException>(() => new OwnerCampaignMenuCache(fixture)); }
            finally { Directory.Delete(fixture, true); }
        }

        private static void CopyOwnerMenuFixture(string source, string destination)
        {
            foreach (var file in Directory.GetFiles(source, "*", SearchOption.AllDirectories))
            {
                var target = Path.Combine(destination, Path.GetRelativePath(source, file));
                Directory.CreateDirectory(Path.GetDirectoryName(target));
                File.Copy(file, target);
            }
        }

        [Test]
        [Timeout(180000)]
        public void RealInstalledLocaleRejectsBadPackAndRebindsColdNativeOwner()
        {
            var temporary = Path.Combine(Path.GetTempPath(), "mc-real-locale-" + Guid.NewGuid().ToString("N"));
            var cachePath = Path.Combine(temporary, "menu");
            var profile = Path.Combine(temporary, "profile");
            Directory.CreateDirectory(temporary);
            try
            {
                CopyOwnerMenuFixture(CampaignMenuCache, cachePath);
                using var cache = new OwnerCampaignMenuCache(cachePath);
                using var native = NativeSession.OpenGameRuntimeFrontend(ResolverPackages());
                var preferences = new PlayerPreferencesPresenter(json => GameRuntimeTransport.PreferencesCommand(
                    native.Handle, native.GameRuntimeIdentity(), json), profile);
                preferences.BindLanguages(cache);
                var identity = preferences.Current.locale_binding;
                Assert.That(identity, Is.Not.Null);
                var active = cache.PrepareLocale(identity);
                var objective = new Mc2ObjectiveText(cache);
                var stage = new NativeSim.CampaignObjectiveStageSnapshot { textSelector = 0 };
                Assert.That(objective.Resolve(4, stage), Is.EqualTo(active.Record(0x45)));
                Assert.That(cache.BriefingText(cache.ForMission(SecretOriginLevel)), Is.EqualTo(active.Record(27)));
                Assert.That(cache.LanguageText(444), Is.EqualTo(active.Record(444)));
                var forged = JsonUtility.FromJson<PlayerLocaleBinding>(JsonUtility.ToJson(identity));
                forged.text_sha256 = new string('0', 64);
                Assert.Throws<InvalidDataException>(() => cache.PrepareLocale(forged));
                Assert.Throws<ArgumentOutOfRangeException>(() => cache.PrepareLocale(cache.LanguageCachePath, "not-installed"));
                Assert.That(cache.PrepareLocale(preferences.Current.locale_binding), Is.SameAs(active));
                Assert.That(preferences.Current.preferences.selected_locale, Is.Null);
                Assert.That(preferences.ApplySource(next => next.show_help = !next.show_help), Is.True);
                Assert.That(preferences.Current.preferences.selected_locale, Is.Null,
                    "cloning an unrelated source preference preserves the nullable default locale");
                Assert.That(preferences.ApplySource(next => next.selected_locale = identity.id), Is.True);
                var saved = File.ReadAllBytes(Path.Combine(profile, "player-preferences.json"));
                var masks = Path.Combine(cache.LanguageCachePath, "fonts/font0.masks.u8");
                var originalMasks = File.ReadAllBytes(masks);
                var damagedMasks = (byte[])originalMasks.Clone();
                damagedMasks[0] ^= 1;
                File.WriteAllBytes(masks, damagedMasks);
                try
                {
                    preferences.Open(SourceSettingsDialog.Page.Language);
                    Assert.That(preferences.Apply(), Is.False,
                        "real native checked Apply rejects changed owner glyph bytes before publication");
                    Assert.That(preferences.IsOpen, Is.True, "the rejected source dialog remains recoverable");
                    CollectionAssert.AreEqual(saved, File.ReadAllBytes(Path.Combine(profile, "player-preferences.json")));
                    Assert.That(native.GameRuntimeIdentity().player_preferences.locale_binding.text_sha256,
                        Is.EqualTo(identity.text_sha256));
                    Assert.That(cache.PrepareLocale(preferences.Current.locale_binding), Is.SameAs(active));
                    Assert.That(objective.Resolve(4, stage), Is.EqualTo(active.Record(0x45)));
                    preferences.Cancel();
                    Assert.That(preferences.IsOpen, Is.False);
                }
                finally { File.WriteAllBytes(masks, originalMasks); }
                using var coldCache = new OwnerCampaignMenuCache(cachePath);
                using var coldNative = NativeSession.OpenGameRuntimeFrontend(ResolverPackages());
                Assert.That(coldNative.Handle, Is.Not.EqualTo(native.Handle));
                var cold = new PlayerPreferencesPresenter(json => GameRuntimeTransport.PreferencesCommand(
                    coldNative.Handle, coldNative.GameRuntimeIdentity(), json), profile);
                cold.BindLanguages(coldCache);
                Assert.That(cold.Current.preferences.selected_locale, Is.EqualTo(identity.id));
                Assert.That(JsonUtility.ToJson(cold.Current.locale_binding), Is.EqualTo(JsonUtility.ToJson(identity)));
                var coldBinding = coldCache.PrepareLocale(cold.Current.locale_binding);
                Assert.That(new Mc2ObjectiveText(coldCache).Resolve(4, stage), Is.EqualTo(coldBinding.Record(0x45)));
                Assert.That(coldCache.BriefingText(coldCache.ForMission(SecretOriginLevel)), Is.EqualTo(coldBinding.Record(27)));
                Assert.That(coldCache.LanguageText(444), Is.EqualTo(coldBinding.Record(444)));
                Assert.That(coldCache.LanguageFlag(identity.id).width, Is.EqualTo(88));
                // This is a fresh native/profile owner. The installed-client
                // qualification separately exits and relaunches the player.
            }
            finally { if (Directory.Exists(temporary)) Directory.Delete(temporary, true); }
        }

        [Test]
        public void SourceClosedLevelTwoCacheBindsActualNativeAndClientConsumers()
        {
            var generated = Path.Combine(PrivateRoot, "generated");
            var package = Path.Combine(generated, "mc-level2-vault-successor-closure-20261025");
            var aggregate = Path.Combine(generated, "level2-vault-successor-owner-cache-20261025");
            var cache = OwnerAssetCache.OpenAndValidateLevel2(aggregate);
            var environmentPath = Path.Combine(generated, "mc-level2-vault-successor-environment-20261025");
            var environment = OwnerAssetCache.OpenAndValidateEnvironment(environmentPath, package);
            using var session = NativeCampaignSession.Open(package, NativeSim.CampaignSelection.Level2Day(),
                environmentPath);
            using var surface = new CampaignPresentationSurface();
            surface.Present(session, environment);
            Assert.That(surface.WorldReady && surface.LiveMinimapReady, Is.True);
            Assert.That(surface.Tick, Is.Zero, "package/cache admission diagnostic performs no gameplay");
            Assert.That(surface.FrameHash, Is.Not.Zero);
            // The reset view has no in-range world sprite. One public source
            // producer supplies a real flyer for this isolated raster check.
            session.Step(384, 62080, 8000, flags: 0x40, requestedSpell: 0);
            var sourceRows = session.Entities();
            Assert.That(Array.Exists(session.EntityPresentations(0), row =>
                sourceRows[row.entityIndex].classId == 9 && sourceRows[row.entityIndex].model == 0), Is.True,
                "the real Fireball producer admits its source flyer before composition");
            surface.Present(session, environment);
            var player = session.Entities()[session.Players()[0].entityIndex];
            surface.PresentView(new EngineSessionTransport.Pose {
                x = player.x, y = player.y, z = player.z, yaw = player.yaw, display_pitch = player.pitch
            });
            var display = new RenderTexture(320, 200, 0, RenderTextureFormat.ARGB32, RenderTextureReadWrite.Linear);
            var pixels = new Texture2D(320, 200, TextureFormat.RGBA32, false, true);
            var previous = RenderTexture.active;
            try
            {
                var cameras = surface.FrameCamera.transform.parent.GetComponentsInChildren<Camera>();
                Array.Sort(cameras, (left, right) => left.depth.CompareTo(right.depth));
                foreach (var camera in cameras)
                {
                    if (camera.targetTexture == null) camera.targetTexture = display;
                    camera.Render();
                    if (camera.depth == -99f)
                    {
                        RenderTexture.active = camera.targetTexture;
                        pixels.ReadPixels(new Rect(0, 0, 320, 200), 0, 0); pixels.Apply();
                        var scene = pixels.GetPixels32();
                        var changed = 0;
                        for (var y = 0; y < 200; y++)
                            for (var x = 0; x < 320; x++)
                                if (scene[y * 320 + x].r != session.SceneFrameIndices[(199-y)*320+x]) changed++;
                        Assert.That(changed, Is.GreaterThan(0),
                            "the source flyer is drawn before final-camera history capture");
                    }
                    if (camera.depth == -100f)
                    {
                        RenderTexture.active = camera.targetTexture;
                        pixels.ReadPixels(new Rect(0, 0, 320, 200), 0, 0); pixels.Apply();
                        var rawPixels = pixels.GetPixels32();
                        var topDown = new byte[rawPixels.Length];
                        for (var y = 0; y < 200; y++)
                            for (var x = 0; x < 320; x++)
                                topDown[(199 - y) * 320 + x] = rawPixels[y * 320 + x].r;
                        CollectionAssert.AreEqual(session.SceneFrameIndices, topDown,
                            "the index target retains every raw terrain byte before sprites");
                    }
                }
                Assert.That(surface.SceneHistoryRevision, Is.EqualTo(session.SceneFrameRevision));
                Assert.That(surface.ComposedSceneIndices, Is.Not.Null);
                RenderTexture.active = surface.ComposedSceneIndices;
                pixels.ReadPixels(new Rect(0, 0, 320, 200), 0, 0); pixels.Apply();
                var composed = pixels.GetPixels32();
                var spritePixels = 0;
                for (var y = 0; y < 200; y++)
                    for (var x = 0; x < 320; x++)
                        if (composed[y * 320 + x].r != session.SceneFrameIndices[(199 - y) * 320 + x]) spritePixels++;
                Assert.That(spritePixels, Is.GreaterThan(0), "actual scene history includes visible source sprite pixels");
                foreach (var camera in cameras) camera.Render();
                RenderTexture.active = surface.ComposedSceneIndices;
                pixels.ReadPixels(new Rect(0, 0, 320, 200), 0, 0); pixels.Apply();
                CollectionAssert.AreEqual(composed, pixels.GetPixels32(), "same native revision cannot blend history again");
            }
            finally
            {
                RenderTexture.active = previous;
                display.Release();
                UnityEngine.Object.DestroyImmediate(display);
                UnityEngine.Object.DestroyImmediate(pixels);
            }
            var audioRoot = new GameObject("source-closed Level2 audio consumer");
            try
            {
                using var audio = OwnerAudioPresentation.LoadCampaignEnvironment(cache,
                    NativeSim.CampaignSelection.Level2Day(), audioRoot.transform);
                Assert.That(audio, Is.Not.Null);
            }
            finally { UnityEngine.Object.DestroyImmediate(audioRoot); }
        }

        [Test]
        public void IndexedSceneHistoryUsesExactOwnerGtd2CurrentHighPriorLow()
        {
            Assert.That(SystemInfo.graphicsDeviceType, Is.Not.EqualTo(UnityEngine.Rendering.GraphicsDeviceType.Null));
            var generated = Path.Combine(PrivateRoot, "generated");
            var cache = OwnerAssetCache.OpenAndValidateEnvironment(
                Path.Combine(generated, "mc-level3-environment-exit-20261025"),
                Path.Combine(generated, "level3-day-block32-campaign-recipe-v2"));
            var table = cache.ReadOutput(cache.EnvironmentMaterials.day_exit_tables.path);
            Assert.That(table.Length, Is.EqualTo(65536));
            var shader = Shader.Find("MagicCarpet/CampaignIndexedHistory");
            Assert.That(shader, Is.Not.Null);
            Assert.That(shader.isSupported, Is.True);
            var material = new Material(shader);
            var current = new Texture2D(256, 256, TextureFormat.R8, false, true);
            var prior = new Texture2D(256, 256, TextureFormat.R8, false, true);
            var lut = new Texture2D(256, 256, TextureFormat.R8, false, true);
            var output = new RenderTexture(256, 256, 0, RenderTextureFormat.ARGB32, RenderTextureReadWrite.Linear);
            var pixels = new Texture2D(256, 256, TextureFormat.RGBA32, false, true);
            var previous = RenderTexture.active;
            var previousSrgb = GL.sRGBWrite;
            try
            {
                var now = new byte[65536]; var old = new byte[65536];
                for (var i = 0; i < now.Length; i++) { now[i] = (byte)(i >> 8); old[i] = unchecked((byte)i); }
                current.LoadRawTextureData(now); prior.LoadRawTextureData(old); lut.LoadRawTextureData(table);
                foreach (var texture in new[] { current, prior, lut }) { texture.filterMode = FilterMode.Point; texture.Apply(); }
                material.SetTexture("_SceneHistory", prior); material.SetTexture("_BlendLut", lut);
                material.SetFloat("_BlendHistory", 1);
                GL.sRGBWrite = false;
                Graphics.Blit(current, output, material);
                RenderTexture.active = output;
                pixels.ReadPixels(new Rect(0, 0, 256, 256), 0, 0); pixels.Apply();
                var rgba = pixels.GetPixels32(); var actual = new byte[rgba.Length];
                for (var i = 0; i < actual.Length; i++) actual[i] = rgba[i].r;
                CollectionAssert.AreEqual(table, actual, "each actual source LUT entry uses current-high/prior-low indices");
            }
            finally
            {
                GL.sRGBWrite = previousSrgb;
                RenderTexture.active = previous; output.Release();
                foreach (var resource in new UnityEngine.Object[] { material, current, prior, lut, output, pixels })
                    UnityEngine.Object.DestroyImmediate(resource);
            }
        }

        [Test]
        public void NightWebUsesPackageBankAndAuthoritativeImmobilization()
        {
            var generated = Path.Combine(PrivateRoot, "generated");
            var package = Path.Combine(generated, "mc-bhq8-level1-collision-v5-night");
            var environmentPath = Path.Combine(generated, "mc-oi62-environment-level1-v2");
            var environment = OwnerAssetCache.OpenAndValidateEnvironment(environmentPath, package);
            Assert.That(environment.EnvironmentMaterials.profile.web, Is.EqualTo("MWEBN0-0"));
            using var session = NativeCampaignSession.Open(package,
                NativeSim.CampaignSelection.Create(1, 0, 1), environmentPath);
            using var surface = new CampaignPresentationSurface();
            surface.PresentNight(session, environment, 0);
            const System.Reflection.BindingFlags flags = System.Reflection.BindingFlags.Instance |
                System.Reflection.BindingFlags.NonPublic;
            var texture = (Texture2D)typeof(CampaignPresentationSurface).GetField("webTexture", flags).GetValue(surface);
            var visible = typeof(CampaignPresentationSurface).GetField("webVisible", flags);
            Assert.That(texture, Is.Not.Null);
            Assert.That(Array.Exists(texture.GetPixels32(), pixel => pixel.a != 0), Is.True);
            var webPixels = texture.GetPixels32();
            var webUploads = texture.updateCount;
            surface.RefreshPreferences();
            surface.RefreshPreferences();
            Assert.That(texture.updateCount, Is.EqualTo(webUploads),
                "unchanged preference polling must not recolor/upload the full WEB image");
            surface.SetPaletteTransform(palette => Array.ConvertAll(palette, value => (byte)(255 - value)));
            Assert.That(texture.updateCount, Is.GreaterThan(webUploads),
                "a changed presentation palette must invalidate the retained WEB colors");
            var transformedWeb = texture.GetPixels32();
            var visiblePixel = Array.FindIndex(webPixels, pixel => pixel.a != 0);
            Assert.That(transformedWeb[visiblePixel].r, Is.EqualTo(255 - webPixels[visiblePixel].r));
            Assert.That(transformedWeb[visiblePixel].g, Is.EqualTo(255 - webPixels[visiblePixel].g));
            Assert.That(transformedWeb[visiblePixel].b, Is.EqualTo(255 - webPixels[visiblePixel].b));
            surface.PresentImmobilization(2, true);
            Assert.That(visible.GetValue(surface), Is.True);
            surface.PresentImmobilization(0, true);
            Assert.That(visible.GetValue(surface), Is.False);
            surface.PresentImmobilization(2, false);
            Assert.That(visible.GetValue(surface), Is.False);
            Assert.That(session.Tick, Is.Zero, "WEB consumer diagnostic performs no gameplay");
        }

        [Test]
        public void CaveSpriteShaderClipsLaterTerrainPerPixelAndKeepsEqualRank()
        {
            Assert.That(SystemInfo.graphicsDeviceType, Is.Not.EqualTo(UnityEngine.Rendering.GraphicsDeviceType.Null),
                "This focused shader check requires a graphics device, not a Null renderer");
            var shader = Shader.Find("MagicCarpet/EntitySprite");
            Assert.That(shader, Is.Not.Null);
            Assert.That(shader.isSupported, Is.True);
            var material = new Material(shader);
            var indices = new Texture2D(1, 1, TextureFormat.R8, false, true);
            var palette = new Texture2D(256, 1, TextureFormat.RGBA32, false, true);
            var orders = new Texture2D(320, 200, TextureFormat.RGBA32, false, true);
            var output = new RenderTexture(320, 200, 0, RenderTextureFormat.ARGB32, RenderTextureReadWrite.Linear);
            var captured = new Texture2D(320, 200, TextureFormat.RGBA32, false, true);
            var previous = RenderTexture.active;
            var fog = RenderSettings.fog;
            try
            {
                indices.LoadRawTextureData(new byte[] { 1 }); indices.Apply();
                var palettePixels = new Color32[256];
                Array.Fill(palettePixels, new Color32(255, 0, 0, 255));
                palette.SetPixels32(palettePixels); palette.Apply();
                var ranks = new Color32[320 * 200];
                for (var y = 0; y < 200; y++)
                    for (var x = 0; x < 320; x++)
                    {
                        var rank = x < 160 ? 501 : 500;
                        ranks[y * 320 + x] = new Color32(unchecked((byte)rank), (byte)(rank >> 8), 0, 255);
                    }
                orders.SetPixels32(ranks); orders.Apply(); orders.filterMode = FilterMode.Point;
                material.SetTexture("_Indices", indices);
                material.SetTexture("_Palette", palette);
                material.SetTexture("_CaveOrders", orders);
                material.SetFloat("_CaveOcclusionEnabled", 1);
                material.SetFloat("_CaveCellOrder", 500);
                RenderSettings.fog = false;
                output.Create();
                RenderTexture.active = output; GL.Clear(true, true, Color.black);
                Graphics.Blit(Texture2D.whiteTexture, output, material);
                RenderTexture.active = output;
                captured.ReadPixels(new Rect(0, 0, 320, 200), 0, 0); captured.Apply();
                Assert.That(captured.GetPixel(80, 100).r, Is.LessThan(.05f), "later terrain clips only covered fragments");
                Assert.That(captured.GetPixel(240, 100).r, Is.GreaterThan(.95f), "the source draws sprites after their own cell");
                material.SetFloat("_CaveCellOrder", 0);
                RenderTexture.active = output; GL.Clear(true, true, Color.black);
                Graphics.Blit(Texture2D.whiteTexture, output, material);
                RenderTexture.active = output;
                captured.ReadPixels(new Rect(0, 0, 320, 200), 0, 0); captured.Apply();
                Assert.That(captured.GetPixel(240, 100).r, Is.LessThan(.05f), "an unsubmitted source cell cannot draw a sprite");
            }
            finally
            {
                RenderTexture.active = previous; RenderSettings.fog = fog;
                output.Release();
                foreach (var resource in new UnityEngine.Object[] { material, indices, palette, orders, output, captured })
                    UnityEngine.Object.DestroyImmediate(resource);
            }
        }

        [UnityEngine.TestTools.UnityTest]
        public System.Collections.IEnumerator CampaignQuicksaveRetainsItsEdgeAndFailedWriteKeepsTheOwner()
        {
            var activeBefore = NativeSession.ActiveSessionCount;
            var temporary = Path.Combine(Path.GetTempPath(), "mc-map-quicksave-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(temporary);
            var save = Path.Combine(temporary, "campaign.json");
            var root = new GameObject("campaign F5 save owner");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
            var update = typeof(MagicCarpetRuntime).GetMethod("Update", flags);
            var fixedUpdate = typeof(MagicCarpetRuntime).GetMethod("FixedUpdateGameRuntimeCampaign", flags);
            var savePath = typeof(MagicCarpetRuntime).GetField("gameRuntimeReplaySavePath", flags);
            try
            {
                var generated = Path.Combine(PrivateRoot, "generated");
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(new[] {
                    "-mc-game-runtime", "-mc-runtime-menu",
                    "-mc-runtime-loading-cache", Path.Combine(generated, "mc2-level1-night-cache-v2"),
                    "-mc-runtime-menu-cache", CampaignMenuCache,
                    "-mc-runtime-media-cache", Path.Combine(generated, "mc2-campaign-media-cache-decoded"),
                    "-mc-runtime-replay-save", save,
                    "-mc-runtime-package", LevelOnePackage,
                    "-mc-runtime-package", LevelTwoPackage,
                    "-mc-runtime-package", LevelThreePackage });
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.NewGame), Is.True);
                Assert.That(runtime.GameRuntimeProductionStartupReady, Is.False,
                    "the source campaign map has no live gameplay owner");
                var before = JsonUtility.ToJson(runtime.GameRuntimeProductionIdentity);
                runtime.QualificationRequestGameRuntimeProductionReplaySave();
                runtime.QualificationRequestGameRuntimeProductionReplaySave(false);
                update.Invoke(runtime, null);
                update.Invoke(runtime, null);
                Assert.That(File.Exists(save), Is.False, "render frames retain the edge until command dispatch");
                fixedUpdate.Invoke(runtime, null);
                Assert.That(File.Exists(save), Is.True);
                Assert.That(JsonUtility.ToJson(runtime.GameRuntimeProductionIdentity), Is.EqualTo(before),
                    "saving the map adds no gameplay step or campaign mutation");
                var saved = File.ReadAllBytes(save);
                File.SetLastWriteTimeUtc(save, DateTime.UtcNow.AddDays(-1));
                var publicationTime = File.GetLastWriteTimeUtc(save);
                fixedUpdate.Invoke(runtime, null);
                CollectionAssert.AreEqual(saved, File.ReadAllBytes(save), "the consumed edge is not repeated");
                Assert.That(File.GetLastWriteTimeUtc(save), Is.EqualTo(publicationTime),
                    "a later dispatcher pass must not silently republish identical bytes");
                var native = (NativeSession)typeof(MagicCarpetRuntime).GetField("session", flags).GetValue(runtime);
                using (var cold = NativeSession.OpenGameRuntimeReplayFrom(native, saved, ResolverPackages()))
                {
                    var restored = cold.GameRuntimeIdentity();
                    Assert.That(restored.runtime_id, Is.Not.EqualTo(runtime.GameRuntimeProductionIdentity.runtime_id));
                    Assert.That(restored.mode, Is.EqualTo(GameRuntimeTransport.CampaignMode));
                    Assert.That(restored.session, Is.Null);
                    Assert.That(JsonUtility.ToJson(restored.campaign_menu),
                        Is.EqualTo(JsonUtility.ToJson(runtime.GameRuntimeProductionIdentity.campaign_menu)));
                }
                var blocked = Path.Combine(temporary, "blocked");
                File.WriteAllText(blocked, "an owned file blocks a save directory");
                savePath.SetValue(runtime, Path.Combine(blocked, "campaign.json"));
                runtime.QualificationRequestGameRuntimeProductionReplaySave();
                Assert.DoesNotThrow(() => fixedUpdate.Invoke(runtime, null),
                    "a publication error must not reach the fatal fixed-update handler");
                Assert.That(JsonUtility.ToJson(runtime.GameRuntimeProductionIdentity), Is.EqualTo(before));
                CollectionAssert.AreEqual(saved, File.ReadAllBytes(save), "the previous usable save survives");
                Assert.That((string)typeof(MagicCarpetRuntime).GetField("gameRuntimeMenuStatus", flags).GetValue(runtime),
                    Does.Contain("Could not save"));
                Assert.That((bool)typeof(MagicCarpetRuntime).GetField("runtimeExitRequested", flags).GetValue(runtime), Is.False);
                savePath.SetValue(runtime, save);
                runtime.QualificationRequestGameRuntimeProductionReplaySave();
                fixedUpdate.Invoke(runtime, null);
                Assert.That((string)typeof(MagicCarpetRuntime).GetField("gameRuntimeMenuStatus", flags).GetValue(runtime),
                    Is.EqualTo("Game saved."), "a fresh edge remains usable after the failed write");

                var proofRoot = Path.Combine(generated, "continuity-20261115");
                var resourcePath = Environment.GetEnvironmentVariable("MC_CAMPAIGN_SAVE_RESOURCES") ??
                    Path.Combine(proofRoot, "installed-package-resources.json");
                GameRuntimeTransport.InstalledPackageResource[] installed;
                using (var resources = File.OpenRead(resourcePath))
                    installed = (GameRuntimeTransport.InstalledPackageResource[])new DataContractJsonSerializer(
                        typeof(GameRuntimeTransport.InstalledPackageResource[])).ReadObject(resources);
                var completedSaves = new[] {
                    Environment.GetEnvironmentVariable("MC_COMPLETED_RESULTS_SAVE_OUTPUT") ??
                        Path.Combine(proofRoot, "controlled-results.mcs"),
                    Environment.GetEnvironmentVariable("MC_COMPLETED_SECRET_SAVE_OUTPUT") ??
                        Path.Combine(proofRoot, "controlled-secret-map.mcs")
                };
                foreach (var inputSave in completedSaves)
                {
                    runtime.QualificationDisposeGameRuntimeProduction();
                    var outputSave = Path.Combine(temporary, Path.GetFileName(inputSave) + ".quicksave");
                    var arguments = new System.Collections.Generic.List<string> {
                        "-mc-game-runtime", "-mc-runtime-replay-load", inputSave,
                        "-mc-runtime-loading-cache", Path.Combine(generated, "mc2-level1-night-cache-v2"),
                        "-mc-runtime-menu-cache", CampaignMenuCache,
                        "-mc-runtime-media-cache", Path.Combine(generated, "mc2-campaign-media-cache-decoded"),
                        "-mc-runtime-replay-save", outputSave
                    };
                    foreach (var resource in installed)
                    {
                        arguments.Add("-mc-runtime-package"); arguments.Add(resource.package_dir);
                        if (!string.IsNullOrWhiteSpace(resource.owner_cache_path))
                        { arguments.Add("-mc-runtime-owner-cache"); arguments.Add(resource.owner_cache_path); }
                    }
                    runtime.QualificationInitializeGameRuntimeProductionFromArguments(arguments.ToArray());
                    typeof(MagicCarpetRuntime).GetMethod("EnsureGameRuntimeMenu", flags).Invoke(runtime, null);
                    var completedIdentity = runtime.GameRuntimeProductionIdentity;
                    Assert.That(completedIdentity.mode, Is.EqualTo(GameRuntimeTransport.CampaignMode));
                    Assert.That(completedIdentity.session, Is.Null);
                    var isResults = inputSave == completedSaves[0];
                    Assert.That(completedIdentity.campaign_menu.phase, Is.EqualTo(isResults ? "results" : "mission_select"));
                    Assert.That(completedIdentity.campaign_menu.completed_levels, Is.Not.Empty,
                        "these are retained real source-completion packets, not manufactured menu phases");
                    void Reject(Action<GameRuntimeTransport.Identity> mutate, string reason)
                    {
                        var invalid = DataContractJsonRoundTrip(completedIdentity);
                        mutate(invalid);
                        Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateIdentity(
                            invalid, ulong.Parse(completedIdentity.runtime_id)), reason);
                    }
                    if (isResults)
                        Reject(invalid => invalid.campaign_menu.completed_level_id = null,
                            "Results must retain its real completed-level identity");
                    else
                    {
                        Assert.That(completedIdentity.campaign_menu.completed_level_id, Is.Null,
                            "the real completed-secret return retains a progressed map without an ordinary Results owner");
                        var future = Array.Find(completedIdentity.active_runtime_flow.entries,
                            entry => Array.IndexOf(completedIdentity.campaign_menu.available_levels, entry.level_id) < 0).level_id;
                        var discovered = completedIdentity.secret_campaign.discovered_levels[0];
                        var undiscovered = Array.Find(completedIdentity.active_runtime_flow.secret_entries,
                            entry => Array.IndexOf(completedIdentity.secret_campaign.discovered_levels, entry.secret_level_id) < 0).secret_level_id;
                        Assert.That(completedIdentity.campaign_menu.available_levels, Does.Contain(discovered));
                        Reject(invalid => invalid.campaign_menu.phase = "results",
                            "a progressed null-completed return cannot become Results");
                        Reject(invalid => {
                            var levels = new System.Collections.Generic.List<string>(invalid.campaign_menu.available_levels) { future };
                            invalid.campaign_menu.available_levels = levels.ToArray();
                        }, "a real return cannot open a premature ordinary portal");
                        Reject(invalid => invalid.campaign_menu.available_levels = Array.FindAll(
                            invalid.campaign_menu.available_levels, level => level != discovered),
                            "the exact discovered-secret portal cannot be omitted");
                        Reject(invalid => {
                            var levels = new System.Collections.Generic.List<string>(invalid.campaign_menu.available_levels) { undiscovered };
                            invalid.campaign_menu.available_levels = levels.ToArray();
                        }, "an undiscovered secret portal cannot be appended");
                        Reject(invalid => invalid.campaign_menu.briefing_level_id = completedIdentity.active_runtime_flow.entries[0].level_id,
                            "the return briefing must remain bound to its real frontier");
                        Reject(invalid => invalid.campaign_menu.selected_level_id = future,
                            "the return cannot select an unavailable mission");
                    }
                    var gui = ScriptableObject.CreateInstance<CampaignGuiWindow>();
                    gui.DrawRuntime = () => typeof(MagicCarpetRuntime).GetMethod("OnGUI", flags).Invoke(runtime, null);
                    try
                    {
                        gui.minSize = new Vector2(640, 480);
                        gui.position = new Rect(0, 0, 640, 480);
                        gui.Show();
                        gui.position = new Rect(0, 0, 640, 480);
                        gui.SendEvent(new Event { type = EventType.Layout });
                        gui.SendEvent(new Event { type = EventType.Repaint });
                        Assert.That(gui.DrawCount, Is.GreaterThan(0), "the actual restored map/results presenter has drawn");
                        var menu = (CampaignMenuPresenter)typeof(MagicCarpetRuntime).GetField("gameRuntimeMenu", flags).GetValue(runtime);
                        if (!isResults)
                        {
                            var deadline = Time.realtimeSinceStartupAsDouble + 10;
                            while (!menu.CanQuickSave)
                            {
                                Assert.That(Time.realtimeSinceStartupAsDouble, Is.LessThan(deadline),
                                    "the actual secret-return map must finish its normal source entry");
                                UnityEditor.EditorApplication.QueuePlayerLoopUpdate();
                                yield return null;
                                gui.SendEvent(new Event { type = EventType.Repaint });
                            }
                        }
                        Assert.That(menu.CanQuickSave, Is.True,
                            "drawn campaign save admission: phase=" + completedIdentity.campaign_menu.phase +
                            " ready=" + menu.LocaleCache.Campaign.Motion.Ready +
                            " travelling=" + menu.LocaleCache.Campaign.Motion.Travelling +
                            " dialog=" + menu.IsSourceDialogOpen);
                        var nativeBefore = runtime.QualificationSaveGameRuntimeProductionReplay();
                        CollectionAssert.AreEqual(File.ReadAllBytes(inputSave), nativeBefore,
                            "native replay admission preserves the original complete controlled source packet");
                        var identityBefore = JsonUtility.ToJson(runtime.GameRuntimeProductionIdentity);
                        var f5Consumed = gui.SendEvent(new Event { type = EventType.KeyDown, keyCode = KeyCode.F5 });
                        if (isResults) Assert.That(f5Consumed, Is.True, "Results consumes F5 without dismissing itself");
                        runtime.QualificationRequestGameRuntimeProductionReplaySave();
                        runtime.QualificationRequestGameRuntimeProductionReplaySave(false);
                        update.Invoke(runtime, null);
                        gui.SendEvent(new Event { type = EventType.Repaint });
                        Assert.That(File.Exists(outputSave), Is.False, "the real completed phase retains one queued edge");
                        fixedUpdate.Invoke(runtime, null);
                        Assert.That(JsonUtility.ToJson(runtime.GameRuntimeProductionIdentity), Is.EqualTo(identityBefore),
                            "F5 cannot dismiss Results, alter the frontier/secret state, or add a gameplay tick");
                        var completedBytes = File.ReadAllBytes(outputSave);
                        CollectionAssert.AreEqual(nativeBefore, completedBytes,
                            "the exact native packet retains all map, media, source-context and semantic-carry bytes");
                        var live = (NativeSession)typeof(MagicCarpetRuntime).GetField("session", flags).GetValue(runtime);
                        using var cold = NativeSession.OpenGameRuntimeReplayFrom(live, completedBytes, installed);
                        var coldIdentity = cold.GameRuntimeIdentity();
                        Assert.That(coldIdentity.runtime_id, Is.Not.EqualTo(runtime.GameRuntimeProductionIdentity.runtime_id));
                        Assert.That(JsonUtility.ToJson(coldIdentity.campaign_menu),
                            Is.EqualTo(JsonUtility.ToJson(runtime.GameRuntimeProductionIdentity.campaign_menu)));
                        Assert.That(JsonUtility.ToJson(coldIdentity.secret_campaign),
                            Is.EqualTo(JsonUtility.ToJson(runtime.GameRuntimeProductionIdentity.secret_campaign)));
                        CollectionAssert.AreEqual(nativeBefore, cold.GameRuntimeSaveReplay(),
                            "a fresh native owner reserializes the same media and full pending carry without consuming or awarding it");
                    }
                    finally
                    {
                        gui.DrawRuntime = null;
                        gui.Close();
                        if (gui != null) UnityEngine.Object.DestroyImmediate(gui);
                    }
                }

            }
            finally
            {
                runtime.QualificationDisposeGameRuntimeProduction();
                UnityEngine.Object.DestroyImmediate(root);
                Directory.Delete(temporary, true);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
        }

        [UnityEngine.TestTools.UnityTest]
        public System.Collections.IEnumerator ColdReturnedParentUsesRegisteredPresentationAndKeepsCarry()
        {
            var activeBefore = NativeSession.ActiveSessionCount;
            var campaignBefore = NativeCampaignSession.ActiveSessionCount;
            var temporary = Path.Combine(Path.GetTempPath(), "mc-parent-live-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(temporary);
            var root = new GameObject("returned parent cold scene owner");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
            var fixedUpdate = typeof(MagicCarpetRuntime).GetMethod("FixedUpdateGameRuntimeCampaign", flags);
            try
            {
                var generated = Path.Combine(PrivateRoot, "generated");
                var proofRoot = Path.Combine(generated, "continuity-20261115");
                var resourcePath = Environment.GetEnvironmentVariable("MC_CAMPAIGN_SAVE_RESOURCES") ??
                    Path.Combine(proofRoot, "installed-full-resources.json");
                GameRuntimeTransport.InstalledPackageResource[] installed;
                using (var resources = File.OpenRead(resourcePath))
                    installed = (GameRuntimeTransport.InstalledPackageResource[])new DataContractJsonSerializer(
                        typeof(GameRuntimeTransport.InstalledPackageResource[])).ReadObject(resources);
                var badDeclaration = new System.Collections.Generic.List<string> { "-mc-game-runtime" };
                foreach (var resource in installed)
                {
                    badDeclaration.Add("-mc-runtime-package"); badDeclaration.Add(resource.package_dir);
                    badDeclaration.Add("-mc-runtime-owner-cache"); badDeclaration.Add(resource.owner_cache_path);
                    if (resource.package_dir == LevelTwoPackage)
                    { badDeclaration.Add("-mc-runtime-presentation-cache"); badDeclaration.Add(resource.owner_cache_path); }
                }
                Assert.That(badDeclaration, Does.Contain("-mc-runtime-presentation-cache"));
                var rejectedDeclaration = Assert.Throws<InvalidOperationException>(() =>
                    runtime.QualificationInitializeGameRuntimeProductionFromArguments(badDeclaration.ToArray()));
                Assert.That(rejectedDeclaration.Message, Does.Contain("selection-bound owner presentation cache"),
                    "a declared presentation for another source cannot become a missing-cache Night fallback");
                Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));

                var mapInput = Environment.GetEnvironmentVariable("MC_COMPLETED_SECRET_SAVE_OUTPUT") ??
                    Path.Combine(proofRoot, "controlled-secret-map.mcs");
                var mapBytes = File.ReadAllBytes(mapInput);
                var liveOutput = Path.Combine(temporary, "parent-live-quicksave.mcs");
                var liveArguments = ReturnedParentArguments(mapInput, Path.Combine(temporary, "live-profile"), liveOutput);
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(liveArguments);
                var liveSession = (NativeSession)typeof(MagicCarpetRuntime).GetField("session", flags).GetValue(runtime);
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.CampaignMode));
                CollectionAssert.AreEqual(mapBytes, runtime.QualificationSaveGameRuntimeProductionReplay());
                typeof(MagicCarpetRuntime).GetMethod("PreparePlayerPreferences", flags).Invoke(runtime,
                    new object[] { liveArguments });
                yield return StartReturnedParentFromNormalMap(runtime);
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo("0"));
                runtime.QualificationDriveGameRuntimeFrames(1);
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                runtime.QualificationDriveGameRuntimeFrames(1);
                var liveIdentity = runtime.GameRuntimeProductionIdentity;
                Assert.That(liveIdentity.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                Assert.That(liveIdentity.active_runtime_flow.bound_level_id, Is.EqualTo("level.magic_carpet_2.ammyridia"));
                var liveBinding = GameRuntimeTransport.ReadActiveEntryPresentation(liveIdentity);
                Assert.That(liveBinding.source_selection.external_level, Is.EqualTo(20));
                Assert.That(liveBinding.source_selection.internal_level, Is.EqualTo(19));
                Assert.That(liveBinding.environment, Is.EqualTo("day"));
                Assert.That(runtime.GameRuntimeProductionStartupReady, Is.True);
                var surface = (CampaignPresentationSurface)typeof(MagicCarpetRuntime).GetField("gameRuntimeSecretSurface", flags).GetValue(runtime);
                Assert.That(surface, Is.Not.Null, "the actual ordinary source20 restores through the registered environment composer");
                Assert.That(surface.NativeSceneComplete, Is.True);
                Assert.That(surface.FrameHash, Is.Not.Zero);
                Assert.That(surface.Generation.ToString(), Is.EqualTo(liveIdentity.session.generation));
                Assert.That(surface.Tick.ToString(), Is.EqualTo(liveIdentity.session.tick));
                Assert.That(runtime.Level2ProductionDayHudPresenterReady, Is.True);
                Assert.That(runtime.QualificationHasMaterializedLevel2DayHudSprite(40), Is.True);
                Assert.That(runtime.Level2ProductionDayHud.localPlayerMana,
                    Is.EqualTo(runtime.GameRuntimeProductionCampaignSnapshot.value.player_vitals.mana));
                var liveBytes = runtime.QualificationSaveGameRuntimeProductionReplay();
                AssertReturnedParentCarry(mapBytes, liveBytes);
                var currentState = liveSession.GameRuntimeCampaignSnapshot();
                Assert.That(ulong.Parse(ReadReplayDocument(liveBytes).SelectSingleNode("/root/expected_state_hash").InnerText).ToString("x16"),
                    Is.EqualTo(currentState.value.state_hash));
                runtime.QualificationRequestGameRuntimeProductionReplaySave();
                fixedUpdate.Invoke(runtime, null);
                CollectionAssert.AreEqual(liveBytes, File.ReadAllBytes(liveOutput),
                    "Paused F5 saves before the next source paused outer frame");
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo(liveIdentity.session.tick));
                var oldLiveInput = Environment.GetEnvironmentVariable("MC_LIVE_PARENT_REPLAY_SAVE") ??
                    Path.Combine(proofRoot, "installed-parent-hud-5/parent-entry-save.mcs");
                var oldLiveBytes = File.ReadAllBytes(oldLiveInput);
                var beforeRejectedOwner = runtime.GameRuntimeProductionIdentity;
                var beforeRejectedPreferences = JsonUtility.ToJson(beforeRejectedOwner.player_preferences);
                var beforeRejectedSave = runtime.QualificationSaveGameRuntimeProductionReplay();
                File.WriteAllBytes(liveOutput, oldLiveBytes);
                try
                {
                    Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Load), Is.False,
                        "the historical later-level live receipt from the incorrect clock must fail without a history shim");
                    var afterRejectedOwner = runtime.GameRuntimeProductionIdentity;
                    Assert.That((afterRejectedOwner.runtime_id, afterRejectedOwner.generation, afterRejectedOwner.session.tick),
                        Is.EqualTo((beforeRejectedOwner.runtime_id, beforeRejectedOwner.generation, beforeRejectedOwner.session.tick)));
                    Assert.That(JsonUtility.ToJson(afterRejectedOwner.player_preferences), Is.EqualTo(beforeRejectedPreferences));
                    CollectionAssert.AreEqual(beforeRejectedSave, runtime.QualificationSaveGameRuntimeProductionReplay());
                    CollectionAssert.AreEqual(oldLiveBytes, File.ReadAllBytes(liveOutput));
                    CollectionAssert.AreEqual(oldLiveBytes, File.ReadAllBytes(oldLiveInput));
                    Assert.That(runtime.GameRuntimeProductionStartupReady, Is.True);
                }
                finally { File.WriteAllBytes(liveOutput, liveBytes); }
                var previousLiveOwner = runtime.GameRuntimeProductionIdentity.runtime_id;
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Load), Is.True,
                    "the normal source Load replaces the paused owner and composes its actual environment again");
                var restoredLive = runtime.GameRuntimeProductionIdentity;
                Assert.That(restoredLive.runtime_id, Is.Not.EqualTo(previousLiveOwner));
                Assert.That(restoredLive.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                Assert.That(restoredLive.session.tick, Is.EqualTo(liveIdentity.session.tick));
                Assert.That(restoredLive.session.definition_hash, Is.EqualTo(liveIdentity.session.definition_hash));
                CollectionAssert.AreEqual(liveBytes, runtime.QualificationSaveGameRuntimeProductionReplay(),
                    "a second live owner cannot award or reconsume the restored carry");
                Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore + 1));
                Assert.That(NativeCampaignSession.ActiveSessionCount, Is.EqualTo(campaignBefore),
                    "the scene borrows the Rust owner instead of opening a parallel campaign handle");
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Resume), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                var resumed = runtime.GameRuntimeProductionIdentity;
                Assert.That(resumed.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(ulong.Parse(resumed.session.tick), Is.EqualTo(ulong.Parse(liveIdentity.session.tick) + 1));
                var resumedSurface = (CampaignPresentationSurface)typeof(MagicCarpetRuntime).GetField("gameRuntimeSecretSurface", flags).GetValue(runtime);
                Assert.That(resumedSurface.NativeSceneComplete, Is.True);
                Assert.That(resumedSurface.Generation.ToString(), Is.EqualTo(resumed.session.generation));
                Assert.That(resumedSurface.Tick.ToString(), Is.EqualTo(resumed.session.tick));
                Assert.That(runtime.Level2ProductionDayHud.localPlayerMana,
                    Is.EqualTo(runtime.GameRuntimeProductionCampaignSnapshot.value.player_vitals.mana));
            }
            finally
            {
                runtime.QualificationDisposeGameRuntimeProduction();
                UnityEngine.Object.DestroyImmediate(root);
                Directory.Delete(temporary, true);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.EqualTo(campaignBefore));
        }

        private static string[] ReturnedParentArguments(string input, string profile, string output)
        {
            var generated = Path.Combine(PrivateRoot, "generated");
            var arguments = new System.Collections.Generic.List<string> {
                "-mc-game-runtime", "-mc-runtime-replay-load", input,
                "-mc-runtime-loading-cache", Path.Combine(generated, "mc2-level1-night-cache-v2"),
                "-mc-runtime-menu-cache", CampaignMenuCache,
                "-mc-runtime-speech-cache", CampaignSpeechCache,
                "-mc-runtime-media-cache", Path.Combine(generated, "mc2-campaign-media-cache-decoded"),
                "-mc-preferences-profile", profile, "-mc-runtime-replay-save", output
            };
            using var resources = File.OpenRead(Environment.GetEnvironmentVariable("MC_CAMPAIGN_SAVE_RESOURCES") ??
                Path.Combine(generated, "continuity-20261115/installed-full-resources.json"));
            foreach (var resource in (GameRuntimeTransport.InstalledPackageResource[])new DataContractJsonSerializer(
                typeof(GameRuntimeTransport.InstalledPackageResource[])).ReadObject(resources))
            {
                arguments.Add("-mc-runtime-package"); arguments.Add(resource.package_dir);
                arguments.Add("-mc-runtime-owner-cache"); arguments.Add(resource.owner_cache_path);
                arguments.Add("-mc-runtime-presentation-cache"); arguments.Add(resource.presentation_cache_path);
            }
            return arguments.ToArray();
        }

        [UnityEngine.TestTools.UnityTest]
        public System.Collections.IEnumerator FreshPavColdAnnouncementUsesSourceSpeechAndSelectedGlyphs()
        {
            var input = Environment.GetEnvironmentVariable("MC_SECRET_ANNOUNCEMENT_LIVE_SAVE_OUTPUT");
            if (string.IsNullOrEmpty(input)) Assert.Ignore("Requires the real Jaleen-to-Pav transfer replay");
            var activeBefore = NativeSession.ActiveSessionCount;
            var temporary = Path.Combine(Path.GetTempPath(), "mc-pav-announcement-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(temporary);
            var root = new GameObject("actual Pav cold secret announcement");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var gui = ScriptableObject.CreateInstance<CampaignGuiWindow>();
            var guiShown = false;
            var display = new RenderTexture(320, 200, 0, RenderTextureFormat.ARGB32, RenderTextureReadWrite.Linear);
            const System.Reflection.BindingFlags flags = System.Reflection.BindingFlags.Instance |
                System.Reflection.BindingFlags.NonPublic;
            object Field(string name) => typeof(MagicCarpetRuntime).GetField(name, flags).GetValue(runtime);
            try
            {
                var original = File.ReadAllBytes(input);
                var arguments = ReturnedParentArguments(input, Path.Combine(temporary, "profile"),
                    Path.Combine(temporary, "save.mcs"));
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(arguments);
                var owner = (NativeSession)Field("session");
                var identity = runtime.GameRuntimeProductionIdentity;
                Assert.That(identity.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(identity.active_runtime_flow.bound_level_id, Is.EqualTo("level.magic_carpet_2.pav_durivium"));
                CollectionAssert.AreEqual(original, runtime.QualificationSaveGameRuntimeProductionReplay(),
                    "cold open retains the real pre-frame transfer and does not award it again");
                typeof(MagicCarpetRuntime).GetMethod("PreparePlayerPreferences", flags).Invoke(runtime,
                    new object[] { arguments });
                var preferences = (PlayerPreferencesPresenter)Field("playerPreferences");
                Assert.That(preferences.Current.preferences.speech_enabled, Is.True);
                var menu = (CampaignMenuPresenter)Field("gameRuntimeMenu");
                var selected = menu.LocaleCache.PrepareLocale(preferences.Current.locale_binding);
                Assert.That(selected.Record(284), Is.EqualTo("\"You have discovered one of the demon lord's hidden realms.\""));
                Assert.That(owner.GameRuntimeCampaignSnapshot().value.objective_secret_announcement_record, Is.EqualTo(284));
                var speech = (CampaignSpeechPresenter)Field("campaignSpeech");
                Assert.That(speech.PlayCount, Is.Zero);
                // A fresh level warms its source palette for two frames before
                // phase 1 begins; phase 8 is reached on the tenth frame.
                runtime.QualificationDriveGameRuntimeFrames(10);
                Assert.That(speech.PlayCount, Is.EqualTo(1), "the real source phase8 emits one announcement");
                Assert.That((speech.LastTrack, speech.LastStartMs, speech.LastLengthMs), Is.EqualTo((26, 0u, 4999u)));
                Assert.That(speech.SoundMix, Is.LessThan(1f));
                Assert.That(speech.MusicMix, Is.LessThan(1f));

                var shown = owner.GameRuntimeCampaignRenderStep(0, 0, 0, 0, 0, showObjective: true);
                Assert.That(shown.value.objective_secret_announcement_record, Is.EqualTo(284));
                Assert.That(shown.value.objective_textbox_ticks, Is.EqualTo(1));
                typeof(MagicCarpetRuntime).GetField("gameRuntimeCampaignSnapshot", flags).SetValue(runtime, shown);
                runtime.QualificationRefreshGameRuntimeProductionEvents();
                typeof(MagicCarpetRuntime).GetMethod("UpdateGameRuntimeSecretPresentation", flags).Invoke(runtime,
                    new object[] { runtime.GameRuntimeProductionIdentity, shown });
                Assert.That(runtime.GameRuntimeObjectiveInstruction, Is.EqualTo(selected.Record(284)));
                var surface = (CampaignPresentationSurface)Field("gameRuntimeSecretSurface");
                var cameras = surface.FrameCamera.transform.parent.GetComponentsInChildren<Camera>();
                Array.Sort(cameras, (left, right) => left.depth.CompareTo(right.depth));
                foreach (var camera in cameras)
                {
                    if (camera.targetTexture == null) camera.targetTexture = display;
                    camera.Render();
                }
                gui.DrawRuntime = () => typeof(MagicCarpetRuntime).GetMethod("OnGUI", flags).Invoke(runtime, null);
                gui.minSize = new Vector2(640, 480); gui.position = new Rect(0, 0, 640, 480);
                gui.Show(); guiShown = true; gui.position = new Rect(0, 0, 640, 480);
                gui.SendEvent(new Event { type = EventType.Layout });
                gui.SendEvent(new Event { type = EventType.Repaint });
                Assert.That(gui.DrawCount, Is.GreaterThan(0));
                var presenter = (SourceObjectiveTextboxPresenter)Field("gameRuntimeObjectiveTextbox");
                var output = (RenderTexture)typeof(SourceObjectiveTextboxPresenter).GetField("output", flags).GetValue(presenter);
                Assert.That(output, Is.Not.Null, "the actual GUI composites the source announcement over the current scene");
                var artwork = menu.ObjectiveForProfile("cave");
                var raster = ObjectiveTextboxCompositor.BuildSourceForeground(
                    Encoding.GetEncoding(28591).GetBytes(selected.Record(284)), artwork.Glyphs, artwork.GlyphMasks,
                    artwork.Corner, artwork.Side, artwork.Horizontal, artwork.TextColor);
                var pixels = ReadObjectivePixels(output);
                var palette = preferences.ApplyBrightnessToPalette(surface.FramePaletteRgb8, 2);
                var textPixels = 0;
                Assert.That((output.width, output.height), Is.EqualTo((raster.Width, raster.Height)));
                for (var y = 0; y < raster.Height; y++)
                for (var x = 0; x < raster.Width; x++)
                {
                    var offset = y * raster.Width + x;
                    if (raster.IndicesThenMask[raster.Width * raster.Height + offset] == 0 ||
                        raster.IndicesThenMask[offset] != artwork.TextColor) continue;
                    var color = artwork.TextColor * 3;
                    Assert.That(pixels[(raster.Height - 1 - y) * raster.Width + x], Is.EqualTo(new Color32(
                        palette[color], palette[color + 1], palette[color + 2], 255)));
                    textPixels++;
                }
                Assert.That(textPixels, Is.GreaterThan(0), "original selected glyph pixels reach the composed frame");
                var tick = runtime.GameRuntimeProductionIdentity.session.tick;
                var replay = runtime.QualificationSaveGameRuntimeProductionReplay();
                gui.SendEvent(new Event { type = EventType.Repaint });
                CollectionAssert.AreEqual(pixels, ReadObjectivePixels(output));
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo(tick));
                Assert.That(speech.PlayCount, Is.EqualTo(1));
                CollectionAssert.AreEqual(replay, runtime.QualificationSaveGameRuntimeProductionReplay());
                var released = owner.GameRuntimeCampaignRenderStep(0, 0, 0, 0, 0, showObjective: false);
                Assert.That(released.value.objective_textbox_ticks, Is.Zero, "releasing O removes the source textbox");
                typeof(MagicCarpetRuntime).GetField("gameRuntimeCampaignSnapshot", flags).SetValue(runtime, released);
                runtime.QualificationRefreshGameRuntimeProductionEvents();
                typeof(MagicCarpetRuntime).GetMethod("UpdateGameRuntimeSecretPresentation", flags).Invoke(runtime,
                    new object[] { runtime.GameRuntimeProductionIdentity, released });
                Assert.That(speech.PlayCount, Is.EqualTo(1));
                Assert.That(presenter.Render(0, surface.ComposedSceneIndices, surface.SceneHistoryGeneration,
                    surface.SceneHistoryRevision, palette), Is.Null);

                var last = Time.realtimeSinceStartupAsDouble;
                var deadline = last + speech.LastLengthMs / 1000.0 + 5;
                do
                {
                    UnityEditor.EditorApplication.QueuePlayerLoopUpdate();
                    yield return null;
                    var now = Time.realtimeSinceStartupAsDouble;
                    Assert.That(now, Is.LessThan(deadline), "real speech and fade callbacks complete");
                    speech.Tick((float)(now - last));
                    last = now;
                } while (speech.IsPlaying || speech.SoundMix < 1f || speech.MusicMix < 1f);
                Assert.That(speech.SoundMix, Is.EqualTo(1f));
                Assert.That(speech.MusicMix, Is.EqualTo(1f));
            }
            finally
            {
                gui.DrawRuntime = null;
                if (guiShown) gui.Close();
                if (gui != null) UnityEngine.Object.DestroyImmediate(gui);
                runtime.QualificationDisposeGameRuntimeProduction();
                display.Release();
                UnityEngine.Object.DestroyImmediate(display);
                UnityEngine.Object.DestroyImmediate(root);
                Directory.Delete(temporary, true);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
        }

        private static System.Collections.IEnumerator StartReturnedParentFromNormalMap(MagicCarpetRuntime runtime)
        {
            var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
            var identity = runtime.GameRuntimeProductionIdentity;
            Assert.That(identity.mode, Is.EqualTo(GameRuntimeTransport.CampaignMode));
            Assert.That(identity.campaign_menu.phase, Is.EqualTo("mission_select"));
            Assert.That(identity.campaign_menu.selected_level_id, Is.EqualTo("level.magic_carpet_2.ammyridia"));
            var menu = (CampaignMenuPresenter)typeof(MagicCarpetRuntime).GetField("gameRuntimeMenu", flags).GetValue(runtime);
            var gui = ScriptableObject.CreateInstance<CampaignGuiWindow>();
            var shown = false;
            try
            {
                gui.DrawRuntime = () => typeof(MagicCarpetRuntime).GetMethod("OnGUI", flags).Invoke(runtime, null);
                gui.minSize = new Vector2(640, 480); gui.position = new Rect(0, 0, 640, 480);
                gui.Show(); shown = true; gui.position = new Rect(0, 0, 640, 480);
                var deadline = Time.realtimeSinceStartupAsDouble + 10;
                do
                {
                    gui.SendEvent(new Event { type = EventType.Layout });
                    gui.SendEvent(new Event { type = EventType.Repaint });
                    if (menu.SourceBriefingReady) break;
                    Assert.That(Time.realtimeSinceStartupAsDouble, Is.LessThan(deadline),
                        "the returned map completes its real source viewport motion");
                    UnityEditor.EditorApplication.QueuePlayerLoopUpdate();
                    yield return null;
                } while (true);
                Assert.That(gui.DrawCount, Is.GreaterThan(0));
                Assert.That(menu.SourceBriefingReady, Is.True, "the actual returned map reaches its normal Ready draw");
                var updateBriefing = typeof(MagicCarpetRuntime).GetMethod("UpdateGameRuntimeBriefing", flags);
                updateBriefing.Invoke(runtime, null); updateBriefing.Invoke(runtime, null);
                var speech = (CampaignSpeechPresenter)typeof(MagicCarpetRuntime).GetField("campaignSpeech", flags).GetValue(runtime);
                var segment = identity.campaign_menu.briefing_speech;
                Assert.That(segment, Is.Not.Null);
                Assert.That(identity.campaign_menu.briefing_level_id, Is.EqualTo("level.magic_carpet_2.cresidan"));
                Assert.That(segment.track, Is.EqualTo(21), "the source map briefs its first unfinished ordinary frontier");
                Assert.That(segment.segment, Is.Zero);
                Assert.That(speech.PlayCount, Is.EqualTo(1), "repeated map observation plays the real later briefing once");
                Assert.That(speech.LastTrack, Is.EqualTo(segment.track));
                Assert.That(speech.LastStartMs, Is.EqualTo(segment.start_ms));
                Assert.That(speech.LastLengthMs, Is.EqualTo(segment.length_ms));
                Assert.That(speech.LastLengthMs, Is.GreaterThan(0));
            }
            finally
            {
                gui.DrawRuntime = null;
                if (shown) gui.Close(); else UnityEngine.Object.DestroyImmediate(gui);
            }
            Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.StartMission), Is.True);
            Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.LoadingMode));
            runtime.QualificationDriveGameRuntimeFrames(1);
            Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.LoadingMode));
            runtime.QualificationObserveGameRuntimeLoadingPresentationFrame();
            runtime.QualificationDriveGameRuntimeFrames(1);
            Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
            Assert.That(runtime.GameRuntimeProductionIdentity.active_runtime_flow.bound_level_id,
                Is.EqualTo("level.magic_carpet_2.ammyridia"));
            Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo("0"));
        }

        private static void AssertReturnedParentCarry(byte[] mapBytes, byte[] liveBytes)
        {
            var map = ReadReplayDocument(mapBytes);
            var live = ReadReplayDocument(liveBytes);
            Assert.That(live.SelectSingleNode("/root/save_contract").InnerText, Is.EqualTo(GameRuntimeTransport.ReplaySaveContract));
            Assert.That(live.SelectSingleNode("/root/semantic_spell_carry").OuterXml,
                Is.EqualTo(map.SelectSingleNode("/root/semantic_spell_carry").OuterXml),
                "the replay retains the complete original entry carry before replaying live commands");
            Assert.That(live.SelectNodes("/root/semantic_spell_carry/acquired/item").Count, Is.EqualTo(7));
            Assert.That(live.SelectSingleNode("/root/semantic_spell_carry/selected").InnerText, Is.EqualTo("possession"));
            Assert.That(live.SelectSingleNode("/root/completed_levels").OuterXml,
                Is.EqualTo(map.SelectSingleNode("/root/state/completed_levels").OuterXml));
            Assert.That(live.SelectSingleNode("/root/secret_flow").OuterXml, Is.EqualTo(map.SelectSingleNode("/root/secret_flow").OuterXml));
            Assert.That(live.SelectSingleNode("/root/retained_campaign").InnerXml, Is.EqualTo(map.SelectSingleNode("/root/state").InnerXml));
            Assert.That(live.SelectSingleNode("/root/initial_mission_rewards").InnerText, Is.EqualTo("suppressed"));
        }

        [UnityEngine.TestTools.UnityTest]
        public System.Collections.IEnumerator UnaffordableSpellUsesSelectedSourceHintAndOnlyOuterFramesConsumeIt()
        {
            var activeBefore = NativeSession.ActiveSessionCount;
            var generated = Path.Combine(PrivateRoot, "generated");
            var proof = Path.Combine(generated, "continuity-20261115");
            var temporary = Path.Combine(Path.GetTempPath(), "mc-help-owner-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(temporary);
            var root = new GameObject("actual unaffordable spell source hint");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var gui = ScriptableObject.CreateInstance<CampaignGuiWindow>();
            var guiShown = false;
            var display = new RenderTexture(320, 200, 0, RenderTextureFormat.ARGB32, RenderTextureReadWrite.Linear);
            var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
            object Field(string name) => typeof(MagicCarpetRuntime).GetField(name, flags).GetValue(runtime);
            MagicCarpetRuntime.ManaHelpHint Hint() => (MagicCarpetRuntime.ManaHelpHint)Field("gameRuntimeManaHint");
            int Remaining() => (int)typeof(MagicCarpetRuntime.ManaHelpHint).GetField("remaining", flags).GetValue(Hint());
            void AssertLiveVolcano()
            {
                var observed = ((NativeSession)Field("session")).GameRuntimeCampaignSnapshot();
                var volcano = CampaignSpellPresenter.FindSpell(observed.value, "spell.volcano");
                Assert.That(volcano, Is.Not.Null);
                Assert.That(volcano.selected, Is.True, "the authoritative live UI selection is Volcano, independently of its replay entry seed");
                Assert.That(volcano.tier, Is.Zero);
                Assert.That(runtime.Level2ProductionDayHud.left.model, Is.EqualTo(18));
                Assert.That(runtime.Level2ProductionDayHud.left.tier, Is.Zero);
            }
            EngineSessionTransport.GameEvent[] EventsSince(ulong cursor)
            {
                var owner = (NativeSession)Field("session");
                var observation = owner.GameRuntimeCampaignSnapshot();
                var head = ulong.Parse(observation.value.effect_head);
                Assert.That(cursor, Is.LessThanOrEqualTo(head));
                var events = new System.Collections.Generic.List<EngineSessionTransport.GameEvent>();
                // Read this command interval, not the HUD's last256 display rows.
                while (cursor < head)
                {
                    var page = owner.GameRuntimeCampaignEvents(cursor.ToString(), 128);
                    Assert.That((page.identity.session_id, page.identity.generation, page.identity.tick,
                        page.identity.definition_hash, page.identity.package_graph_hash),
                        Is.EqualTo((observation.identity.session_id, observation.identity.generation, observation.identity.tick,
                            observation.identity.definition_hash, observation.identity.package_graph_hash)));
                    Assert.That(page.value.requested_cursor, Is.EqualTo(cursor.ToString()));
                    Assert.That(page.value.event_head, Is.EqualTo(head.ToString()));
                    Assert.That(page.value.events, Is.Not.Empty);
                    foreach (var effect in page.value.events)
                    {
                        cursor++;
                        Assert.That(effect.sequence, Is.EqualTo(cursor.ToString()));
                        events.Add(effect);
                    }
                    Assert.That(page.value.next_cursor, Is.EqualTo(cursor.ToString()));
                    Assert.That(page.value.has_more, Is.EqualTo(cursor < head));
                }
                Assert.That(cursor, Is.EqualTo(head));
                return events.ToArray();
            }
            void RenderCurrentScene()
            {
                var currentSurface = (CampaignPresentationSurface)Field("gameRuntimeSecretSurface");
                var currentCameras = currentSurface.FrameCamera.transform.parent.GetComponentsInChildren<Camera>();
                Array.Sort(currentCameras, (left, right) => left.depth.CompareTo(right.depth));
                foreach (var camera in currentCameras)
                {
                    if (camera.targetTexture == null) camera.targetTexture = display;
                    camera.Render();
                }
            }
            void CompleteObjectiveContextPopup()
            {
                // The real expiry frame already consumed the first of the
                // eight delayed source passes. Selection still precedes draw.
                for (var pass = 0; pass < 8; pass++)
                {
                    runtime.QualificationDriveGameRuntimeFrames(1);
                    Assert.That(Hint().Visible, Is.EqualTo(pass == 7));
                }
                Assert.That(Hint().VisibleRow, Is.EqualTo(76),
                    "the retained Ammyridia stage0 is source kind7, which requests context row76");
                var menu = (CampaignMenuPresenter)Field("gameRuntimeMenu");
                var binding = menu.GameplayHelpForRow(76);
                Assert.That(binding.TextRecord, Is.EqualTo(338));
                var text = menu.GameplayLanguageText(338);
                Assert.That(text, Is.EqualTo("Your current objective is to kill creatures"),
                    "the actual available GOG context record is retained byte-for-byte");
                RenderCurrentScene();
                gui.SendEvent(new Event { type = EventType.Layout });
                gui.SendEvent(new Event { type = EventType.Repaint });
                var presenter = (SourceObjectiveTextboxPresenter)Field("gameRuntimeManaHintPresenter");
                Assert.That(presenter, Is.Not.Null);
                Assert.That(typeof(SourceObjectiveTextboxPresenter).GetField("currentText", flags).GetValue(presenter),
                    Is.EqualTo(text), "the accepted source row reaches selected-locale text and the actual glyph renderer");
                var raster = (ObjectiveTextboxCompositor.Raster)typeof(SourceObjectiveTextboxPresenter)
                    .GetField("raster", flags).GetValue(presenter);
                Assert.That((raster.X, raster.Y, raster.Width, raster.Height), Is.EqualTo((176, 80, 288, 52)),
                    "pinned source width loop224, two lines28px, and actual Day171/172/173 extents");
                Assert.That((raster.BlendX, raster.BlendY, raster.BlendWidth, raster.BlendHeight),
                    Is.EqualTo((192, 90, 256, 32)));
                var output = (RenderTexture)typeof(SourceObjectiveTextboxPresenter).GetField("output", flags).GetValue(presenter);
                Assert.That(output, Is.Not.Null);
                var pixels = ReadObjectivePixels(output);
                var surface = (CampaignPresentationSurface)Field("gameRuntimeSecretSurface");
                var palette = ((PlayerPreferencesPresenter)Field("playerPreferences"))
                    .ApplyBrightnessToPalette(surface.FramePaletteRgb8, 0);
                var artwork = menu.ObjectiveForProfile("day");
                Assert.That(artwork.HelpTextColor, Is.EqualTo(224));
                var ink = new Color32(palette[224 * 3], palette[224 * 3 + 1], palette[224 * 3 + 2], 255);
                Assert.That(pixels[(raster.Height - 1 - (94 - raster.Y)) * raster.Width + 208 - raster.X],
                    Is.EqualTo(ink), "actual HFONT3 Y mask(0,2), at source text origin208/92, reaches the current-frame output");
                var owner = (NativeSession)Field("session");
                var before = owner.GameRuntimeCampaignSnapshot();
                var replay = runtime.QualificationSaveGameRuntimeProductionReplay();
                var remaining = Remaining();
                var speechCount = ((CampaignSpeechPresenter)Field("campaignSpeech")).PlayCount;
                gui.SendEvent(new Event { type = EventType.Repaint });
                gui.SendEvent(new Event { type = EventType.Repaint });
                CollectionAssert.AreEqual(pixels, ReadObjectivePixels(output));
                CollectionAssert.AreEqual(replay, runtime.QualificationSaveGameRuntimeProductionReplay());
                Assert.That(owner.GameRuntimeCampaignSnapshot().value.state_hash, Is.EqualTo(before.value.state_hash));
                Assert.That(Remaining(), Is.EqualTo(remaining));
                Assert.That(((CampaignSpeechPresenter)Field("campaignSpeech")).PlayCount, Is.EqualTo(speechCount));
                Assert.That(remaining, Is.EqualTo(49), "the first visible frame consumed one of fifty source draws");
                for (var draw = 0; draw < remaining; draw++)
                {
                    runtime.QualificationDriveGameRuntimeFrames(1);
                    Assert.That(Hint().Visible, Is.True);
                    Assert.That(Hint().VisibleRow, Is.EqualTo(76));
                }
                Assert.That(Remaining(), Is.Zero);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(Hint().Visible, Is.False, "the following source frame clears the expired overlay");
            }
            void ObserveObjectiveInput(bool held)
            {
                var owner = (NativeSession)Field("session");
                var beforeTick = ulong.Parse(runtime.GameRuntimeProductionIdentity.session.tick);
                var observed = owner.GameRuntimeCampaignRenderStep(0, 0, 0, 0, 0, showObjective: held);
                typeof(MagicCarpetRuntime).GetField("gameRuntimeCampaignSnapshot", flags).SetValue(runtime, observed);
                runtime.QualificationRefreshGameRuntimeProductionEvents();
                typeof(MagicCarpetRuntime).GetMethod("UpdateGameRuntimeSecretPresentation", flags).Invoke(runtime,
                    new object[] { runtime.GameRuntimeProductionIdentity, observed });
                typeof(MagicCarpetRuntime).GetMethod("AdvanceGameRuntimeHelpFrame", flags).Invoke(runtime, null);
                Assert.That(ulong.Parse(runtime.GameRuntimeProductionIdentity.session.tick), Is.EqualTo(beforeTick + 1));
                Assert.That(observed.value.objective_textbox_ticks, Is.EqualTo(held ? 1 : 0),
                    "genuine held O installs2 and the same source outer frame consumes1; release expires it");
            }
            try
            {
                var mapInput = Environment.GetEnvironmentVariable("MC_COMPLETED_SECRET_SAVE_OUTPUT") ??
                    Path.Combine(proof, "controlled-secret-map.mcs");
                var mapBytes = File.ReadAllBytes(mapInput);
                var save = Path.Combine(temporary, "current-parent.mcs");
                var arguments = ReturnedParentArguments(mapInput, Path.Combine(temporary, "profile"), save);
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(arguments);
                typeof(MagicCarpetRuntime).GetMethod("PreparePlayerPreferences", flags).Invoke(runtime,
                    new object[] { arguments });
                yield return StartReturnedParentFromNormalMap(runtime);
                var preferences = (PlayerPreferencesPresenter)Field("playerPreferences");
                var menu = (CampaignMenuPresenter)Field("gameRuntimeMenu");
                var accepted = menu.LocaleCache.PrepareLocale(preferences.Current.locale_binding);
                Assert.That(Field("gameRuntimeLocaleBinding"), Is.SameAs(accepted));
                Assert.That(menu.LocaleCache.LanguageText(329), Is.EqualTo("Not enough mana to cast"),
                    "the available GOG source record is the independent geometry oracle");
                Assert.That(preferences.Current.preferences.show_help, Is.True);
                gui.DrawRuntime = () => typeof(MagicCarpetRuntime).GetMethod("OnGUI", flags).Invoke(runtime, null);
                gui.minSize = new Vector2(640, 480); gui.position = new Rect(0, 0, 640, 480);
                gui.Show(); guiShown = true; gui.position = new Rect(0, 0, 640, 480);
                Assert.That(runtime.SubmitGameRuntimeSpellSelection("spell.volcano"), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                runtime.QualificationAssignGameRuntimeProductionSpell("spell.volcano", 0, false);
                runtime.QualificationDriveGameRuntimeFrames(1);
                var slot = runtime.Level2ProductionDayHud.left;
                Assert.That(slot.model, Is.EqualTo(18));
                Assert.That(slot.maxMana, Is.EqualTo(12000));
                Assert.That(slot.casterMana, Is.LessThan(slot.maxMana));
                Assert.That(slot.helpUnavailable, Is.EqualTo(1), "native sub_68D50 owns eligibility");
                AssertLiveVolcano();
                Assert.That(Hint().Visible, Is.False);
                var cursor = ulong.Parse(runtime.GameRuntimeProductionSemanticEventCursor);
                Assert.That(runtime.RequestGameRuntimeSelectedSpellCast(), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(Array.Exists(EventsSince(cursor), effect =>
                    ulong.Parse(effect.sequence) > cursor && effect.event_id == "ui.help_hint_requested"), Is.True,
                    "the ordinary failed cast produces the real retained native help event");
                Assert.That(Hint().Priority, Is.EqualTo(6), "the real native41/6 effect reaches the consumer");
                Assert.That(runtime.Level2ProductionDayHud.localPlayerMana, Is.GreaterThanOrEqualTo(slot.casterMana),
                    "an unaffordable cast does not pay the spell cost");
                ObserveObjectiveInput(true);
                Assert.That(Hint().Visible, Is.False, "native promotion cannot bypass the real held-O objective");
                RenderCurrentScene();
                var objectiveTick = runtime.GameRuntimeProductionIdentity.session.tick;
                var objectiveSpeech = ((CampaignSpeechPresenter)Field("campaignSpeech")).PlayCount;
                gui.SendEvent(new Event { type = EventType.Layout });
                gui.SendEvent(new Event { type = EventType.Repaint });
                var objectivePresenter = (SourceObjectiveTextboxPresenter)Field("gameRuntimeObjectiveTextbox");
                var objectiveOutput = (RenderTexture)typeof(SourceObjectiveTextboxPresenter).GetField("output", flags).GetValue(objectivePresenter);
                Assert.That(objectiveOutput, Is.Not.Null, "source20's actual objective glyphs shade the current scene");
                var objectivePixels = ReadObjectivePixels(objectiveOutput);
                gui.SendEvent(new Event { type = EventType.Repaint });
                CollectionAssert.AreEqual(objectivePixels, ReadObjectivePixels(objectiveOutput));
                Assert.That(runtime.GameRuntimeProductionCampaignSnapshot.value.objective_textbox_ticks, Is.EqualTo(1));
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo(objectiveTick));
                Assert.That(((CampaignSpeechPresenter)Field("campaignSpeech")).PlayCount, Is.EqualTo(objectiveSpeech));
                ObserveObjectiveInput(false);
                CompleteObjectiveContextPopup();
                // At most the source post-suppression poll and its following draw.
                for (var pass = 0; pass < 21 && !Hint().Visible; pass++) runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(Hint().Visible, Is.True);
                Assert.That(Hint().AnchorX, Is.EqualTo(540));
                var surface = (CampaignPresentationSurface)Field("gameRuntimeSecretSurface");
                RenderCurrentScene();
                Assert.That(surface.SceneHistoryGeneration, Is.EqualTo(surface.Generation));
                var tick = runtime.GameRuntimeProductionIdentity.session.tick;
                var replay = runtime.QualificationSaveGameRuntimeProductionReplay();
                var remaining = Remaining();
                var speech = (CampaignSpeechPresenter)Field("campaignSpeech");
                Assert.That(speech.PlayCount, Is.EqualTo(2), "map briefing and genuine source20 phase8 narration each play once");
                Assert.That(speech.LastTrack, Is.EqualTo(20));
                Assert.That(speech.LastStartMs, Is.EqualTo(11999));
                Assert.That(speech.LastLengthMs, Is.EqualTo(6999));
                var playCount = speech?.PlayCount ?? 0;
                gui.DrawRuntime = () => typeof(MagicCarpetRuntime).GetMethod("OnGUI", flags).Invoke(runtime, null);
                gui.minSize = new Vector2(640, 480); gui.position = new Rect(0, 0, 640, 480);
                for (var redraw = 0; redraw < 3; redraw++)
                {
                    runtime.QualificationRefreshGameRuntimeProductionEvents();
                    gui.SendEvent(new Event { type = EventType.Layout });
                    gui.SendEvent(new Event { type = EventType.Repaint });
                }
                Assert.That(gui.DrawCount, Is.GreaterThan(0));
                Assert.That(Remaining(), Is.EqualTo(remaining), "event reads and real GUI redraws cannot consume a source frame");
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo(tick));
                CollectionAssert.AreEqual(replay, runtime.QualificationSaveGameRuntimeProductionReplay());
                Assert.That(speech?.PlayCount ?? 0, Is.EqualTo(playCount));
                var presenter = (SourceObjectiveTextboxPresenter)Field("gameRuntimeManaHintPresenter");
                Assert.That(presenter, Is.Not.Null, "the actual runtime GUI binds the selected source glyph consumer");
                var artwork = menu.ObjectiveForProfile("day");
                var palette = preferences.ApplyBrightnessToPalette(surface.FramePaletteRgb8, 0);
                foreach (var anchor in new[] { 540, 600 })
                {
                    var raster = ObjectiveTextboxCompositor.BuildSourceHintForeground(
                        Encoding.GetEncoding(28591).GetBytes(accepted.Record(329)), artwork.Glyphs,
                        artwork.GlyphMasks, artwork.Corner, artwork.Side, artwork.Horizontal,
                        artwork.HelpTextColor, artwork.BlendSelector, artwork.TextColor, anchor, 48);
                    var shift = anchor - 540;
                    Assert.That((raster.X, raster.Y, raster.Width, raster.Height), Is.EqualTo((293 + shift, 48, 249, 85)));
                    Assert.That((raster.BlendX, raster.BlendY, raster.BlendWidth, raster.BlendHeight),
                        Is.EqualTo((309 + shift, 105, 192, 18)), "pinned source329 geometry, independent of builder output");
                    presenter.PresentHint(accepted.Record(329), surface.Generation, anchor);
                    var output = presenter.Render(1, surface.ComposedSceneIndices, surface.Generation,
                        surface.SceneHistoryRevision, palette);
                    var pixels = ReadObjectivePixels(output);
                    Color32 Color(byte index) => new Color32(palette[index * 3], palette[index * 3 + 1], palette[index * 3 + 2], 255);
                    Color32 At(int x, int y) => pixels[(raster.Height - 1 - (y - raster.Y)) * raster.Width + x - raster.X];
                    Assert.That(At(anchor, 48), Is.EqualTo(Color(artwork.BlendSelector)));
                    Assert.That(At(anchor + 1, 48), Is.EqualTo(Color(artwork.TextColor)));
                    Assert.That(At(raster.X, 48).a, Is.Zero, "outside the source frame/line the overlay preserves the world");
                    var scene = surface.ComposedSceneIndices;
                    Assert.That((scene.width, scene.height), Is.EqualTo((320, 200)));
                    Assert.That(scene.filterMode, Is.EqualTo(FilterMode.Point));
                    // The source textbox uses 640x480 logical coordinates;
                    // its shader samples the actual indexed scene by UV.
                    int SceneX(int x) => Mathf.FloorToInt((x + .5f) / 640f * scene.width);
                    int SceneY(int y) => Mathf.FloorToInt((1f - (y + .5f) / 480f) * scene.height);
                    var left = SceneX(raster.BlendX);
                    var right = SceneX(raster.BlendX + raster.BlendWidth - 1);
                    var bottom = SceneY(raster.BlendY + raster.BlendHeight - 1);
                    var top = SceneY(raster.BlendY);
                    Assert.That(left, Is.InRange(0, scene.width - 1));
                    Assert.That(right, Is.InRange(left, scene.width - 1));
                    Assert.That(bottom, Is.InRange(0, scene.height - 1));
                    Assert.That(top, Is.InRange(bottom, scene.height - 1));
                    var previousTarget = RenderTexture.active;
                    var region = new Texture2D(right - left + 1, top - bottom + 1, TextureFormat.RGBA32, false, true);
                    Texture2D zeroScene = null;
                    try
                    {
                        RenderTexture.active = scene;
                        region.ReadPixels(new Rect(left, bottom, region.width, region.height), 0, 0);
                        region.Apply();
                        var scenePixels = region.GetPixels32();
                        var blendRow = artwork.TableOffset + (artwork.BlendSelector << 8);
                        var zeroColor = Color(artwork.Tables[blendRow]);
                        Vector2Int? distinguishingPoint = null;
                        var samples = 0;
                        for (var y = raster.BlendY; y < raster.BlendY + raster.BlendHeight; y++)
                        for (var x = raster.BlendX; x < raster.BlendX + raster.BlendWidth; x++)
                        {
                            if (raster.IndicesThenMask[raster.Width * raster.Height +
                                (y - raster.Y) * raster.Width + x - raster.X] != 0) continue;
                            var index = scenePixels[(SceneY(y) - bottom) * region.width + SceneX(x) - left].r;
                            var expected = Color(artwork.Tables[blendRow + index]);
                            Assert.That(At(x, y), Is.EqualTo(expected),
                                $"current scene/LUT logical({x},{y}) texture({SceneX(x)},{SceneY(y)}) index{index}");
                            if (!distinguishingPoint.HasValue && !expected.Equals(zeroColor) &&
                                (expected.r != 0 || expected.g != 0 || expected.b != 0))
                                distinguishingPoint = new Vector2Int(x, y);
                            samples++;
                        }
                        Assert.That(samples, Is.GreaterThan(0));
                        if (distinguishingPoint is { } point)
                        {
                            zeroScene = new Texture2D(scene.width, scene.height, TextureFormat.R8, false, true)
                                { filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp };
                            zeroScene.LoadRawTextureData(new byte[scene.width * scene.height]);
                            zeroScene.Apply();
                            var substituted = ReadObjectivePixels(presenter.Render(1, zeroScene, surface.Generation,
                                surface.SceneHistoryRevision, palette));
                            var pixel = (raster.Height - 1 - (point.y - raster.Y)) * raster.Width + point.x - raster.X;
                            Assert.That(substituted[pixel], Is.EqualTo(zeroColor));
                            Assert.That(substituted[pixel], Is.Not.EqualTo(pixels[pixel]),
                                "an index-zero substitute cannot pass the actual current-scene blend oracle");
                            TestContext.WriteLine($"MC_HELP_BLEND_CURRENT anchor={anchor} logical={point} " +
                                $"texture=({SceneX(point.x)},{SceneY(point.y)}) samples={samples}");
                        }
                        else TestContext.WriteLine($"MC_HELP_BLEND_NO_DISCRIMINATING_POINT anchor={anchor} samples={samples} " +
                            "current region matches index-zero/black after the source LUT; no substitute rejection claim");
                    }
                    finally
                    {
                        RenderTexture.active = previousTarget;
                        if (zeroScene != null) UnityEngine.Object.DestroyImmediate(zeroScene);
                        UnityEngine.Object.DestroyImmediate(region);
                        presenter.Render(1, scene, surface.Generation, surface.SceneHistoryRevision, palette);
                    }
                    var glyph = artwork.Glyphs['N' + 1];
                    var glyphPixels = 0;
                    for (var y = 0; y < glyph.Height; y++)
                    for (var x = 0; x < glyph.Width; x++)
                        if (artwork.GlyphMasks[glyph.Offset + y * glyph.Width + x] != 0)
                        {
                            Assert.That(At(313 + shift + x, 107 + y), Is.EqualTo(Color(artwork.HelpTextColor)),
                                "retained source HFONT3 mask and CLR4095 reach the current scene overlay");
                            glyphPixels++;
                        }
                    Assert.That(glyphPixels, Is.GreaterThan(0));
                    Assert.That(presenter.Render(1, surface.ComposedSceneIndices, surface.Generation + 1,
                        surface.SceneHistoryRevision, palette), Is.Null, "stale text cannot shade another owner");
                }
                CollectionAssert.AreEqual(replay, runtime.QualificationSaveGameRuntimeProductionReplay(),
                    "current-scene sampling and the wrong-scene negative remain purely observational");
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo(tick));
                Assert.That(speech.PlayCount, Is.EqualTo(playCount));
                cursor = ulong.Parse(runtime.GameRuntimeProductionSemanticEventCursor);
                Assert.That(runtime.SubmitGameRuntimeSpellSelection("spell.volcano"), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(Hint().Visible, Is.True, "opcode40 selection preview cannot close the current popup");
                typeof(MagicCarpetRuntime).GetField("gameRuntimeSpellMarkRequested", flags).SetValue(runtime,
                    new EngineSessionTransport.SpellMark { spell_id = "spell.volcano", hand = "left" });
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(Hint().Visible, Is.True, "source marking sound14 is not an accepted hand assignment");
                runtime.QualificationAssignGameRuntimeProductionSpell("spell.volcano", byte.MaxValue, false);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(Hint().Visible, Is.True, "a rejected assignment keeps the source popup");
                Assert.That(Remaining(), Is.EqualTo(remaining - 3), "only the three ordinary source frames elapsed");
                Assert.That(Array.Exists(EventsSince(cursor), effect =>
                    ulong.Parse(effect.sequence) > cursor && effect.event_id == "ui.help_popup_closed"), Is.False);
                remaining = Remaining();
                runtime.QualificationPauseGameRuntimeProduction();
                runtime.QualificationDriveGameRuntimeFrames(2);
                Assert.That(Hint().Visible, Is.False);
                Assert.That(Remaining(), Is.EqualTo(remaining), "pause preserves the carrier without ordinary hint draws");
                Assert.That(preferences.ApplySource(value => value.show_help = false), Is.True);
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Resume), Is.True);
                cursor = ulong.Parse(runtime.GameRuntimeProductionSemanticEventCursor);
                Assert.That(runtime.RequestGameRuntimeSelectedSpellCast(), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(Array.Exists(EventsSince(cursor), effect =>
                    ulong.Parse(effect.sequence) > cursor && effect.event_id == "ui.help_hint_requested"), Is.False);
                Assert.That(Hint().Visible, Is.False);
                Assert.That(Remaining(), Is.EqualTo(remaining), "disabled help cannot advance a retained popup");
                runtime.QualificationPauseGameRuntimeProduction();
                Assert.That(preferences.ApplySource(value => value.show_help = true), Is.True);
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Resume), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(remaining);
                Assert.That(Remaining(), Is.Zero, "exactly the retained number of ordinary frames expires the popup");
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(Hint().Visible, Is.False);
                var oldHint = Hint();
                runtime.QualificationResetGameRuntimeProduction();
                Assert.That(Hint(), Is.Not.SameAs(oldHint));
                Assert.That(Hint().Visible, Is.False);
                Assert.That(Hint().Priority, Is.EqualTo(2));
                Assert.That(Field("gameRuntimeManaHintPresenter"), Is.Null);
                ObserveObjectiveInput(true);
                Assert.That(Hint().Visible, Is.False, "retry's real objective still suppresses help");
                RenderCurrentScene();
                var resetObjectiveTick = runtime.GameRuntimeProductionIdentity.session.tick;
                var resetSpeechPlays = ((CampaignSpeechPresenter)Field("campaignSpeech")).PlayCount;
                gui.SendEvent(new Event { type = EventType.Repaint });
                gui.SendEvent(new Event { type = EventType.Repaint });
                Assert.That(runtime.GameRuntimeProductionCampaignSnapshot.value.objective_textbox_ticks, Is.EqualTo(1));
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo(resetObjectiveTick));
                Assert.That(((CampaignSpeechPresenter)Field("campaignSpeech")).PlayCount, Is.EqualTo(resetSpeechPlays));
                ObserveObjectiveInput(false);
                Assert.That(runtime.SubmitGameRuntimeSpellSelection("spell.volcano"), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                runtime.QualificationAssignGameRuntimeProductionSpell("spell.volcano", 0, false);
                runtime.QualificationDriveGameRuntimeFrames(1);
                for (var pass = 0; pass < 21 && !Hint().Visible; pass++) runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(Hint().Visible, Is.True);
                cursor = ulong.Parse(runtime.GameRuntimeProductionSemanticEventCursor);
                runtime.QualificationAssignGameRuntimeProductionSpell("spell.volcano", 0, false);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(Array.Exists(EventsSince(cursor), effect =>
                    ulong.Parse(effect.sequence) > cursor && effect.event_id == "ui.help_popup_closed"), Is.True,
                    "an accepted same-value hand assignment produces the typed source closure");
                Assert.That(Hint().Visible, Is.False);
                Assert.That(Remaining(), Is.Zero);
                Assert.That(Hint().Priority, Is.EqualTo(2));
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                runtime.QualificationDriveGameRuntimeFrames(1);
                AssertLiveVolcano();
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Save), Is.True);
                var currentBytes = File.ReadAllBytes(save);
                AssertReturnedParentCarry(mapBytes, currentBytes);
                var savedIdentity = runtime.GameRuntimeProductionIdentity;
                var savedState = ((NativeSession)Field("session")).GameRuntimeCampaignSnapshot();
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Load), Is.True,
                    "a current source-clock v8 owner cold-restores through the normal Load consumer");
                Assert.That(runtime.GameRuntimeProductionIdentity.runtime_id, Is.Not.EqualTo(savedIdentity.runtime_id));
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo(savedIdentity.session.tick));
                AssertLiveVolcano();
                Assert.That(runtime.GameRuntimeProductionCampaignSnapshot.value.state_hash, Is.EqualTo(savedState.value.state_hash));
                CollectionAssert.AreEqual(currentBytes, runtime.QualificationSaveGameRuntimeProductionReplay());
                Assert.That(((CampaignPresentationSurface)Field("gameRuntimeSecretSurface")).NativeSceneComplete, Is.True);
                Assert.That(runtime.Level2ProductionDayHudPresenterReady, Is.True);

                // The same real paused owner exercises the modifier dispatch
                // without adding a gameplay step or inventing a display state.
                var f5 = typeof(MagicCarpetRuntime).GetMethod("ApplyGameRuntimeF5Input", flags);
                void F5(bool physicalShift, bool admittedShift, bool alt = false) =>
                    f5.Invoke(runtime, new object[] { true, physicalShift, admittedShift, alt });
                var pausedOwner = (NativeSession)Field("session");
                var beforeToggle = runtime.GameRuntimeProductionIdentity;
                Assert.That((beforeToggle.display_settings.minimap, beforeToggle.display_settings.top_bar), Is.EqualTo((true, true)));
                var surfaceAfterLoad = (CampaignPresentationSurface)Field("gameRuntimeSecretSurface");
                Assert.That(surfaceAfterLoad.MinimapVisible, Is.True);
                F5(true, false); // Physical modifier is still behind the release barrier.
                F5(true, true, true); // Original Alt branch has precedence.
                Assert.That(runtime.GameRuntimeProductionIdentity.display_settings.minimap, Is.True);
                Assert.That((bool)Field("gameRuntimeReplaySaveRequested"), Is.False,
                    "neither a blocked Shift chord nor Alt+Shift+F5 becomes a bare save");
                F5(true, true);
                var hidden = runtime.GameRuntimeProductionIdentity;
                Assert.That((hidden.display_settings.minimap, hidden.display_settings.top_bar), Is.EqualTo((false, false)));
                Assert.That((hidden.generation, hidden.session.generation, hidden.session.tick),
                    Is.EqualTo((beforeToggle.generation, beforeToggle.session.generation, beforeToggle.session.tick)));
                Assert.That(surfaceAfterLoad.MinimapVisible, Is.False, "the actual indexed minimap consumer is hidden immediately");
                Assert.That((bool)Field("gameRuntimeReplaySaveRequested"), Is.False, "Shift+F5 must not queue a save");
                var notices = pausedOwner.GameRuntimeParticipantNotifications().value;
                Assert.That(notices, Has.Length.EqualTo(1));
                Assert.That((notices[0].kind, notices[0].remaining_frames), Is.EqualTo(("participant.icons_map_hidden", (short)50)));
                var rows = SourcePlayerNotificationsPresenter.Rows(notices, false, false, false, 14, menu.GameplayLanguageText);
                Assert.That(rows[0].Text, Is.EqualTo("Icons and Map On"), "source indexes record365 when the new flag is zero");
                Assert.That(pausedOwner.GameRuntimeParticipantNotifications().value[0].remaining_frames, Is.EqualTo(50),
                    "observing the notice does not consume a source draw");
                var hiddenReplay = runtime.QualificationSaveGameRuntimeProductionReplay();
                using (var coldHidden = NativeSession.OpenGameRuntimeReplayFrom(pausedOwner, hiddenReplay,
                    (GameRuntimeTransport.InstalledPackageResource[])Field("gameRuntimePackages")))
                {
                    var coldIdentity = coldHidden.GameRuntimeIdentity();
                    Assert.That((coldIdentity.display_settings.minimap, coldIdentity.display_settings.top_bar), Is.EqualTo((false, false)));
                    Assert.That(coldHidden.GameRuntimeParticipantNotifications().value[0].kind, Is.EqualTo("participant.icons_map_hidden"));
                    Assert.That(coldHidden.GameRuntimePresentationFrame(false).value[0].remaining_frames, Is.EqualTo(50),
                        "a source-hidden notice frame does not consume the timer");
                    for (var draw = 0; draw < 50; draw++)
                    {
                        var frame = coldHidden.GameRuntimePresentationFrame(true);
                        Assert.That(frame.value, Has.Length.EqualTo(1));
                        Assert.That(frame.value[0].remaining_frames, Is.EqualTo(49 - draw));
                    }
                    Assert.That(coldHidden.GameRuntimePresentationFrame(true).value, Is.Empty,
                        "source timer zero is the last drawn frame");
                    Assert.That(coldHidden.GameRuntimeIdentity().session.tick, Is.EqualTo(coldIdentity.session.tick));
                }
                F5(false, false);
                Assert.That((bool)Field("gameRuntimeReplaySaveRequested"), Is.True, "bare F5 retains its save edge");
                typeof(MagicCarpetRuntime).GetMethod("ServiceGameRuntimeReplaySave", flags).Invoke(runtime, null);
                Assert.That((bool)Field("gameRuntimeReplaySaveRequested"), Is.False);
                CollectionAssert.AreEqual(hiddenReplay, File.ReadAllBytes(save), "bare F5 publishes the real hidden-display replay");
                F5(true, true);
                Assert.That((runtime.GameRuntimeProductionIdentity.display_settings.minimap,
                    runtime.GameRuntimeProductionIdentity.display_settings.top_bar), Is.EqualTo((true, true)));
                Assert.That(surfaceAfterLoad.MinimapVisible, Is.True);
                notices = pausedOwner.GameRuntimeParticipantNotifications().value;
                Assert.That(notices[0].kind, Is.EqualTo("participant.icons_map_visible"));
                rows = SourcePlayerNotificationsPresenter.Rows(notices, false, false, false, 14, menu.GameplayLanguageText);
                Assert.That(rows[0].Text, Is.EqualTo("Icons and Map Off"), "source indexes record366 when the new flag is one");
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Resume), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(ulong.Parse(runtime.GameRuntimeProductionIdentity.session.tick),
                    Is.EqualTo(ulong.Parse(savedIdentity.session.tick) + 1));
                var sourceMenus = (SourceCampaignMenus)Field("gameRuntimeSourceMenus");
                // These are the two ordered consumers of a single declared
                // Ctrl+Shift+F5 input frame: source Shift processing first,
                // then the Ctrl spell-menu proposal and real admission.
                var simultaneousTick = runtime.GameRuntimeProductionIdentity.session.tick;
                F5(true, true);
                Assert.That(runtime.GameRuntimeProductionIdentity.display_settings.minimap, Is.False);
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo(simultaneousTick));
                var spellInput = sourceMenus.Step(false, true, false, false, false, true);
                Assert.That(sourceMenus.QueueAction(spellInput.Action, sourceMenus.MapOpen,
                    sourceMenus.SpellOpen, sourceMenus.PlayerInformationOpen, spellInput), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(sourceMenus.SpellOpen, Is.True, "held Ctrl reaches the actual Rust spell-menu admission");
                Assert.That((bool)Field("gameRuntimeSpellMenuOpen"), Is.True);
                Assert.That(runtime.GameRuntimeProductionIdentity.display_settings.minimap, Is.False,
                    "Ctrl menu admission must not discard or replay the preceding Shift+F5 action");
                var spellTick = runtime.GameRuntimeProductionIdentity.session.tick;
                F5(true, true);
                Assert.That(runtime.GameRuntimeProductionIdentity.display_settings.minimap, Is.True,
                    "source Shift+F5 remains admitted while the Ctrl spell menu is open");
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo(spellTick));
                Assert.That((bool)Field("gameRuntimeReplaySaveRequested"), Is.False);
                var closeSpells = sourceMenus.Step(false, false, false, false, false, true);
                Assert.That(sourceMenus.QueueAction(closeSpells.Action, sourceMenus.MapOpen,
                    sourceMenus.SpellOpen, sourceMenus.PlayerInformationOpen, closeSpells), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(sourceMenus.SpellOpen, Is.False);
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Title), Is.True);
                Assert.That(Field("gameRuntimeManaHint"), Is.Null);
                Assert.That(Field("gameRuntimeManaHintPresenter"), Is.Null);
            }
            finally
            {
                try
                {
                    if (gui != null)
                    {
                        gui.DrawRuntime = null;
                        if (guiShown) gui.Close();
                        else UnityEngine.Object.DestroyImmediate(gui);
                    }
                }
                finally
                {
                    runtime.QualificationDisposeGameRuntimeProduction();
                    display.Release(); UnityEngine.Object.DestroyImmediate(display);
                    UnityEngine.Object.DestroyImmediate(root);
                    Directory.Delete(temporary, true);
                }
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
        }

        [UnityEngine.TestTools.UnityTest]
        public System.Collections.IEnumerator SourcePanelsFollowShiftF6AndPausedColdReplay()
        {
            var activeBefore = NativeSession.ActiveSessionCount;
            var temporary = Path.Combine(Path.GetTempPath(), "mc-panel-setting-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(temporary);
            var root = new GameObject("actual source panel transparency");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var gui = ScriptableObject.CreateInstance<CampaignGuiWindow>();
            var shown = false;
            var display = new RenderTexture(320, 200, 0, RenderTextureFormat.ARGB32, RenderTextureReadWrite.Linear);
            var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
            object Field(string name) => typeof(MagicCarpetRuntime).GetField(name, flags).GetValue(runtime);
            var previousTarget = RenderTexture.active;
            try
            {
                var input = Environment.GetEnvironmentVariable("MC_COMPLETED_SECRET_SAVE_OUTPUT") ??
                    Path.Combine(PrivateRoot, "generated/continuity-20261115/controlled-secret-map.mcs");
                var save = Path.Combine(temporary, "save.mcs");
                var arguments = ReturnedParentArguments(input, Path.Combine(temporary, "profile"), save);
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(arguments);
                typeof(MagicCarpetRuntime).GetMethod("PreparePlayerPreferences", flags).Invoke(runtime, new object[] { arguments });
                yield return StartReturnedParentFromNormalMap(runtime);
                Assert.That(runtime.GameRuntimeProductionIdentity.display_settings.panels_opaque, Is.False,
                    "the actual Day LevelInit default is transparent");
                Assert.That(runtime.SubmitGameRuntimeSpellSelection("spell.volcano"), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                runtime.QualificationAssignGameRuntimeProductionSpell("spell.volcano", 0, false);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.RequestGameRuntimeSelectedSpellCast(), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                var hint = (MagicCarpetRuntime.ManaHelpHint)Field("gameRuntimeManaHint");
                for (var frame = 0; frame < 240 && !hint.Visible; frame++) runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(hint.Visible, Is.True, "a real source popup is active before the display action");
                var remainingField = typeof(MagicCarpetRuntime.ManaHelpHint).GetField("remaining", flags);
                var remaining = (int)remainingField.GetValue(hint);
                runtime.QualificationPauseGameRuntimeProduction();
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That((int)remainingField.GetValue(hint), Is.EqualTo(remaining), "the ordinary paused frame retains the popup");
                var surface = (CampaignPresentationSurface)Field("gameRuntimeSecretSurface");
                var cameras = surface.FrameCamera.transform.parent.GetComponentsInChildren<Camera>();
                Array.Sort(cameras, (left, right) => left.depth.CompareTo(right.depth));
                foreach (var camera in cameras)
                {
                    if (camera.targetTexture == null) camera.targetTexture = display;
                    camera.Render();
                }
                gui.DrawRuntime = () => typeof(MagicCarpetRuntime).GetMethod("OnGUI", flags).Invoke(runtime, null);
                gui.minSize = new Vector2(640, 480); gui.position = new Rect(0, 0, 640, 480);
                gui.Show(); shown = true; gui.position = new Rect(0, 0, 640, 480);
                gui.SendEvent(new Event { type = EventType.Layout });
                gui.SendEvent(new Event { type = EventType.Repaint });
                var underlay = surface.HudUnderlayIndices;
                var transparentHud = (RenderTexture)Field("gameRuntimeHudIndices");
                Assert.That(underlay, Is.Not.Null);
                Assert.That(transparentHud, Is.Not.Null);
                Color32[] Read(RenderTexture source)
                {
                    var texture = new Texture2D(source.width, source.height, TextureFormat.RGBA32, false, true);
                    try
                    {
                        RenderTexture.active = source;
                        texture.ReadPixels(new Rect(0, 0, source.width, source.height), 0, 0); texture.Apply();
                        return texture.GetPixels32();
                    }
                    finally { UnityEngine.Object.DestroyImmediate(texture); RenderTexture.active = previousTarget; }
                }
                var world = Read(underlay);
                var transparent = Read(transparentHud);
                var hud = runtime.Level2ProductionDayHud;
                var indices = (System.Collections.Generic.Dictionary<int, byte[]>)Field("hudBitmapIndices");
                var masks = (System.Collections.Generic.Dictionary<int, byte[]>)Field("hudBitmapMasks");
                var rows = (System.Collections.IDictionary)Field("hudBitmapDescriptors");
                var descriptor = rows[(int)hud.minimapPanelSelector];
                Assert.That(descriptor, Is.Not.Null, "the actual source selector resolves in the int-keyed HUD bank");
                int Extent(string name) => (int)descriptor.GetType().GetField(name).GetValue(descriptor);
                var width = Extent("stored_width"); var height = Extent("stored_height");
                var drawWidth = Extent("runtime_width"); var drawHeight = Extent("runtime_height");
                var assets = ((CampaignMenuPresenter)Field("gameRuntimeMenu")).ObjectiveForProfile("day");
                Vector2Int? point = null; byte panelIndex = 0;
                for (var dy = 0; dy < drawHeight && !point.HasValue; dy++)
                for (var dx = 0; dx < drawWidth && !point.HasValue; dx++)
                {
                    var source = dy * height / drawHeight * width + dx * width / drawWidth;
                    if (masks[hud.minimapPanelSelector][source] == 0) continue;
                    var x = 2 + dx; var y = 2 + dy;
                    var worldX = Mathf.Clamp(Mathf.FloorToInt((x + .5f) / 640f * underlay.width), 0, underlay.width - 1);
                    var worldY = Mathf.Clamp(Mathf.FloorToInt((1f - (y + .5f) / 480f) * underlay.height), 0, underlay.height - 1);
                    var previous = world[worldY * underlay.width + worldX].r;
                    var value = indices[hud.minimapPanelSelector][source];
                    var blended = assets.Tables[assets.TableOffset + (previous << 8) + value];
                    if (blended == value) continue;
                    Assert.That(transparent[(479 - y) * 640 + x].r, Is.EqualTo(blended),
                        "the actual transparent HUD frame blends the current indexed minimap/world");
                    point = new Vector2Int(x, y); panelIndex = value;
                }
                Assert.That(point.HasValue, Is.True, "the actual source frame contains a discriminating blend pixel");
                var before = runtime.GameRuntimeProductionIdentity;
                var f6 = typeof(MagicCarpetRuntime).GetMethod("ApplyGameRuntimeF6Input", flags);
                void F6(bool pressed, bool physical, bool admitted, bool alt) => f6.Invoke(runtime, new object[] { pressed, physical, admitted, alt });
                F6(true, false, false, false); F6(true, true, false, false); F6(true, true, true, true);
                Assert.That(runtime.GameRuntimeProductionIdentity.display_settings.panels_opaque, Is.False);
                F6(true, true, true, false);
                var after = runtime.GameRuntimeProductionIdentity;
                Assert.That(after.display_settings.panels_opaque, Is.True);
                Assert.That((after.generation, after.session.generation, after.session.tick),
                    Is.EqualTo((before.generation, before.session.generation, before.session.tick)), "the accepted paused action does not step gameplay");
                Assert.That((int)remainingField.GetValue(hint), Is.EqualTo(remaining), "source transparency does not close the retained help popup");
                Assert.That((bool)Field("gameRuntimeReplaySaveRequested"), Is.False, "Shift+F6 does not queue F5 save");
                var owner = (NativeSession)Field("session");
                var notices = owner.GameRuntimeParticipantNotifications().value;
                Assert.That((notices[0].kind, notices[0].remaining_frames), Is.EqualTo(("participant.panel_transparency_off", (short)50)));
                gui.SendEvent(new Event { type = EventType.Repaint });
                var opaque = Read((RenderTexture)Field("gameRuntimeHudIndices"));
                Assert.That(opaque[(479 - point.Value.y) * 640 + point.Value.x].r, Is.EqualTo(panelIndex),
                    "the same actual HUD pixel immediately becomes source-opaque without a fixed step");
                var bytes = runtime.QualificationSaveGameRuntimeProductionReplay();
                using (var cold = NativeSession.OpenGameRuntimeReplayFrom(owner, bytes,
                    (GameRuntimeTransport.InstalledPackageResource[])Field("gameRuntimePackages")))
                {
                    Assert.That(cold.GameRuntimeIdentity().display_settings.panels_opaque, Is.True);
                    Assert.That(cold.GameRuntimeIdentity().session.tick, Is.EqualTo(after.session.tick));
                }
                typeof(MagicCarpetRuntime).GetMethod("ApplyGameRuntimeF5Input", flags).Invoke(runtime,
                    new object[] { true, false, false, false });
                Assert.That((bool)Field("gameRuntimeReplaySaveRequested"), Is.True, "bare F5 retains its independent save edge");
                typeof(MagicCarpetRuntime).GetMethod("ServiceGameRuntimeReplaySave", flags).Invoke(runtime, null);
                CollectionAssert.AreEqual(bytes, File.ReadAllBytes(save));
                var campaign = (NativeCampaignSession)Field("campaignSession");
                var unshaded = (byte[])campaign.SceneFrameIndices.Clone();
                var beforeFlat = runtime.GameRuntimeProductionIdentity;
                Assert.That(beforeFlat.display_settings.flat_shading, Is.False);
                var f7 = typeof(MagicCarpetRuntime).GetMethod("ApplyGameRuntimeF7Input", flags);
                void F7(bool pressed, bool physical, bool admitted, bool alt) => f7.Invoke(runtime, new object[] { pressed, physical, admitted, alt });
                F7(true, false, false, false); F7(true, true, false, false); F7(true, true, true, true);
                Assert.That(runtime.GameRuntimeProductionIdentity.display_settings.flat_shading, Is.False,
                    "unshifted, held-through-dialog and Alt chords do not toggle flat shading");
                F7(true, true, true, false);
                var flat = runtime.GameRuntimeProductionIdentity;
                Assert.That(flat.display_settings.flat_shading, Is.True);
                Assert.That((flat.generation, flat.session.generation, flat.session.tick),
                    Is.EqualTo((beforeFlat.generation, beforeFlat.session.generation, beforeFlat.session.tick)),
                    "Shift+F7 re-renders without a gameplay step");
                Assert.That(System.Linq.Enumerable.SequenceEqual(campaign.SceneFrameIndices, unshaded), Is.False,
                    "the actual source indexed scene changes at the same pose and tick");
                Assert.That((int)remainingField.GetValue(hint), Is.EqualTo(remaining), "flat shading does not close help");
                var flatNotice = owner.GameRuntimeParticipantNotifications().value;
                Assert.That((flatNotice[0].kind, flatNotice[0].remaining_frames), Is.EqualTo(("participant.flat_shading_on", (short)50)));
                var flatSave = runtime.QualificationSaveGameRuntimeProductionReplay();
                using (var cold = NativeSession.OpenGameRuntimeReplayFrom(owner, flatSave,
                    (GameRuntimeTransport.InstalledPackageResource[])Field("gameRuntimePackages")))
                    Assert.That(cold.GameRuntimeIdentity().display_settings.flat_shading, Is.True,
                        "the accepted source setting survives normal cold replay");
                F7(true, true, true, false);
                Assert.That(runtime.GameRuntimeProductionIdentity.display_settings.flat_shading, Is.False);
                CollectionAssert.AreEqual(unshaded, campaign.SceneFrameIndices,
                    "toggling back restores the same actual indexed scene");
                F7(true, true, true, false);
                Assert.That(runtime.GameRuntimeProductionIdentity.display_settings.flat_shading, Is.True);
                runtime.QualificationResetGameRuntimeProduction();
                Assert.That(runtime.GameRuntimeProductionIdentity.display_settings.panels_opaque, Is.False,
                    "a real Day retry re-applies source LevelInit default");
                Assert.That(runtime.GameRuntimeProductionIdentity.display_settings.flat_shading, Is.True,
                    "a real retry preserves the global source flat-shading setting");
            }
            finally
            {
                RenderTexture.active = previousTarget;
                gui.DrawRuntime = null;
                if (shown) gui.Close(); else UnityEngine.Object.DestroyImmediate(gui);
                runtime.QualificationDisposeGameRuntimeProduction();
                display.Release(); UnityEngine.Object.DestroyImmediate(display);
                UnityEngine.Object.DestroyImmediate(root);
                Directory.Delete(temporary, true);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
        }

        [UnityEngine.TestTools.UnityTest]
        public System.Collections.IEnumerator CavePanelToggleKeepsTheActualMinimapOpaque()
        {
            var activeBefore = NativeSession.ActiveSessionCount;
            var temporary = Path.Combine(Path.GetTempPath(), "mc-cave-panels-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(temporary);
            var root = new GameObject("actual Cave panel transparency");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var gui = ScriptableObject.CreateInstance<CampaignGuiWindow>();
            var shown = false;
            var display = new RenderTexture(320, 200, 0, RenderTextureFormat.ARGB32, RenderTextureReadWrite.Linear);
            var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
            object Field(string name) => typeof(MagicCarpetRuntime).GetField(name, flags).GetValue(runtime);
            var previousTarget = RenderTexture.active;
            try
            {
                // Retained replay is the real Jaleen→Pav transfer at the
                // source32 opening boundary; no mission completion is staged.
                var input = Environment.GetEnvironmentVariable("MC_SECRET_ANNOUNCEMENT_LIVE_SAVE_OUTPUT") ??
                    Path.Combine(PrivateRoot, "generated/secret-announcement-20261115/fresh-pav-1.mcs");
                var arguments = ReturnedParentArguments(input, Path.Combine(temporary, "profile"),
                    Path.Combine(temporary, "save.mcs"));
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(arguments);
                typeof(MagicCarpetRuntime).GetMethod("PreparePlayerPreferences", flags).Invoke(runtime, new object[] { arguments });
                runtime.QualificationDriveGameRuntimeFrames(10);
                var session = (NativeCampaignSession)Field("campaignSession");
                Assert.That(session.Selection.mapType, Is.EqualTo(2));
                Assert.That(runtime.GameRuntimeProductionIdentity.display_settings.panels_opaque, Is.True,
                    "actual Cave LevelInit starts with opaque panels");
                var surface = (CampaignPresentationSurface)Field("gameRuntimeSecretSurface");
                var cameras = surface.FrameCamera.transform.parent.GetComponentsInChildren<Camera>();
                Array.Sort(cameras, (left, right) => left.depth.CompareTo(right.depth));
                foreach (var camera in cameras)
                {
                    if (camera.targetTexture == null) camera.targetTexture = display;
                    camera.Render();
                }
                gui.DrawRuntime = () => typeof(MagicCarpetRuntime).GetMethod("OnGUI", flags).Invoke(runtime, null);
                gui.minSize = new Vector2(640, 480); gui.position = new Rect(0, 0, 640, 480);
                gui.Show(); shown = true; gui.position = new Rect(0, 0, 640, 480);
                gui.SendEvent(new Event { type = EventType.Layout });
                gui.SendEvent(new Event { type = EventType.Repaint });
                Color32[] Read(RenderTexture source)
                {
                    Assert.That(source, Is.Not.Null);
                    var texture = new Texture2D(source.width, source.height, TextureFormat.RGBA32, false, true);
                    try
                    {
                        RenderTexture.active = source;
                        texture.ReadPixels(new Rect(0, 0, source.width, source.height), 0, 0); texture.Apply();
                        return texture.GetPixels32();
                    }
                    finally { UnityEngine.Object.DestroyImmediate(texture); RenderTexture.active = previousTarget; }
                }
                var mapBefore = Read(surface.HudUnderlayIndices);
                var hudBefore = Read((RenderTexture)Field("gameRuntimeHudIndices"));
                var before = runtime.GameRuntimeProductionIdentity;
                typeof(MagicCarpetRuntime).GetMethod("ApplyGameRuntimeF6Input", flags).Invoke(runtime,
                    new object[] { true, true, true, false });
                var after = runtime.GameRuntimeProductionIdentity;
                Assert.That(after.display_settings.panels_opaque, Is.False);
                Assert.That((after.generation, after.session.generation, after.session.tick),
                    Is.EqualTo((before.generation, before.session.generation, before.session.tick)));
                gui.SendEvent(new Event { type = EventType.Repaint });
                CollectionAssert.AreEqual(mapBefore, Read(surface.HudUnderlayIndices),
                    "source Cave minimap stays opaque when the shared panel flag changes");
                var hudAfter = Read((RenderTexture)Field("gameRuntimeHudIndices"));
                Assert.That(hudAfter.Length, Is.EqualTo(hudBefore.Length));
                var different = 0;
                for (var i = 0; i < hudAfter.Length; i++) if (!hudAfter[i].Equals(hudBefore[i])) different++;
                Assert.That(different, Is.GreaterThan(0), "real Cave HUD panels still follow the accepted transparency setting");
                yield return null;
            }
            finally
            {
                RenderTexture.active = previousTarget;
                gui.DrawRuntime = null;
                if (shown) gui.Close(); else UnityEngine.Object.DestroyImmediate(gui);
                runtime.QualificationDisposeGameRuntimeProduction();
                display.Release(); UnityEngine.Object.DestroyImmediate(display);
                UnityEngine.Object.DestroyImmediate(root);
                Directory.Delete(temporary, true);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
        }

        [UnityEngine.TestTools.UnityTest]
        public System.Collections.IEnumerator FullMapHandsFollowAcceptedSourceSelectionAndExpiry()
        {
            var activeBefore = NativeSession.ActiveSessionCount;
            var temporary = Path.Combine(Path.GetTempPath(), "mc-map-hands-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(temporary);
            var root = new GameObject("actual source map hand selection");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var gui = ScriptableObject.CreateInstance<CampaignGuiWindow>();
            var shown = false;
            var previousTarget = RenderTexture.active;
            var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
            object Field(string name) => typeof(MagicCarpetRuntime).GetField(name, flags).GetValue(runtime);
            byte[] Counters() => runtime.GameRuntimeProductionCampaignSnapshot.value.player_hand_selection_flash_ticks;
            Color32[] Read(RenderTexture source)
            {
                Assert.That(source, Is.Not.Null);
                var texture = new Texture2D(source.width, source.height, TextureFormat.RGBA32, false, true);
                try
                {
                    RenderTexture.active = source;
                    texture.ReadPixels(new Rect(0, 0, source.width, source.height), 0, 0); texture.Apply();
                    return texture.GetPixels32();
                }
                finally { UnityEngine.Object.DestroyImmediate(texture); RenderTexture.active = previousTarget; }
            }
            try
            {
                var input = Environment.GetEnvironmentVariable("MC_COMPLETED_SECRET_SAVE_OUTPUT") ??
                    Path.Combine(PrivateRoot, "generated/continuity-20261115/controlled-secret-map.mcs");
                var arguments = ReturnedParentArguments(input, Path.Combine(temporary, "profile"),
                    Path.Combine(temporary, "save.mcs"));
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(arguments);
                typeof(MagicCarpetRuntime).GetMethod("PreparePlayerPreferences", flags).Invoke(runtime, new object[] { arguments });
                yield return StartReturnedParentFromNormalMap(runtime);
                var sourceMenus = (SourceCampaignMenus)Field("gameRuntimeSourceMenus");
                var map = sourceMenus.Step(true, false, false, false, false, true);
                Assert.That(sourceMenus.QueueAction(map.Action, sourceMenus.MapOpen, sourceMenus.SpellOpen,
                    sourceMenus.PlayerInformationOpen, map), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(sourceMenus.MapOpen, Is.True);
                for (var frame = 0; frame < 8 && (Counters()[0] != 0 || Counters()[1] != 0); frame++)
                    runtime.QualificationDriveGameRuntimeFrames(1);
                CollectionAssert.AreEqual(new byte[] { 0, 0 }, Counters(), "real source updates exhaust any inherited counters");
                typeof(MagicCarpetRuntime).GetMethod("ApplyGameRuntimeF5Input", flags).Invoke(runtime,
                    new object[] { true, true, true, false });
                Assert.That(runtime.GameRuntimeProductionIdentity.display_settings.top_bar, Is.False);
                Assert.That(runtime.GameRuntimeProductionIdentity.display_settings.panels_opaque, Is.False);
                runtime.QualificationAssignGameRuntimeProductionSpell("spell.volcano", 0, false);
                runtime.QualificationDriveGameRuntimeFrames(1);
                CollectionAssert.AreEqual(new byte[] { 8, 0 }, Counters(), "accepted left source action sets only its own counter");
                gui.DrawRuntime = () => typeof(MagicCarpetRuntime).GetMethod("OnGUI", flags).Invoke(runtime, null);
                gui.minSize = new Vector2(640, 480); gui.position = new Rect(0, 0, 640, 480);
                gui.Show(); shown = true; gui.position = new Rect(0, 0, 640, 480);
                gui.SendEvent(new Event { type = EventType.Layout });
                gui.SendEvent(new Event { type = EventType.Repaint });
                var surface = (CampaignPresentationSurface)Field("gameRuntimeSecretSurface");
                var mapIndices = surface.HudUnderlayIndices;
                var handIndices = (RenderTexture)Field("gameRuntimeHudIndices");
                Assert.That(mapIndices, Is.Not.Null); Assert.That(handIndices, Is.Not.Null);
                var mapPixels = Read(mapIndices); var handPixels = Read(handIndices);
                var hud = runtime.Level2ProductionDayHud;
                var descriptors = (System.Collections.IDictionary)Field("hudBitmapDescriptors");
                var indices = (System.Collections.Generic.Dictionary<int, byte[]>)Field("hudBitmapIndices");
                var masks = (System.Collections.Generic.Dictionary<int, byte[]>)Field("hudBitmapMasks");
                int Extent(int selector, string name) => (int)descriptors[selector].GetType().GetField(name).GetValue(descriptors[selector]);
                bool Masked(int selector, int x, int y)
                {
                    var rw = Extent(selector, "runtime_width"); var rh = Extent(selector, "runtime_height");
                    if (x < 0 || y < 0 || x >= rw || y >= rh) return false;
                    var sw = Extent(selector, "stored_width"); var sh = Extent(selector, "stored_height");
                    return masks[selector][y * sh / rh * sw + x * sw / rw] != 0;
                }
                var frameWidth = Extent(hud.left.frameSelector, "runtime_width");
                var frameHeight = Extent(hud.left.frameSelector, "runtime_height");
                var storedWidth = Extent(hud.left.frameSelector, "stored_width");
                var storedHeight = Extent(hud.left.frameSelector, "stored_height");
                var assets = ((CampaignMenuPresenter)Field("gameRuntimeMenu")).ObjectiveForProfile("day");
                Vector2Int? point = null; byte frameIndex = 0;
                for (var dy = 20; dy < Math.Min(frameHeight, 36) && !point.HasValue; dy++)
                for (var dx = 0; dx < frameWidth && !point.HasValue; dx++)
                {
                    if (!Masked(hud.left.frameSelector, dx, dy) || Masked(hud.left.iconSelector, dx, dy)) continue;
                    var x = 510 + dx; var y = 2 + dy;
                    if (x >= 640) continue;
                    var sx = Mathf.Clamp(Mathf.FloorToInt((x + .5f) / 640f * mapIndices.width), 0, mapIndices.width - 1);
                    var sy = Mathf.Clamp(Mathf.FloorToInt((1f - (y + .5f) / 480f) * mapIndices.height), 0, mapIndices.height - 1);
                    var background = mapPixels[sy * mapIndices.width + sx].r;
                    var value = indices[hud.left.frameSelector][dy * storedHeight / frameHeight * storedWidth + dx * storedWidth / frameWidth];
                    var blended = assets.Tables[assets.TableOffset + (background << 8) + value];
                    if (blended == background || blended == value) continue;
                    Assert.That(handPixels[(479 - y) * 640 + x].r, Is.EqualTo(blended),
                        "the selected map hand uses the source transparent frame over the current map");
                    point = new Vector2Int(x, y); frameIndex = value;
                }
                Assert.That(point.HasValue, Is.True, "the actual source hand has a discriminating indexed frame pixel");
                var statusX = 250; var statusY = 10;
                var mapX = Mathf.FloorToInt((statusX + .5f) / 640f * mapIndices.width);
                var mapY = Mathf.FloorToInt((1f - (statusY + .5f) / 480f) * mapIndices.height);
                Assert.That(handPixels[(479 - statusY) * 640 + statusX].r, Is.EqualTo(mapPixels[mapY * mapIndices.width + mapX].r),
                    "full map never draws the ordinary top status strip");
                var tick = runtime.GameRuntimeProductionIdentity.session.tick;
                var replay = runtime.QualificationSaveGameRuntimeProductionReplay();
                // A long source-font row is a declared compositor control: it
                // exercises the real overlap without changing the native owner.
                var glyph = assets.Glyphs['W' + 1];
                var overlap = false;
                var originalDraw = gui.DrawRuntime;
                try
                {
                    gui.DrawRuntime = () => {
                        originalDraw();
                        typeof(MagicCarpetRuntime).GetField("gameRuntimeMapNoticeRows", flags).SetValue(runtime,
                            new[] { new SourcePlayerNotificationsPresenter.Row(new string('W', 40) + "\r" + new string('W', 40), 6, 6) });
                        typeof(MagicCarpetRuntime).GetMethod("DrawGameRuntimeIndexedHud", flags).Invoke(runtime,
                            new object[] { hud, true });
                    };
                    gui.SendEvent(new Event { type = EventType.Repaint });
                    var composed = Read((RenderTexture)Field("gameRuntimeHudIndices"));
                    for (var column = 0; column < 80 && !overlap; column++)
                    for (var gy = 0; gy < glyph.Height && !overlap; gy++)
                    for (var gx = 0; gx < glyph.Width && !overlap; gx++)
                    {
                        var x = 6 + column * glyph.Width + gx; var y = 6 + gy;
                        var dx = x - 510; var dy = y - 2;
                        if (x >= 640 || dx < 0 || dx >= frameWidth / 2 || dy < 0 || dy >= frameHeight ||
                            assets.GlyphMasks[glyph.Offset + gy * glyph.Width + gx] == 0 ||
                            !Masked(hud.left.frameSelector, dx, dy) || Masked(hud.left.iconSelector, dx, dy)) continue;
                        var sx = Mathf.FloorToInt((x + .5f) / 640f * mapIndices.width);
                        var sy = Mathf.FloorToInt((1f - (y + .5f) / 480f) * mapIndices.height);
                        var background = mapPixels[sy * mapIndices.width + sx].r;
                        var frame = indices[hud.left.frameSelector][dy * storedHeight / frameHeight * storedWidth + dx * storedWidth / frameWidth];
                        var expected = assets.Tables[assets.TableOffset + (assets.TextColor << 8) + frame];
                        if (expected == assets.Tables[assets.TableOffset + (background << 8) + frame]) continue;
                        Assert.That(composed[(479 - y) * 640 + x].r, Is.EqualTo(expected),
                            "the transparent hand blends over source notice glyphs, not bare map");
                        overlap = true;
                    }
                    Assert.That(overlap, Is.True, "the source glyph/frame overlap has a discriminating pixel");
                }
                finally { gui.DrawRuntime = originalDraw; }
                CollectionAssert.AreEqual(replay, runtime.QualificationSaveGameRuntimeProductionReplay(),
                    "the declared overlap control does not change the native owner");
                for (var repaint = 0; repaint < 3; repaint++) gui.SendEvent(new Event { type = EventType.Repaint });
                CollectionAssert.AreEqual(new byte[] { 8, 0 }, Counters(), "repaint cannot decrement source counters");
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo(tick));
                CollectionAssert.AreEqual(replay, runtime.QualificationSaveGameRuntimeProductionReplay());
                var f6 = typeof(MagicCarpetRuntime).GetMethod("ApplyGameRuntimeF6Input", flags);
                f6.Invoke(runtime, new object[] { true, true, true, false });
                Assert.That(runtime.GameRuntimeProductionIdentity.display_settings.panels_opaque, Is.True);
                gui.SendEvent(new Event { type = EventType.Repaint });
                var opaque = Read((RenderTexture)Field("gameRuntimeHudIndices"));
                Assert.That(opaque[(479 - point.Value.y) * 640 + point.Value.x].r, Is.EqualTo(frameIndex),
                    "the selected map hand follows the accepted opaque setting on the same tick");
                f6.Invoke(runtime, new object[] { true, true, true, false });
                Assert.That(runtime.GameRuntimeProductionIdentity.display_settings.panels_opaque, Is.False);
                runtime.QualificationAssignGameRuntimeProductionSpell("spell.volcano", 0, true);
                runtime.QualificationDriveGameRuntimeFrames(1);
                CollectionAssert.AreEqual(new byte[] { 7, 8 }, Counters(), "right assignment preserves the independently aging left counter");
                hud = runtime.Level2ProductionDayHud;
                runtime.QualificationPauseGameRuntimeProduction();
                runtime.QualificationDriveGameRuntimeFrames(1);
                CollectionAssert.AreEqual(new byte[] { 6, 7 }, Counters(), "a real paused source pass ages both counters once");
                for (var expected = 6; expected >= 0; expected--)
                {
                    runtime.QualificationDriveGameRuntimeFrames(1);
                    Assert.That(Counters()[1], Is.EqualTo(expected));
                    if (expected == 1)
                    {
                        CollectionAssert.AreEqual(new byte[] { 0, 1 }, Counters());
                        gui.SendEvent(new Event { type = EventType.Repaint });
                        var rightMap = surface.HudUnderlayIndices;
                        var rightBackground = Read(rightMap);
                        var rightOutput = Read((RenderTexture)Field("gameRuntimeHudIndices"));
                        var width = Extent(hud.right.frameSelector, "runtime_width");
                        var height = Extent(hud.right.frameSelector, "runtime_height");
                        var sw = Extent(hud.right.frameSelector, "stored_width");
                        var sh = Extent(hud.right.frameSelector, "stored_height");
                        var foundRight = false;
                        for (var dy = 20; dy < Math.Min(height, 36) && !foundRight; dy++)
                        for (var dx = 0; dx < width && !foundRight; dx++)
                        {
                            var x = 574 + dx; var y = 2 + dy;
                            if (x >= 640 || !Masked(hud.right.frameSelector, dx, dy) ||
                                Masked(hud.right.iconSelector, dx, dy)) continue;
                            var sx = Mathf.FloorToInt((x + .5f) / 640f * rightMap.width);
                            var sy = Mathf.FloorToInt((1f - (y + .5f) / 480f) * rightMap.height);
                            var background = rightBackground[sy * rightMap.width + sx].r;
                            var value = indices[hud.right.frameSelector][dy * sh / height * sw + dx * sw / width];
                            var blend = assets.Tables[assets.TableOffset + (background << 8) + value];
                            if (blend == background) continue;
                            Assert.That(rightOutput[(479 - y) * 640 + x].r, Is.EqualTo(blend),
                                "the independently live right hand is rendered over the current map");
                            foundRight = true;
                        }
                        Assert.That(foundRight, Is.True);
                    }
                }
                CollectionAssert.AreEqual(new byte[] { 0, 0 }, Counters());
                gui.SendEvent(new Event { type = EventType.Repaint });
                Assert.That(Field("gameRuntimeHudIndices"), Is.Null, "expired map hands leave no stale indexed overlay");
                runtime.QualificationResumeGameRuntimeProduction();
                runtime.QualificationAssignGameRuntimeProductionSpell("spell.volcano", byte.MaxValue, false);
                runtime.QualificationDriveGameRuntimeFrames(1);
                CollectionAssert.AreEqual(new byte[] { 0, 0 }, Counters(), "a rejected source assignment does not start a hand counter");
                runtime.QualificationResetGameRuntimeProduction();
                Assert.That(Field("gameRuntimeHudIndices"), Is.Null, "real reset discards the old indexed hand generation");
            }
            finally
            {
                RenderTexture.active = previousTarget;
                gui.DrawRuntime = null;
                if (shown) gui.Close(); else UnityEngine.Object.DestroyImmediate(gui);
                runtime.QualificationDisposeGameRuntimeProduction();
                UnityEngine.Object.DestroyImmediate(root);
                Directory.Delete(temporary, true);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
        }

        [UnityEngine.TestTools.UnityTest]
        public System.Collections.IEnumerator SourceSpellMenuHoverUsesCurrentOwnerAndSharedClock()
        {
            var activeBefore = NativeSession.ActiveSessionCount;
            var temporary = Path.Combine(Path.GetTempPath(), "mc-spell-hover-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(temporary);
            var root = new GameObject("actual source spell menu hover");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var gui = ScriptableObject.CreateInstance<CampaignGuiWindow>();
            var shown = false;
            var display = new RenderTexture(320, 200, 0, RenderTextureFormat.ARGB32, RenderTextureReadWrite.Linear);
            var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
            object Field(string name) => typeof(MagicCarpetRuntime).GetField(name, flags).GetValue(runtime);
            MagicCarpetRuntime.ManaHelpHint Hint() => (MagicCarpetRuntime.ManaHelpHint)Field("gameRuntimeManaHint");
            int Remaining() => (int)typeof(MagicCarpetRuntime.ManaHelpHint).GetField("remaining", flags).GetValue(Hint());
            void RenderCurrentScene()
            {
                var surface = (CampaignPresentationSurface)Field("gameRuntimeSecretSurface");
                var cameras = surface.FrameCamera.transform.parent.GetComponentsInChildren<Camera>();
                Array.Sort(cameras, (left, right) => left.depth.CompareTo(right.depth));
                foreach (var camera in cameras)
                {
                    if (camera.targetTexture == null) camera.targetTexture = display;
                    camera.Render();
                }
            }
            try
            {
                var input = Environment.GetEnvironmentVariable("MC_COMPLETED_SECRET_SAVE_OUTPUT") ??
                    Path.Combine(PrivateRoot, "generated/continuity-20261115/controlled-secret-map.mcs");
                var arguments = ReturnedParentArguments(input, Path.Combine(temporary, "profile"),
                    Path.Combine(temporary, "save.mcs"));
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(arguments);
                typeof(MagicCarpetRuntime).GetMethod("PreparePlayerPreferences", flags).Invoke(runtime,
                    new object[] { arguments });
                yield return StartReturnedParentFromNormalMap(runtime);
                var menu = (CampaignMenuPresenter)Field("gameRuntimeMenu");
                Assert.That(runtime.SubmitGameRuntimeSpellSelection("spell.volcano"), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                runtime.QualificationAssignGameRuntimeProductionSpell("spell.volcano", 0, false);
                runtime.QualificationDriveGameRuntimeFrames(1);
                for (var frame = 0; frame < 220 && runtime.GameRuntimeProductionCampaignSnapshot.value.objective_textbox_ticks != 0; frame++)
                    runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.GameRuntimeProductionCampaignSnapshot.value.objective_textbox_ticks, Is.Zero,
                    "normal source frames leave the objective before hover help");
                var sourceMenus = (SourceCampaignMenus)Field("gameRuntimeSourceMenus");
                var spellInput = sourceMenus.Step(false, true, false, false, false, true);
                Assert.That(sourceMenus.QueueAction(spellInput.Action, sourceMenus.MapOpen,
                    sourceMenus.SpellOpen, sourceMenus.PlayerInformationOpen, spellInput), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(sourceMenus.SpellOpen, Is.True);
                Assert.That((bool)Field("gameRuntimeSpellMenuOpen"), Is.True,
                    "the accepted source menu reaches the actual GUI consumer");
                // The EditorWindow host has no focused mouse. Match the
                // existing pause-hover host: one declared logical pointer is
                // transformed through the fitted view for every GUI callback.
                var view = new Rect();
                var origin = Vector2.zero;
                var controlledCursor = Vector2.zero;
                gui.DrawRuntime = () => {
                    var scale = Mathf.Min(Screen.width / 640f, Screen.height / 480f);
                    view = new Rect((Screen.width - 640 * scale) / 2, (Screen.height - 480 * scale) / 2,
                        640 * scale, 480 * scale);
                    origin = GUIUtility.GUIToScreenPoint(Vector2.zero) - gui.position.position;
                    var inputEvent = Event.current;
                    var hostCursor = inputEvent.mousePosition;
                    try
                    {
                        inputEvent.mousePosition = view.position + controlledCursor * scale;
                        typeof(MagicCarpetRuntime).GetMethod("OnGUI", flags).Invoke(runtime, null);
                    }
                    finally { inputEvent.mousePosition = hostCursor; }
                };
                gui.wantsMouseMove = true;
                gui.minSize = new Vector2(640, 480); gui.position = new Rect(0, 0, 640, 480);
                gui.Show(); shown = true; gui.position = new Rect(0, 0, 640, 480);
                var deadline = Time.realtimeSinceStartupAsDouble + 10;
                while (view.width <= 0)
                {
                    Assert.That(Time.realtimeSinceStartupAsDouble, Is.LessThan(deadline), "the real GUI host draws its fitted viewport");
                    gui.Repaint();
                    UnityEditor.EditorApplication.QueuePlayerLoopUpdate();
                    yield return null;
                }
                void Send(EventType type, Vector2 source)
                {
                    controlledCursor = source;
                    var local = view.position + source * (view.width / 640f);
                    var beforeDraw = gui.DrawCount;
                    gui.SendEvent(new Event { type = type, mousePosition = local + origin });
                    Assert.That(gui.DrawCount, Is.GreaterThan(beforeDraw), "the actual GUI host dispatches the source event");
                }
                var liveMenu = runtime.GameRuntimeProductionCampaignSnapshot.value;
                var hoveredSpell = Array.Find(liveMenu.player_spells, item => item?.id == "spell.volcano");
                Assert.That(hoveredSpell?.menu_help?.is_standard, Is.True,
                    "the selected actual Volcano holder admits source spell help");
                var pointer = SourceSpellMenu.GridPointer(hoveredSpell.menu_slot);
                RenderCurrentScene();
                Send(EventType.MouseMove, pointer);
                var sourceSpellMenu = (SourceSpellMenu)Field("gameRuntimeSourceSpellMenu");
                Assert.That(sourceSpellMenu.HoveredSlot, Is.EqualTo(hoveredSpell.menu_slot), "the delivered MouseMove reaches the source slot");
                Send(EventType.Repaint, pointer);
                Assert.That(sourceSpellMenu.HoveredSlot, Is.EqualTo(hoveredSpell.menu_slot));
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(Hint().SpellVisible, Is.True);
                Assert.That(Hint().VisibleRow, Is.EqualTo(85));
                Assert.That(Hint().VisibleSpellRecords, Is.EqualTo(hoveredSpell.menu_help.title_records));
                Assert.That(Hint().VisibleSpellHighlight, Is.EqualTo(hoveredSpell.menu_help.selected_tier));
                Assert.That(Remaining(), Is.EqualTo(69), "source row85 lasts 70 draws including the current frame");
                // Exercise all eight source scan positions with the same real
                // native hover observation, without advancing the simulation.
                var cycle = new MagicCarpetRuntime.ManaHelpHint(menu.GameplayHelpForRow);
                var expectedRows = new System.Collections.Generic.List<int> { 85, 69 };
                if (Array.Exists(hoveredSpell.menu_help.castle_blocked, blocked => blocked)) expectedRows.Add(70);
                if (hoveredSpell.unlocked_tier > 0) expectedRows.Add(71);
                expectedRows.Add(42); // source case4 can be replaced by case5 in the same scan
                if (hoveredSpell.menu_help.mana_unavailable) expectedRows.Add(41);
                expectedRows.Add(85);
                var observedRows = new System.Collections.Generic.List<int>();
                var priorRow = 0;
                var beforeCycle = runtime.GameRuntimeProductionIdentity.session.tick;
                for (var frame = 0; frame < 400 && observedRows.Count < expectedRows.Count; frame++)
                {
                    cycle.AdvanceSpell(hoveredSpell, hoveredSpell.menu_slot, hoveredSpell.menu_help.selected_tier,
                        liveMenu.spell_menu_has_any_spell, true, false,
                        sourceSpellMenu.PanelExtent.x, sourceSpellMenu.PanelExtent.y,
                        sourceSpellMenu.TierExtent.x, sourceSpellMenu.TierExtent.y);
                    var row = cycle.SpellVisible ? cycle.VisibleRow : 0;
                    if (row != 0 && row != priorRow) observedRows.Add(row);
                    priorRow = row;
                }
                Assert.That(observedRows, Is.EqualTo(expectedRows), "actual source conditions select the eight-position cycle");
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo(beforeCycle),
                    "the isolated presentation-clock check cannot step the native owner");
                RenderCurrentScene();
                var spellHelpTick = runtime.GameRuntimeProductionIdentity.session.tick;
                var spellHelpReplay = runtime.QualificationSaveGameRuntimeProductionReplay();
                for (var redraw = 0; redraw < 3; redraw++)
                    Send(EventType.Repaint, pointer);
                Assert.That(Remaining(), Is.EqualTo(69), "repaint cannot advance the shared source popup clock");
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo(spellHelpTick));
                CollectionAssert.AreEqual(spellHelpReplay, runtime.QualificationSaveGameRuntimeProductionReplay());
                var spellPresenter = (SourceObjectiveTextboxPresenter)Field("gameRuntimeManaHintPresenter");
                var spellOutput = (RenderTexture)typeof(SourceObjectiveTextboxPresenter).GetField("output", flags).GetValue(spellPresenter);
                Assert.That(spellOutput, Is.Not.Null, "the real Ctrl menu reaches the indexed source popup consumer");
                var spellPrior = (Color32[])Field("gameRuntimeSpellPriorPixels");
                var spellPriorRect = (RectInt)typeof(SourceObjectiveTextboxPresenter).GetField("priorRect", flags).GetValue(spellPresenter);
                var tierWidth = sourceSpellMenu.TierExtent.x;
                var tierStart = MagicCarpetRuntime.SpellTierMenuStartX(hoveredSpell.menu_slot, 13, 8, 48, 3, tierWidth);
                var tierTop = 408 - sourceSpellMenu.TierExtent.y;
                var sourceAssets = menu.ObjectiveForProfile("day");
                var threshold = hoveredSpell.menu_help.experience_thresholds;
                var delta = unchecked(threshold[1] - threshold[0]);
                var progress = unchecked(hoveredSpell.menu_help.experience - threshold[0]);
                var filled = delta > 0 ? Mathf.Clamp(unchecked((short)(unchecked(54 * progress) / delta)), 0, 54) : 0;
                var barPixel = spellPrior[(tierTop + 28 - spellPriorRect.y) * spellPriorRect.width + tierStart + 6];
                Assert.That((barPixel.r, barPixel.a), Is.EqualTo((filled > 0 ? sourceAssets.TextColor : sourceAssets.BlendSelector, (byte)5)),
                    "the actual source XP bar paints the expected indexed foreground over its tier panel");
                // Detached renderer controls cover two source arithmetic cases
                // without changing the real native owner, inventory or replay.
                var detached = JsonUtility.FromJson<EngineSessionTransport.CampaignRenderValue>(JsonUtility.ToJson(liveMenu));
                var detachedSpell = Array.Find(detached.player_spells, item => item?.id == hoveredSpell.id);
                Assert.That(detachedSpell.unlocked_tier, Is.Zero);
                Assert.That(detachedSpell.menu_help.tier_mana_costs[0], Is.GreaterThan(0));
                Assert.That(detached.player_vitals.mana / detachedSpell.menu_help.tier_mana_costs[0], Is.Zero,
                    "the retained real caster mana is below the source tier cost");
                detachedSpell.menu_help.castle_blocked[0] = false;
                detachedSpell.menu_help.experience = -300000;
                detachedSpell.menu_help.experience_thresholds = new[] { 0, 400, 12000 };
                var textures = (System.Collections.Generic.Dictionary<int, Texture2D>)Field("hudBitmapTextures");
                var indices = (System.Collections.Generic.Dictionary<int, byte[]>)Field("hudBitmapIndices");
                var masks = (System.Collections.Generic.Dictionary<int, byte[]>)Field("hudBitmapMasks");
                var descriptors = (System.Collections.IDictionary)Field("hudBitmapDescriptors");
                int Width(int selector) => (int)descriptors[selector].GetType().GetField("stored_width").GetValue(descriptors[selector]);
                int Height(int selector) => (int)descriptors[selector].GetType().GetField("stored_height").GetValue(descriptors[selector]);
                int RuntimeWidth(int selector) => (int)descriptors[selector].GetType().GetField("runtime_width").GetValue(descriptors[selector]);
                int RuntimeHeight(int selector) => (int)descriptors[selector].GetType().GetField("runtime_height").GetValue(descriptors[selector]);
                int SourcePixel(int selector, int x, int y) =>
                    (y * Height(selector) / RuntimeHeight(selector)) * Width(selector) + x * Width(selector) / RuntimeWidth(selector);
                bool Masked(int selector, int x, int y) => x >= 0 && y >= 0 &&
                    x < RuntimeWidth(selector) && y < RuntimeHeight(selector) && masks[selector][SourcePixel(selector, x, y)] != 0;
                ObjectiveTextboxCompositor.Sprite Indexed(int selector) =>
                    new ObjectiveTextboxCompositor.Sprite(Width(selector), Height(selector), indices[selector], masks[selector]);
                var component = new SourceSpellMenu();
                var ordinaryDraw = gui.DrawRuntime;
                try
                {
                    gui.DrawRuntime = () => {
                        var inputEvent = Event.current;
                        var hostCursor = inputEvent.mousePosition;
                        try
                        {
                            inputEvent.mousePosition = view.position + pointer * (view.width / 640f);
                            component.Draw(detached, selector => textures[selector],
                                selector => new Vector2Int(RuntimeWidth(selector), RuntimeHeight(selector)), false, sourceAssets, out _);
                        }
                        finally { inputEvent.mousePosition = hostCursor; }
                    };
                    Send(EventType.Repaint, pointer);
                    Assert.That(component.HoveredSlot, Is.EqualTo(hoveredSpell.menu_slot),
                        "the detached renderer consumes the same declared source slot");
                    Assert.That(component.TierExtent, Is.EqualTo(sourceSpellMenu.TierExtent),
                        "the detached renderer uses the actual source runtime scale");
                    Assert.That(component.TryIndexedPrior(sourceAssets, Indexed, sourceAssets.Tables, sourceAssets.TableOffset,
                        out var detachedRect, out var detachedPixels), Is.True);
                    // Source int16 narrows -40500 to +25036 before the 54px clamp.
                    var lastBar = detachedPixels[(tierTop + 28 - detachedRect.y) * detachedRect.width + tierStart + 59 - detachedRect.x];
                    Assert.That((lastBar.r, lastBar.a), Is.EqualTo((sourceAssets.TextColor, (byte)5)),
                        "the real indexed bar reflects source signed-16 narrowing before clamping");
                    var icon = detachedSpell.menu_artwork.tier_icons[0];
                    var iconSamples = 0;
                    for (var iy = 0; iy < RuntimeHeight(icon); iy++)
                    for (var ix = 0; ix < RuntimeWidth(icon); ix++)
                    {
                        var px = 18 + ix; var py = 6 + iy;
                        if (!Masked(icon, ix, iy) || !Masked(162, px, py) || Masked(165, px - 6, py - 10) ||
                            (component.HoveredTier == 0 && Masked(164, px, py)) || py >= 28) continue;
                        var value = indices[icon][SourcePixel(icon, ix, iy)];
                        var background = indices[162][SourcePixel(162, px, py)];
                        if (sourceAssets.Tables[sourceAssets.TableOffset + (background << 8) + value] == value) continue;
                        var actual = detachedPixels[(tierTop + py - detachedRect.y) * detachedRect.width + tierStart + px - detachedRect.x];
                        Assert.That((actual.r, actual.a), Is.EqualTo((value, (byte)5)),
                            "a Castle-eligible but unaffordable source icon is opaque over its dimmed panel");
                        iconSamples++;
                    }
                    Assert.That(iconSamples, Is.GreaterThan(0), "the actual source masks contain a discriminating icon pixel");
                }
                finally { gui.DrawRuntime = ordinaryDraw; component.Close(); }
                CollectionAssert.AreEqual(spellHelpReplay, runtime.QualificationSaveGameRuntimeProductionReplay(),
                    "detached renderer controls do not alter the actual native owner or replay");
                for (var draw = 0; draw < 69; draw++) runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(Remaining(), Is.Zero);
                for (var wait = 0; wait < 6; wait++) runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(Hint().Visible, Is.False, "source waits six polls before selecting the next spell row");
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That((Hint().SpellVisible, Hint().VisibleRow), Is.EqualTo((true, 69)),
                    "the real source cycle reaches spell experience after its six-poll delay");
                var disabledSpell = Array.Find(runtime.GameRuntimeProductionCampaignSnapshot.value.player_spells,
                    item => item?.menu_help != null && !item.menu_help.is_standard);
                Assert.That(disabledSpell, Is.Not.Null, "the real source inventory contains an unavailable family");
                var disabledPointer = SourceSpellMenu.GridPointer(disabledSpell.menu_slot);
                RenderCurrentScene();
                Send(EventType.MouseMove, disabledPointer);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(sourceSpellMenu.HoveredSlot, Is.EqualTo(disabledSpell.menu_slot));
                Assert.That(Hint().SpellVisible, Is.False, "an unavailable source family closes the current spell help");
                RenderCurrentScene();
                Send(EventType.MouseMove, pointer);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That((Hint().SpellVisible, Hint().VisibleRow), Is.EqualTo((true, 85)),
                    "returning to an acquired family restarts the source title cycle");
                var beforeSpellPause = Remaining();
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That((Hint().SpellVisible, Hint().VisibleRow, Remaining()),
                    Is.EqualTo((true, 85, beforeSpellPause - 1)),
                    "source spell-menu status takes precedence over the independent pause flag for a real paused frame");
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                var closeSpells = sourceMenus.Step(false, false, false, false, false, true);
                Assert.That(sourceMenus.QueueAction(closeSpells.Action, sourceMenus.MapOpen,
                    sourceMenus.SpellOpen, sourceMenus.PlayerInformationOpen, closeSpells), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(sourceMenus.SpellOpen, Is.False);
                Assert.That(Hint().SpellVisible, Is.False, "releasing Ctrl removes the spell popup");
                var openMap = sourceMenus.Step(true, false, false, false, false, true);
                Assert.That(sourceMenus.QueueAction(openMap.Action, sourceMenus.MapOpen,
                    sourceMenus.SpellOpen, sourceMenus.PlayerInformationOpen, openMap), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(sourceMenus.MapOpen, Is.True);
                var mapSpells = sourceMenus.Step(false, true, false, false, false, true);
                Assert.That(sourceMenus.QueueAction(mapSpells.Action, sourceMenus.MapOpen,
                    sourceMenus.SpellOpen, sourceMenus.PlayerInformationOpen, mapSpells), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That((sourceMenus.MapOpen, sourceMenus.SpellOpen), Is.EqualTo((true, true)));
                Send(EventType.MouseMove, pointer);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That((Hint().SpellVisible, Hint().VisibleRow), Is.EqualTo((true, 85)));
                Send(EventType.Repaint, pointer);
                var mapSurface = (CampaignPresentationSurface)Field("gameRuntimeSecretSurface");
                var mapPresenter = (SourceObjectiveTextboxPresenter)Field("gameRuntimeManaHintPresenter");
                var material = (Material)typeof(SourceObjectiveTextboxPresenter).GetField("material", flags).GetValue(mapPresenter);
                Assert.That(mapSurface.HudUnderlayIndices, Is.Not.Null);
                var mapHud = (Texture)Field("gameRuntimeHudIndices") ?? mapSurface.HudUnderlayIndices;
                Assert.That(mapHud, Is.Not.Null);
                Assert.That(material.GetTexture("_SceneIndices"), Is.SameAs(mapHud),
                    "source map+spell help blends the current map and indexed HUD in source order");
                Assert.That(material.GetFloat("_SceneTopDown"), Is.Zero);
                var closeMapSpells = sourceMenus.Step(false, false, false, false, false, true);
                Assert.That(sourceMenus.QueueAction(closeMapSpells.Action, sourceMenus.MapOpen,
                    sourceMenus.SpellOpen, sourceMenus.PlayerInformationOpen, closeMapSpells), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                var closeMap = sourceMenus.Step(true, false, false, false, false, true);
                Assert.That(sourceMenus.QueueAction(closeMap.Action, sourceMenus.MapOpen,
                    sourceMenus.SpellOpen, sourceMenus.PlayerInformationOpen, closeMap), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That((sourceMenus.MapOpen, sourceMenus.SpellOpen), Is.EqualTo((false, false)));
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Title), Is.True);
                Assert.That(Field("gameRuntimeManaHint"), Is.Null);
                Assert.That(Field("gameRuntimeManaHintPresenter"), Is.Null);
            }
            finally
            {
                gui.DrawRuntime = null;
                if (shown) gui.Close();
                else UnityEngine.Object.DestroyImmediate(gui);
                runtime.QualificationDisposeGameRuntimeProduction();
                display.Release(); UnityEngine.Object.DestroyImmediate(display);
                UnityEngine.Object.DestroyImmediate(root);
                Directory.Delete(temporary, true);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
        }

        [UnityEngine.TestTools.UnityTest]
        public System.Collections.IEnumerator ControlledResultsRevisitAdmitsRealSpellTierDrag()
        {
            var activeBefore = NativeSession.ActiveSessionCount;
            var temporary = Path.Combine(Path.GetTempPath(), "mc-spell-tier-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(temporary);
            var root = new GameObject("source upgraded spell tier revisit");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var gui = ScriptableObject.CreateInstance<CampaignGuiWindow>();
            var shown = false;
            var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
            try
            {
                // Existing controlled Results includes declared tier-2 carry;
                // it is a prior-inventory boundary, not earned progression.
                var input = Environment.GetEnvironmentVariable("MC_COMPLETED_RESULTS_SAVE_OUTPUT") ??
                    Path.Combine(PrivateRoot, "generated/continuity-20261115/controlled-results.mcs");
                var arguments = ReturnedParentArguments(input, Path.Combine(temporary, "profile"),
                    Path.Combine(temporary, "save.mcs"));
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(arguments);
                typeof(MagicCarpetRuntime).GetMethod("PreparePlayerPreferences", flags).Invoke(runtime,
                    new object[] { arguments });
                Assert.That(runtime.GameRuntimeProductionIdentity.campaign_menu.phase, Is.EqualTo("results"));
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.DismissResults), Is.True);
                var owner = (NativeSession)typeof(MagicCarpetRuntime).GetField("session", flags).GetValue(runtime);
                const string level = "level.magic_carpet_2.uluth";
                Assert.That(runtime.GameRuntimeProductionIdentity.campaign_menu.available_levels, Does.Contain(level));
                owner.GameRuntimeCampaignMenuAction("select_mission", level);
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.StartMission), Is.True,
                    "the real source menu must admit revisiting this completed portal");
                runtime.QualificationDriveGameRuntimeFrames(1);
                runtime.QualificationObserveGameRuntimeLoadingPresentationFrame();
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                var menus = (SourceCampaignMenus)typeof(MagicCarpetRuntime).GetField("gameRuntimeSourceMenus", flags).GetValue(runtime);
                var open = menus.Step(false, true, false, false, false, true);
                Assert.That(menus.QueueAction(open.Action, menus.MapOpen, menus.SpellOpen, menus.PlayerInformationOpen, open), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                var observed = runtime.GameRuntimeProductionCampaignSnapshot.value;
                var spell = Array.Find(observed.player_spells, row => row != null && row.menu_slot >= 2 &&
                    row.acquired && row.menu_help != null && row.menu_help.is_standard && row.unlocked_tier == 2 &&
                    !string.IsNullOrEmpty(row.holder_actor));
                Assert.That(spell, Is.Not.Null, "the controlled carry has a current, acquired upgraded source holder");
                var menu = (SourceSpellMenu)typeof(MagicCarpetRuntime).GetField("gameRuntimeSourceSpellMenu", flags).GetValue(runtime);
                gui.DrawRuntime = () => typeof(MagicCarpetRuntime).GetMethod("OnGUI", flags).Invoke(runtime, null);
                gui.minSize = new Vector2(640, 480); gui.position = new Rect(0, 0, 640, 480);
                gui.Show(); shown = true; gui.position = new Rect(0, 0, 640, 480);
                var grid = SourceSpellMenu.GridPointer(spell.menu_slot);
                gui.SendEvent(new Event { type = EventType.MouseMove, mousePosition = grid });
                var waitUntil = Time.realtimeSinceStartupAsDouble + 3;
                var warp = typeof(SourceSpellMenu).GetField("warpPending", flags);
                while ((bool)warp.GetValue(menu))
                {
                    Assert.That(Time.realtimeSinceStartupAsDouble, Is.LessThan(waitUntil), "actual source cursor warp completes");
                    gui.SendEvent(new Event { type = EventType.Repaint, mousePosition = grid });
                    UnityEditor.EditorApplication.QueuePlayerLoopUpdate();
                    yield return null;
                }
                gui.SendEvent(new Event { type = EventType.MouseMove, mousePosition = grid });
                gui.SendEvent(new Event { type = EventType.MouseDown, mousePosition = grid, button = 0 });
                Assert.That(typeof(SourceSpellMenu).GetField("choice", flags).GetValue(menu), Is.Not.Null,
                    "the real source pointer admits the current acquired holder");
                waitUntil = Time.realtimeSinceStartupAsDouble + 3;
                while ((bool)warp.GetValue(menu))
                {
                    Assert.That(Time.realtimeSinceStartupAsDouble, Is.LessThan(waitUntil), "actual tier cursor warp completes");
                    gui.SendEvent(new Event { type = EventType.Repaint, mousePosition = grid });
                    UnityEditor.EditorApplication.QueuePlayerLoopUpdate();
                    yield return null;
                }
                var tierWidth = menu.TierExtent.x;
                var start = MagicCarpetRuntime.SpellTierMenuStartX(spell.menu_slot, 13, 8, 48, 3, tierWidth);
                var physical = (float)typeof(SourceSpellMenu).GetField("physicalX", flags).GetValue(menu);
                var current = (float)typeof(SourceSpellMenu).GetField("virtualX", flags).GetValue(menu);
                var target = new Vector2(physical + start + 1.5f * tierWidth - current, grid.y);
                gui.SendEvent(new Event { type = EventType.MouseDrag, mousePosition = target, button = 0 });
                Assert.That(menu.HoveredTier, Is.EqualTo(1), "the admitted source drag selects tier1");
                gui.SendEvent(new Event { type = EventType.MouseUp, mousePosition = target, button = 0 });
                runtime.QualificationDriveGameRuntimeFrames(1);
                var after = Array.Find(runtime.GameRuntimeProductionCampaignSnapshot.value.player_spells,
                    row => row?.id == spell.id);
                Assert.That((after.holder_actor, after.menu_help.selected_tier), Is.EqualTo((spell.holder_actor, (sbyte)1)),
                    "the ordinary accepted assignment retains the actual holder and source tier");
            }
            finally
            {
                gui.DrawRuntime = null;
                if (shown) gui.Close();
                if (gui != null) UnityEngine.Object.DestroyImmediate(gui);
                runtime.QualificationDisposeGameRuntimeProduction();
                UnityEngine.Object.DestroyImmediate(root);
                Directory.Delete(temporary, true);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
        }

        private static System.Xml.XmlDocument ReadReplayDocument(byte[] bytes)
        {
            using var reader = JsonReaderWriterFactory.CreateJsonReader(bytes,
                System.Xml.XmlDictionaryReaderQuotas.Max);
            var document = new System.Xml.XmlDocument();
            document.Load(reader);
            return document;
        }

        [UnityEngine.TestTools.UnityTest]
        public System.Collections.IEnumerator SourceNamedSlotsRecoverThroughTheirRealModalAnimation()
        {
            var activeBefore = NativeSession.ActiveSessionCount;
            var temporary = Path.Combine(Path.GetTempPath(), "mc-slot-gui-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(temporary);
            var save = Path.Combine(temporary, "campaign.json");
            var root = new GameObject("source named slot modal owner");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
            var update = typeof(MagicCarpetRuntime).GetMethod("Update", flags);
            var gui = ScriptableObject.CreateInstance<CampaignGuiWindow>();
            try
            {
                var generated = Path.Combine(PrivateRoot, "generated");
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(new[] {
                    "-mc-game-runtime", "-mc-runtime-menu",
                    "-mc-runtime-loading-cache", Path.Combine(generated, "mc2-level1-night-cache-v2"),
                    "-mc-runtime-menu-cache", CampaignMenuCache,
                    "-mc-runtime-media-cache", Path.Combine(generated, "mc2-campaign-media-cache-decoded"),
                    "-mc-runtime-replay-save", save,
                    "-mc-runtime-package", LevelOnePackage,
                    "-mc-runtime-package", LevelTwoPackage,
                    "-mc-runtime-package", LevelThreePackage });
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.NewGame), Is.True);
                var before = JsonUtility.ToJson(runtime.GameRuntimeProductionIdentity);
                var payload = runtime.QualificationSaveGameRuntimeProductionReplay();
                SourceCampaignSaveSlots.Write(save, 1, "ORIGINAL", payload);
                var slotOne = SourceCampaignSaveSlots.PathFor(save, 1);
                var original = File.ReadAllBytes(slotOne);
                Directory.CreateDirectory(SourceCampaignSaveSlots.PathFor(save, 2));
                var badHeader = SourceCampaignSaveSlots.PathFor(save, 3);
                File.WriteAllBytes(badHeader, new byte[] { 1, 2, 3, 4 });
                SourceCampaignSaveSlots.Write(save, 4, "BROKEN", Encoding.UTF8.GetBytes("{}"));
                var menu = (CampaignMenuPresenter)typeof(MagicCarpetRuntime).GetField("gameRuntimeMenu", flags).GetValue(runtime);
                var cache = menu.LocaleCache;
                SourceScrollDialog Dialog() => (SourceScrollDialog)typeof(CampaignMenuPresenter).GetField("scrollDialog", flags).GetValue(menu);
                string Status() => (string)typeof(MagicCarpetRuntime).GetField("gameRuntimeMenuStatus", flags).GetValue(runtime);
                int Height(SourceScrollDialog dialog) => (int)typeof(SourceScrollDialog).GetField("height", flags).GetValue(dialog);
                int Closing(SourceScrollDialog dialog) => (int)typeof(SourceScrollDialog).GetField("closeResult", flags).GetValue(dialog);
                int Selected(SourceScrollDialog dialog) => (int)typeof(SourceScrollDialog).GetField("selected", flags).GetValue(dialog);
                string Draft(SourceScrollDialog dialog) => (string)typeof(SourceScrollDialog).GetField("draft", flags).GetValue(dialog);
                var view = new Rect();
                var origin = Vector2.zero;
                gui.DrawRuntime = () => {
                    view = cache.SourceView(runtime.GameRuntimeProductionIdentity.campaign_menu);
                    origin = GUIUtility.GUIToScreenPoint(Vector2.zero) - gui.position.position;
                    typeof(MagicCarpetRuntime).GetMethod("OnGUI", flags).Invoke(runtime, null);
                };
                gui.minSize = new Vector2(640, 480);
                gui.position = new Rect(0, 0, 640, 480);
                gui.Show();
                gui.position = new Rect(0, 0, 640, 480);
                void Click(float x, float y)
                {
                    var pointer = view.position + new Vector2(x, y) * (view.width / 640f) + origin;
                    gui.SendEvent(new Event { type = EventType.MouseDown, button = 0, mousePosition = pointer });
                    gui.SendEvent(new Event { type = EventType.MouseUp, button = 0, mousePosition = pointer });
                }
                void Key(KeyCode key, char character = '\0') =>
                    gui.SendEvent(new Event { type = EventType.KeyDown, keyCode = key, character = character });
                void Name(string name)
                {
                    foreach (var letter in name)
                        Key((KeyCode)((int)KeyCode.A + letter - 'A'), letter);
                }
                System.Collections.IEnumerator WaitFor(Func<bool> ready, string reason)
                {
                    var deadline = Time.realtimeSinceStartupAsDouble + 10;
                    do
                    {
                        gui.SendEvent(new Event { type = EventType.Repaint });
                        update.Invoke(runtime, null);
                        if (ready()) yield break;
                        Assert.That(Time.realtimeSinceStartupAsDouble, Is.LessThan(deadline), reason);
                        // Repaint alone can leave the EditMode engine idle.
                        // Request its ordinary player loop, without changing source clocks.
                        UnityEditor.EditorApplication.QueuePlayerLoopUpdate();
                        yield return null;
                    } while (true);
                }
                System.Collections.IEnumerator Open(int action)
                {
                    var control = Array.Find(cache.Campaign.Controls, entry => entry.source_action == action);
                    Click(control.x + 20, control.y + 20);
                    yield return WaitFor(() => Dialog() != null && Height(Dialog()) >= 200,
                        "the normal source scroll must open through real presentation frames");
                }
                yield return WaitFor(() => cache.Campaign.Motion.Ready && !cache.Campaign.Motion.Travelling,
                    "the normal campaign map must finish its source entry before input");
                yield return Open(0x25f800);
                Click(69, 100);
                Name("CANCELLED");
                Key(KeyCode.F5);
                typeof(MagicCarpetRuntime).GetMethod("ApplyGameRuntimeReplaySaveInput", flags).Invoke(runtime, new object[] { true });
                typeof(MagicCarpetRuntime).GetMethod("FixedUpdateGameRuntimeCampaign", flags).Invoke(runtime, null);
                Assert.That(File.Exists(save), Is.False, "the real named-slot modal owns F5");
                Key(KeyCode.Escape);
                yield return WaitFor(() => Dialog() == null, "source cancel must finish its closing animation");
                CollectionAssert.AreEqual(original, File.ReadAllBytes(slotOne), "draft cancellation leaves the occupied slot intact");

                yield return Open(0x25f800);
                Click(69, 116);
                Name("FAILED");
                var failedSaveDialog = Dialog();
                Key(KeyCode.Return);
                yield return WaitFor(() => Status()?.Contains("Could not save") == true &&
                    ReferenceEquals(Dialog(), failedSaveDialog) && Closing(Dialog()) == 0 && Height(Dialog()) >= 200,
                    "failed publication must reopen the same source dialog normally");
                Assert.That(Selected(Dialog()), Is.EqualTo(2));
                Assert.That(Draft(Dialog()), Is.EqualTo("FAILED"));
                CollectionAssert.AreEqual(original, File.ReadAllBytes(slotOne));
                Assert.That(JsonUtility.ToJson(runtime.GameRuntimeProductionIdentity), Is.EqualTo(before));
                Key(KeyCode.Escape);
                yield return WaitFor(() => Dialog() == null, "failed save remains cancellable");

                foreach (var slot in new[] { 3, 4 })
                {
                    yield return Open(0x25f820);
                    Click(550, 60 + 16 + 16 * slot + 8);
                    var failedLoadDialog = Dialog();
                    var label = Draft(failedLoadDialog);
                    Key(KeyCode.Return);
                    yield return WaitFor(() => Status()?.Contains("Could not load") == true &&
                        ReferenceEquals(Dialog(), failedLoadDialog) && Closing(Dialog()) == 0 && Height(Dialog()) >= 200,
                        "invalid header or native payload must reopen the source Load dialog");
                    Assert.That(Selected(Dialog()), Is.EqualTo(slot));
                    Assert.That(Draft(Dialog()), Is.EqualTo(label));
                    Assert.That(JsonUtility.ToJson(runtime.GameRuntimeProductionIdentity), Is.EqualTo(before));
                    Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore + 1));
                    CollectionAssert.AreEqual(original, File.ReadAllBytes(slotOne));
                    Key(KeyCode.Escape);
                    yield return WaitFor(() => Dialog() == null, "failed load remains cancellable");
                }
                CollectionAssert.AreEqual(new byte[] { 1, 2, 3, 4 }, File.ReadAllBytes(badHeader));
                yield return Open(0x25f800);
                Click(69, 100);
                Name("REPLACED");
                Key(KeyCode.Return);
                yield return WaitFor(() => Status() == "Game saved." && Dialog() == null && menu.CanQuickSave,
                    "the source final confirmation must publish and release the completed dialog");
                Assert.That(SourceCampaignSaveSlots.Scan(save)[0].Label, Is.EqualTo("REPLACED"));
                Assert.That(JsonUtility.ToJson(runtime.GameRuntimeProductionIdentity), Is.EqualTo(before));
                var native = (NativeSession)typeof(MagicCarpetRuntime).GetField("session", flags).GetValue(runtime);
                using var cold = NativeSession.OpenGameRuntimeReplayFrom(native, SourceCampaignSaveSlots.Read(save, 1), ResolverPackages());
                Assert.That(JsonUtility.ToJson(cold.GameRuntimeIdentity().campaign_menu),
                    Is.EqualTo(JsonUtility.ToJson(runtime.GameRuntimeProductionIdentity.campaign_menu)));
            }
            finally
            {
                gui.DrawRuntime = null;
                gui.Close();
                if (gui != null) UnityEngine.Object.DestroyImmediate(gui);
                runtime.QualificationDisposeGameRuntimeProduction();
                UnityEngine.Object.DestroyImmediate(root);
                Directory.Delete(temporary, true);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
        }

        [Test]
        public void ProductionCampaignMapSaveRestoresWithoutGameplayOwner()
        {
            var generated = Path.Combine(PrivateRoot, "generated");
            var packages = new[] {
                Package(Path.Combine(generated, "mc-bhq8-level1-collision-v5-night")),
                Package(Path.Combine(generated, "mc-level2-vault-successor-closure-20261025")),
                Package(Path.Combine(generated, "level3-day-block32-campaign-recipe-v2")),
                Package(Path.Combine(generated, "mc-bhq6-ordinary-20261025-zclu5fjj/level4"))
            };
            using (var startup = NativeSession.OpenGameRuntimeFrontend(packages))
            {
                var title = startup.GameRuntimeIdentity();
                GameRuntimeTransport.MediaCommand(ulong.Parse(title.runtime_id), title,
                    "{\"operation\":\"install\",\"startup\":true,\"catalog\":" +
                    File.ReadAllText(Path.Combine(generated, "mc2-frontend-media-cache-decoded/media.json")) + "}");
                var media = startup.GameRuntimeIdentity().frontend_media;
                Assert.That(media, Is.Not.Null);
                Assert.That(media.active, Is.True, "actual startup producer remains decodable");
            }
            using var frontend = NativeSession.OpenGameRuntimeFrontend(packages);
            var titleBeforeSave = frontend.GameRuntimeIdentity();
            var titleSave = frontend.GameRuntimeSaveReplay();
            Assert.That(frontend.GameRuntimeIdentity().mode, Is.EqualTo(GameRuntimeTransport.TitleMode));
            Assert.That(frontend.GameRuntimeIdentity().generation, Is.EqualTo(titleBeforeSave.generation));
            using (var titleLoad = NativeSession.OpenGameRuntimeReplayFrom(frontend, titleSave, packages))
            {
                Assert.That(titleLoad.GameRuntimeIdentity().mode, Is.EqualTo(GameRuntimeTransport.CampaignMode),
                    "the original title Save stores the retained campaign and loads at its map");
                Assert.That(titleLoad.GameRuntimeIdentity().session, Is.Null);
            }
            var map = frontend.GameRuntimeBeginNewGame();
            Assert.That(map.mode, Is.EqualTo(GameRuntimeTransport.CampaignMode));
            var saved = frontend.GameRuntimeSaveReplay();
            var slotDirectory = Path.Combine(Path.GetTempPath(), "mc-source-slots-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(slotDirectory);
            byte[] slotReplay;
            try
            {
                var quicksave = Path.Combine(slotDirectory, "quicksave.json");
                File.WriteAllBytes(quicksave, saved);
                for (var index = 1; index <= SourceCampaignSaveSlots.Count; index++)
                {
                    SourceCampaignSaveSlots.Write(quicksave, index, "SLOT " + index, saved);
                    CollectionAssert.AreEqual(saved, SourceCampaignSaveSlots.Read(quicksave, index),
                        "slot metadata must not change Rust's receipt-qualified replay");
                }
                SourceCampaignSaveSlots.Write(quicksave, 8, "REPLACED", saved);
                var entries = SourceCampaignSaveSlots.Scan(quicksave);
                Assert.That(entries, Has.Length.EqualTo(8));
                Assert.That(Array.TrueForAll(entries, entry => entry.Present), Is.True);
                Assert.That(entries[7].Label, Is.EqualTo("REPLACED"));
                CollectionAssert.AreEqual(saved, File.ReadAllBytes(quicksave), "named slots cannot overwrite the F5 file");
                Assert.Throws<InvalidDataException>(() => SourceCampaignSaveSlots.Write(quicksave, 1, "INVALID!", saved));
                Assert.Throws<ArgumentOutOfRangeException>(() => SourceCampaignSaveSlots.Read(quicksave, 9));
                slotReplay = SourceCampaignSaveSlots.Read(quicksave, 8);
            }
            finally { Directory.Delete(slotDirectory, true); }
            var loaded = NativeSession.OpenGameRuntimeReplayFrom(frontend, slotReplay, packages);
            var root = new GameObject("campaign map save attachment");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                typeof(MagicCarpetRuntime).GetMethod("AttachAndValidateGameRuntimeProductionSession",
                    System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)
                    .Invoke(runtime, new object[] { loaded, false });
                var restored = runtime.GameRuntimeProductionIdentity;
                Assert.That(restored.mode, Is.EqualTo(GameRuntimeTransport.CampaignMode));
                Assert.That(restored.session, Is.Null);
                Assert.That(restored.campaign_menu.selected_level_id, Is.EqualTo(map.campaign_menu.selected_level_id));
                CollectionAssert.AreEqual(map.campaign_menu.available_levels, restored.campaign_menu.available_levels);
                Assert.That(runtime.GameRuntimeProductionStartupReady, Is.False);
                var loading = loaded.GameRuntimeCampaignMenuAction("start_mission");
                Assert.That(GameRuntimeTransport.ReadLoadingPresentation(loading).level_id,
                    Is.EqualTo(restored.campaign_menu.selected_level_id),
                    "the loading observation preserves its embedded identity too");
            }
            finally
            {
                runtime.QualificationDisposeGameRuntimeProduction();
                loaded.Dispose();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void PortalClockPreservesSourceOvershootAndDoesNotCatchUp()
        {
            var state = new CampaignMenuPresenter.PortalState { Selector = 83 };
            Assert.That(state.Advance(7, 70, 83, 8), Is.False);
            Assert.That(state.DrawSelector, Is.EqualTo(83));
            Assert.That(state.Advance(8, 70, 83, 8), Is.False);
            Assert.That(state.DrawSelector, Is.EqualTo(84));
            Assert.That(state.Advance(16, 70, 83, 8), Is.True);
            Assert.That(state.DrawSelector, Is.EqualTo(70));
            state.Selector = 33;
            Assert.That(state.Advance(1000, 33, 35, 8), Is.False);
            Assert.That(state.DrawSelector, Is.EqualTo(34), "a delayed frame advances once");
        }

        [Test]
        public void FreshMenuV2BindsOwnerBriefingAndRejectsAlteredLanguage()
        {
            var source = Environment.GetEnvironmentVariable("MC_MENU_CACHE_V2") ??
                Path.Combine(PrivateRoot, "generated/mc-zdo5-source-title-pause-20261026");
            Assert.That(File.ReadAllText(Path.Combine(source, "manifest.json")),
                Does.Contain("mc2-owner-campaign-menu-cache/v2"));
            using (var cache = new OwnerCampaignMenuCache(source))
            {
                var language = Encoding.GetEncoding(28591).GetString(File.ReadAllBytes(
                    Path.Combine(LevelOneOwnerCache, "font/language-l1.records-nul.bin"))).Split('\0');
                Assert.That(cache.ForMission("level.magic_carpet_2.night_spire").briefing, Is.EqualTo(language[23]));
                Assert.That(cache.ForMission("level.magic_carpet_2.spider_cave").briefing, Is.EqualTo(language[26]));
                Assert.That(cache.StartupBackground.width, Is.EqualTo(640));
                Assert.That(cache.Startup.animations.Length, Is.EqualTo(4));
                Assert.That(cache.Startup.font.glyph_mapping, Is.EqualTo("mc2-hscreen-bank4-byte-controls/v1"));
                Assert.That(cache.Startup.font.glyphs[65].width, Is.EqualTo(7));
                Assert.That(cache.Startup.font.glyphs[65].height, Is.EqualTo(7));
                Assert.That(cache.Startup.help.Length, Is.EqualTo(9));
                foreach (var entry in cache.Startup.buttons)
                {
                    var help = Array.Find(cache.Startup.help, row => row.help_key == entry.help_key);
                    Assert.That(cache.StartupHoverText(new Vector2(entry.x + entry.width / 2f,
                        entry.y + entry.height / 2f)), Is.EqualTo(language[help.text_selector]),
                        "every title icon uses its original source callout record");
                }
                Assert.That(cache.StartupHoverText(new Vector2(639, 479)), Is.Null);
                Assert.That(cache.PortalBitmap(84), Is.Not.Null, "source opening overshoot is retained");
                Assert.That(cache.ReadPortalOpeningSound().Length, Is.GreaterThan(44));
                var first = "level.magic_carpet_2.night_spire";
                var menu = new GameRuntimeTransport.CampaignMenuSnapshot {
                    phase = "mission_select", selected_level_id = first,
                    available_levels = new[] { first }, completed_levels = Array.Empty<string>()
                };
                var view = cache.SourceView(menu);
                Assert.That(cache.HitTestPortal(menu, view.position + new Vector2(305, 343) * (view.width / 640)), Is.EqualTo(first));
                Assert.That(cache.HitTestPortal(menu, view.position + new Vector2(303, 343) * (view.width / 640)), Is.Null);
            }
            var fixture = Path.Combine(Path.GetTempPath(), "mc-menu-v2-tamper-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(fixture);
            try
            {
                CopyOwnerMenuFixture(source, fixture);
                var records = Directory.GetFiles(Path.Combine(fixture, "language/text"), "*.records-nul.u8");
                Assert.That(records.Length, Is.GreaterThan(0));
                using (var stream = File.Open(records[0], FileMode.Open, FileAccess.ReadWrite))
                {
                    var first = stream.ReadByte();
                    stream.Position = 0;
                    stream.WriteByte((byte)(first ^ 1));
                }
                Assert.Throws<InvalidDataException>(() => new OwnerCampaignMenuCache(fixture));
            }
            finally { Directory.Delete(fixture, true); }
        }

        [Test]
        public void CampaignResultsAndMissionSelectionKeepLoadingAndGameplayUnavailable()
        {
            const string first = "level.magic_carpet_2.night_spire";
            const string second = "level.magic_carpet_2.payahandra_ruins";
            const string third = "level.magic_carpet_2.revealed_way";
            var identity = new GameRuntimeTransport.Identity {
                runtime_contract = GameRuntimeTransport.RuntimeContract, runtime_id = "771", generation = "4",
                display_settings = new GameRuntimeTransport.DisplaySettings { minimap = true, top_bar = true },
                mode = GameRuntimeTransport.CampaignMode,
                active_runtime_flow = new GameRuntimeTransport.RuntimeFlowDefinition {
                    game_id = "game.magic_carpet_2", bound_level_id = first, entry_level_id = first,
                    entries = new[] {
                        new GameRuntimeTransport.RuntimeFlowEntry { level_id = first, completion_destination = new GameRuntimeTransport.RuntimeFlowDestination { kind = "level", level_id = second } },
                        new GameRuntimeTransport.RuntimeFlowEntry { level_id = second, completion_destination = new GameRuntimeTransport.RuntimeFlowDestination { kind = "level", level_id = third } },
                        new GameRuntimeTransport.RuntimeFlowEntry { level_id = third, completion_destination = new GameRuntimeTransport.RuntimeFlowDestination { kind = "title", node_id = "flow.magic_carpet_2.return_to_title" } }
                    }
                },
                presentation = new GameRuntimeTransport.RuntimePresentationCue {
                    cue_id = "presentation.magic_carpet_2.night_spire.completion", phase = GameRuntimeTransport.LevelCompletionPresentationPhase,
                    level_id = first, completion_destination = new GameRuntimeTransport.RuntimeFlowDestination { kind = "level", level_id = second }
                },
                campaign_menu = new GameRuntimeTransport.CampaignMenuSnapshot {
                    phase = "results", completed_level_id = first, completed_levels = new[] { first },
                    available_levels = new[] { first, second }, briefing_level_id = second
                }
            };
            Assert.DoesNotThrow(() => GameRuntimeTransport.ValidateIdentity(identity, 771));
            // The transport must retain input ownership after a media frame
            // disappears, before the Rust owner exposes usable results.
            identity.frontend_media = DataContractJsonRoundTrip(new GameRuntimeTransport.MediaSnapshot {
                contract = "mc2-frontend-media/v1", active = false,
                campaign_return = new GameRuntimeTransport.CampaignReturnMediaSnapshot {
                    phase = "input_release", blocking = true
                }
            });
            Assert.DoesNotThrow(() => GameRuntimeTransport.ValidateIdentity(identity, 771));
            Assert.That(GameRuntimeTransport.CampaignReturnMediaBlocksInput(identity), Is.True);
            var mediaReturn = identity.frontend_media.campaign_return;
            mediaReturn.phase = "credits";
            mediaReturn.credits = new GameRuntimeTransport.CreditsSnapshot {
                lines = Array.Empty<string>(), cursor = 117
            };
            Assert.DoesNotThrow(() => GameRuntimeTransport.ValidateIdentity(identity, 771));
            mediaReturn.credits.cursor = 118;
            Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateIdentity(identity, 771));
            mediaReturn.credits = null;
            mediaReturn.phase = "cutscene";
            mediaReturn.cutscene_id = 7;
            Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateIdentity(identity, 771));
            identity.frontend_media = null;
            identity.campaign_menu.selected_level_id = second;
            Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateIdentity(identity, 771),
                "Results have not yet selected a mission on the campaign map");
            identity.campaign_menu.selected_level_id = null;
            Assert.That(CampaignMenuPresenter.Allows(identity.mode, CampaignMenuPresenter.Action.NewGame, false, false, false), Is.False);
            identity.mode = GameRuntimeTransport.LoadingMode;
            Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateIdentity(identity, 771));
            identity.mode = GameRuntimeTransport.CampaignMode;
            identity.campaign_menu.phase = "mission_select";
            identity.campaign_menu.selected_level_id = second;
            identity.presentation = new GameRuntimeTransport.RuntimePresentationCue {
                cue_id = "presentation.campaign.map", phase = GameRuntimeTransport.CampaignMapPresentationPhase, level_id = second
            };
            Assert.DoesNotThrow(() => GameRuntimeTransport.ValidateIdentity(identity, 771));
            identity.campaign_menu.selected_level_id = third;
            Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateIdentity(identity, 771));
        }

        [Test]
        public void MissingCampaignMapMediaReportsOnceWithoutInterruptingTheMenu()
        {
            var root = new GameObject("Missing campaign map media");
            var missing = Path.Combine(PrivateRoot, "generated", "missing-media-" + Guid.NewGuid().ToString("N"));
            using var presenter = new FrontendMediaPresenter(root.transform, missing, false);
            try
            {
                UnityEngine.TestTools.LogAssert.Expect(LogType.Error,
                    new System.Text.RegularExpressions.Regex("^MC_FRONTEND_MEDIA_ERROR "));
                Assert.DoesNotThrow(presenter.StartMapMusic,
                    "optional missing media must leave the campaign map usable");
                Assert.That(presenter.Error, Is.Not.Null.And.Not.Empty);
                Assert.That(presenter.Visible, Is.False);
                var error = presenter.Error;
                Assert.DoesNotThrow(presenter.StartMapMusic,
                    "the next map frame must not retry or log the same broken cache");
                Assert.That(presenter.Error, Is.EqualTo(error));
                UnityEngine.TestTools.LogAssert.NoUnexpectedReceived();
            }
            finally { UnityEngine.Object.DestroyImmediate(root); }
        }

        [DllImport("libc", EntryPoint = "symlink", SetLastError = true)]
        private static extern int LinkMediaCacheFixture(string source, string destination);

        [DllImport("libc", EntryPoint = "unlink", SetLastError = true)]
        private static extern int UnlinkMediaCacheFixture(string path);

        [TestCase(false)]
        [TestCase(true)]
        [Platform("Linux")]
        public void FailedTitleMediaRetriesOnceAtTheCampaignReturnAdapterBoundary(bool repairCache)
        {
            // The native owner here remains at Title. Only the adapter's
            // return notification is a DTO fixture; native ending selection
            // and held-input draining are covered by the Rust owner checks.
            var source = Environment.GetEnvironmentVariable("MC_ENDING_MEDIA_CACHE") ??
                Path.Combine(PrivateRoot, "generated/ending-media-20261112-owner-v1");
            Assert.That(File.Exists(Path.Combine(source, "media.json")), Is.True);
            var fixture = Path.Combine(PrivateRoot, "generated", "media-retry-" + Guid.NewGuid().ToString("N"));
            var root = new GameObject("Frontend media retry adapter");
            var linked = false;
            using var runtime = new GameRuntimeTransport();
            using var presenter = new FrontendMediaPresenter(root.transform, fixture, false);
            try
            {
                UnityEngine.TestTools.LogAssert.Expect(LogType.Error,
                    new System.Text.RegularExpressions.Regex("^MC_FRONTEND_MEDIA_ERROR "));
                Assert.That(presenter.Step(runtime.Handle, runtime.Current, 0, false, false), Is.True);
                Assert.That(presenter.Error, Is.Not.Null.And.Not.Empty);
                Assert.That(runtime.IdentitySnapshot().mode, Is.EqualTo(GameRuntimeTransport.TitleMode));
                Assert.DoesNotThrow(() => presenter.Step(runtime.Handle, runtime.Current, 0, false, false),
                    "an absent optional Title cache is not retried each frame");
                if (repairCache)
                {
                    Assert.That(LinkMediaCacheFixture(source, fixture), Is.Zero,
                        "link the genuine owner cache without copying its large frame payloads");
                    linked = true;
                }
                var returnNotification = DataContractJsonRoundTrip(runtime.IdentitySnapshot());
                returnNotification.frontend_media.campaign_return = new GameRuntimeTransport.CampaignReturnMediaSnapshot {
                    phase = "cutscene", blocking = true, cutscene_id = 6
                };
                if (!repairCache)
                    UnityEngine.TestTools.LogAssert.Expect(LogType.Error,
                        new System.Text.RegularExpressions.Regex("^MC_FRONTEND_MEDIA_ERROR "));
                else
                    UnityEngine.TestTools.LogAssert.Expect(LogType.Log,
                        "MC_FRONTEND_MEDIA_AUDIO op=M id=4 asset=audio/music-4.wav");
                Assert.That(presenter.Step(runtime.Handle, returnNotification, 0, true, true), Is.True);
                if (repairCache)
                {
                    Assert.That(presenter.Error, Is.Null, "a successful retry clears the adapter failure");
                    var media = runtime.IdentitySnapshot().frontend_media;
                    Assert.That(Array.Exists(media.actions, action => action.op == "M" && action.value == 4), Is.True,
                        "the real Title owner accepted the repaired catalog and emitted menu music");
                    Assert.That(media.campaign_return, Is.Null, "the DTO fixture never manufactures a native ending");
                }
                else
                {
                    Assert.That(presenter.Error, Is.Not.Null.And.Not.Empty);
                    Assert.DoesNotThrow(() => presenter.Step(runtime.Handle, returnNotification, 0, false, false),
                        "a failed return retry must not become a repeated installation loop");
                }
                UnityEngine.TestTools.LogAssert.NoUnexpectedReceived();
            }
            finally
            {
                if (linked) Assert.That(UnlinkMediaCacheFixture(fixture), Is.Zero,
                    "remove only the temporary link; preserve the genuine cache");
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void OriginalCreditsComposeTheFirstPageFromOwnerArt()
        {
            var source = Environment.GetEnvironmentVariable("MC_ENDING_MEDIA_CACHE") ??
                Path.Combine(PrivateRoot, "generated/ending-media-20261112-owner-v1");
            var root = new GameObject("Original credits texture");
            using var presenter = new FrontendMediaPresenter(root.transform, source, false);
            const System.Reflection.BindingFlags flags = System.Reflection.BindingFlags.Instance |
                System.Reflection.BindingFlags.NonPublic;
            var type = typeof(FrontendMediaPresenter);
            try
            {
                type.GetMethod("LoadCatalog", flags).Invoke(presenter, null);
                var page = new GameRuntimeTransport.CreditsSnapshot {
                    cursor = 0, lines = new[] { "Designed By", "Bullfrog Productions Ltd" }
                };
                var view = new GameRuntimeTransport.MediaSnapshot { fade_in_level = 31 };
                type.GetField("current", flags).SetValue(presenter, view);
                var compose = type.GetMethod("ComposeCredits", flags);
                compose.Invoke(presenter, new object[] { page });
                var texture = (Texture2D)type.GetField("creditsTexture", flags).GetValue(presenter);
                Assert.That((texture.width, texture.height), Is.EqualTo((640, 480)));
                var visible = texture.GetPixels32();
                view.fade_in_level = 0;
                compose.Invoke(presenter, new object[] { page });
                var black = texture.GetPixels32();
                var textPixels = 0;
                var coloredPixels = 0;
                for (var i = 0; i < visible.Length; i++)
                {
                    var color = visible[i];
                    if (color.r != 0 || color.g != 0 || color.b != 0) coloredPixels++;
                    var sourceY = 479 - i / 640;
                    if (sourceY >= 200 && sourceY < 230 && color.r >= 236 &&
                        color.g >= 236 && color.b >= 236) textPixels++;
                    Assert.That(black[i].r | black[i].g | black[i].b, Is.Zero);
                }
                Assert.That(coloredPixels, Is.GreaterThan(10_000));
                Assert.That(textPixels, Is.GreaterThan(50));
            }
            finally { UnityEngine.Object.DestroyImmediate(root); }
        }

        [Test]
        public void CompletionDispositionRequiresExactResultsOrReplayMapPresentation()
        {
            // Contract fixtures only: these do not establish an earned
            // completion or choose the Rust-owned replay disposition.
            var source = "level.magic_carpet_2.night_spire";
            var next = "level.magic_carpet_2.payahandra_ruins";
            var destination = new GameRuntimeTransport.RuntimeFlowDestination {
                kind = "level", level_id = next
            };
            var completion = new GameRuntimeTransport.RuntimePresentationCue {
                cue_id = "presentation.magic_carpet_2.night_spire.completion",
                phase = GameRuntimeTransport.LevelCompletionPresentationPhase,
                level_id = source, completion_destination = destination
            };
            var map = new GameRuntimeTransport.RuntimePresentationCue {
                cue_id = "presentation.campaign.map",
                phase = GameRuntimeTransport.CampaignMapPresentationPhase,
                level_id = source
            };
            var receipt = new GameRuntimeTransport.CompletionTransitionReceipt {
                receipt_contract = GameRuntimeTransport.CompletionTransitionReceiptContract,
                completed_level_id = source, completion_destination = destination,
                disposition = GameRuntimeTransport.ResultsCompletionDisposition,
                after = new GameRuntimeTransport.Identity {
                    mode = GameRuntimeTransport.CampaignMode,
                    campaign_menu = new GameRuntimeTransport.CampaignMenuSnapshot {
                        phase = "results", completed_level_id = source
                    },
                    presentation = completion
                },
                presentation_sequence = new[] { completion, completion }
            };
            receipt = DataContractJsonRoundTrip(receipt);
            Assert.That(receipt.disposition, Is.EqualTo("results"));
            Assert.DoesNotThrow(() => GameRuntimeTransport.ValidateCompletionPresentationSequence(receipt));
            receipt.disposition = GameRuntimeTransport.ReplayMapCompletionDisposition;
            Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateCompletionPresentationSequence(receipt),
                "a replay disposition cannot keep Results or its success sequence");
            receipt.after.campaign_menu.phase = "mission_select";
            receipt.after.campaign_menu.selected_level_id = source;
            receipt.after.presentation = map;
            Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateCompletionPresentationSequence(receipt),
                "a replay map cannot add a level-completion cue");
            receipt.presentation_sequence = new[] { map };
            Assert.DoesNotThrow(() => GameRuntimeTransport.ValidateCompletionPresentationSequence(receipt));
            receipt.receipt_contract = "carpet-flight-completion-transition-receipt/v2";
            Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateCompletionPresentationSequence(receipt),
                "the old receipt contract cannot silently adopt the new replay disposition");
            receipt.receipt_contract = GameRuntimeTransport.CompletionTransitionReceiptContract;
            receipt.after.frontend_media = new GameRuntimeTransport.MediaSnapshot {
                campaign_return = new GameRuntimeTransport.CampaignReturnMediaSnapshot {
                    phase = "cutscene", blocking = true, cutscene_id = 1
                }
            };
            Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateCompletionPresentationSequence(receipt),
                "a replay cannot schedule campaign return media");
            receipt.after.frontend_media = null;
            receipt.disposition = GameRuntimeTransport.ResultsCompletionDisposition;
            Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateCompletionPresentationSequence(receipt),
                "an ordinary result cannot use the replay map sequence");
            receipt.disposition = null;
            Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateCompletionPresentationSequence(receipt),
                "the new disposition is required, never inferred from the destination");
            receipt.disposition = "map";
            Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateCompletionPresentationSequence(receipt),
                "unknown dispositions do not widen campaign return modes");
            receipt.disposition = GameRuntimeTransport.ReplayMapCompletionDisposition;
            receipt.after.presentation = new GameRuntimeTransport.RuntimePresentationCue {
                cue_id = GameRuntimeTransport.TitlePresentationCue,
                phase = GameRuntimeTransport.TitlePresentationPhase
            };
            receipt.presentation_sequence = new[] { receipt.after.presentation };
            Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateCompletionPresentationSequence(receipt),
                "the replay map disposition does not admit a Title shortcut");
        }

        [Test]
        public void CampaignIdentityAllowsOnlyDiscoveredSecretsAndKeepsOrdinaryCompletionsSeparate()
        {
            var identity = SecretCampaignMapIdentity();
            Assert.DoesNotThrow(() => GameRuntimeTransport.ValidateIdentity(identity, 771));

            identity.secret_campaign = null;
            Assert.Throws<InvalidOperationException>(
                () => GameRuntimeTransport.ValidateIdentity(identity, 771),
                "a catalog secret without discovered progress is not a selectable campaign choice");

            identity = SecretCampaignMapIdentity();
            var levels = SecretOrdinaryLevels();
            identity.campaign_menu.completed_levels = new[] {
                levels[0], levels[1], SecretDestinationLevel
            };
            Assert.Throws<InvalidOperationException>(
                () => GameRuntimeTransport.ValidateIdentity(identity, 771),
                "secret completions belong to secret_campaign, not the ordinary frontier");
        }

        private static GameRuntimeTransport.CompletedLevelStatistics SecretStatistics(int accuracy = 37) =>
            new GameRuntimeTransport.CompletedLevelStatistics {
                level_id = SecretDestinationLevel,
                statistics = new GameRuntimeTransport.CompletionStatistics {
                    spells_found_percent = 13, hit_accuracy_percent = accuracy,
                    creatures_killed_percent = 42, mana_collected_percent = 55,
                    time_taken_seconds = 219
                }
            };

        [Test]
        public void SecretStatisticsRequireAnEarnedUniqueBoundRowAndReachTheResultsLookup()
        {
            var identity = SecretCampaignMapIdentity();
            Assert.DoesNotThrow(() => GameRuntimeTransport.ValidateIdentity(identity, 771),
                "older identities may omit the new statistics field");
            var row = SecretStatistics();
            identity.secret_campaign.level_statistics = new[] { row };
            Assert.DoesNotThrow(() => GameRuntimeTransport.ValidateIdentity(identity, 771));
            var restored = DataContractJsonRoundTrip(identity);
            Assert.DoesNotThrow(() => GameRuntimeTransport.ValidateIdentity(restored, 771));
            Assert.That(GameRuntimeTransport.SameSecretCampaign(identity.secret_campaign, restored.secret_campaign), Is.True);
            Assert.That(CampaignMenuPresenter.CanShowStatistics(restored.campaign_menu, restored.secret_campaign, SecretDestinationLevel), Is.True);
            Assert.That(CampaignMenuPresenter.StatisticsForLevel(restored.campaign_menu, restored.secret_campaign, SecretDestinationLevel).hit_accuracy_percent, Is.EqualTo(37));
            Assert.That(CampaignMenuPresenter.StatisticsForLevel(restored.campaign_menu, restored.secret_campaign, SecretOriginLevel), Is.Null,
                "a secret row never substitutes for its ordinary parent");
            restored.secret_campaign.level_statistics[0].statistics.hit_accuracy_percent++;
            Assert.That(GameRuntimeTransport.SameSecretCampaign(identity.secret_campaign, restored.secret_campaign), Is.False,
                "save/transition equality observes changed result values");

            identity.secret_campaign.level_statistics = new[] { row, SecretStatistics() };
            Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateIdentity(identity, 771), "duplicate secret rows");
            identity.secret_campaign.level_statistics = new[] { SecretStatistics() };
            identity.secret_campaign.level_statistics[0].level_id = SecretOriginLevel;
            Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateIdentity(identity, 771), "ordinary row spliced into secret state");
            identity.secret_campaign.level_statistics = new[] { SecretStatistics() };
            identity.secret_campaign.completed_levels = Array.Empty<string>();
            Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateIdentity(identity, 771), "uncompleted secret row");
            identity.secret_campaign.completed_levels = new[] { SecretDestinationLevel };
            identity.secret_campaign.level_statistics[0].statistics.hit_accuracy_percent = 101;
            Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateIdentity(identity, 771), "invalid source percentage");
            identity.secret_campaign.level_statistics[0].statistics = null;
            Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateIdentity(identity, 771), "missing source statistics");
            identity.secret_campaign.level_statistics = new[] { SecretStatistics() };
            identity.secret_campaign.level_statistics[0].level_id = "level.magic_carpet_2.unrelated_secret";
            identity.secret_campaign.discovered_levels = new[] { SecretDestinationLevel, "level.magic_carpet_2.unrelated_secret" };
            identity.secret_campaign.completed_levels = new[] { SecretDestinationLevel, "level.magic_carpet_2.unrelated_secret" };
            Assert.Throws<InvalidOperationException>(() => GameRuntimeTransport.ValidateIdentity(identity, 771), "wrong secret outside this catalog");
        }

        [Test]
        public void SecretReceiptOnlyAllowsItsActualCompletedSecretRowToChange()
        {
            var success = SecretCompletedReturnReceipt();
            success.after.secret_campaign.level_statistics = new[] { SecretStatistics() };
            Assert.DoesNotThrow(() => SecretTransitionTransport.ValidateSecretTransition(
                DecodeBoundaryEnvelope(success), success.before, success.after));

            var replay = SecretCompletedReturnReceipt();
            replay.before.secret_campaign.completed_levels = new[] { SecretDestinationLevel };
            replay.before.secret_campaign.level_statistics = new[] { SecretStatistics(20) };
            replay.after.secret_campaign.level_statistics = new[] { SecretStatistics(80) };
            Assert.DoesNotThrow(() => SecretTransitionTransport.ValidateSecretTransition(
                DecodeBoundaryEnvelope(replay), replay.before, replay.after), "successful replay replaces that source row");

            var abandoned = SecretAbandonedReturnReceipt();
            abandoned.before.secret_campaign.completed_levels = new[] { SecretDestinationLevel };
            abandoned.after.secret_campaign.completed_levels = new[] { SecretDestinationLevel };
            abandoned.before.secret_campaign.level_statistics = new[] { SecretStatistics(20) };
            abandoned.after.secret_campaign.level_statistics = new[] { SecretStatistics(80) };
            Assert.Throws<InvalidDataException>(() => SecretTransitionTransport.ValidateSecretTransition(
                DecodeBoundaryEnvelope(abandoned), abandoned.before, abandoned.after), "abandonment cannot replace the prior row");
            abandoned.after.secret_campaign.level_statistics = null;
            Assert.Throws<InvalidDataException>(() => SecretTransitionTransport.ValidateSecretTransition(
                DecodeBoundaryEnvelope(abandoned), abandoned.before, abandoned.after), "abandonment cannot discard the prior row");

            var entry = SecretEntryReceipt();
            entry.before.secret_campaign = SecretProgress(null, completed: true);
            entry.after.secret_campaign = SecretProgress(SecretOriginLevel, completed: true);
            entry.before.secret_campaign.level_statistics = new[] { SecretStatistics(20) };
            entry.after.secret_campaign.level_statistics = new[] { SecretStatistics(80) };
            Assert.Throws<InvalidDataException>(() => SecretTransitionTransport.ValidateSecretTransition(
                DecodeBoundaryEnvelope(entry), entry.before, entry.after), "entry cannot rewrite an earned row");
        }

        [Test]
        public void SecretCampaignSnapshotBindsReturnOriginToLoadingAndPausedBoundSecret()
        {
            var loading = SecretLoadingIdentity(returnOrigin: null);
            Assert.DoesNotThrow(() => GameRuntimeTransport.ValidateIdentity(loading, 771),
                "a discovered secret may be Loading before its parent return frame is opened");
            loading.secret_campaign.return_origin = SecretOriginLevel;
            Assert.DoesNotThrow(() => GameRuntimeTransport.ValidateIdentity(loading, 771));
            loading.secret_campaign.return_origin = SecretOrdinaryLevels()[1];
            Assert.Throws<InvalidOperationException>(
                () => GameRuntimeTransport.ValidateIdentity(loading, 771));

            loading = SecretLoadingIdentity(returnOrigin: null);
            loading.secret_campaign = null;
            Assert.Throws<InvalidOperationException>(
                () => GameRuntimeTransport.ValidateIdentity(loading, 771),
                "a secret Loading identity must carry its discovered destination");

            var paused = SecretPlayingIdentity(
                GameRuntimeTransport.PausedMode, "32", SecretDestinationLevel,
                SecretSession("54", "18", 'c'),
                SecretProgress(SecretOriginLevel, completed: false));
            Assert.DoesNotThrow(() => GameRuntimeTransport.ValidateIdentity(paused, 771));

            // Production runtime identities are decoded with this serializer;
            // JsonUtility materializes absent nested DTOs as empty objects.
            var roundTrip = DataContractJsonRoundTrip(paused);
            Assert.That(roundTrip, Is.Not.Null,
                "the production identity serializer reconstructs the typed DTO");
            Assert.That(roundTrip.campaign_menu, Is.Null,
                "Rust omits campaign_menu outside Campaign mode");
            Assert.That(
                GameRuntimeTransport.SameSecretCampaign(
                    paused.secret_campaign, roundTrip.secret_campaign), Is.True,
                "the typed DTO preserves discovered, completed and return-origin save state");
            Assert.DoesNotThrow(() => GameRuntimeTransport.ValidateIdentity(roundTrip, 771));

            roundTrip.secret_campaign.return_origin = null;
            Assert.That(
                GameRuntimeTransport.SameSecretCampaign(
                    paused.secret_campaign, roundTrip.secret_campaign), Is.False,
                "save and pause guards compare the active parent return frame");
            Assert.Throws<InvalidOperationException>(
                () => GameRuntimeTransport.ValidateIdentity(roundTrip, 771),
                "Playing and Paused secret sessions require the exact parent return frame");
        }

        [Test]
        public void SecretBoundaryReceiptsBindEntryAndReturnAndRejectAmbiguity()
        {
            var ordinaryBoundary = new SecretTransitionTransport.AcceptedBoundaryReceipt {
                kind = SecretTransitionTransport.OrdinaryCompletionBoundaryKind
            };
            ordinaryBoundary.ordinary_completion =
                new GameRuntimeTransport.CompletionTransitionReceipt();
            var decodedOrdinary = SecretTransitionTransport.Decode(
                SerializeBoundaryEnvelope(ordinaryBoundary), 1024 * 1024);
            Assert.That(decodedOrdinary.kind,
                Is.EqualTo(SecretTransitionTransport.OrdinaryCompletionBoundaryKind));
            Assert.That(decodedOrdinary.ordinary_completion, Is.Not.Null);
            Assert.That(decodedOrdinary.secret_transition, Is.Null);
            Assert.That(decodedOrdinary.HasOrdinaryCompletionField, Is.True);
            Assert.That(decodedOrdinary.HasSecretTransitionField, Is.False);

            var entry = SecretEntryReceipt();
            GameRuntimeTransport.ValidateIdentity(entry.before, 771);
            GameRuntimeTransport.ValidateIdentity(entry.after, 771);
            var acceptedEntry = SecretTransitionTransport.ValidateSecretTransition(
                DecodeBoundaryEnvelope(entry), entry.before, entry.after);
            Assert.That(acceptedEntry.kind, Is.EqualTo(SecretTransitionTransport.EntryKind));
            Assert.That(acceptedEntry.after.secret_campaign.return_origin,
                Is.EqualTo(SecretOriginLevel));

            var completedReturn = SecretCompletedReturnReceipt();
            GameRuntimeTransport.ValidateIdentity(completedReturn.before, 771);
            GameRuntimeTransport.ValidateIdentity(completedReturn.after, 771);
            var acceptedReturn = SecretTransitionTransport.ValidateSecretTransition(
                DecodeBoundaryEnvelope(completedReturn),
                completedReturn.before, completedReturn.after);
            Assert.That(acceptedReturn.after.campaign_menu.selected_level_id,
                Is.EqualTo(SecretOriginLevel));
            Assert.That(acceptedReturn.after.active_runtime_flow.bound_level_id,
                Is.Not.EqualTo(SecretOriginLevel),
                "the retained ordinary flow binding need not equal the selected return origin");

            var skippedReturnGeneration = SecretCompletedReturnReceipt();
            skippedReturnGeneration.after.generation = "25";
            Assert.Throws<InvalidDataException>(() =>
                SecretTransitionTransport.ValidateSecretTransition(
                    DecodeBoundaryEnvelope(skippedReturnGeneration),
                    skippedReturnGeneration.before, skippedReturnGeneration.after),
                "a secret boundary must advance runtime generation by exactly one");

            var abandonedReturn = SecretAbandonedReturnReceipt();
            GameRuntimeTransport.ValidateIdentity(abandonedReturn.before, 771);
            GameRuntimeTransport.ValidateIdentity(abandonedReturn.after, 771);
            Assert.That(SecretTransitionTransport.ValidateSecretTransition(
                DecodeBoundaryEnvelope(abandonedReturn),
                abandonedReturn.before, abandonedReturn.after).kind,
                Is.EqualTo(SecretTransitionTransport.AbandonedReturnKind));

            var pausedAbandon = SecretAbandonedReturnReceipt();
            pausedAbandon.before.mode = GameRuntimeTransport.PausedMode;
            var acceptedPausedAbandon = SecretTransitionTransport.ValidateSecretTransition(
                DecodeBoundaryEnvelope(pausedAbandon), pausedAbandon.before, pausedAbandon.after);
            Assert.That(acceptedPausedAbandon.kind, Is.EqualTo(SecretTransitionTransport.AbandonedReturnKind));
            var pausedCompletion = SecretCompletedReturnReceipt();
            pausedCompletion.before.mode = GameRuntimeTransport.PausedMode;
            Assert.Throws<InvalidDataException>(() =>
                SecretTransitionTransport.ValidateSecretTransition(
                    DecodeBoundaryEnvelope(pausedCompletion), pausedCompletion.before, pausedCompletion.after),
                "paused Close can abandon a secret but cannot produce a completed return");

            var abandonmentGrantedCompletion = SecretAbandonedReturnReceipt();
            abandonmentGrantedCompletion.after.secret_campaign.completed_levels =
                new[] { SecretDestinationLevel };
            GameRuntimeTransport.ValidateIdentity(abandonmentGrantedCompletion.after, 771);
            Assert.Throws<InvalidDataException>(() =>
                SecretTransitionTransport.ValidateSecretTransition(
                    DecodeBoundaryEnvelope(abandonmentGrantedCompletion),
                    abandonmentGrantedCompletion.before, abandonmentGrantedCompletion.after),
                "an abandoned return cannot add a secret completion result");

            var wrongReturnMap = SecretCompletedReturnReceipt();
            wrongReturnMap.after.campaign_menu.selected_level_id = SecretDestinationLevel;
            wrongReturnMap.after.campaign_menu.briefing_level_id = SecretDestinationLevel;
            wrongReturnMap.after.presentation.level_id = SecretDestinationLevel;
            GameRuntimeTransport.ValidateIdentity(wrongReturnMap.after, 771);
            Assert.Throws<InvalidDataException>(() =>
                SecretTransitionTransport.ValidateSecretTransition(
                    DecodeBoundaryEnvelope(wrongReturnMap),
                    wrongReturnMap.before, wrongReturnMap.after),
                "a valid selectable secret is not authority to skip restoring its origin");

            var wrongReceiptKind = SecretEntryReceipt();
            wrongReceiptKind.kind = "future_transition";
            Assert.Throws<InvalidDataException>(() =>
                SecretTransitionTransport.ValidateSecretTransition(
                    DecodeBoundaryEnvelope(wrongReceiptKind),
                    wrongReceiptKind.before, wrongReceiptKind.after));

            var unknownEnvelope = new SecretTransitionTransport.AcceptedBoundaryReceipt {
                kind = "future_boundary"
            };
            unknownEnvelope.secret_transition = SecretEntryReceipt();
            Assert.Throws<InvalidDataException>(() => SecretTransitionTransport.Decode(
                SerializeBoundaryEnvelope(unknownEnvelope), 1024 * 1024));

            var unknownFieldJson = Encoding.UTF8.GetString(
                SerializeBoundaryEnvelope(DecodeBoundaryEnvelope(SecretEntryReceipt())));
            unknownFieldJson = unknownFieldJson.Insert(
                unknownFieldJson.IndexOf('{') + 1, "\"unexpected\":null,");
            Assert.Throws<InvalidDataException>(() => SecretTransitionTransport.Decode(
                Encoding.UTF8.GetBytes(unknownFieldJson), 1024 * 1024),
                "source-shaped boundary JSON still rejects an unknown member");

            var unknownReceiptFieldJson = Encoding.UTF8.GetString(
                SerializeBoundaryEnvelope(DecodeBoundaryEnvelope(SecretEntryReceipt())));
            var receiptStart = unknownReceiptFieldJson.IndexOf(
                "\"secret_transition\":{", StringComparison.Ordinal);
            Assert.That(receiptStart, Is.GreaterThanOrEqualTo(0));
            receiptStart = receiptStart + "\"secret_transition\":{".Length;
            unknownReceiptFieldJson = unknownReceiptFieldJson.Insert(
                receiptStart, "\"unexpected\":null,");
            Assert.Throws<InvalidDataException>(() => SecretTransitionTransport.Decode(
                Encoding.UTF8.GetBytes(unknownReceiptFieldJson), 1024 * 1024),
                "secret receipt still rejects unknown members");

            var duplicateKindJson = Encoding.UTF8.GetString(
                SerializeBoundaryEnvelope(DecodeBoundaryEnvelope(SecretEntryReceipt())));
            duplicateKindJson = duplicateKindJson.Insert(
                duplicateKindJson.IndexOf('{') + 1,
                "\"kind\":\"secret_transition\",");
            Assert.Throws<InvalidDataException>(() => SecretTransitionTransport.Decode(
                Encoding.UTF8.GetBytes(duplicateKindJson), 1024 * 1024),
                "tagged envelope still rejects duplicate members");

            var duplicateReceiptFieldJson = Encoding.UTF8.GetString(
                SerializeBoundaryEnvelope(DecodeBoundaryEnvelope(SecretEntryReceipt())));
            receiptStart = duplicateReceiptFieldJson.IndexOf(
                "\"secret_transition\":{", StringComparison.Ordinal);
            Assert.That(receiptStart, Is.GreaterThanOrEqualTo(0));
            receiptStart = receiptStart + "\"secret_transition\":{".Length;
            duplicateReceiptFieldJson = duplicateReceiptFieldJson.Insert(
                receiptStart,
                "\"receipt_contract\":\"" + SecretTransitionTransport.ReceiptContract + "\",");
            Assert.Throws<InvalidDataException>(() => SecretTransitionTransport.Decode(
                Encoding.UTF8.GetBytes(duplicateReceiptFieldJson), 1024 * 1024),
                "secret receipt still rejects duplicate members");

            var missingReceiptFieldJson = Encoding.UTF8.GetString(
                SerializeBoundaryEnvelope(DecodeBoundaryEnvelope(SecretEntryReceipt())));
            var digestMember = missingReceiptFieldJson.IndexOf(
                "\"receipt_sha256\":", StringComparison.Ordinal);
            Assert.That(digestMember, Is.GreaterThanOrEqualTo(0));
            var digestValueEnd = missingReceiptFieldJson.IndexOf('}', digestMember);
            Assert.That(digestValueEnd, Is.GreaterThan(digestMember));
            var precedingComma = digestMember > 0 && missingReceiptFieldJson[digestMember - 1] == ',';
            if (precedingComma)
            {
                missingReceiptFieldJson = missingReceiptFieldJson.Remove(
                    digestMember - 1, digestValueEnd - digestMember + 1);
            }
            else
            {
                var followingComma = missingReceiptFieldJson.IndexOf(',', digestMember);
                missingReceiptFieldJson = missingReceiptFieldJson.Remove(
                    digestMember, followingComma - digestMember + 1);
            }
            Assert.Throws<InvalidDataException>(() => SecretTransitionTransport.Decode(
                Encoding.UTF8.GetBytes(missingReceiptFieldJson), 1024 * 1024),
                "secret receipt still requires every v1 member");

            var ambiguousJson = Encoding.UTF8.GetString(
                SerializeBoundaryEnvelope(DecodeBoundaryEnvelope(SecretEntryReceipt())));
            ambiguousJson = ambiguousJson.Insert(
                ambiguousJson.IndexOf('{') + 1, "\"ordinary_completion\":null,");
            Assert.Throws<InvalidDataException>(() => SecretTransitionTransport.Decode(
                Encoding.UTF8.GetBytes(ambiguousJson), 1024 * 1024),
                "even an explicit null opposite payload makes the tagged union ambiguous");

            var malformedHash = SecretEntryReceipt();
            malformedHash.receipt_sha256 = new string('A', 64);
            Assert.Throws<InvalidDataException>(() =>
                SecretTransitionTransport.ValidateSecretTransition(
                    DecodeBoundaryEnvelope(malformedHash),
                    malformedHash.before, malformedHash.after));
        }

        [TestCase(2, "objective.reach.payahandra_ruins", "Fly to Payahandra's ruins.")]
        [TestCase(3, "objective.clear.undead_horde", "Destroy the undead horde.")]
        [Timeout(180000)]
        public void CurrentDayPlayerEntryShowsRustSelectedObjectiveAndClearsItOnTitle(
            int externalLevel, string objectiveId, string instruction)
        {
            var activeBefore = NativeSession.ActiveSessionCount;
            var generated = Path.Combine(PrivateRoot, "generated");
            var entry = Path.Combine(generated,
                $"opened-l{externalLevel}-block32-castle-boot-c330992-20261005.json");
            Assert.That(File.Exists(entry), Is.True, "exact Rust-generated current player entry");
            var root = new GameObject($"current Level {externalLevel} objective instruction");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(new[] {
                    "-mc-game-runtime", "-mc-runtime-replay-load", entry,
                    "-mc-runtime-package", LevelOnePackage,
                    "-mc-runtime-owner-cache", Path.Combine(generated, "level1-unity-asset-cache-plan29-final"),
                    "-mc-runtime-presentation-cache", Path.Combine(generated, "level1-unity-asset-cache-plan29-final"),
                    "-mc-runtime-package", Path.Combine(generated, "level2-day-block32-campaign-v2"),
                    "-mc-runtime-owner-cache", Path.Combine(generated, "level2-day-spell-owner-cache-audio-v4/components/material"),
                    "-mc-runtime-presentation-cache", Path.Combine(generated, "level2-day-spell-owner-cache-audio-v4"),
                    "-mc-runtime-package", Path.Combine(generated, "level3-day-block32-campaign-recipe-v2"),
                    "-mc-runtime-owner-cache", Path.Combine(generated, "level3-day-spell-owner-cache-audio-v4/components/material"),
                    "-mc-runtime-presentation-cache", Path.Combine(generated, "level3-day-spell-owner-cache-audio-v4") });
                var opened = runtime.GameRuntimeProductionIdentity;
                var before = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(opened.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(before.value.objective_progress.current_objective, Is.EqualTo(objectiveId));
                Assert.That(runtime.GameRuntimeObjectiveInstruction, Is.EqualTo(instruction));
                Assert.That(runtime.GameRuntimeProductionCampaignSnapshot.value.state_hash,
                    Is.EqualTo(before.value.state_hash), "caption reads are observational");
                runtime.QualificationDriveGameRuntimeFrames(2);
                var stepped = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(stepped.identity.session_id, Is.EqualTo(opened.session.session_id));
                Assert.That(ulong.Parse(stepped.identity.tick), Is.EqualTo(ulong.Parse(before.identity.tick) + 2));
                Assert.That(stepped.value.objective_progress.current_objective, Is.EqualTo(objectiveId));
                Assert.That(runtime.GameRuntimeObjectiveInstruction, Is.EqualTo(instruction));
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Title), Is.True);
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.TitleMode));
                Assert.That(runtime.GameRuntimeObjectiveInstruction, Is.Null,
                    "the former level must not leave an instruction on Title or the next level");
            }
            finally
            {
                runtime.QualificationDisposeGameRuntimeProduction();
                UnityEngine.Object.DestroyImmediate(root);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
        }

        [Test]
        [Timeout(300000)]
        public void ProductionNightNewGameCastSaveLoadResumeAndRetryKeepsRustAuthority()
        {
            var activeBefore = NativeSession.ActiveSessionCount;
            var root = new GameObject("Night production New Game play loop");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var save = Path.Combine(Path.GetTempPath(), $"mc-night-play-{Guid.NewGuid():N}.json");
            var preferencesProfile = save + ".preferences";
            Action installPreferences = () => typeof(MagicCarpetRuntime).GetMethod(
                "PreparePlayerPreferences", System.Reflection.BindingFlags.Instance |
                System.Reflection.BindingFlags.NonPublic).Invoke(runtime,
                new object[] { new[] { "-mc-preferences-profile", preferencesProfile } });
            var generated = Path.Combine(PrivateRoot, "generated");
            var cache = LevelOneOwnerCache;
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(new[] {
                    "-mc-game-runtime", "-mc-runtime-menu",
                    "-mc-runtime-loading-cache", Path.Combine(generated, "mc2-level1-night-cache-v2"),
                    "-mc-runtime-menu-cache", CampaignMenuCache,
                    "-mc-runtime-media-cache", Path.Combine(generated, "mc2-campaign-media-cache-decoded"),
                    "-mc-runtime-replay-save", save,
                    "-mc-runtime-package", LevelOnePackage,
                    "-mc-runtime-owner-cache", cache,
                    "-mc-runtime-presentation-cache", Path.Combine(generated,
                        "settlement-20261105-integration/owner-closed-v6/night-presentation-v6"),
                    "-mc-runtime-package", LevelTwoPackage,
                    "-mc-runtime-owner-cache", Path.Combine(generated, "mc-b9s2-cursor-level2-vault-20261029"),
                    "-mc-runtime-presentation-cache", Path.Combine(generated, "level2-vault-successor-owner-cache-20261025"),
                    "-mc-runtime-package", LevelThreePackage });
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.TitleMode));
                Assert.That(runtime.GameRuntimeProductionIdentity.session?.session_id, Is.Null.Or.Empty,
                    "menu startup has not opened a hidden gameplay session");
                Assert.That(runtime.GameRuntimeNightPresentationReady, Is.False);
                installPreferences();
                // This focused locale fixture enters the ordinary returned-Title
                // route. Installed-client qualification covers cold startup/skip.
                Assert.That(runtime.StepGameRuntimeTitleMedia(0, false, false, startup: false), Is.True,
                    "the real Title media owner is installed before source controls play their click");
                Assert.That(runtime.GameRuntimeProductionIdentity.frontend_media.active, Is.False);
                var localePreferences = (PlayerPreferencesPresenter)typeof(MagicCarpetRuntime).GetField(
                    "playerPreferences", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)
                    .GetValue(runtime);
                var localeMenu = (CampaignMenuPresenter)typeof(MagicCarpetRuntime).GetField("gameRuntimeMenu",
                    System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic).GetValue(runtime);
                var localeCache = localeMenu.LocaleCache;
                var localeSnapshot = localePreferences.Current.locale_binding;
                Assert.That(localeSnapshot, Is.Not.Null, "normal Title startup installs the real source language owner");
                Assert.That(localePreferences.Current.installed_locales, Is.Not.Empty);
                var localeBinding = localeCache.PrepareLocale(localeSnapshot);
                Assert.That(localeCache.PrepareLocale(localeCache.LanguageCachePath, localeSnapshot.id), Is.SameAs(localeBinding));
                Assert.That(localeCache.BriefingText(localeCache.ForMission(SecretOriginLevel)), Is.EqualTo(localeBinding.Record(27)));
                Assert.That(localeCache.LanguageText(444), Is.EqualTo(localeBinding.Record(444)),
                    "source key-dialog labels read the retained language owner");
                var originalSelection = localePreferences.Current.preferences.selected_locale;
                Assert.That(originalSelection, Is.Null, "the fresh source profile uses the nullable default locale");
                var localeGui = ScriptableObject.CreateInstance<CampaignGuiWindow>();
                var localeScreen = Vector2.zero;
                var localeGuiOrigin = Vector2.zero;
                Vector2? localeReceivedDown = null, localeReceivedUp = null;
                localeGui.DrawRuntime = () => {
                    var eventType = Event.current.type;
                    var pointer = Event.current.mousePosition;
                    localeScreen = new Vector2(Screen.width, Screen.height);
                    localeGuiOrigin = GUIUtility.GUIToScreenPoint(Vector2.zero) - localeGui.position.position;
                    if (eventType == EventType.MouseDown) localeReceivedDown = pointer;
                    if (eventType == EventType.MouseUp) localeReceivedUp = pointer;
                    typeof(MagicCarpetRuntime).GetMethod("OnGUI",
                        System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic).Invoke(runtime, null);
                };
                try
                {
                    localeGui.minSize = new Vector2(640, 480);
                    localeGui.position = new Rect(0, 0, 640, 480);
                    localeGui.Show();
                    localeGui.position = new Rect(0, 0, 640, 480);
                    localeGui.SendEvent(new Event { type = EventType.Layout });
                    localeGui.SendEvent(new Event { type = EventType.Repaint });
                    Assert.That(localeGui.DrawCount, Is.GreaterThan(0), "the real Title GUI host has drawn");
                    var button = Array.Find(localeCache.Startup.buttons, entry => entry.source_action == 0x2589e0);
                    Assert.That(button, Is.Not.Null, "the original LanguageSettingDialog button is bound");
                    var localeScale = Mathf.Min(localeScreen.x / 640f, localeScreen.y / 480f);
                    var localeClick = new Vector2((localeScreen.x - 640 * localeScale) * .5f +
                        (button.x + button.width * .5f) * localeScale,
                        (localeScreen.y - 480 * localeScale) * .5f + (button.y + button.height * .5f) * localeScale);
                    Assert.That(new Rect(Vector2.zero, localeGui.position.size).Contains(localeClick), Is.True,
                        "the real source button is inside the GUI host viewport");
                    var localeHit = new Rect((localeScreen.x - 640 * localeScale) * .5f + button.x * localeScale,
                        (localeScreen.y - 480 * localeScale) * .5f + button.y * localeScale,
                        button.width * localeScale, button.height * localeScale);
                    Action openLanguage = () => {
                        localeReceivedDown = localeReceivedUp = null;
                        var sendPoint = localeClick + localeGuiOrigin;
                        localeGui.SendEvent(new Event { type = EventType.MouseDown, button = 0, mousePosition = sendPoint });
                        localeGui.SendEvent(new Event { type = EventType.MouseUp, button = 0, mousePosition = sendPoint });
                        Assert.That(localeReceivedDown.HasValue && localeHit.Contains(localeReceivedDown.Value), Is.True,
                            "the actual mouse-down reaches the source button after the Editor GUI transform");
                        Assert.That(localeReceivedUp.HasValue && localeHit.Contains(localeReceivedUp.Value), Is.True,
                            "the actual mouse-up reaches the source button after the Editor GUI transform");
                        Assert.That(localePreferences.IsOpen, Is.True, "the actual source button opens its settings page");
                        var localeDraft = (PlayerPreferences)typeof(PlayerPreferencesPresenter).GetField("draft",
                            System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic).GetValue(localePreferences);
                        Assert.That(localeDraft.selected_locale, Is.Null,
                            "opening the default locale page preserves null before drawing or input");
                        localeGui.SendEvent(new Event { type = EventType.Repaint });
                    };
                    openLanguage();
                    localeGui.SendEvent(new Event { type = EventType.KeyDown, keyCode = KeyCode.RightArrow });
                    localeGui.SendEvent(new Event { type = EventType.KeyDown, keyCode = KeyCode.Escape });
                    Assert.That(localePreferences.IsOpen, Is.False);
                    Assert.That(localePreferences.Current.preferences.selected_locale, Is.EqualTo(originalSelection));
                    Assert.That(localeCache.PrepareLocale(localePreferences.Current.locale_binding), Is.SameAs(localeBinding));
                    openLanguage();
                    localeGui.SendEvent(new Event { type = EventType.KeyDown, keyCode = KeyCode.Return });
                    var localeStatus = (string)typeof(PlayerPreferencesPresenter).GetField("status",
                        System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic).GetValue(localePreferences);
                    Assert.That(localePreferences.IsOpen, Is.False, "source locale Return applies: " + localeStatus);
                    Assert.That(localePreferences.Current.preferences.selected_locale, Is.EqualTo(localeSnapshot.id),
                        "accepting the real displayed English pack retains its explicit content ID");
                    Assert.That(File.Exists(Path.Combine(preferencesProfile, "player-preferences.json")), Is.True);
                    Assert.That(runtime.GameRuntimeProductionIdentity.player_preferences.locale_binding.id,
                        Is.EqualTo(localeSnapshot.id));

                    // SetKeysDialog's selector109 acts immediately on the
                    // source owner; Done and Cancel remain separate actions.
                    var priorSoundEnabled = localePreferences.Current.preferences.sound_enabled;
                    var priorMusicEnabled = localePreferences.Current.preferences.music_enabled;
                    Assert.That(localePreferences.ApplySource(value => {
                        value.keys[0] = "W"; value.sound_volume = 31; value.music_volume = 32;
                        value.sound_enabled = false; value.music_enabled = false;
                        value.show_help = false; value.brightness[0] = 2; value.brightness[1] = 3; value.brightness[2] = 4;
                    }), Is.True);
                    var keysButton = Array.Find(localeCache.Startup.buttons, entry => entry.source_action == 0x25a610);
                    Assert.That(keysButton, Is.Not.Null);
                    var keysPoint = new Vector2((localeScreen.x - 640 * localeScale) * .5f +
                        (keysButton.x + keysButton.width * .5f) * localeScale,
                        (localeScreen.y - 480 * localeScale) * .5f + (keysButton.y + keysButton.height * .5f) * localeScale);
                    void OpenKeys()
                    {
                        localeGui.SendEvent(new Event { type = EventType.MouseDown, button = 0, mousePosition = keysPoint + localeGuiOrigin });
                        localeGui.SendEvent(new Event { type = EventType.MouseUp, button = 0, mousePosition = keysPoint + localeGuiOrigin });
                        Assert.That(localePreferences.IsOpen, Is.True, "the real source Title button opens Set Keys");
                    }
                    var nonDefault = JsonUtility.ToJson(localePreferences.Current.preferences);
                    OpenKeys();
                    localeGui.SendEvent(new Event { type = EventType.KeyDown, keyCode = KeyCode.Escape });
                    localeGui.SendEvent(new Event { type = EventType.KeyDown, keyCode = KeyCode.Escape });
                    Assert.That(localePreferences.IsOpen, Is.False);
                    Assert.That(JsonUtility.ToJson(localePreferences.Current.preferences), Is.EqualTo(nonDefault),
                        "Cancel does not invoke source defaults");
                    OpenKeys();
                    localeGui.SendEvent(new Event { type = EventType.KeyDown, keyCode = KeyCode.Escape });
                    localeGui.SendEvent(new Event { type = EventType.KeyDown, keyCode = KeyCode.Return });
                    Assert.That(localePreferences.IsOpen, Is.False);
                    Assert.That(JsonUtility.ToJson(localePreferences.Current.preferences), Is.EqualTo(nonDefault),
                        "ordinary Done does not invoke source defaults");
                    OpenKeys();
                    localeGui.SendEvent(new Event { type = EventType.Layout });
                    localeGui.SendEvent(new Event { type = EventType.Repaint });
                    var defaultButton = localeCache.Settings.Bitmap(109);
                    Assert.That((defaultButton.width, defaultButton.height), Is.EqualTo((73, 37)),
                        "the actual source selector109 owns its hit extent");
                    var defaultPoint = new Vector2((localeScreen.x - 640 * localeScale) * .5f +
                        (273 + defaultButton.width * .5f) * localeScale,
                        (localeScreen.y - 480 * localeScale) * .5f + (10 + defaultButton.height * .5f) * localeScale);
                    var beforeDefault = JsonUtility.ToJson(localePreferences.Current.preferences);
                    var outsideDefault = new Vector2((localeScreen.x - 640 * localeScale) * .5f + 272 * localeScale,
                        (localeScreen.y - 480 * localeScale) * .5f + 20 * localeScale);
                    localeGui.SendEvent(new Event { type = EventType.MouseDown, button = 0,
                        mousePosition = outsideDefault + localeGuiOrigin });
                    Assert.That(JsonUtility.ToJson(localePreferences.Current.preferences), Is.EqualTo(beforeDefault),
                        "a click outside the actual source sprite cannot reset preferences");
                    localeGui.SendEvent(new Event { type = EventType.MouseDown, button = 0,
                        mousePosition = defaultPoint + localeGuiOrigin });
                    Assert.That(localePreferences.IsOpen, Is.True, "Default resets in place, rather than acting as Done");
                    var resetPrefs = localePreferences.Current.preferences;
                    CollectionAssert.AreEqual(new[] { "UpArrow", "DownArrow", "LeftArrow", "RightArrow", "Return",
                        "LeftControl", "LeftAlt", "RightShift", "RightShift", "LeftAlt" }, resetPrefs.keys);
                    Assert.That((resetPrefs.sound_volume, resetPrefs.music_volume, resetPrefs.show_help), Is.EqualTo((127, 127, true)));
                    CollectionAssert.AreEqual(new[] { 0, 3, 0 }, resetPrefs.brightness,
                        "source resets brightness11/13, preserving brightness12");
                    Assert.That((resetPrefs.sound_enabled, resetPrefs.music_enabled), Is.EqualTo((false, false)),
                        "source volume defaults do not unmute the independent enable flags");
                    Assert.That(resetPrefs.selected_locale, Is.EqualTo(localeSnapshot.id));
                    Assert.That(localeCache.PrepareLocale(localePreferences.Current.locale_binding), Is.SameAs(localeBinding));
                    var resetDraft = (PlayerPreferences)typeof(PlayerPreferencesPresenter).GetField("draft",
                        System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic).GetValue(localePreferences);
                    CollectionAssert.AreEqual(resetPrefs.keys, resetDraft.keys);
                    Assert.That((int)typeof(PlayerPreferencesPresenter).GetField("selectedKeySlot",
                        System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic).GetValue(localePreferences), Is.Zero);
                    using (var coldOwner = new GameRuntimeTransport())
                    {
                        var heldKeys = new System.Collections.Generic.HashSet<KeyCode> { KeyCode.W };
                        var coldPreferences = new PlayerPreferencesPresenter(json => GameRuntimeTransport.PreferencesCommand(
                            coldOwner.Handle, coldOwner.IdentitySnapshot(), json), preferencesProfile,
                            keyHeld: heldKeys.Contains, keyPressed: _ => false,
                            mouseHeld: _ => false, mousePressed: _ => false);
                        coldPreferences.BindLanguages(localeCache);
                        CollectionAssert.AreEqual(resetPrefs.keys, coldPreferences.Current.preferences.keys);
                        CollectionAssert.AreEqual(resetPrefs.brightness, coldPreferences.Current.preferences.brightness);
                        Assert.That(coldPreferences.Current.preferences.selected_locale, Is.EqualTo(localeSnapshot.id));
                        Assert.That((coldPreferences.Current.preferences.sound_volume, coldPreferences.Current.preferences.music_volume,
                            coldPreferences.Current.preferences.show_help), Is.EqualTo((127, 127, true)));
                        Assert.That(coldPreferences.SampleInput(0, true).Forward, Is.False,
                            "the replaced W mapping no longer drives flight after cold profile load");
                        heldKeys.Add(KeyCode.UpArrow);
                        Assert.That(coldPreferences.SampleInput(0, true).Forward, Is.True,
                            "the reset source forward key reaches the normal input consumer");
                    }
                    // A later uncommitted edit is cancellable without undoing
                    // the already accepted and persisted Default action.
                    localeGui.SendEvent(new Event { type = EventType.KeyDown, keyCode = KeyCode.W, character = 'w' });
                    localeGui.SendEvent(new Event { type = EventType.KeyDown, keyCode = KeyCode.Escape });
                    localeGui.SendEvent(new Event { type = EventType.KeyDown, keyCode = KeyCode.Escape });
                    Assert.That(localePreferences.IsOpen, Is.False);
                    Assert.That(localePreferences.Current.preferences.keys[0], Is.EqualTo("UpArrow"));
                    Assert.That(localePreferences.ApplySource(value => {
                        value.sound_enabled = priorSoundEnabled; value.music_enabled = priorMusicEnabled; value.brightness[1] = 0;
                    }), Is.True);
                }
                finally
                {
                    localeGui.DrawRuntime = null;
                    localeGui.Close();
                    if (localeGui != null) UnityEngine.Object.DestroyImmediate(localeGui);
                }
                runtime.StepGameRuntimeTitleMedia(50_000, false, false);
                Assert.That(runtime.GameRuntimeProductionIdentity.frontend_media.clear_input, Is.False,
                    "the normal source Title frame completes its 50ms input-release delay before NewGame");
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.NewGame), Is.True);
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.CampaignMode));
                Assert.That(runtime.GameRuntimeProductionIdentity.campaign_menu.completed_levels, Is.Empty);
                Assert.That(runtime.GameRuntimeProductionIdentity.campaign_menu.completed_level_id, Is.Null.Or.Empty);
                Assert.That(runtime.GameRuntimeProductionIdentity.campaign_menu.briefing_speech.track, Is.EqualTo(1));
                Assert.That(runtime.GameRuntimeProductionIdentity.session?.session_id, Is.Null.Or.Empty);
                var presentBriefing = typeof(MagicCarpetRuntime).GetMethod("UpdateGameRuntimeBriefing",
                    System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic);
                var menu = (CampaignMenuPresenter)typeof(MagicCarpetRuntime)
                    .GetField("gameRuntimeMenu", System.Reflection.BindingFlags.Instance |
                        System.Reflection.BindingFlags.NonPublic).GetValue(runtime);
                Assert.That(menu.SourceBriefingReady, Is.False, "the map has not entered its normal GUI phase yet");
                var drawGui = typeof(MagicCarpetRuntime).GetMethod("OnGUI",
                    System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic);
                var gui = ScriptableObject.CreateInstance<CampaignGuiWindow>();
                gui.DrawRuntime = () => drawGui.Invoke(runtime, null);
                try
                {
                    gui.Show();
                    gui.SendEvent(new Event { type = EventType.Layout });
                    gui.SendEvent(new Event { type = EventType.Repaint });
                    Assert.That(gui.DrawCount, Is.GreaterThan(0), "the actual runtime GUI consumed its map events");
                    Assert.That(menu.SourceBriefingReady, Is.True,
                        "ordinary first-map Enter/Draw reaches Ready without forced state or shortened motion");
                }
                finally
                {
                    gui.DrawRuntime = null;
                    gui.Close();
                    if (gui != null) UnityEngine.Object.DestroyImmediate(gui);
                }
                presentBriefing.Invoke(runtime, null);
                presentBriefing.Invoke(runtime, null);
                var speech = (CampaignSpeechPresenter)typeof(MagicCarpetRuntime)
                    .GetField("campaignSpeech", System.Reflection.BindingFlags.Instance |
                        System.Reflection.BindingFlags.NonPublic).GetValue(runtime);
                Assert.That(speech.PlayCount, Is.EqualTo(1), "map polling consumes its authored briefing once");
                Assert.That(speech.LastStartMs, Is.Zero);
                Assert.That(speech.LastLengthMs, Is.EqualTo(9999));
                Assert.That(speech.SoundMix, Is.EqualTo(1), "map briefing does not use objective ducking");
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.StartMission), Is.True);
                Assert.That(runtime.GameRuntimeLoadingPresentationVisible, Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.LoadingMode));
                runtime.QualificationObserveGameRuntimeLoadingPresentationFrame();
                runtime.QualificationDriveGameRuntimeFrames(1);
                var opened = runtime.GameRuntimeProductionIdentity;
                Assert.That(opened.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(opened.active_runtime_flow.bound_level_id, Is.EqualTo("level.magic_carpet_2.night_spire"));
                Assert.That(opened.session.session_id, Is.EqualTo(opened.runtime_id));
                Assert.That(opened.session.tick, Is.EqualTo("0"));
                Assert.DoesNotThrow(() => installPreferences(),
                    "the initial tick-zero preference install preserves its existing admission path");
                Assert.That(runtime.GameRuntimeProductionIdentity.generation, Is.EqualTo(opened.generation));
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo("0"));
                Assert.That(runtime.GameRuntimeProductionIdentity.player_preferences.installed, Is.True);
                Assert.DoesNotThrow(() => installPreferences(),
                    "repeated preparation observes the already installed Rust profile");
                Assert.That(runtime.GameRuntimeProductionIdentity.generation, Is.EqualTo(opened.generation),
                    "hydrating preferences must not pause or resume an already prepared owner");
                Assert.That(runtime.GameRuntimeNightPresentationReady, Is.True);
                Assert.That(runtime.GameRuntimeNightMinimapReady, Is.True);
                Assert.That(runtime.Level2ProductionDayHudPresenterReady, Is.True);
                var resetHash = runtime.GameRuntimeProductionCampaignSnapshot.value.state_hash;
                var resetWorldFrame = runtime.GameRuntimeNightPresentationHash;
                Assert.That(resetWorldFrame, Is.Not.Zero);
                Assert.That(runtime.GameRuntimeObjectiveInstruction, Does.Contain("Fly towards my beacon"));

                runtime.QualificationApplyGameRuntimeProductionMovement(true, false, true, false);
                runtime.QualificationApplyGameRuntimeProductionSteering(24f, -8f);
                runtime.QualificationDriveGameRuntimeFrames(3);
                var moved = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(moved.identity.session_id, Is.EqualTo(opened.session.session_id));
                Assert.That(moved.identity.tick, Is.EqualTo("3"));
                Assert.That(moved.value.player_motion.actual_speed, Is.Not.Zero);
                Assert.That(runtime.GameRuntimeNightPresentationHash, Is.Not.EqualTo(resetWorldFrame));
                Assert.That(runtime.GameRuntimeProductionFlightHudText, Does.Contain($"tick {moved.identity.tick}"));
                Assert.That(runtime.Level2ProductionDayHud.localPlayerMana, Is.EqualTo(moved.value.player_vitals.mana));

                Assert.That(runtime.SubmitGameRuntimeSpellKey(1), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.RequestGameRuntimeSelectedSpellCast(), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                var cast = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(cast.identity.session_id, Is.EqualTo(opened.session.session_id));
                Assert.That(Array.Exists(cast.value.presentation_entities,
                    entity => entity.kind == "spell.fireball.projectile"), Is.True,
                    "New Game's acquired Fireball must create the real Rust semantic product");

                // Exercise the same nonzero Playing save as ordinary F5/cold
                // restore, before separately checking the paused-save path.
                runtime.QualificationSaveGameRuntimeProductionReplayToPath(save);
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Title), Is.True);
                Assert.That(runtime.GameRuntimeObjectiveInstruction, Is.Null,
                    "Title clears the former mission's source objective text");
                Assert.That(typeof(MagicCarpetRuntime).GetField("gameRuntimeObjectiveTextbox",
                    System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)
                    .GetValue(runtime), Is.Null, "Title releases the former glyph presentation owner");
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Load), Is.True);
                var restoredPlaying = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(restoredPlaying.identity.tick, Is.EqualTo(cast.identity.tick));
                Assert.That(restoredPlaying.value.state_hash, Is.EqualTo(cast.value.state_hash));
                Assert.That(JsonUtility.ToJson(restoredPlaying.value.player_pose), Is.EqualTo(JsonUtility.ToJson(cast.value.player_pose)));
                Assert.That(runtime.GameRuntimeProductionIdentity.player_preferences.installed, Is.True);

                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                var paused = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Save), Is.True);
                Assert.That(new FileInfo(save).Length, Is.GreaterThan(0));
                Assert.That(runtime.GameRuntimeProductionCampaignSnapshot.value.state_hash, Is.EqualTo(paused.value.state_hash));
                var validPausedSave = File.ReadAllBytes(save);
                var ownerBeforeInvalidLoad = runtime.GameRuntimeProductionIdentity;
                var preferencesBeforeInvalidLoad = JsonUtility.ToJson(ownerBeforeInvalidLoad.player_preferences);
                var sessionsBeforeInvalidLoad = NativeSession.ActiveSessionCount;
                File.WriteAllBytes(save, System.Text.Encoding.UTF8.GetBytes("{\"invalid_saved_campaign\":true}"));
                try
                {
                    Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Load), Is.False,
                        "the normal Load action must reject an invalid replay before replacing its live paused owner");
                    var retainedOwner = runtime.GameRuntimeProductionIdentity;
                    Assert.That(retainedOwner.runtime_id, Is.EqualTo(ownerBeforeInvalidLoad.runtime_id));
                    Assert.That(retainedOwner.generation, Is.EqualTo(ownerBeforeInvalidLoad.generation));
                    Assert.That(retainedOwner.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                    Assert.That(runtime.GameRuntimeProductionCampaignSnapshot.value.state_hash,
                        Is.EqualTo(paused.value.state_hash));
                    Assert.That(JsonUtility.ToJson(retainedOwner.player_preferences), Is.EqualTo(preferencesBeforeInvalidLoad));
                    Assert.That(runtime.GameRuntimeNightPresentationReady, Is.True);
                    Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(sessionsBeforeInvalidLoad),
                        "failed load cannot leak a replacement native owner");
                }
                finally { File.WriteAllBytes(save, validPausedSave); }
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Title), Is.True);
                Assert.That(runtime.GameRuntimeNightPresentationReady, Is.False);
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Load), Is.True);
                var loaded = runtime.GameRuntimeProductionIdentity;
                var restored = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.DoesNotThrow(() => installPreferences());
                Assert.That(runtime.GameRuntimeProductionIdentity.generation, Is.EqualTo(loaded.generation),
                    "an already Paused restore stays paused without another transition");
                Assert.That(loaded.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                Assert.That(loaded.active_runtime_flow.bound_level_id, Is.EqualTo(opened.active_runtime_flow.bound_level_id));
                Assert.That(loaded.session.session_id, Is.EqualTo(loaded.runtime_id));
                Assert.That(restored.identity.tick, Is.EqualTo(paused.identity.tick));
                Assert.That(restored.value.state_hash, Is.EqualTo(paused.value.state_hash));
                Assert.That(restored.value.effect_journal_sha256, Is.EqualTo(paused.value.effect_journal_sha256));
                Assert.That(JsonUtility.ToJson(restored.value.player_pose), Is.EqualTo(JsonUtility.ToJson(paused.value.player_pose)));
                Assert.That(runtime.GameRuntimeNightPresentationReady, Is.True);
                Assert.That(runtime.GameRuntimeNightMinimapReady, Is.True);
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Resume), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.GameRuntimeProductionIdentity.runtime_id, Is.EqualTo(loaded.runtime_id));
                Assert.That(ulong.Parse(runtime.GameRuntimeProductionIdentity.session.tick),
                    Is.EqualTo(ulong.Parse(restored.identity.tick) + 1));
                Assert.That(runtime.Level2ProductionDayHud.localPlayerMana,
                    Is.EqualTo(runtime.GameRuntimeProductionCampaignSnapshot.value.player_vitals.mana));

                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                var beforeRetry = runtime.GameRuntimeProductionIdentity;
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Restart), Is.True);
                var retried = runtime.GameRuntimeProductionIdentity;
                Assert.That(retried.runtime_id, Is.EqualTo(loaded.runtime_id));
                Assert.That(retried.active_runtime_flow.bound_level_id, Is.EqualTo(opened.active_runtime_flow.bound_level_id));
                Assert.That(ulong.Parse(retried.session.generation), Is.GreaterThan(ulong.Parse(beforeRetry.session.generation)));
                Assert.That(retried.session.tick, Is.EqualTo("0"));
                Assert.That(runtime.GameRuntimeProductionCampaignSnapshot.value.state_hash, Is.EqualTo(resetHash));
                Assert.That(runtime.GameRuntimeNightPresentationHash, Is.EqualTo(resetWorldFrame));
                Assert.That(runtime.GameRuntimeNightPresentationReady, Is.True);
                Assert.That(runtime.GameRuntimeNightMinimapReady, Is.True);
                const System.Reflection.BindingFlags objectiveFlags =
                    System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
                var objectiveOwner = (NativeSession)typeof(MagicCarpetRuntime)
                    .GetField("session", objectiveFlags).GetValue(runtime);
                var objectiveSurface = (CampaignPresentationSurface)typeof(MagicCarpetRuntime)
                    .GetField("gameRuntimeNightSurface", objectiveFlags).GetValue(runtime);
                var objectivePresenter = (SourceObjectiveTextboxPresenter)typeof(MagicCarpetRuntime)
                    .GetField("gameRuntimeObjectiveTextbox", objectiveFlags).GetValue(runtime);
                Assert.That(objectivePresenter, Is.Not.Null);
                var objectiveWindow = ScriptableObject.CreateInstance<CampaignGuiWindow>();
                var objectiveDisplay = new RenderTexture(320, 200, 0, RenderTextureFormat.ARGB32,
                    RenderTextureReadWrite.Linear);
                try
                {
                    objectiveWindow.DrawRuntime = () => typeof(MagicCarpetRuntime).GetMethod("OnGUI", objectiveFlags)
                        .Invoke(runtime, null);
                    objectiveWindow.Show();
                    // The ordinary source O-input producer sets two, then the
                    // post-phase draw clock consumes one, including while paused.
                    Action<bool> observeObjective = held => {
                        var observed = objectiveOwner.GameRuntimeCampaignPausedRenderFrame(0, held);
                        typeof(MagicCarpetRuntime).GetField("gameRuntimeCampaignSnapshot", objectiveFlags)
                            .SetValue(runtime, observed);
                        typeof(MagicCarpetRuntime).GetMethod("UpdateGameRuntimeNightPresentation", objectiveFlags)
                            .Invoke(runtime, new object[] { runtime.GameRuntimeProductionIdentity, observed });
                    };
                    runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                    observeObjective(true);
                    Assert.That(runtime.GameRuntimeProductionCampaignSnapshot.value.objective_textbox_ticks, Is.EqualTo(1));
                    var objectiveCameras = objectiveSurface.FrameCamera.transform.parent.GetComponentsInChildren<Camera>();
                    Array.Sort(objectiveCameras, (left, right) => left.depth.CompareTo(right.depth));
                    foreach (var camera in objectiveCameras)
                    {
                        if (camera.targetTexture == null) camera.targetTexture = objectiveDisplay;
                        camera.Render();
                    }
                    var speechPlays = speech.PlayCount;
                    var objectiveTick = runtime.GameRuntimeProductionIdentity.session.tick;
                    objectiveWindow.SendEvent(new Event { type = EventType.Layout });
                    objectiveWindow.SendEvent(new Event { type = EventType.Repaint });
                    var objectiveOutput = (RenderTexture)typeof(SourceObjectiveTextboxPresenter)
                        .GetField("output", objectiveFlags).GetValue(objectivePresenter);
                    Assert.That(objectiveOutput, Is.Not.Null, "normal paused GUI renders the real source objective");
                    var objectivePixels = ReadObjectivePixels(objectiveOutput);
                    objectiveWindow.SendEvent(new Event { type = EventType.Repaint });
                    CollectionAssert.AreEqual(objectivePixels, ReadObjectivePixels(objectiveOutput));
                    Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo(objectiveTick));
                    Assert.That(runtime.GameRuntimeProductionCampaignSnapshot.value.objective_textbox_ticks, Is.EqualTo(1));
                    Assert.That(speech.PlayCount, Is.EqualTo(speechPlays), "repeated GUI cannot replay objective narration");
                    observeObjective(false);
                    Assert.That(runtime.GameRuntimeProductionCampaignSnapshot.value.objective_textbox_ticks, Is.Zero);
                    objectiveWindow.SendEvent(new Event { type = EventType.Repaint });

                    runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                    var sourceMenus = (SourceCampaignMenus)typeof(MagicCarpetRuntime)
                        .GetField("gameRuntimeSourceMenus", objectiveFlags).GetValue(runtime);
                    var mapInput = sourceMenus.Step(true, false, false, false, false, true);
                    Assert.That(sourceMenus.QueueAction(mapInput.Action, sourceMenus.MapOpen,
                        sourceMenus.SpellOpen, sourceMenus.PlayerInformationOpen, mapInput), Is.True);
                    runtime.QualificationDriveGameRuntimeFrames(1);
                    Assert.That(sourceMenus.MapOpen, Is.True,
                        "the ordinary Return-key proposal receives actual Rust map admission");
                    runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                    observeObjective(true);
                    var mapSpeechPlays = speech.PlayCount;
                    objectiveWindow.SendEvent(new Event { type = EventType.Repaint });
                    Assert.That(objectiveSurface.WorldIndices, Is.Not.Null);
                    var objectiveMaterial = (Material)typeof(SourceObjectiveTextboxPresenter)
                        .GetField("material", objectiveFlags).GetValue(objectivePresenter);
                    Assert.That(objectiveSurface.HudUnderlayIndices, Is.Not.Null);
                    var mapHudIndices = (Texture)typeof(MagicCarpetRuntime)
                        .GetField("gameRuntimeHudIndices", objectiveFlags).GetValue(runtime) ?? objectiveSurface.HudUnderlayIndices;
                    Assert.That(mapHudIndices, Is.Not.Null);
                    Assert.That(objectiveMaterial.GetTexture("_SceneIndices"), Is.SameAs(mapHudIndices),
                        "the live map textbox samples the current map and HUD indexed result");
                    Assert.That(objectiveMaterial.GetFloat("_SceneTopDown"), Is.Zero,
                        "the composed GPU scene has bottom-up orientation");
                    var objectivePreferences = (PlayerPreferencesPresenter)typeof(MagicCarpetRuntime)
                        .GetField("playerPreferences", objectiveFlags).GetValue(runtime);
                    var objectivePalette = objectivePreferences.ApplyBrightnessToPalette(objectiveSurface.FramePaletteRgb8, 1);
                    CollectionAssert.AreEqual(objectivePalette,
                        ((Texture2D)objectiveMaterial.GetTexture("_Palette")).GetRawTextureData<byte>().ToArray(),
                        "the textbox uses this displayed map's palette and current brightness");
                    Assert.That(runtime.GameRuntimeProductionCampaignSnapshot.value.objective_textbox_ticks, Is.EqualTo(1));
                    Assert.That(speech.PlayCount, Is.EqualTo(mapSpeechPlays));
                    observeObjective(false);
                    Assert.That(runtime.GameRuntimeProductionCampaignSnapshot.value.objective_textbox_ticks, Is.Zero);
                    runtime.QualificationResetGameRuntimeProduction();
                    Assert.That(runtime.GameRuntimeProductionCampaignSnapshot.value.objective_textbox_ticks, Is.Zero,
                        "reset clears the old source textbox generation");
                }
                finally
                {
                    objectiveWindow.DrawRuntime = null;
                    objectiveWindow.Close();
                    if (objectiveWindow != null) UnityEngine.Object.DestroyImmediate(objectiveWindow);
                    objectiveDisplay.Release();
                    UnityEngine.Object.DestroyImmediate(objectiveDisplay);
                }
            }
            finally
            {
                runtime.QualificationDisposeGameRuntimeProduction();
                UnityEngine.Object.DestroyImmediate(root);
                if (File.Exists(save)) File.Delete(save);
                if (Directory.Exists(preferencesProfile)) Directory.Delete(preferencesProfile, true);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
        }

        [Test]
        public void ProductionNightCampaignDrawsWorldFromSameRustHandleThroughStepPauseAndReset()
        {
            var root = new GameObject("Night campaign production consumer");
            root.SetActive(false);
            try
            {
                var runtime = root.AddComponent<MagicCarpetRuntime>();
                var cache = Path.GetFullPath(Path.Combine(Application.dataPath,
                    "../../generated/level1-unity-asset-cache-plan29-final"));
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(new[] {
                    "-mc-game-runtime", "-mc-runtime-package", LevelOnePackage,
                    "-mc-runtime-owner-cache", cache, "-mc-runtime-presentation-cache", cache,
                    "-mc-runtime-loading-cache", cache,
                    "-mc-runtime-package", LevelTwoPackage, "-mc-runtime-package", LevelThreePackage });
                Assert.That(runtime.GameRuntimeProductionStartupReady, Is.True);
                Assert.That(runtime.GameRuntimeNightPresentationReady, Is.True);
                Assert.That(runtime.GameRuntimeNightMinimapReady, Is.True);
                Assert.That(runtime.GameRuntimeObjectiveInstruction, Does.Contain("Fly towards my beacon"));
                Assert.That(runtime.Level2ProductionDayHudPresenterReady, Is.True);
                Assert.That(runtime.Level2ProductionDayHud.manaPaletteIndex, Is.EqualTo(0xA4));
                Assert.That(runtime.QualificationHasMaterializedLevel2DayHudSprite(40), Is.True);
                var before = runtime.GameRuntimeProductionIdentity;
                var frame = runtime.GameRuntimeNightPresentationHash;
                Assert.That(frame, Is.Not.Zero);
                runtime.QualificationApplyGameRuntimeProductionSteering(24f, -8f);
                runtime.QualificationDriveGameRuntimeFrames(3);
                Assert.That(runtime.GameRuntimeProductionIdentity.runtime_id, Is.EqualTo(before.runtime_id));
                Assert.That(runtime.GameRuntimeNightPresentationTick, Is.EqualTo(3));
                Assert.That(runtime.GameRuntimeNightMinimapReady, Is.True);
                Assert.That(runtime.Level2ProductionDayHud.localPlayerMana,
                    Is.EqualTo(runtime.GameRuntimeProductionCampaignSnapshot.value.player_vitals.mana));
                Assert.That(runtime.GameRuntimeNightPresentationHash, Is.Not.EqualTo(frame));
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                runtime.QualificationDriveGameRuntimeFrames(2);
                Assert.That(runtime.GameRuntimeNightPresentationTick, Is.EqualTo(3));
                runtime.QualificationResetGameRuntimeProduction();
                Assert.That(runtime.GameRuntimeNightPresentationReady, Is.True);
                Assert.That(runtime.GameRuntimeNightPresentationTick, Is.Zero);
                Assert.That(runtime.GameRuntimeNightPresentationHash, Is.EqualTo(frame));
                Assert.That(runtime.GameRuntimeProductionIdentity.session.generation, Is.Not.EqualTo(before.session.generation));
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Title), Is.True);
                Assert.That(runtime.GameRuntimeNightPresentationReady, Is.False);
                Assert.That(runtime.GameRuntimeObjectiveInstruction, Is.Null);
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.NewGame), Is.True);
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.CampaignMode));
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.StartMission), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.LoadingMode));
                runtime.QualificationObserveGameRuntimeLoadingPresentationFrame();
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(runtime.GameRuntimeProductionIdentity.runtime_id, Is.EqualTo(before.runtime_id));
                Assert.That(runtime.GameRuntimeNightPresentationReady, Is.True);
                Assert.That(runtime.GameRuntimeNightPresentationHash, Is.EqualTo(frame));
            }
            finally { UnityEngine.Object.DestroyImmediate(root); }
        }

        [Test]
        public void CampaignMenuUsesRustPauseSaveTitleAndVisibleNewGameLoading()
        {
            var before = NativeSession.ActiveSessionCount;
            var root = new GameObject("Campaign menu production actions");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var save = Path.Combine(Path.GetTempPath(), $"mc-menu-{Guid.NewGuid():N}.json");
            try
            {
                var generated = Path.Combine(PrivateRoot, "generated");
                // Use the same installed four-package input closure as run-campaign.sh.
                // Native environment owners and presentation companions are distinct.
                var args = new[] {
                    "-mc-game-runtime", "-mc-runtime-package", Path.Combine(generated, "level3-day-block32-campaign-recipe-v2"),
                    "-mc-runtime-owner-cache", Path.Combine(generated, "mc-level3-environment-exit-20261025"),
                    "-mc-runtime-presentation-cache", Path.Combine(generated, "level3-day-spell-owner-cache-audio-v4"),
                    "-mc-runtime-package", Path.Combine(generated, "mc-bhq8-level1-collision-v5-night"),
                    "-mc-runtime-owner-cache", Path.Combine(generated, "mc-oi62-environment-level1-v2"),
                    "-mc-runtime-presentation-cache", Path.Combine(generated, "mc2-level1-night-cache-v2"),
                    "-mc-runtime-package", Path.Combine(generated, "mc-level2-vault-successor-closure-20261025"),
                    "-mc-runtime-owner-cache", Path.Combine(generated, "mc-level2-vault-successor-environment-20261025"),
                    "-mc-runtime-presentation-cache", Path.Combine(generated, "level2-vault-successor-owner-cache-20261025"),
                    "-mc-runtime-package", Path.Combine(generated, "mc-bhq6-ordinary-20261025-zclu5fjj/level4"),
                    "-mc-runtime-owner-cache", Path.Combine(generated, "mc-oi62-environment-level4-audio-v3"),
                    "-mc-runtime-presentation-cache", Path.Combine(generated, "mc-oi62-environment-level4-audio-v3"),
                    "-mc-runtime-loading-cache", Path.Combine(generated, "mc2-level1-night-cache-v2"),
                    "-mc-runtime-menu-cache", Path.Combine(generated, "mc-shared-source-map-motion-20261026"),
                    "-mc-runtime-replay-save", save };
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(args);
                var runtimeId = runtime.GameRuntimeProductionIdentity.runtime_id;
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.NewGame), Is.False);
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                runtime.QualificationDriveGameRuntimeFrames(1);
                var pausedFrame = runtime.GameRuntimeProductionCampaignSnapshot;
                runtime.QualificationDriveGameRuntimeFrames(2);
                // remc2 paused outer frames service bookkeeping (and change
                // the full hash), but must not advance gameplay tick or pose.
                Assert.That(runtime.GameRuntimeProductionCampaignSnapshot.identity.tick, Is.EqualTo(pausedFrame.identity.tick));
                Assert.That(JsonUtility.ToJson(runtime.GameRuntimeProductionCampaignSnapshot.value.player_pose),
                    Is.EqualTo(JsonUtility.ToJson(pausedFrame.value.player_pose)));
                var paused = runtime.GameRuntimeProductionCampaignSnapshot.value.state_hash;
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Continue), Is.False,
                    "UI cannot advance an unfinished level");
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Save), Is.True);
                Assert.That(new FileInfo(save).Length, Is.GreaterThan(0));
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Title), Is.True);
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.TitleMode));
                Assert.That(runtime.StepGameRuntimeTitleMedia(0, false, false), Is.True,
                    "direct-game return binds media and drains input once");
                var directTitleMedia = runtime.GameRuntimeProductionIdentity.frontend_media;
                Assert.That(directTitleMedia.active, Is.False, "return to Title skips cold welcome/intro");
                Assert.That(Array.Exists(directTitleMedia.actions, action => action.op == "M" && action.value == 4),
                    Is.True, "direct-game Title restores original menu music");
                runtime.StepGameRuntimeTitleMedia(50_000, false, false);
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.NewGame), Is.True);
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.CampaignMode));
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.StartMission), Is.True);
                Assert.That(runtime.GameRuntimeLoadingPresentationVisible, Is.True);
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.LoadingMode));
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.NewGame), Is.False,
                    "double clicks cannot issue another New Game while loading");
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.LoadingMode));
                runtime.QualificationObserveGameRuntimeLoadingPresentationFrame();
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(runtime.GameRuntimeProductionIdentity.runtime_id, Is.EqualTo(runtimeId));
                Assert.That(runtime.GameRuntimeLoadingPresentationVisible, Is.False);
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Title), Is.True);
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Load), Is.True);
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                Assert.That(runtime.GameRuntimeProductionCampaignSnapshot.value.state_hash, Is.EqualTo(paused));
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Resume), Is.True);
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(runtime.GameRuntimeProductionIdentity.runtime_id, Is.Not.EqualTo(runtimeId),
                    "Load replaced the Rust runtime that owned the original media catalog");
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Title), Is.True);
                runtime.StepGameRuntimeTitleMedia(0, false, false);
                var loadedTitleMedia = runtime.GameRuntimeProductionIdentity.frontend_media;
                Assert.That(loadedTitleMedia.error, Is.Null.Or.Empty);
                Assert.That(loadedTitleMedia.active, Is.False);
                Assert.That(Array.Exists(loadedTitleMedia.actions, action => action.op == "M" && action.value == 4),
                    Is.True, "loaded runtime installs its own catalog and restores menu music");
                runtime.StepGameRuntimeTitleMedia(50_000, false, false);
                for (var frame = 0; frame < 13; frame++)
                    runtime.StepGameRuntimeTitleMedia(5_000_000, false, false);
                Assert.That(runtime.GameRuntimeProductionIdentity.frontend_media.active, Is.True);
                Assert.That(runtime.GameRuntimeProductionIdentity.frontend_media.clip_id, Is.EqualTo("intro1"),
                    "the loaded owner's first idle attract sequence starts normally");
            }
            finally
            {
                runtime.QualificationDisposeGameRuntimeProduction();
                UnityEngine.Object.DestroyImmediate(root);
                if (File.Exists(save)) File.Delete(save);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(before));
        }

        [Serializable]
        private sealed class RecordedCastInput
        {
            public bool cast_left;
            public bool cast_right;
            public bool cast_selected;
        }

        [Serializable]
        private sealed class RecordedInputOperation
        {
            public string operation;
            public RecordedCastInput command;
        }

        [Serializable]
        private sealed class RecordedInputReplay
        {
            public RecordedInputOperation[] operations;
        }

        [Test]
        public void ProductionCampaignForwardsBothSpellHandsOnceAndClearsPausedInput()
        {
            var root = new GameObject("campaign two-hand input");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                runtime.QualificationApplyGameRuntimeProductionInput(false, false, true);
                runtime.QualificationDriveGameRuntimeFrames(1);
                runtime.QualificationDriveGameRuntimeFrames(1);
                runtime.QualificationApplyGameRuntimeProductionInput(false, true, false);
                runtime.QualificationDriveGameRuntimeFrames(1);
                runtime.QualificationApplyGameRuntimeProductionInput(true, true, true);
                runtime.QualificationApplyGameRuntimeProductionInput(false, true, true);
                runtime.QualificationApplyGameRuntimeProductionInput(true, true, true);
                runtime.QualificationDriveGameRuntimeFrames(1);
                var replay = JsonUtility.FromJson<RecordedInputReplay>(Encoding.UTF8.GetString(
                    runtime.QualificationSaveGameRuntimeProductionReplay()));
                Assert.That(replay.operations, Has.Length.EqualTo(4));
                foreach (var operation in replay.operations)
                    Assert.That(operation.operation, Is.EqualTo("step"));
                Assert.That(replay.operations[0].command.cast_right, Is.True);
                Assert.That(replay.operations[0].command.cast_left, Is.False);
                Assert.That(replay.operations[1].command.cast_right, Is.False, "one-shot latch");
                Assert.That(replay.operations[1].command.cast_left, Is.False);
                Assert.That(replay.operations[2].command.cast_left, Is.True);
                Assert.That(replay.operations[2].command.cast_right, Is.False);
                Assert.That(replay.operations[3].command.cast_left, Is.False, "pause clears input");
                Assert.That(replay.operations[3].command.cast_right, Is.False);
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void ProductionCampaignRetainsHeldHandsAcrossFixedStepsAndReleasesOnPause()
        {
            var root = new GameObject("campaign held hand input");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                runtime.ApplyGameRuntimeHeldSpellInput(true, true);
                runtime.QualificationDriveGameRuntimeFrames(2);
                runtime.ApplyGameRuntimeHeldSpellInput(true, true);
                runtime.ApplyGameRuntimeHeldSpellInput(false, false);
                runtime.QualificationDriveGameRuntimeFrames(1);
                // Both render samples can precede the next fixed step.
                runtime.ApplyGameRuntimeHeldSpellInput(true, true);
                runtime.ApplyGameRuntimeHeldSpellInput(false, false);
                runtime.QualificationDriveGameRuntimeFrames(2);
                runtime.ApplyGameRuntimeHeldSpellInput(true, true);
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                runtime.ApplyGameRuntimeHeldSpellInput(true, true);
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                runtime.QualificationDriveGameRuntimeFrames(1);
                var replay = JsonUtility.FromJson<RecordedInputReplay>(Encoding.UTF8.GetString(
                    runtime.QualificationSaveGameRuntimeProductionReplay()));
                Assert.That(replay.operations, Has.Length.EqualTo(6));
                var expected = new[] { true, true, false, true, false, false };
                for (var index = 0; index < expected.Length; index++)
                {
                    Assert.That(replay.operations[index].operation, Is.EqualTo("step"));
                    Assert.That(replay.operations[index].command.cast_left, Is.EqualTo(expected[index]),
                        $"left hand at step {index}: hold, release, brief click, pause");
                    Assert.That(replay.operations[index].command.cast_right, Is.EqualTo(expected[index]),
                        $"right hand at step {index}: hold, release, brief click, pause");
                }
                runtime.ApplyGameRuntimeHeldSpellInput(true, true);
                runtime.QualificationResetGameRuntimeProduction();
                runtime.QualificationDriveGameRuntimeFrames(1);
                var reset = JsonUtility.FromJson<RecordedInputReplay>(Encoding.UTF8.GetString(
                    runtime.QualificationSaveGameRuntimeProductionReplay()));
                var last = reset.operations[reset.operations.Length - 1].command;
                Assert.That(last.cast_left || last.cast_right, Is.False, "reset clears held hands");
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        private static string PrivateRoot =>
            Path.GetFullPath(Path.Combine(Application.dataPath, "../.."));

        private static string LevelOnePackage =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_LEVEL1_PACKAGE") ??
            Path.Combine(PrivateRoot, "generated/mc-cf15-level1-package-v3-current");

        private static string LevelTwoPackage =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_PACKAGE") ??
            Path.Combine(
                PrivateRoot,
                "generated/mc-cf02-campaign-package-v2-b/qualified-level2");

        private static string LevelThreePackage =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_PACKAGE_LEVEL3_V1") ??
            Path.Combine(
                PrivateRoot,
                "generated/mc-cf14-campaign-package-v1-a/qualified-level3");

        private static string LevelTwoOwnerCache =>
            Path.Combine(PrivateRoot, "generated/level2-day-owner-cache");

        private static string LevelThreeOwnerCache =>
            Path.Combine(PrivateRoot, "generated/level3-day-owner-cache");

        private static string LevelThreeDayCache =>
            Path.Combine(PrivateRoot, "generated/level3-day-material-cache");

        private static string LevelOneOwnerCache =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_LEVEL1_CACHE") ??
            Path.Combine(
                PrivateRoot,
                "generated/level1-unity-asset-cache-plan29-final");

        private static string CampaignMenuCache =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_MENU_CACHE") ??
            Path.Combine(PrivateRoot, "generated/mc-shared-source-map-motion-20261026");

        private static string CampaignSpeechCache =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_SPEECH_CACHE") ??
            Path.Combine(PrivateRoot, "generated/mc2-campaign-speech-current");

        private static string OpenedLevelTwoReplay =>
            Path.Combine(PrivateRoot, "generated/opened-l2-runtime-replay-v5.json");

        private static string OpenedLevelThreeReplay =>
            Path.Combine(PrivateRoot, "generated/opened-l3-runtime-replay-v5.json");

        private static GameRuntimeTransport.InstalledPackageResource Package(string path)
        {
            return new GameRuntimeTransport.InstalledPackageResource
            {
                package_dir = path,
                owner_cache_path = null
            };
        }

        private static GameRuntimeTransport.InstalledPackageResource[] ResolverPackages()
        {
            return new[]
            {
                Package(LevelThreePackage),
                Package(LevelOnePackage),
                Package(LevelTwoPackage)
            };
        }

        private static string[] RuntimeArguments(params string[] packages)
        {
            var arguments = new string[1 + packages.Length * 2];
            arguments[0] = "-mc-game-runtime";
            for (var index = 0; index < packages.Length; index++)
            {
                arguments[1 + index * 2] = "-mc-runtime-package";
                arguments[2 + index * 2] = packages[index];
            }
            return arguments;
        }

        private static string[] RuntimeReplayArguments(
            string replayPath,
            params string[] packages)
        {
            var arguments = RuntimeArguments(packages);
            Array.Resize(ref arguments, arguments.Length + 2);
            arguments[arguments.Length - 2] = "-mc-runtime-replay-load";
            arguments[arguments.Length - 1] = replayPath;
            return arguments;
        }

        private static string[] OpenedLevelTwoReplayArguments()
        {
            return new[]
            {
                "-mc-game-runtime",
                "-mc-runtime-package", LevelThreePackage,
                "-mc-runtime-package", LevelOnePackage,
                "-mc-runtime-package", LevelTwoPackage,
                "-mc-runtime-presentation-cache", LevelTwoOwnerCache,
                "-mc-runtime-replay-load", OpenedLevelTwoReplay
            };
        }

        private static string[] OpenedLevelThreeReplayArguments()
        {
            return new[]
            {
                "-mc-game-runtime",
                "-mc-runtime-package", LevelThreePackage,
                "-mc-runtime-owner-cache", LevelThreeDayCache,
                "-mc-runtime-presentation-cache", LevelThreeOwnerCache,
                "-mc-runtime-package", LevelOnePackage,
                "-mc-runtime-package", LevelTwoPackage,
                "-mc-runtime-presentation-cache", LevelTwoOwnerCache,
                "-mc-runtime-loading-cache", LevelOneOwnerCache,
                "-mc-runtime-replay-load", OpenedLevelThreeReplay
            };
        }

        private static string[] RuntimeReplaySaveArguments(
            string replayPath,
            params string[] packages)
        {
            var arguments = RuntimeArguments(packages);
            Array.Resize(ref arguments, arguments.Length + 2);
            arguments[arguments.Length - 2] = "-mc-runtime-replay-save";
            arguments[arguments.Length - 1] = replayPath;
            return arguments;
        }

        private static string[] PlayableLevelArguments(int externalLevel, string replayPath = null)
        {
            if (externalLevel == 1)
            {
                return string.IsNullOrWhiteSpace(replayPath)
                    ? RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage)
                    : RuntimeReplayArguments(
                        replayPath,
                        LevelThreePackage,
                        LevelOnePackage,
                        LevelTwoPackage);
            }
            var arguments = externalLevel == 2
                ? OpenedLevelTwoReplayArguments()
                : externalLevel == 3
                    ? OpenedLevelThreeReplayArguments()
                    : throw new ArgumentOutOfRangeException(nameof(externalLevel));
            if (!string.IsNullOrWhiteSpace(replayPath))
            {
                arguments[^1] = replayPath;
            }
            return arguments;
        }

        private static void AssertPlayableLevelPresentation(
            MagicCarpetRuntime runtime,
            int externalLevel)
        {
            Assert.That(runtime.GameRuntimeProductionStartupReady, Is.True);
            Assert.That(runtime.GameRuntimeActiveLevelPresentationInstanceId, Is.Not.Zero);
            if (externalLevel == 1)
            {
                Assert.That(runtime.GameRuntimeProductionFireballTexture, Is.Not.Null);
            }
            else if (externalLevel == 2)
            {
                Assert.That(runtime.GameRuntimeLevel2PresentationReady, Is.True);
            }
            else
            {
                Assert.That(runtime.GameRuntimeLevel3PresentationReady, Is.True);
                Assert.That(runtime.Level2ProductionDayHudPresenterReady, Is.True,
                    "Level 3 consumes the shared drawable Day HUD, not just cached textures");
                Assert.That(runtime.QualificationHasMaterializedLevel2DayHudSprite(1), Is.True);
            }
        }

        private static bool ContainsInstance(GameObject root, int instanceId)
        {
            return Array.Exists(
                root.GetComponentsInChildren<Transform>(true),
                candidate => candidate.GetInstanceID() == instanceId);
        }

        private static byte[] PackageSetJson(
            params GameRuntimeTransport.InstalledPackageResource[] packages)
        {
            using var stream = new MemoryStream();
            new DataContractJsonSerializer(typeof(GameRuntimeTransport.InstalledPackageSetRequest)).WriteObject(stream,
                new GameRuntimeTransport.InstalledPackageSetRequest
                {
                    request_contract = GameRuntimeTransport.InstalledPackageSetContract,
                    packages = packages
                });
            return stream.ToArray();
        }

        private static void AssertSameCampaignState(
            EngineSessionTransport.CampaignSnapshotObservation expected,
            EngineSessionTransport.CampaignSnapshotObservation actual)
        {
            Assert.That(actual.value.effect_head, Is.EqualTo(expected.value.effect_head));
            Assert.That(actual.value.state_hash, Is.EqualTo(expected.value.state_hash));
            Assert.That(actual.value.terrain_hash, Is.EqualTo(expected.value.terrain_hash));
            Assert.That(
                actual.value.effect_journal_sha256,
                Is.EqualTo(expected.value.effect_journal_sha256));
            Assert.That(actual.value.player_pose.x, Is.EqualTo(expected.value.player_pose.x));
            Assert.That(actual.value.player_pose.y, Is.EqualTo(expected.value.player_pose.y));
            Assert.That(actual.value.player_pose.z, Is.EqualTo(expected.value.player_pose.z));
            Assert.That(actual.value.player_pose.yaw, Is.EqualTo(expected.value.player_pose.yaw));
            Assert.That(
                actual.value.player_pose.display_pitch,
                Is.EqualTo(expected.value.player_pose.display_pitch));
            Assert.That(
                actual.value.player_pose.effective_pitch,
                Is.EqualTo(expected.value.player_pose.effective_pitch));
            Assert.That(actual.value.player_pose.roll, Is.EqualTo(expected.value.player_pose.roll));
            Assert.That(
                actual.value.player_motion.target_speed,
                Is.EqualTo(expected.value.player_motion.target_speed));
            Assert.That(
                actual.value.player_motion.actual_speed,
                Is.EqualTo(expected.value.player_motion.actual_speed));
            Assert.That(
                actual.value.player_motion.strafe_speed,
                Is.EqualTo(expected.value.player_motion.strafe_speed));
            Assert.That(
                actual.value.player_vitals.life,
                Is.EqualTo(expected.value.player_vitals.life));
            Assert.That(
                actual.value.player_vitals.max_life,
                Is.EqualTo(expected.value.player_vitals.max_life));
            Assert.That(
                actual.value.player_vitals.mana,
                Is.EqualTo(expected.value.player_vitals.mana));
            Assert.That(
                actual.value.player_vitals.max_mana,
                Is.EqualTo(expected.value.player_vitals.max_mana));
            Assert.That(
                actual.value.objective_progress.current_objective,
                Is.EqualTo(expected.value.objective_progress.current_objective));
            Assert.That(
                actual.value.objective_progress.completed_count,
                Is.EqualTo(expected.value.objective_progress.completed_count));
            Assert.That(
                actual.value.objective_progress.total_count,
                Is.EqualTo(expected.value.objective_progress.total_count));
            Assert.That(
                actual.value.objective_progress.all_objectives_satisfied,
                Is.EqualTo(expected.value.objective_progress.all_objectives_satisfied));
        }

        private static int SignedAngle11(short value)
        {
            var wrapped = value & 0x07ff;
            return wrapped >= 1024 ? wrapped - 2048 : wrapped;
        }

        private static Quaternion ExpectedRuntimeCameraRotation(
            EngineSessionTransport.Pose pose)
        {
            return Quaternion.Euler(
                -SignedAngle11(pose.display_pitch) * (360f / 2048f),
                180f - pose.yaw * (360f / 2048f),
                -SignedAngle11(pose.roll) * (360f / 2048f));
        }

        [Test]
        [Timeout(180000)]
        public void RustGameRuntimeOwnsTitleLoadingAndRealLevelOnePlaying()
        {
            var runtime = new GameRuntimeTransport();
            try
            {
                Assert.That(
                    runtime.NegotiatedContract.runtime_contract,
                    Is.EqualTo(GameRuntimeTransport.RuntimeContract));
                Assert.That(
                    runtime.NegotiatedContract.installed_package_set.request_contract,
                    Is.EqualTo(GameRuntimeTransport.InstalledPackageSetContract));
                Assert.That(runtime.NegotiatedContract.definition_contract,
                    Is.EqualTo(GameRuntimeTransport.DefinitionContract));
                Assert.That(runtime.NegotiatedContract.semantic_contract_version,
                    Is.EqualTo(GameRuntimeTransport.SemanticContractVersion));
                var title = runtime.IdentitySnapshot();
                Assert.That(title.mode, Is.EqualTo(GameRuntimeTransport.TitleMode));
                Assert.That(title.generation, Is.EqualTo("1"));
                Assert.That(title.pending_definition.definition_hash, Is.Null.Or.Empty);
                Assert.That(
                    title.pending_definition.runtime_flow?.game_id,
                    Is.Null.Or.Empty);
                Assert.That(title.active_runtime_flow?.game_id, Is.Null.Or.Empty);
                Assert.That(title.session.session_id, Is.Null.Or.Empty);
                Assert.That(title.presentation.cue_id,
                    Is.EqualTo(GameRuntimeTransport.TitlePresentationCue));
                Assert.That(title.presentation.phase,
                    Is.EqualTo(GameRuntimeTransport.TitlePresentationPhase));

                title.pending_definition.runtime_flow =
                    new GameRuntimeTransport.RuntimeFlowDefinition
                    {
                        bound_level_id = "level.partial.binding"
                    };
                Assert.Throws<InvalidOperationException>(() =>
                    GameRuntimeTransport.ValidateIdentity(title, runtime.Handle));
                title = runtime.IdentitySnapshot();

                title.mode = GameRuntimeTransport.PlayingMode;
                Assert.That(
                    runtime.IdentitySnapshot().mode,
                    Is.EqualTo(GameRuntimeTransport.TitleMode));

                var packageSet = PackageSetJson(ResolverPackages());
                Assert.That(
                    NativeSim.mc_game_runtime_bootstrap_package_set_json_v1(
                        runtime.Handle,
                        2,
                        packageSet,
                        checked((uint)packageSet.Length)),
                    Is.EqualTo(NativeSim.GenerationMismatch));
                Assert.That(
                    runtime.IdentitySnapshot().mode,
                    Is.EqualTo(GameRuntimeTransport.TitleMode));

                using (var laterRuntime = new GameRuntimeTransport())
                {
                    var later = Assert.Throws<GameRuntimeTransport.TransitionException>(
                        () => laterRuntime.BootstrapPackageSet(
                            Package(LevelTwoPackage),
                            Package(LevelOnePackage)));
                    Assert.That(later.Status, Is.EqualTo(NativeSim.ValidationFailed));
                    Assert.That(
                        laterRuntime.IdentitySnapshot().mode,
                        Is.EqualTo(GameRuntimeTransport.TitleMode));
                }

                var loading = runtime.BootstrapPackageSet(ResolverPackages());
                Assert.That(loading.mode, Is.EqualTo(GameRuntimeTransport.LoadingMode));
                Assert.That(loading.generation, Is.EqualTo("2"));
                Assert.That(loading.pending_definition.definition_hash, Is.Not.Empty);
                Assert.That(
                    loading.pending_definition.runtime_flow.game_id,
                    Is.EqualTo("game.magic_carpet_2"));
                Assert.That(
                    loading.pending_definition.runtime_flow.bound_level_id,
                    Is.EqualTo("level.magic_carpet_2.night_spire"));
                Assert.That(
                    loading.pending_definition.runtime_flow.entries.Length,
                    Is.EqualTo(3));
                Assert.That(loading.active_runtime_flow?.game_id, Is.Null.Or.Empty);
                Assert.That(loading.session.session_id, Is.Null.Or.Empty);
                Assert.That(loading.presentation.cue_id,
                    Is.EqualTo(
                        "presentation.magic_carpet_2.night_spire.loading"));
                Assert.That(loading.presentation.phase,
                    Is.EqualTo(GameRuntimeTransport.LoadingPresentationPhase));
                Assert.That(loading.presentation.level_id,
                    Is.EqualTo("level.magic_carpet_2.night_spire"));

                loading.active_runtime_flow = new GameRuntimeTransport.RuntimeFlowDefinition
                {
                    bound_level_id = "level.partial.binding"
                };
                Assert.Throws<InvalidOperationException>(() =>
                    GameRuntimeTransport.ValidateIdentity(loading, runtime.Handle));
                loading = runtime.IdentitySnapshot();

                var repeatedNewGame = Assert.Throws<GameRuntimeTransport.TransitionException>(
                    () => runtime.BootstrapPackageSet(ResolverPackages()));
                Assert.That(repeatedNewGame.Status, Is.EqualTo(NativeSim.InvalidArgument));
                Assert.That(runtime.IdentitySnapshot().mode, Is.EqualTo(loading.mode));
                Assert.That(
                    runtime.Current.pending_definition.definition_hash,
                    Is.EqualTo(loading.pending_definition.definition_hash));

                var playing = runtime.OpenLevel();
                Assert.That(playing.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(playing.generation, Is.EqualTo("3"));
                Assert.That(playing.pending_definition.definition_hash, Is.Null.Or.Empty);
                Assert.That(
                    playing.pending_definition.runtime_flow?.game_id,
                    Is.Null.Or.Empty);
                Assert.That(
                    playing.active_runtime_flow.game_id,
                    Is.EqualTo("game.magic_carpet_2"));
                Assert.That(
                    playing.active_runtime_flow.bound_level_id,
                    Is.EqualTo("level.magic_carpet_2.night_spire"));
                Assert.That(playing.session.session_id, Is.EqualTo(runtime.Handle.ToString()));
                Assert.That(playing.session.generation, Is.EqualTo("1"));
                Assert.That(playing.session.tick, Is.EqualTo("0"));
                Assert.That(playing.presentation.cue_id,
                    Is.EqualTo(
                        "presentation.magic_carpet_2.night_spire.entry"));
                Assert.That(playing.presentation.phase,
                    Is.EqualTo(GameRuntimeTransport.LevelEntryPresentationPhase));

                playing.pending_definition.runtime_flow =
                    new GameRuntimeTransport.RuntimeFlowDefinition
                    {
                        bound_level_id = "level.partial.binding"
                    };
                Assert.Throws<InvalidOperationException>(() =>
                    GameRuntimeTransport.ValidateIdentity(playing, runtime.Handle));
                playing = runtime.IdentitySnapshot();

                playing.active_runtime_flow.bound_level_id =
                    "level.magic_carpet_2.payahandra_ruins";
                Assert.That(
                    runtime.IdentitySnapshot().active_runtime_flow.bound_level_id,
                    Is.EqualTo("level.magic_carpet_2.night_spire"));

                var snapshot = runtime.CampaignSnapshot();
                Assert.That(snapshot.identity.session_id, Is.EqualTo(playing.session.session_id));
                Assert.That(snapshot.identity.tick, Is.EqualTo("0"));
                for (var poll = 0; poll < 5; poll++)
                {
                    var observed = runtime.IdentitySnapshot();
                    Assert.That(observed.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                    Assert.That(observed.generation, Is.EqualTo("3"));
                    Assert.That(observed.session.tick, Is.EqualTo("0"));
                }

                runtime.Dispose();
                runtime.Dispose();
                Assert.Throws<ObjectDisposedException>(() => runtime.IdentitySnapshot());

                // No-Claim: this proves complete-set admission and a
                // resolver-derived entry open only. It does not expose
                // completion acceptance, materialize later L2/L3 sessions,
                // discover/install packages, migrate production presentation,
                // or prove replay persistence, results, or shell wiring.
            }
            finally
            {
                runtime.Dispose();
            }
        }

        [Test]
        [Timeout(180000)]
        public void RuntimeCompletionTransitionRejectsInProgressAndStaleGuardsAtomically()
        {
            using (var runtime = new GameRuntimeTransport())
            {
                Assert.That(
                    runtime.NegotiatedContract.completion_transition.receipt_contract,
                    Is.EqualTo(GameRuntimeTransport.CompletionTransitionReceiptContract));
                runtime.BootstrapPackageSet(ResolverPackages());
                var before = runtime.OpenLevel();
                var rejected = Assert.Throws<GameRuntimeTransport.TransitionException>(
                    () => runtime.AcceptCompletionTransition());
                Assert.That(rejected.Status, Is.EqualTo(NativeSim.InvalidArgument));
                var after = runtime.IdentitySnapshot();
                Assert.That(after.mode, Is.EqualTo(before.mode));
                Assert.That(after.generation, Is.EqualTo(before.generation));
                Assert.That(after.session.generation, Is.EqualTo(before.session.generation));
                Assert.That(after.session.tick, Is.EqualTo(before.session.tick));

                Assert.That(
                    NativeSim.mc_game_runtime_accept_transition_v1(
                        runtime.Handle,
                        ulong.Parse(before.generation) - 1,
                        ulong.Parse(before.session.generation),
                        ulong.Parse(before.session.tick)),
                    Is.EqualTo(NativeSim.GenerationMismatch));
                Assert.That(
                    NativeSim.mc_game_runtime_accept_transition_v1(
                        runtime.Handle,
                        ulong.Parse(before.generation),
                        ulong.Parse(before.session.generation) + 1,
                        ulong.Parse(before.session.tick)),
                    Is.EqualTo(NativeSim.GenerationMismatch));
                Assert.That(
                    NativeSim.mc_game_runtime_accept_transition_v1(
                        runtime.Handle,
                        ulong.Parse(before.generation),
                        ulong.Parse(before.session.generation),
                        ulong.Parse(before.session.tick) + 1),
                    Is.EqualTo(NativeSim.InvalidArgument));
                Assert.That(
                    NativeSim.mc_game_runtime_completion_transition_receipt_json_v1(
                        runtime.Handle,
                        IntPtr.Zero,
                        0,
                        out _),
                    Is.EqualTo(NativeSim.InvalidArgument));
                var final = runtime.IdentitySnapshot();
                Assert.That(final.mode, Is.EqualTo(before.mode));
                Assert.That(final.generation, Is.EqualTo(before.generation));
                Assert.That(final.session.generation, Is.EqualTo(before.session.generation));
                Assert.That(final.session.tick, Is.EqualTo(before.session.tick));
            }
        }

        [Test]
        [Timeout(180000)]
        public void OpenedLevelTwoReplayKeepsLivePresentationOnOneRuntimeHandle()
        {
            Assert.That(File.Exists(OpenedLevelTwoReplay), Is.True,
                "focused owner-local opened-Level-2 replay fixture");
            var root = new GameObject("Game Runtime Level 2 Composition Test");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    OpenedLevelTwoReplayArguments());
                var opened = runtime.GameRuntimeProductionIdentity;
                Assert.That(opened.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(
                    opened.active_runtime_flow.bound_level_id,
                    Is.EqualTo("level.magic_carpet_2.payahandra_ruins"));
                var handle = ulong.Parse(opened.runtime_id);
                Assert.That(opened.session.session_id, Is.EqualTo(opened.runtime_id));
                Assert.That(ulong.Parse(opened.session.generation), Is.GreaterThan(0));
                Assert.That(runtime.GameRuntimeLevel2PresentationReady, Is.True);
                Assert.That(runtime.Level2ProductionWorldInfo.externalLevel, Is.EqualTo(2));
                Assert.That(runtime.Level2ProductionDayHudPresenterReady, Is.True);
                Assert.That(runtime.Level2ProductionMinimapProjectionReady, Is.True);

                runtime.QualificationApplyGameRuntimeProductionMovement(
                    accelerate: true,
                    decelerate: false,
                    strafeLeft: false,
                    strafeRight: false);
                var tickBefore = ulong.Parse(opened.session.tick);
                runtime.QualificationDriveGameRuntimeFrames(1);
                var stepped = runtime.GameRuntimeProductionIdentity;
                Assert.That(ulong.Parse(stepped.session.tick), Is.EqualTo(tickBefore + 1));
                Assert.That(runtime.GameRuntimeLevel2PresentationReady, Is.True);
                Assert.That(runtime.Level2ProductionDayHud.tick, Is.EqualTo(tickBefore + 1));

                Assert.That(
                    NativeSim.mc_game_runtime_accept_transition_v1(
                        handle,
                        ulong.Parse(opened.generation) - 1,
                        ulong.Parse(opened.session.generation),
                        ulong.Parse(opened.session.tick)),
                    Is.EqualTo(NativeSim.GenerationMismatch));
                Assert.That(
                    runtime.GameRuntimeProductionIdentity.active_runtime_flow.bound_level_id,
                    Is.EqualTo("level.magic_carpet_2.payahandra_ruins"));
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [UnityEngine.TestTools.UnityTest]
        [Timeout(180000)]
        public System.Collections.IEnumerator PaidCastleStatusReachesTheActualIndexedHudAfterColdAttach()
        {
            var activeBefore = NativeSession.ActiveSessionCount;
            var generated = Path.Combine(PrivateRoot, "generated");
            var levelOneCache = Environment.GetEnvironmentVariable("MC_CAMPAIGN_LEVEL1_CACHE") ??
                Path.Combine(generated, "mc2-level1-night-cache-v2");
            // The real Level 3 Loading control declares incoming Castle and
            // other spell carry. Old v5 receipts are not admissible.
            var entry = Environment.GetEnvironmentVariable("MC_CORRECTED_DAY_REPLAY_OUTPUT") ??
                Path.Combine(generated, "top-status-20261116/fresh-level3-full-role-entry.mcs");
            var second = Environment.GetEnvironmentVariable("MC_CASTLE_LEVEL2_PACKAGE") ??
                Path.Combine(generated, "level2-day-block32-campaign-v2");
            var package = Environment.GetEnvironmentVariable("MC_CASTLE_LEVEL3_PACKAGE") ??
                Path.Combine(generated, "level3-day-block32-campaign-recipe-v2");
            var environment = Environment.GetEnvironmentVariable("MC_CASTLE_LEVEL3_OWNER") ??
                Path.Combine(generated, "mc-level3-environment-exit-20261025");
            var presentation = Environment.GetEnvironmentVariable("MC_CASTLE_LEVEL3_PRESENTATION") ??
                Path.Combine(generated, "level3-day-spell-owner-cache-audio-v4");
            var temporary = Path.Combine(Path.GetTempPath(), "mc-source-top-status-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(temporary);
            var replayPath = Path.Combine(temporary, "paid-castle.mcs");
            var root = new GameObject("actual paid Castle indexed HUD");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var gui = ScriptableObject.CreateInstance<CampaignGuiWindow>();
            var display = new RenderTexture(320, 200, 0, RenderTextureFormat.ARGB32, RenderTextureReadWrite.Linear);
            var shown = false;
            byte[] paidReplay = null;
            var previousTarget = RenderTexture.active;
            const System.Reflection.BindingFlags flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
            object Field(string name) => typeof(MagicCarpetRuntime).GetField(name, flags).GetValue(runtime);
            try
            {
                using (var producer = new GameRuntimeTransport())
                {
                    producer.LoadReplay(File.ReadAllBytes(entry),
                        new GameRuntimeTransport.InstalledPackageResource { package_dir = LevelOnePackage,
                            presentation_cache_path = levelOneCache }, Package(second),
                        new GameRuntimeTransport.InstalledPackageResource { package_dir = package,
                            owner_cache_path = environment, presentation_cache_path = presentation });
                    var initial = producer.CampaignSnapshot().value;
                    Assert.That(initial.source_top_status.castle_rows, Is.Empty, "the qualified entry starts without a linked Castle");
                    Assert.That(initial.player_vitals.mana, Is.EqualTo(1000));
                    NativeSim.CampaignEntitySnapshot[] CurrentEntities()
                    {
                        var rows = new NativeSim.CampaignEntitySnapshot[1000];
                        Assert.That(NativeSim.mc_native_campaign_entities_copy(producer.Handle, 0, rows, (uint)rows.Length,
                            (uint)Marshal.SizeOf<NativeSim.CampaignEntitySnapshot>(), out var written), Is.EqualTo(NativeSim.Ok));
                        Assert.That(written, Is.EqualTo((uint)rows.Length));
                        return rows;
                    }
                    var player = Array.Find(CurrentEntities(), row => row.active != 0 && row.classId == 3 && row.model == 0);
                    Assert.That(player.entityIndex, Is.Not.Zero, "the current source player has a physical identity");
                    var trace = new System.Collections.Generic.List<string>();
                    void Record(string boundary)
                    {
                        var current = producer.CampaignSnapshot().value;
                        var selected = Array.Find(current.player_spells, spell => spell?.id == "spell.castle");
                        var entities = new NativeSim.CampaignEntitySnapshot[1000];
                        var code = NativeSim.mc_native_campaign_entities_copy(producer.Handle, 0, entities,
                            (uint)entities.Length, (uint)Marshal.SizeOf<NativeSim.CampaignEntitySnapshot>(), out var written);
                        var relevant = code == NativeSim.Ok ? string.Join(",", Array.ConvertAll(
                            Array.FindAll(entities, row => row.classId == 9 && row.model == 10 ||
                                row.active != 0 && (row.classId == 3 && row.model == 2 || row.classId == 10 && row.model == 45)),
                            row => $"{row.entityIndex}/{row.classId}/{row.model}/active{row.active}/source{row.sourceId}/action{row.action}/phase{row.phase}@{row.x},{row.y},{row.z}")) : "unavailable";
                        trace.Add($"{boundary}: tick={current.tick} spell={(selected == null ? "absent" : JsonUtility.ToJson(selected))} " +
                            $"vitals={JsonUtility.ToJson(current.player_vitals)} pose={JsonUtility.ToJson(current.player_pose)} " +
                            $"motion={JsonUtility.ToJson(current.player_motion)} entities_status={code}/{written} [{relevant}]");
                    }
                    Record("entry");
                    byte[] ReadAngles()
                    {
                        var angles = new byte[256 * 256];
                        Assert.That(NativeSim.mc_native_campaign_terrain_plane_copy_u8(producer.Handle,
                            ulong.Parse(producer.Current.session.generation),
                            NativeSim.TerrainAngle, angles, (uint)angles.Length, out var written), Is.EqualTo(NativeSim.Ok));
                        Assert.That(written, Is.EqualTo((uint)angles.Length));
                        return angles;
                    }
                    int BlockedTiles(byte[] angles, int x, int y)
                    {
                        var count = 0;
                        for (var row = 0; row < 8; row++)
                        for (var column = 0; column < 8; column++)
                            if ((angles[((y - 8 + row) & 255) * 256 + ((x - 8 + column) & 255)] & 0x80) != 0) count++;
                        return count;
                    }
                    var initialPose = initial.player_pose;
                    Assert.That(initialPose.yaw, Is.Zero, "the declared source-left launch control uses actual yaw zero");
                    var startAngles = ReadAngles();
                    var startLeftX = unchecked((ushort)(initialPose.x - 256)) >> 8;
                    var startX = initialPose.x >> 8; var startY = initialPose.y >> 8;
                    Assert.That(BlockedTiles(startAngles, startLeftX, startY), Is.GreaterThan(0),
                        "the real entry terrain blocks the source-left 8x8 Castle admission scan");
                    Assert.That(BlockedTiles(startAngles, startX, startY), Is.GreaterThan(0),
                        "the real entry terrain also blocks strict ground-rollback launch");
                    // Source speed step16 and reverse limit -80 predict 2880
                    // units south in 38 reverse frames (16+32+48+64+34*80).
                    // The reset geometry's clear band is y53888..55168.
                    // Only public controls are submitted; no state is set.
                    for (var frame = 0; frame < 38; frame++) producer.CampaignStep(0, 0, 0x02, 0, 0);
                    Record("after_fixed_travel");
                    TestContext.Progress.WriteLine(trace[trace.Count - 1]);
                    producer.CampaignStep(0, 0, 0, 0, 0, fullStop: true);
                    var arrival = producer.CampaignSnapshot().value;
                    Record("after_public_full_stop");
                    TestContext.Progress.WriteLine(trace[trace.Count - 1]);
                    Assert.That(arrival.player_vitals.life, Is.GreaterThanOrEqualTo(0));
                    Assert.That(arrival.player_pose.yaw, Is.Zero);
                    Assert.That(arrival.player_motion.target_speed, Is.Zero);
                    Assert.That(arrival.player_motion.actual_speed, Is.Zero);
                    Assert.That(arrival.player_pose.x, Is.EqualTo(initialPose.x), "the actual flight stays on the source-inspected corridor");
                    Assert.That(arrival.player_pose.y, Is.InRange(53888, 55168), "the actual flight reaches the source-inspected clear band");
                    var arrivalAngles = ReadAngles();
                    var leftX = unchecked((ushort)(arrival.player_pose.x - 256)) >> 8;
                    var parentX = arrival.player_pose.x >> 8; var parentY = arrival.player_pose.y >> 8;
                    var footprint = $"entry=({initialPose.x},{initialPose.y}) tiles=({startLeftX},{startY})/({startX},{startY}); " +
                        $"arrival=({arrival.player_pose.x},{arrival.player_pose.y}) tiles=({leftX},{parentY})/({parentX},{parentY})";
                    string Scan(byte[] angles, int x, int y)
                    {
                        var cells = new System.Text.StringBuilder(128);
                        for (var row = 0; row < 8; row++)
                        for (var column = 0; column < 8; column++)
                            cells.Append(angles[((y - 8 + row) & 255) * 256 + ((x - 8 + column) & 255)].ToString("X2"));
                        return cells.ToString();
                    }
                    TestContext.Progress.WriteLine(footprint + " entry-left=" + Scan(startAngles, startLeftX, startY) +
                        " entry-rollback=" + Scan(startAngles, startX, startY) + " arrival-left=" + Scan(arrivalAngles, leftX, parentY) +
                        " arrival-rollback=" + Scan(arrivalAngles, parentX, parentY));
                    Assert.That(BlockedTiles(arrivalAngles, leftX, parentY), Is.Zero, "actual source-left terrain footprint: " + footprint);
                    Assert.That(BlockedTiles(arrivalAngles, parentX, parentY), Is.Zero, "actual rollback terrain footprint: " + footprint);
                    producer.CampaignStep(0, 0, 0, 0, 0, "spell.castle");
                    Record("selected");
                    producer.CampaignStep(0, 0, 0x40, 0, 0);
                    Record("cast_selected");
                    var minimumMana = initial.player_vitals.mana;
                    EngineSessionTransport.SourceCastleStatus built = null;
                    for (var turn = 0; turn < 64; turn++)
                    {
                        var current = producer.CampaignSnapshot().value;
                        if (turn == 1 || turn == 3 || turn == 15 || turn == 63) Record($"wait{turn}");
                        minimumMana = Math.Min(minimumMana, current.player_vitals.mana);
                        var castles = current.source_top_status.castle_rows;
                        if (castles.Length == 1 && castles[0].stage > 0)
                        { built = castles[0]; break; }
                        producer.CampaignStep(0, 0, 0, 0, 0);
                    }
                    Assert.That(built, Is.Not.Null, "public paid Castle input creates the live linked source owner; " + string.Join(" | ", trace));
                    Assert.That(minimumMana, Is.Zero, "the real 1000-mana Castle cost is consumed");
                    Assert.That(built.icon_selector, Is.EqualTo(43 + built.stage));
                    NativeSim.CampaignEntitySnapshot CurrentRoot() => Array.Find(CurrentEntities(), row =>
                        row.active != 0 && row.classId == 3 && row.model == 2 && row.sourceId == player.entityIndex);
                    var rootRow = CurrentRoot();
                    Assert.That(rootRow.entityIndex, Is.Not.Zero, "the paid Castle has a current source root and player binding");
                    var rootSlot = rootRow.entityIndex;
                    // Initial 37BC0 counts 19 down to -1 and then zero (20
                    // visits); one scheduling boundary and one root state2
                    // visit bound its ordinary transition to action4.
                    for (var frame = 0; frame < 22 && rootRow.action != 4; frame++)
                    {
                        producer.CampaignStep(0, 0, 0, 0, 0);
                        rootRow = CurrentRoot();
                        Assert.That(rootRow.entityIndex, Is.EqualTo(rootSlot), "the same live Castle reaches source action4");
                    }
                    Assert.That(rootRow.action, Is.EqualTo(4), "the original BUILD callback completes naturally");
                    // EndOfCastleProjectile runs sub_5FF50 on its next even
                    // phase: at most two actual root callbacks after action4.
                    for (var frame = 0; frame < 2 && !producer.CampaignSnapshot().value.source_top_status.balloons[0].present; frame++)
                    {
                        producer.CampaignStep(0, 0, 0, 0, 0);
                        Assert.That(CurrentRoot().entityIndex, Is.EqualTo(rootSlot));
                    }
                    var bornStatus = producer.CampaignSnapshot().value.source_top_status;
                    Assert.That(bornStatus.balloons[0].present, Is.True, "the same real Castle naturally produces its first collector");
                    var collectorRow = Array.Find(CurrentEntities(), row => row.active != 0 && row.classId == 3 &&
                        row.model == 3 && row.sourceId == player.entityIndex);
                    Assert.That(collectorRow.entityIndex, Is.Not.Zero, "the current collector is bound to the same source player");
                    Assert.That(bornStatus.balloons[0].max_life, Is.GreaterThan(0));
                    Assert.That(bornStatus.balloons[0].life, Is.InRange(0, bornStatus.balloons[0].max_life));
                    Assert.That(bornStatus.balloons[0].max_mana, Is.GreaterThan(0));
                    Assert.That(bornStatus.balloons[0].mana, Is.InRange(0, bornStatus.balloons[0].max_mana));
                    paidReplay = producer.SaveReplay();
                    File.WriteAllBytes(replayPath, paidReplay);
                    // Retain an unqualified diagnostic before cold-load so a
                    // failed receipt can be examined without rewriting it.
                    var diagnosticPath = Environment.GetEnvironmentVariable("MC_SOURCE_TOP_STATUS_DIAGNOSTIC_REPLAY_OUTPUT");
                    if (!string.IsNullOrWhiteSpace(diagnosticPath))
                    {
                        using var diagnostic = new FileStream(diagnosticPath, FileMode.CreateNew, FileAccess.Write, FileShare.None);
                        diagnostic.Write(paidReplay, 0, paidReplay.Length);
                        diagnostic.Flush(true);
                    }
                }
                var arguments = new[] {
                    "-mc-game-runtime", "-mc-runtime-loading-cache", levelOneCache,
                    "-mc-runtime-menu-cache", CampaignMenuCache, "-mc-runtime-speech-cache", CampaignSpeechCache,
                    "-mc-preferences-profile", Path.Combine(temporary, "profile"),
                    "-mc-runtime-package", LevelOnePackage,
                    "-mc-runtime-presentation-cache", levelOneCache,
                    "-mc-runtime-package", second,
                    "-mc-runtime-package", package, "-mc-runtime-owner-cache", environment,
                    "-mc-runtime-presentation-cache", presentation,
                    "-mc-runtime-replay-load", replayPath
                };
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(arguments);
                typeof(MagicCarpetRuntime).GetMethod("PreparePlayerPreferences", flags).Invoke(runtime, new object[] { arguments });
                var full = runtime.GameRuntimeProductionCampaignSnapshot.value.source_top_status;
                var compact = runtime.GameRuntimeProductionCampaignRenderObservation.value.source_top_status;
                Assert.That(full.castle_rows, Has.Length.EqualTo(1));
                Assert.That(JsonUtility.ToJson(compact), Is.EqualTo(JsonUtility.ToJson(full)),
                    "the actual compact and full native observations carry the same source status");
                Assert.That(compact.castle_rows, Has.Length.EqualTo(1));
                var status = compact.castle_rows[0];
                var actualTopStatus = compact;
                var actualCollector = actualTopStatus.balloons[0];
                Assert.That(actualCollector.present, Is.True, "the real first collector survives the cold owner load");
                Assert.That(actualCollector.life, Is.InRange(0, actualCollector.max_life));
                Assert.That(actualCollector.mana, Is.InRange(0, actualCollector.max_mana));
                Assert.That(actualCollector.max_life, Is.GreaterThan(0));
                Assert.That(actualCollector.max_mana, Is.GreaterThan(0));
                Assert.That(status.stage, Is.InRange(1, 7));
                Assert.That(status.icon_selector, Is.EqualTo(43 + status.stage));
                var sourceWidth = typeof(MagicCarpetRuntime).GetMethod("SourceTopStatusBarWidth", flags | System.Reflection.BindingFlags.Static);
                Assert.That((int)sourceWidth.Invoke(null, new object[] { 300000000, 300000000 }), Is.EqualTo(6),
                    "the source int32 shift wraps before division and int16 narrowing");
                Assert.That((int)sourceWidth.Invoke(null, new object[] { 100, 6400 }), Is.Zero,
                    "DrawBar suppresses an actual source width of one");
                var surface = (CampaignPresentationSurface)Field("level3PresentationSurface");
                Assert.That(surface, Is.Not.Null);
                var cameras = surface.FrameCamera.transform.parent.GetComponentsInChildren<Camera>();
                Array.Sort(cameras, (left, right) => left.depth.CompareTo(right.depth));
                foreach (var camera in cameras)
                {
                    if (camera.targetTexture == null) camera.targetTexture = display;
                    camera.Render();
                }
                var guiEvents = new System.Collections.Generic.List<string>();
                gui.DrawRuntime = () => {
                    var kind = Event.current.type;
                    typeof(MagicCarpetRuntime).GetMethod("OnGUI", flags).Invoke(runtime, null);
                    guiEvents.Add($"{kind}:{Field("gameRuntimeHudIndices") != null}");
                };
                gui.minSize = new Vector2(640, 480); gui.position = new Rect(0, 0, 640, 480);
                var beforeRenderTick = runtime.GameRuntimeProductionIdentity.session.tick;
                var beforeRenderReplay = runtime.QualificationSaveGameRuntimeProductionReplay();
                gui.Show(); shown = true; gui.position = new Rect(0, 0, 640, 480);
                yield return null;
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo(beforeRenderTick),
                    "the normal Editor frame does not advance the inactive runtime");
                CollectionAssert.AreEqual(beforeRenderReplay, runtime.QualificationSaveGameRuntimeProductionReplay(),
                    "the normal Editor frame preserves the real cold owner receipt");
                gui.SendEvent(new Event { type = EventType.Layout });
                gui.SendEvent(new Event { type = EventType.Repaint });
                var output = (RenderTexture)Field("gameRuntimeHudIndices");
                var directHud = false;
                var directIndexed = false;
                var directFailure = "";
                if (output == null)
                {
                    gui.DrawRuntime = () => {
                        if (Event.current.type != EventType.Repaint) return;
                        try
                        {
                            typeof(MagicCarpetRuntime).GetMethod("DrawLevel2DayHud", flags).Invoke(runtime, null);
                            directHud = Field("gameRuntimeHudIndices") != null;
                            typeof(MagicCarpetRuntime).GetField("gameRuntimeHudIndices", flags).SetValue(runtime, null);
                            typeof(MagicCarpetRuntime).GetMethod("DrawGameRuntimeIndexedHud", flags)
                                .Invoke(runtime, new object[] { runtime.Level2ProductionDayHud, false });
                            directIndexed = Field("gameRuntimeHudIndices") != null;
                        }
                        catch (Exception exception) { directFailure = exception.ToString(); }
                        finally { typeof(MagicCarpetRuntime).GetField("gameRuntimeHudIndices", flags).SetValue(runtime, null); }
                    };
                    gui.SendEvent(new Event { type = EventType.Repaint });
                }
                var currentHud = (NativeSim.CampaignDayHudSnapshot)Field("level2DayHud");
                var presentedHud = (NativeSim.CampaignDayHudSnapshot)Field("level2PresentedDayHud");
                var campaign = (NativeCampaignSession)Field("campaignSession");
                var menus = (SourceCampaignMenus)Field("gameRuntimeSourceMenus");
                var media = (FrontendMediaPresenter)Field("frontendMedia");
                var transition = (CampaignTransitionPresenter)Field("gameRuntimeTransitionPresenter");
                Assert.That(output, Is.Not.Null,
                    $"GUI draws={gui.DrawCount}, events={string.Join(",", guiEvents)}, active={root.activeInHierarchy}, " +
                    $"frame/media={Time.frameCount}/{Field("gameRuntimeCampaignMediaFrame")}, " +
                    $"HUD ready={Field("level2DayHudPresenterReady")}, top bar={runtime.GameRuntimeProductionIdentity.display_settings.top_bar}, " +
                    $"HUD current={currentHud.generation}/{currentHud.tick}, presented={presentedHud.generation}/{presentedHud.tick}, " +
                    $"campaign={campaign?.Generation}/{campaign?.Tick}, mode={runtime.GameRuntimeProductionIdentity.mode}, " +
                    $"menu map/spell/pending={menus?.MapOpen}/{menus?.SpellOpen}/{menus?.HasPendingAction}, " +
                    $"fatal={Field("fatalError")}, mediaVisible={media?.Visible}, transitionFailure={transition?.HasFailure}, " +
                    $"runtime/level2/level3/secretPending={Field("gameRuntimeMode")}/{Field("level2Mode")}/{Field("level3Mode")}/{Field("gameRuntimeSecretEntryPending")}, " +
                    $"nativeFinal={typeof(MagicCarpetRuntime).GetMethod("CurrentNativeFinalPresentationOwns", flags).Invoke(runtime, null)}, " +
                    $"directHud={directHud}, directIndexed={directIndexed}, directFailure={directFailure}, " +
                    $"surface gen/tick={surface.Generation}/{surface.Tick}, history gen/revision={surface.SceneHistoryGeneration}/{surface.SceneHistoryRevision}, " +
                    $"underlay={surface.HudUnderlayIndices != null}, cameras={cameras.Length}");
                var pixels = new Texture2D(output.width, output.height, TextureFormat.RGBA32, false, true);
                try
                {
                    RenderTexture.active = output;
                    pixels.ReadPixels(new Rect(0, 0, output.width, output.height), 0, 0); pixels.Apply();
                    var raw = pixels.GetPixels32();
                    var hud = runtime.Level2ProductionDayHud;
                    var descriptors = (System.Collections.IDictionary)Field("hudBitmapDescriptors");
                    var indices = (System.Collections.Generic.Dictionary<int, byte[]>)Field("hudBitmapIndices");
                    var masks = (System.Collections.Generic.Dictionary<int, byte[]>)Field("hudBitmapMasks");
                    int Extent(int selector, string member) => (int)descriptors[selector].GetType().GetField(member).GetValue(descriptors[selector]);
                    var castleX = 2 + Extent(hud.minimapPanelSelector, "runtime_width");
                    var balloonX = castleX + Extent(41, "runtime_width");
                    void AssertActualIcon(int selector, int x)
                    {
                        var sw = Extent(selector, "stored_width"); var sh = Extent(selector, "stored_height");
                        var rw = Extent(selector, "runtime_width"); var rh = Extent(selector, "runtime_height");
                        for (var dy = 0; dy < rh; dy++)
                        for (var dx = 0; dx < rw; dx++)
                        {
                            var source = dy * sh / rh * sw + dx * sw / rw;
                            if (masks[selector][source] == 0) continue;
                            Assert.That(raw[(479 - (2 + dy)) * output.width + x + 2 + dx].r,
                                Is.EqualTo(indices[selector][source]), $"actual source icon {selector} reaches the indexed HUD");
                            return;
                        }
                        Assert.Fail($"actual source icon {selector} has no visible pixels");
                    }
                    AssertActualIcon(status.icon_selector, castleX);
                    AssertActualIcon(compact.balloon_icon_selector, balloonX);
                    var lifeWidth = (int)sourceWidth.Invoke(null, new object[] { status.life, status.max_life });
                    Assert.That(lifeWidth, Is.GreaterThanOrEqualTo(2), "the real constructed Castle has a visible health bar");
                    Assert.That(raw[(479 - 10) * output.width + castleX + 58].r, Is.EqualTo(hud.lifePaletteIndex),
                        "the actual source Castle life reaches its indexed bar");
                    var realCollectorPixels = raw;
                    var collectorLifeWidth = (int)sourceWidth.Invoke(null, new object[] { actualCollector.life, actualCollector.max_life });
                    var collectorManaWidth = (int)sourceWidth.Invoke(null, new object[] { actualCollector.mana, actualCollector.max_mana });
                    Assert.That(collectorLifeWidth, Is.GreaterThanOrEqualTo(2), "the actual newborn collector has a visible life bar");

                    // Rare stage/row combinations are a detached presentation
                    // control. They do not mutate or advance the native owner.
                    var field = typeof(MagicCarpetRuntime).GetField("gameRuntimeCampaignSnapshot", flags);
                    var observed = (EngineSessionTransport.CampaignRenderObservation)field.GetValue(runtime);
                    var shallowClone = typeof(object).GetMethod("MemberwiseClone", flags);
                    var detached = new EngineSessionTransport.CampaignRenderObservation {
                        identity = observed.identity,
                        value = (EngineSessionTransport.CampaignRenderValue)shallowClone.Invoke(observed.value, null)
                    };
                    var normalDraw = gui.DrawRuntime;
                    var detachedHud = hud;
                    Color32[] DrawDetached()
                    {
                        gui.SendEvent(new Event { type = EventType.Repaint });
                        var target = (RenderTexture)Field("gameRuntimeHudIndices");
                        Assert.That(target, Is.Not.Null);
                        RenderTexture.active = target;
                        pixels.ReadPixels(new Rect(0, 0, target.width, target.height), 0, 0); pixels.Apply();
                        return pixels.GetPixels32();
                    }
                    try
                    {
                        field.SetValue(runtime, detached);
                        gui.DrawRuntime = () => typeof(MagicCarpetRuntime).GetMethod("DrawGameRuntimeIndexedHud", flags)
                            .Invoke(runtime, new object[] { detachedHud, false });
                        // Keep every real owner/scene field and omit only row0
                        // in this detached baseline to discriminate even a
                        // genuine zero-width collector mana bar.
                        var noCollector = (EngineSessionTransport.SourceTopStatus)shallowClone.Invoke(actualTopStatus, null);
                        noCollector.balloons = (EngineSessionTransport.SourceBalloonStatus[])actualTopStatus.balloons.Clone();
                        noCollector.balloons[0] = new EngineSessionTransport.SourceBalloonStatus();
                        detached.value.source_top_status = noCollector;
                        var noCollectorPixels = DrawDetached();
                        for (var x = 0; x < 64; x++)
                        {
                            var lifePixel = (479 - 12) * output.width + balloonX + 58 + x;
                            var manaPixel = (479 - 30) * output.width + balloonX + 58 + x;
                            Assert.That(realCollectorPixels[lifePixel].r,
                                Is.EqualTo(x < collectorLifeWidth ? hud.lifePaletteIndex : noCollectorPixels[lifePixel].r),
                                $"actual collector life source pixel {x}");
                            Assert.That(realCollectorPixels[manaPixel].r,
                                Is.EqualTo(x < collectorManaWidth ? hud.manaPaletteIndex : noCollectorPixels[manaPixel].r),
                                $"actual collector mana source pixel {x}");
                        }
                        detachedHud.globalManaDenominator = 10000;
                        detachedHud.colorPhase2 = 0;
                        detached.value.source_top_status = new EngineSessionTransport.SourceTopStatus {
                            castle_rows = Array.Empty<EngineSessionTransport.SourceCastleStatus>(),
                            balloon_panel_selector = 54, balloon_icon_selector = 0,
                            balloons = new[] { new EngineSessionTransport.SourceBalloonStatus(),
                                new EngineSessionTransport.SourceBalloonStatus(), new EngineSessionTransport.SourceBalloonStatus() }
                        };
                        var absent = DrawDetached();
                        detached.value.source_top_status = new EngineSessionTransport.SourceTopStatus {
                            castle_rows = new[] { new EngineSessionTransport.SourceCastleStatus { stage = 0, panel_selector = 54 } },
                            balloon_panel_selector = 41, balloon_icon_selector = 50,
                            balloons = new[] { new EngineSessionTransport.SourceBalloonStatus(),
                                new EngineSessionTransport.SourceBalloonStatus(), new EngineSessionTransport.SourceBalloonStatus() }
                        };
                        raw = DrawDetached();
                        AssertActualIcon(50, balloonX);
                        var distinct = false;
                        for (var y = 2; y < 46 && !distinct; y++)
                        for (var x = balloonX; x < balloonX + 128 && !distinct; x++)
                            distinct = !raw[(479 - y) * output.width + x].Equals(absent[(479 - y) * output.width + x]);
                        Assert.That(distinct, Is.True, "linked stage-zero Castle has a different balloon panel from absence");
                        detached.value.source_top_status = new EngineSessionTransport.SourceTopStatus {
                            castle_rows = new[] { new EngineSessionTransport.SourceCastleStatus { stage = 7, panel_selector = 41,
                                icon_selector = 50, life = 10000, max_life = 10000, mana = 10000, max_mana = 10000 } },
                            balloon_panel_selector = 41, balloon_icon_selector = 53,
                            balloons = new[] { new EngineSessionTransport.SourceBalloonStatus(),
                                new EngineSessionTransport.SourceBalloonStatus(), new EngineSessionTransport.SourceBalloonStatus() },
                            mana_goal_percent = 50, goal_palette_index = hud.manaPaletteIndex
                        };
                        var emptyRows = DrawDetached();
                        raw = emptyRows;
                        AssertActualIcon(53, balloonX);
                        var rows = detached.value.source_top_status.balloons;
                        rows[0] = new EngineSessionTransport.SourceBalloonStatus { present = true, life = 5000,
                            max_life = 10000, mana = 2500, max_mana = 10000 };
                        rows[2] = new EngineSessionTransport.SourceBalloonStatus { present = true, life = -1,
                            max_life = 10000, mana = 5000, max_mana = 10000 };
                        raw = DrawDetached();
                        byte At(int x, int y) => raw[(479 - y) * output.width + x].r;
                        byte Baseline(int x, int y) => emptyRows[(479 - y) * output.width + x].r;
                        Assert.That(At(balloonX + 58, 12), Is.EqualTo(hud.lifePaletteIndex));
                        Assert.That(At(balloonX + 58, 30), Is.EqualTo(hud.manaPaletteIndex));
                        Assert.That(At(balloonX + 58, 14), Is.EqualTo(Baseline(balloonX + 58, 14)), "missing middle slot is not compacted");
                        Assert.That(At(balloonX + 58, 16), Is.EqualTo(Baseline(balloonX + 58, 16)), "negative life skips only health");
                        Assert.That(At(balloonX + 58, 34), Is.EqualTo(hud.manaPaletteIndex), "negative-life row still paints mana");
                        Assert.That(At(castleX + 58 + 32, 26), Is.EqualTo(hud.manaPaletteIndex), "source goal tick retains its raw source position");
                        Assert.That(At(castleX + 58, 28), Is.EqualTo(hud.manaPaletteIndex), "full mana uses phase-zero colour");
                        detachedHud.colorPhase2 = 1;
                        raw = DrawDetached();
                        Assert.That(At(castleX + 58, 28), Is.EqualTo(hud.manaMaxPaletteIndex), "full mana uses phase-one colour");
                    }
                    finally
                    {
                        field.SetValue(runtime, observed);
                        gui.DrawRuntime = normalDraw;
                    }
                }
                finally { UnityEngine.Object.DestroyImmediate(pixels); RenderTexture.active = previousTarget; }
                var before = runtime.QualificationSaveGameRuntimeProductionReplay();
                var tick = runtime.GameRuntimeProductionIdentity.session.tick;
                gui.SendEvent(new Event { type = EventType.Repaint });
                gui.SendEvent(new Event { type = EventType.Repaint });
                CollectionAssert.AreEqual(before, runtime.QualificationSaveGameRuntimeProductionReplay(),
                    "repainting the source status does not advance gameplay or draw clocks");
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo(tick));
                var receiptPath = Environment.GetEnvironmentVariable("MC_SOURCE_TOP_STATUS_PAID_REPLAY_OUTPUT");
                if (!string.IsNullOrWhiteSpace(receiptPath))
                {
                    using var receipt = new FileStream(receiptPath, FileMode.CreateNew, FileAccess.Write, FileShare.None);
                    receipt.Write(paidReplay, 0, paidReplay.Length);
                    receipt.Flush(true);
                }
            }
            finally
            {
                RenderTexture.active = previousTarget;
                gui.DrawRuntime = null;
                if (shown) gui.Close(); else UnityEngine.Object.DestroyImmediate(gui);
                runtime.QualificationDisposeGameRuntimeProduction();
                display.Release(); UnityEngine.Object.DestroyImmediate(display);
                UnityEngine.Object.DestroyImmediate(root);
                Directory.Delete(temporary, true);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
        }

        [Test]
        [Timeout(180000)]
        public void LevelTwoPublicCastleCreatesRealWorldFeedRootAndSurvivesRuntimeReset()
        {
            // This fresh entry replay is produced by Rust's existing
            // corrected_day_recipe_replay_restores_its_exact_package test.
            // Its receipts bind the exact current player-entry recipe; the
            // older opened-Level-2 baseline fixture remains unchanged.
            var generated = Path.Combine(PrivateRoot, "generated");
            var entryReplay = Environment.GetEnvironmentVariable("MC_CASTLE_LEVEL2_REPLAY") ??
                Path.Combine(generated, "opened-l2-block32-castle-boot-v5-20261005.json");
            var levelTwo = Environment.GetEnvironmentVariable("MC_CASTLE_LEVEL2_PACKAGE") ??
                Path.Combine(generated, "level2-day-block32-campaign-v2");
            var levelThree = Environment.GetEnvironmentVariable("MC_CASTLE_LEVEL3_PACKAGE") ??
                Path.Combine(generated, "level3-day-block32-campaign-recipe-v2");
            Assert.That(File.Exists(entryReplay), Is.True,
                "generate a fresh exact Rust entry replay before this native Castle regression");
            using (var runtime = new GameRuntimeTransport())
            {
                runtime.LoadReplay(File.ReadAllBytes(entryReplay),
                    Package(LevelOnePackage), Package(levelTwo), Package(levelThree));
                var handle = runtime.Handle;
                Assert.That(runtime.Current.active_runtime_flow.bound_level_id,
                    Is.EqualTo("level.magic_carpet_2.payahandra_ruins"));
                var firstGeneration = ulong.Parse(runtime.Current.session.generation);

                NativeSim.CampaignEntitySnapshot[] ReadWorldFeed()
                {
                    var before = runtime.CampaignSnapshot();
                    var rows = new NativeSim.CampaignEntitySnapshot[1000];
                    Assert.That(NativeSim.mc_native_campaign_entities_copy(
                        handle, 0, rows, (uint)rows.Length,
                        (uint)Marshal.SizeOf<NativeSim.CampaignEntitySnapshot>(), out var written),
                        Is.EqualTo(NativeSim.Ok));
                    Assert.That(written, Is.EqualTo(rows.Length));
                    Assert.That(runtime.CampaignSnapshot().value.state_hash,
                        Is.EqualTo(before.value.state_hash), "world polling is observational");
                    return rows;
                }

                for (var attempt = 0; attempt < 2; attempt++)
                {
                    var initial = runtime.CampaignSnapshot();
                    Assert.That(initial.value.player_vitals.mana, Is.EqualTo(1000));
                    Assert.That(initial.value.player_vitals.max_mana, Is.EqualTo(1000));
                    Assert.That(Array.Exists(ReadWorldFeed(), row =>
                        row.active != 0 && row.classId == 3 && row.model == 2), Is.False,
                        "each generation starts without a constructed Castle");
                    runtime.CampaignStep(0, 0, 0, 0, 0, "spell.castle");
                    var launch = runtime.CampaignSnapshot();
                    var player = Array.Find(ReadWorldFeed(), row =>
                        row.active != 0 && row.classId == 3 && row.model == 0).entityIndex;
                    runtime.CampaignStep(0, 0, 0x40, 0, 0);
                    var sawFlyer = false;
                    var root = default(NativeSim.CampaignEntitySnapshot);
                    var minimumMana = 1000;
                    for (var turn = 0; turn < 64; turn++)
                    {
                        var rows = ReadWorldFeed();
                        sawFlyer |= Array.Exists(rows, row => row.active != 0 &&
                            row.classId == 9 && row.model == 10 && row.sourceId == player);
                        root = Array.Find(rows, row => row.active != 0 &&
                            row.classId == 3 && row.model == 2 && row.action == 4);
                        minimumMana = Math.Min(minimumMana,
                            runtime.CampaignSnapshot().value.player_vitals.mana);
                        if (sawFlyer && root.active != 0) break;
                        runtime.CampaignStep(0, 0, 0, 0, 0);
                    }
                    Assert.That(sawFlyer, Is.True,
                        $"generation {attempt}: public Castle did not expose its real flyer; " +
                        $"launch=({launch.value.player_pose.x},{launch.value.player_pose.y}," +
                        $"{launch.value.player_pose.z}) yaw={launch.value.player_pose.yaw}; " +
                        $"end={JsonUtility.ToJson(runtime.CampaignSnapshot())}");
                    Assert.That(root.active, Is.EqualTo(1),
                        "the real flyer must construct the stable Castle root within 64 turns");
                    Assert.That(minimumMana, Is.Zero,
                        "the next player source visit must consume the actual 1000-mana Castle cost");
                    Assert.That(CampaignWorldObjectProjection.TryDescribe(root, out var description), Is.True);
                    Assert.That(description.Kind, Is.EqualTo(CampaignWorldObjectKind.Castle));
                    Assert.That(description.AssetSelector, Is.EqualTo((ushort)96));
                    Assert.That(runtime.Handle, Is.EqualTo(handle));
                    Assert.That(runtime.Current.session.session_id, Is.EqualTo(handle.ToString()));
                    if (attempt == 0)
                    {
                        var reset = runtime.CampaignReset();
                        Assert.That(reset.identity.tick, Is.EqualTo("0"));
                        Assert.That(ulong.Parse(reset.identity.generation), Is.EqualTo(firstGeneration + 1));
                        Assert.That(runtime.Current.active_runtime_flow.bound_level_id,
                            Is.EqualTo("level.magic_carpet_2.payahandra_ruins"));
                    }
                }
            }
        }

        [Test]
        [Timeout(180000)]
        public void OpenedLevelThreeReplayKeepsQualifiedSurfaceOnOneRuntimeHandle()
        {
            Assert.That(File.Exists(OpenedLevelThreeReplay), Is.True,
                "owner-local replay is generated by the exact Rust Level 3 replay test");
            Assert.That(Directory.Exists(LevelThreeOwnerCache), Is.True,
                "strict 662-output Level 3 owner cache");
            var activeBefore = NativeSession.ActiveSessionCount;
            var root = new GameObject("Game Runtime Level 3 Composition Test");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var surfaceId = 0;
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    OpenedLevelThreeReplayArguments());
                var opened = runtime.GameRuntimeProductionIdentity;
                var handle = ulong.Parse(opened.runtime_id);
                var generation = ulong.Parse(opened.session.generation);
                var tick = ulong.Parse(opened.session.tick);
                Assert.That(opened.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(
                    opened.active_runtime_flow.bound_level_id,
                    Is.EqualTo("level.magic_carpet_2.revealed_way"));
                Assert.That(opened.session.session_id, Is.EqualTo(opened.runtime_id));
                Assert.That(runtime.GameRuntimeLevel3PresentationReady, Is.True);
                Assert.That(runtime.GameRuntimeLevel3PresentationGeneration,
                    Is.EqualTo(generation));
                Assert.That(runtime.GameRuntimeLevel3PresentationTick, Is.EqualTo(tick));
                Assert.That(runtime.GameRuntimeLevel3EntitySurfaceCount, Is.GreaterThan(0));
                Assert.That(runtime.GameRuntimeLevel3RenderedEntitySurfaceCount,
                    Is.GreaterThan(0));
                surfaceId = runtime.GameRuntimeLevel3PresentationInstanceId;
                Assert.That(surfaceId, Is.Not.EqualTo(0));

                runtime.QualificationApplyGameRuntimeProductionMovement(
                    accelerate: true,
                    decelerate: false,
                    strafeLeft: false,
                    strafeRight: false);
                runtime.QualificationDriveGameRuntimeFrames(1);
                var stepped = runtime.GameRuntimeProductionIdentity;
                Assert.That(stepped.runtime_id, Is.EqualTo(opened.runtime_id));
                Assert.That(stepped.session.session_id, Is.EqualTo(opened.session.session_id));
                Assert.That(stepped.session.generation, Is.EqualTo(opened.session.generation));
                Assert.That(ulong.Parse(stepped.session.tick), Is.EqualTo(tick + 1));
                Assert.That(runtime.GameRuntimeLevel3PresentationTick, Is.EqualTo(tick + 1));
                Assert.That(runtime.Level2ProductionDayHudPresenterReady, Is.True);

                var paused = runtime.QualificationPauseGameRuntimeProduction();
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(paused.runtime_id, Is.EqualTo(opened.runtime_id));
                Assert.That(paused.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                Assert.That(runtime.GameRuntimeLevel3PresentationTick, Is.EqualTo(tick + 1));
                var resumed = runtime.QualificationResumeGameRuntimeProduction();
                Assert.That(resumed.runtime_id, Is.EqualTo(opened.runtime_id));
                Assert.That(resumed.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));

                var replay = runtime.QualificationSaveGameRuntimeProductionReplay();
                Assert.That(replay.Length, Is.GreaterThan(0));
                runtime.QualificationResetGameRuntimeProduction();
                var reset = runtime.GameRuntimeProductionIdentity;
                Assert.That(reset.runtime_id, Is.EqualTo(opened.runtime_id));
                Assert.That(reset.session.session_id, Is.EqualTo(opened.session.session_id));
                Assert.That(ulong.Parse(reset.session.generation), Is.GreaterThan(generation));
                Assert.That(reset.session.tick, Is.EqualTo("0"));
                Assert.That(runtime.GameRuntimeLevel3PresentationGeneration,
                    Is.EqualTo(ulong.Parse(reset.session.generation)));
                Assert.That(runtime.GameRuntimeLevel3PresentationTick, Is.Zero);

                Assert.That(
                    NativeSim.mc_game_runtime_accept_transition_v1(
                        handle,
                        ulong.Parse(reset.generation) - 1,
                        ulong.Parse(reset.session.generation),
                        ulong.Parse(reset.session.tick)),
                    Is.EqualTo(NativeSim.GenerationMismatch));
                var afterStale = runtime.GameRuntimeProductionIdentity;
                Assert.That(afterStale.runtime_id, Is.EqualTo(reset.runtime_id));
                Assert.That(afterStale.generation, Is.EqualTo(reset.generation));
                Assert.That(afterStale.session.generation, Is.EqualTo(reset.session.generation));
                Assert.That(afterStale.session.tick, Is.EqualTo(reset.session.tick));

                Assert.Throws<InvalidDataException>(() =>
                    NativeCampaignSession.ReadPackageIdentity(
                        LevelTwoPackage,
                        NativeSim.CampaignSelection.Create(3, 2, 0)));
            }
            finally
            {
                runtime.QualificationDisposeGameRuntimeProduction();
                UnityEngine.Object.DestroyImmediate(root);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
            Assert.That(
                UnityEngine.Object.FindObjectsByType<CampaignEntitySurfaceMarker>(
                    FindObjectsSortMode.None),
                Is.Empty,
                $"Level 3 surface {surfaceId} is disposed with its runtime");
        }

        [Test]
        [Timeout(180000)]
        public void OpenedLevelThreeReplayCarriesNoAcquiredSpellAcrossLifecycleBoundaries()
        {
            var fixture = File.ReadAllBytes(OpenedLevelThreeReplay);
            StringAssert.Contains(
                "\"semantic_spell_carry\":{\"acquired\":[],\"selected\":null}",
                Encoding.UTF8.GetString(fixture));
            var packages = new[]
            {
                new GameRuntimeTransport.InstalledPackageResource
                {
                    package_dir = LevelThreePackage,
                    owner_cache_path = LevelThreeDayCache
                },
                Package(LevelOnePackage),
                Package(LevelTwoPackage)
            };

            static void AssertRejected(NativeSession session, string boundary)
            {
                var before = session.GameRuntimeCampaignSnapshot();
                var error = Assert.Throws<InvalidOperationException>(() =>
                    session.GameRuntimeCampaignStep(0, 0, 0, 0, 0, "spell.fireball"));
                StringAssert.Contains("spell.fireball is not acquired", error.Message, boundary);
                var after = session.GameRuntimeCampaignSnapshot();
                Assert.That(after.identity.generation, Is.EqualTo(before.identity.generation));
                Assert.That(after.identity.tick, Is.EqualTo(before.identity.tick));
                Assert.That(after.value.state_hash, Is.EqualTo(before.value.state_hash));
            }

            using (var initial = NativeSession.OpenGameRuntimeReplay(fixture, packages))
            {
                Assert.That(initial.GameRuntimeIdentity().mode,
                    Is.EqualTo(GameRuntimeTransport.PlayingMode));
                AssertRejected(initial, "immediately after initial opened-L3 fixture load");
            }

            byte[] pausedSave;
            string pausedTick;
            using (var source = NativeSession.OpenGameRuntimeReplay(fixture, packages))
            {
                Assert.That(source.GameRuntimePause().mode,
                    Is.EqualTo(GameRuntimeTransport.PausedMode));
                pausedTick = source.GameRuntimeIdentity().session.tick;
                pausedSave = source.GameRuntimeSaveReplay();
            }
            StringAssert.Contains(
                "\"semantic_spell_carry\":{\"acquired\":[],\"selected\":null}",
                Encoding.UTF8.GetString(pausedSave));
            using (var restored = NativeSession.OpenGameRuntimeReplay(pausedSave, packages))
            {
                Assert.That(restored.GameRuntimeIdentity().mode,
                    Is.EqualTo(GameRuntimeTransport.PausedMode));
                Assert.That(restored.GameRuntimeCampaignSnapshot().identity.tick,
                    Is.EqualTo(pausedTick));
                Assert.That(restored.GameRuntimeResume().mode,
                    Is.EqualTo(GameRuntimeTransport.PlayingMode));
                AssertRejected(restored, "after pause/save/load and resume");
            }

            using (var reset = NativeSession.OpenGameRuntimeReplay(fixture, packages))
            {
                var beforeGeneration =
                    ulong.Parse(reset.GameRuntimeIdentity().session.generation);
                reset.GameRuntimeCampaignReset();
                Assert.That(
                    ulong.Parse(reset.GameRuntimeIdentity().session.generation),
                    Is.GreaterThan(beforeGeneration));
                AssertRejected(reset, "after reset");
            }
        }

        [TestCase(1)]
        [TestCase(2)]
        [TestCase(3)]
        [Timeout(300000)]
        public void ProductionControlsRemainBoundAcrossVerifiedLevelBoundaries(
            int externalLevel)
        {
            Assert.That(File.Exists(OpenedLevelThreeReplay), Is.True,
                "owner-local opened-Level-3 replay boundary");
            Assert.That(Directory.Exists(LevelThreeOwnerCache), Is.True);

            var nativeSessionsBefore = NativeSession.ActiveSessionCount;
            var campaignSessionsBefore = NativeCampaignSession.ActiveSessionCount;
            var replayPath = Path.Combine(
                Path.GetTempPath(),
                $"magiccarpet-controls-level-{externalLevel}-{Guid.NewGuid():N}.json");
            GameObject sourceRoot = null;
            GameObject restoredRoot = null;
            try
            {
                    sourceRoot = new GameObject($"game runtime controls L{externalLevel} source");
                    sourceRoot.SetActive(false);
                    var source = sourceRoot.AddComponent<MagicCarpetRuntime>();
                    source.QualificationInitializeGameRuntimeProductionFromArguments(
                        PlayableLevelArguments(externalLevel));
                    var opened = source.GameRuntimeProductionIdentity;
                    var openedTick = ulong.Parse(opened.session.tick);
                    var beforeMotion = source.GameRuntimeProductionCampaignSnapshot;
                    AssertPlayableLevelPresentation(source, externalLevel);

                    source.QualificationApplyGameRuntimeProductionMovement(
                        accelerate: true,
                        decelerate: false,
                        strafeLeft: true,
                        strafeRight: false);
                    source.QualificationApplyGameRuntimeProductionSteering(24f, -8f);
                    source.QualificationDriveGameRuntimeFrames(3);
                    var afterMotion = source.GameRuntimeProductionCampaignSnapshot;
                    Assert.That(
                        ulong.Parse(afterMotion.identity.tick),
                        Is.EqualTo(openedTick + 3));
                    Assert.That(
                        afterMotion.value.player_motion.actual_speed !=
                            beforeMotion.value.player_motion.actual_speed ||
                        afterMotion.value.player_pose.yaw != beforeMotion.value.player_pose.yaw ||
                        afterMotion.value.player_pose.roll != beforeMotion.value.player_pose.roll,
                        Is.True,
                        $"Level {externalLevel} consumed ordinary movement and steering");

                    source.QualificationApplyGameRuntimeProductionInput(true, false);
                    var paused = source.GameRuntimeProductionIdentity;
                    var pausedSnapshot = source.GameRuntimeProductionCampaignSnapshot;
                    Assert.That(paused.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                    source.QualificationApplyGameRuntimeProductionMovement(true, false, false, true);
                    source.QualificationApplyGameRuntimeProductionSteering(64f, 64f);
                    source.QualificationApplyGameRuntimeProductionInput(false, true);
                    Assert.That(
                        source.QualificationSelectGameRuntimeProductionSpell("spell.fireball"),
                        Is.False,
                        "paused input cannot queue a semantic spell selection");
                    source.QualificationDriveGameRuntimeFrames(1);
                    var afterPausedInput = source.GameRuntimeProductionCampaignSnapshot;
                    Assert.That(afterPausedInput.identity.tick, Is.EqualTo(pausedSnapshot.identity.tick));
                    Assert.That(afterPausedInput.value.player_pose.x,
                        Is.EqualTo(pausedSnapshot.value.player_pose.x));
                    Assert.That(afterPausedInput.value.player_pose.y,
                        Is.EqualTo(pausedSnapshot.value.player_pose.y));
                    Assert.That(afterPausedInput.value.player_pose.z,
                        Is.EqualTo(pausedSnapshot.value.player_pose.z));

                    File.WriteAllBytes(
                        replayPath,
                        source.QualificationSaveGameRuntimeProductionReplay());
                    var sourcePresentation = source.GameRuntimeActiveLevelPresentationInstanceId;
                    source.QualificationDisposeGameRuntimeProduction();
                    Assert.That(ContainsInstance(sourceRoot, sourcePresentation), Is.False,
                        $"Level {externalLevel} source presentation disposed before replay load");
                    UnityEngine.Object.DestroyImmediate(sourceRoot);
                    sourceRoot = null;

                    restoredRoot = new GameObject(
                        $"game runtime controls L{externalLevel} restored");
                    restoredRoot.SetActive(false);
                    var restored = restoredRoot.AddComponent<MagicCarpetRuntime>();
                    restored.QualificationInitializeGameRuntimeProductionFromArguments(
                        PlayableLevelArguments(externalLevel, replayPath));
                    var replayed = restored.GameRuntimeProductionIdentity;
                    Assert.That(replayed.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                    Assert.That(
                        replayed.active_runtime_flow.bound_level_id,
                        Is.EqualTo(opened.active_runtime_flow.bound_level_id));
                    AssertPlayableLevelPresentation(restored, externalLevel);

                    restored.QualificationApplyGameRuntimeProductionInput(true, false);
                    Assert.That(
                        restored.GameRuntimeProductionIdentity.mode,
                        Is.EqualTo(GameRuntimeTransport.PlayingMode));
                    restored.QualificationDriveGameRuntimeFrames(1);
                    Assert.That(
                        Array.Exists(
                            restored.GameRuntimeProductionCampaignSnapshot.value.presentation_entities,
                            entity => entity.kind == "spell.fireball.projectile"),
                        Is.False,
                        "the fire input suppressed while Paused was not replayed after resume");

                    if (externalLevel != 3)
                    {
                        Assert.That(
                            restored.QualificationSelectGameRuntimeProductionSpell(
                                "spell.fireball"),
                            Is.True);
                        restored.QualificationDriveGameRuntimeFrames(1);
                        var eventCursorBeforeCast = ulong.Parse(
                            restored.GameRuntimeProductionSemanticEventCursor);
                        restored.QualificationRequestGameRuntimeSelectedSpellCast();
                        restored.QualificationDriveGameRuntimeFrames(1);
                        var afterCast = restored.GameRuntimeProductionCampaignSnapshot;
                        if (externalLevel == 1)
                        {
                            Assert.That(
                                Array.Exists(
                                    afterCast.value.presentation_entities,
                                    entity => entity.kind == "spell.fireball.projectile"),
                                Is.True,
                                "Level 1 presented its selected Fireball projectile");
                        }
                        else
                        {
                            Assert.That(
                                Array.Exists(
                                    restored.GameRuntimeProductionRecentSemanticEvents,
                                    gameEvent =>
                                        gameEvent.event_id == "spell.activation_requested" &&
                                        ulong.Parse(gameEvent.sequence) > eventCursorBeforeCast &&
                                        !string.IsNullOrWhiteSpace(gameEvent.actor)),
                                Is.True,
                                "Level 2 requested activation through its acquired Fireball holder");
                        }
                    }

                    var generationBeforeReset =
                        ulong.Parse(restored.GameRuntimeProductionIdentity.session.generation);
                    var presentationBeforeReset =
                        restored.GameRuntimeActiveLevelPresentationInstanceId;
                    restored.QualificationResetGameRuntimeProduction();
                    var reset = restored.GameRuntimeProductionIdentity;
                    Assert.That(
                        ulong.Parse(reset.session.generation),
                        Is.GreaterThan(generationBeforeReset));
                    Assert.That(reset.session.tick, Is.EqualTo("0"));
                    AssertPlayableLevelPresentation(restored, externalLevel);
                    Assert.That(
                        restored.GameRuntimeActiveLevelPresentationInstanceId,
                        Is.EqualTo(presentationBeforeReset),
                        "the active presenter rebound to the reset generation");

                    var runtimeId = reset.runtime_id;
                    var runtimeGeneration = ulong.Parse(reset.generation);
                    var activePresentation =
                        restored.GameRuntimeActiveLevelPresentationInstanceId;
                    restored.QualificationCloseGameRuntimeProduction();
                    var title = restored.GameRuntimeProductionIdentity;
                    Assert.That(title.runtime_id, Is.EqualTo(runtimeId));
                    Assert.That(title.mode, Is.EqualTo(GameRuntimeTransport.TitleMode));
                    Assert.That(
                        ulong.Parse(title.generation),
                        Is.EqualTo(runtimeGeneration + 1));
                    Assert.That(restored.GameRuntimeActiveLevelPresentationInstanceId, Is.Zero);
                    Assert.That(ContainsInstance(restoredRoot, activePresentation), Is.False,
                        $"Level {externalLevel} presentation disposed on close-to-Title");

                    restored.QualificationRestartGameRuntimeProduction();
                    var restarted = restored.GameRuntimeProductionIdentity;
                    Assert.That(restarted.runtime_id, Is.EqualTo(runtimeId));
                    Assert.That(restarted.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                    Assert.That(
                        restarted.active_runtime_flow.bound_level_id,
                        Is.EqualTo(restarted.active_runtime_flow.entry_level_id));
                    Assert.That(restored.GameRuntimeLevel2PresentationReady, Is.False);
                    Assert.That(restored.GameRuntimeLevel3PresentationReady, Is.False);
                    AssertPlayableLevelPresentation(restored, 1);
                    restored.QualificationDriveGameRuntimeFrames(1);
                    Assert.That(restored.GameRuntimeProductionIdentity.session.tick, Is.EqualTo("1"));
            }
            finally
            {
                if (sourceRoot != null)
                {
                    var source = sourceRoot.GetComponent<MagicCarpetRuntime>();
                    if (source != null)
                    {
                        source.QualificationDisposeGameRuntimeProduction();
                    }
                    UnityEngine.Object.DestroyImmediate(sourceRoot);
                }
                if (restoredRoot != null)
                {
                    var restored = restoredRoot.GetComponent<MagicCarpetRuntime>();
                    if (restored != null)
                    {
                        restored.QualificationDisposeGameRuntimeProduction();
                    }
                    UnityEngine.Object.DestroyImmediate(restoredRoot);
                }
                if (File.Exists(replayPath)) File.Delete(replayPath);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessionsBefore));
            Assert.That(
                NativeCampaignSession.ActiveSessionCount,
                Is.EqualTo(campaignSessionsBefore));

            // All first-three-level boundaries are admitted. The current
            // Level 3 boundary explicitly carries no acquired spells; the
            // focused lifecycle test above pins that Rust-owned package state.
        }

        [TestCase(1)]
        [TestCase(2)]
        [TestCase(3)]
        [Timeout(300000)]
        public void ProductionPublicResetAndQuitKeysDispatchAcrossPlayableLevels(
            int externalLevel)
        {
            var nativeSessionsBefore = NativeSession.ActiveSessionCount;
            var campaignSessionsBefore = NativeCampaignSession.ActiveSessionCount;
            var root = new GameObject(
                $"game runtime public keys L{externalLevel}");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    PlayableLevelArguments(externalLevel));
                runtime.QualificationApplyGameRuntimeProductionMovement(
                    accelerate: true,
                    decelerate: false,
                    strafeLeft: true,
                    strafeRight: false);
                runtime.QualificationApplyGameRuntimeProductionSteering(16f, -4f);
                runtime.QualificationDriveGameRuntimeFrames(2);

                var beforeReset = runtime.GameRuntimeProductionIdentity;
                var presentationBeforeReset =
                    runtime.GameRuntimeActiveLevelPresentationInstanceId;
                Assert.That(
                    runtime.QualificationDispatchGameRuntimePublicControlInput(
                        resetPressed: true,
                        quitPressed: false),
                    Is.False,
                    "R dispatches reset without requesting application quit");

                var afterReset = runtime.GameRuntimeProductionIdentity;
                Assert.That(
                    ulong.Parse(afterReset.session.generation),
                    Is.GreaterThan(ulong.Parse(beforeReset.session.generation)));
                Assert.That(afterReset.session.tick, Is.EqualTo("0"));
                Assert.That(
                    runtime.GameRuntimeActiveLevelPresentationInstanceId,
                    Is.EqualTo(presentationBeforeReset),
                    $"Level {externalLevel} presentation rebound to the reset generation");
                AssertPlayableLevelPresentation(runtime, externalLevel);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick,
                    Is.EqualTo("1"));

                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                var beforePausedReset = runtime.GameRuntimeProductionIdentity;
                Assert.That(beforePausedReset.mode,
                    Is.EqualTo(GameRuntimeTransport.PausedMode));
                Assert.That(
                    runtime.QualificationDispatchGameRuntimePublicControlInput(
                        resetPressed: true,
                        quitPressed: false),
                    Is.False);
                var afterPausedReset = runtime.GameRuntimeProductionIdentity;
                Assert.That(afterPausedReset.runtime_id,
                    Is.EqualTo(beforePausedReset.runtime_id));
                Assert.That(afterPausedReset.mode,
                    Is.EqualTo(GameRuntimeTransport.PausedMode),
                    "R resets through Rust while preserving the paused user mode");
                Assert.That(afterPausedReset.session.tick, Is.EqualTo("0"));
                Assert.That(ulong.Parse(afterPausedReset.session.generation),
                    Is.GreaterThan(ulong.Parse(beforePausedReset.session.generation)));
                Assert.That(
                    runtime.GameRuntimeProductionCampaignSnapshot.identity.generation,
                    Is.EqualTo(afterPausedReset.session.generation));
                AssertPlayableLevelPresentation(runtime, externalLevel);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick,
                    Is.EqualTo("0"), "reset does not accidentally resume gameplay");
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick,
                    Is.EqualTo("1"), "P resumes the freshly reset Rust session");

                var beforeQuit = runtime.GameRuntimeProductionIdentity;
                Assert.That(
                    runtime.QualificationDispatchGameRuntimePublicControlInput(
                        resetPressed: false,
                        quitPressed: true),
                    Is.True,
                    "F10 dispatches the application quit request without quitting the editor");
                var afterQuitRequest = runtime.GameRuntimeProductionIdentity;
                Assert.That(afterQuitRequest.runtime_id, Is.EqualTo(beforeQuit.runtime_id));
                Assert.That(afterQuitRequest.generation, Is.EqualTo(beforeQuit.generation));
                Assert.That(
                    afterQuitRequest.session.generation,
                    Is.EqualTo(beforeQuit.session.generation));
                Assert.That(afterQuitRequest.session.tick, Is.EqualTo(beforeQuit.session.tick));
            }
            finally
            {
                runtime.QualificationDisposeGameRuntimeProduction();
                UnityEngine.Object.DestroyImmediate(root);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessionsBefore));
            Assert.That(
                NativeCampaignSession.ActiveSessionCount,
                Is.EqualTo(campaignSessionsBefore));
        }

        [Test]
        [Timeout(180000)]
        public void ProductionLoadingGateWaitsForVisibleFrameBeforeRealRustOpen()
        {
            var root = new GameObject("Game Runtime Loading Gate Test");
            try
            {
                var cache = OwnerAssetCache.OpenAndValidate(
                    LevelOneOwnerCache,
                    MagicCarpetRuntime.ExpectedPackageHash,
                    MagicCarpetRuntime.ExpectedOwnerAssetClosureHash);
                using var presenter = new CampaignTransitionPresenter(
                    root.transform,
                    cache);
                using var transport = new GameRuntimeTransport();
                var gate = new CampaignTransitionPresenter.LoadingGate();

                var title = transport.IdentitySnapshot();
                presenter.Present(title);
                Assert.That(presenter.IsVisible, Is.False);

                var loading = transport.BootstrapPackageSet(ResolverPackages());
                GameRuntimeTransport.ValidateIdentity(loading, transport.Handle);
                Assert.That(loading.mode, Is.EqualTo(GameRuntimeTransport.LoadingMode));
                Assert.That(loading.session.session_id, Is.Null.Or.Empty);
                presenter.Present(loading);
                gate.Begin(
                    loading,
                    loading.pending_definition.runtime_flow.bound_level_id);
                Assert.That(presenter.IsVisible, Is.True);
                Assert.That(
                    presenter.IndicesSha256,
                    Is.EqualTo(
                        "fc27e10c722725e5d4e99ecc01a5bf1f0ff4269e9ff766fdbbad7f424860cc59"));
                Assert.That(
                    presenter.PaletteRgb6Sha256,
                    Is.EqualTo(
                        "7ced7d7bca9414b2890dc086c90efabf424de5694f140262f058ee175f0a1797"));
                Assert.That(
                    gate.TryClaimOpen(loading, presenter.IsVisible),
                    Is.False,
                    "fixed updates cannot skip the presentation-frame boundary");
                Assert.That(
                    gate.TryClaimOpen(transport.IdentitySnapshot(), presenter.IsVisible),
                    Is.False,
                    "repeated Loading polling neither opens nor duplicates state");

                gate.Observe(transport.IdentitySnapshot(), presenter.IsVisible);
                Assert.That(
                    gate.TryClaimOpen(transport.IdentitySnapshot(), presenter.IsVisible),
                    Is.True);
                var opened = transport.OpenLevel();
                GameRuntimeTransport.ValidateIdentity(opened, transport.Handle);
                presenter.Present(opened);
                gate.Complete(opened);
                Assert.That(opened.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(opened.runtime_id, Is.EqualTo(loading.runtime_id));
                Assert.That(opened.session.session_id,
                    Is.EqualTo(transport.Handle.ToString()));
                Assert.That(opened.session.tick, Is.EqualTo("0"));
                Assert.That(opened.active_runtime_flow.bound_level_id,
                    Is.EqualTo(loading.pending_definition.runtime_flow.bound_level_id));
                Assert.That(presenter.IsVisible, Is.False);
                Assert.That(gate.IsPending, Is.False);
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void RuntimeReplaySaveRestoresRealLevelOneIntoFreshManagedOwner()
        {
            using (var source = new GameRuntimeTransport())
            {
                source.BootstrapPackageSet(ResolverPackages());
                source.OpenLevel();
                source.CampaignStep(0, 0, 0, 0, 0);
                source.CampaignStep(0, 3, 1, 0, 0);
                source.CampaignStep(0, 0, 0, 64, unchecked((ushort)-32));
                var sourceIdentity = source.Pause();
                var sourceSnapshot = source.CampaignSnapshot();
                Assert.That(sourceIdentity.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                Assert.That(sourceIdentity.session.tick, Is.EqualTo("3"));

                var save = source.SaveReplay();
                Assert.That(save, Is.Not.Empty);
                Assert.That(save.Length, Is.LessThanOrEqualTo(
                    source.NegotiatedContract.limits.maximum_replay_save_json_bytes));
                var saveJson = Encoding.UTF8.GetString(save);
                Assert.That(saveJson, Does.Contain(GameRuntimeTransport.ReplaySaveContract));
                Assert.That(source.IdentitySnapshot().session.tick, Is.EqualTo("3"));
                AssertSameCampaignState(sourceSnapshot, source.CampaignSnapshot());

                using (var staleTarget = new GameRuntimeTransport())
                {
                    var staleSave = Encoding.UTF8.GetBytes(saveJson.Replace(
                        GameRuntimeTransport.ReplaySaveContract,
                        "carpet-flight-mc2-campaign-replay-save/v4"));
                    Assert.That(staleSave, Is.Not.EqualTo(save));
                    var staleTitle = staleTarget.IdentitySnapshot();
                    var stale = Assert.Throws<GameRuntimeTransport.TransitionException>(() =>
                        staleTarget.LoadReplay(staleSave, ResolverPackages()));
                    Assert.That(stale.Status, Is.EqualTo(NativeSim.ValidationFailed));
                    var afterStale = staleTarget.IdentitySnapshot();
                    Assert.That(afterStale.mode, Is.EqualTo(GameRuntimeTransport.TitleMode));
                    Assert.That(afterStale.generation, Is.EqualTo(staleTitle.generation));
                }

                using (var restored = new GameRuntimeTransport())
                {
                    var playing = restored.LoadReplay(save, ResolverPackages());
                    Assert.That(playing.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                    Assert.That(playing.generation, Is.EqualTo("2"));
                    Assert.That(playing.session.session_id, Is.EqualTo(restored.Handle.ToString()));
                    Assert.That(playing.session.generation, Is.EqualTo("1"));
                    Assert.That(playing.session.tick, Is.EqualTo("3"));
                    Assert.That(
                        playing.session.package_graph_hash,
                        Is.EqualTo(sourceIdentity.session.package_graph_hash));
                    Assert.That(
                        playing.session.definition_hash,
                        Is.EqualTo(sourceIdentity.session.definition_hash));
                    Assert.That(
                        playing.session.definition_revision,
                        Is.EqualTo(sourceIdentity.session.definition_revision));
                    Assert.That(
                        playing.active_runtime_flow.bound_level_id,
                        Is.EqualTo(sourceIdentity.active_runtime_flow.bound_level_id));
                    AssertSameCampaignState(sourceSnapshot, restored.CampaignSnapshot());
                    Assert.Throws<InvalidOperationException>(() =>
                        restored.CampaignStep(0, 0, 0, 0, 0));
                    Assert.That(
                        restored.IdentitySnapshot().mode,
                        Is.EqualTo(GameRuntimeTransport.PausedMode));
                    var resumed = restored.Resume();
                    Assert.That(resumed.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                    Assert.That(resumed.session.generation, Is.EqualTo("2"));
                    Assert.That(
                        restored.CampaignStep(0, 0, 0, 0, 0).identity.tick,
                        Is.EqualTo("4"));
                }
            }

            // This proves opaque v5 quiescent-Paused replay bytes pass through
            // the managed transport, remain mutation-frozen, and resume on one
            // fresh real-L1 Rust owner while stale v4 fails atomically. It does
            // not prove paused-frame service, cross-level saves, migration, or
            // menu wiring.
        }

        [Serializable]
        private sealed class JsonNullWorkspaceProbe
        {
            public string authored_workspace_path;
        }

        [Test]
        public void AuthoredWorkspaceArgumentsBindOnePackageAndEncodeOptionalPath()
        {
            var flags = System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Static;
            var parse = typeof(MagicCarpetRuntime).GetMethod("GameRuntimePackagesFromArguments", flags);
            Assert.That(parse, Is.Not.Null);
            GameRuntimeTransport.InstalledPackageResource[] Parse(params string[] arguments) =>
                (GameRuntimeTransport.InstalledPackageResource[])parse.Invoke(null, new object[] { arguments });
            var workspace = Path.GetFullPath("authored-workspace");
            var packages = Parse("-mc-runtime-package", "first", "-mc-runtime-authored-workspace", "authored-workspace",
                "-mc-runtime-package", "second");
            Assert.That(packages.Length, Is.EqualTo(2));
            Assert.That(packages[0].authored_workspace_path, Is.EqualTo(workspace));
            Assert.That(packages[1].authored_workspace_path, Is.Null);

            System.Xml.XmlDocument ReadJson(byte[] encoded)
            {
                using var reader = JsonReaderWriterFactory.CreateJsonReader(encoded,
                    System.Xml.XmlDictionaryReaderQuotas.Max);
                var document = new System.Xml.XmlDocument();
                document.Load(reader);
                return document;
            }
            var nullJson = JsonUtility.ToJson(new JsonNullWorkspaceProbe());
            var nullToken = ReadJson(Encoding.UTF8.GetBytes(nullJson))
                .SelectSingleNode("/root/authored_workspace_path") as System.Xml.XmlElement;
            TestContext.WriteLine("JsonUtility null string wire JSON: " + nullJson);
            Assert.That(nullToken == null || nullToken.GetAttribute("type") == "null" ||
                (nullToken.GetAttribute("type") == "string" && nullToken.InnerText == ""), Is.True);

            using var runtime = new GameRuntimeTransport();
            var encode = typeof(GameRuntimeTransport).GetMethod("EncodePackageSet",
                System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance);
            Assert.That(encode, Is.Not.Null);
            var request = (byte[])encode.Invoke(runtime, new object[] { packages });
            var json = ReadJson(request);
            Assert.That(json.SelectSingleNode("/root/request_contract")?.InnerText,
                Is.EqualTo(GameRuntimeTransport.InstalledPackageSetContract));
            Assert.That(json.SelectSingleNode("/root/packages/item[1]/authored_workspace_path")?.InnerText,
                Is.EqualTo(workspace));
            Assert.That(json.SelectSingleNode("/root/packages/item[2]/authored_workspace_path"), Is.Null,
                "the omitted workspace must not become an explicitly blank path");

            foreach (var arguments in new[] {
                new[] { "-mc-runtime-authored-workspace", "workspace" },
                new[] { "-mc-runtime-package", "first", "-mc-runtime-authored-workspace" },
                new[] { "-mc-runtime-package", "first", "-mc-runtime-authored-workspace", " " },
                new[] { "-mc-runtime-package", "first", "-mc-runtime-authored-workspace", "one",
                    "-mc-runtime-authored-workspace", "two" }
            })
            {
                var error = Assert.Throws<System.Reflection.TargetInvocationException>(() => Parse(arguments));
                Assert.That(error.InnerException, Is.InstanceOf<InvalidDataException>());
            }
            packages[0].authored_workspace_path = " ";
            var blank = Assert.Throws<System.Reflection.TargetInvocationException>(() => encode.Invoke(runtime, new object[] { packages }));
            Assert.That(blank.InnerException, Is.InstanceOf<ArgumentException>());

            // The new optional field must not invalidate normal installed
            // packages: exercise their actual native bootstrap and world open.
            var defaults = ResolverPackages();
            Array.Find(defaults, resource => resource.package_dir == LevelOnePackage)
                .owner_cache_path = LevelOneOwnerCache;
            var loading = runtime.BootstrapPackageSet(defaults);
            Assert.That(loading.mode, Is.EqualTo(GameRuntimeTransport.LoadingMode));
            var playing = runtime.OpenLevel();
            Assert.That(playing.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
            Assert.That(playing.session.tick, Is.EqualTo("0"));
        }

        [Test]
        public void ProductionStartupRejectsConflictingAndIncompleteArgumentsBeforeOpeningOwner()
        {
            var nativeSessions = NativeSession.ActiveSessionCount;
            var campaignSessions = NativeCampaignSession.ActiveSessionCount;
            var cases = new[]
            {
                new[] { "-mc-game-runtime", "-mc-runtime-menu", "-mc-runtime-replay-load", "unused-save.json" },
                new[] { "-mc-game-runtime", "-mc-runtime-package" },
                new[] { "-mc-game-runtime", "-mc-runtime-package", "-mc-runtime-menu" },
                new[] { "-mc-game-runtime", "-mc-runtime-owner-cache", "unused-cache" },
                new[] { "-mc-game-runtime", "-mc-runtime-package", "unused-package",
                    "-mc-runtime-owner-cache", "first-cache", "-mc-runtime-owner-cache", "second-cache" },
                new[] { "-mc-game-runtime", "-mc-runtime-package", "unused-package",
                    "-mc-runtime-presentation-cache", "first-cache", "-mc-runtime-presentation-cache", "second-cache" },
                new[] { "-mc-game-runtime", "-mc-runtime-replay-save", "first-save.json",
                    "-mc-runtime-replay-save", "second-save.json" },
                new[] { "-mc-game-runtime", "-mc-runtime-loading-cache", "-mc-runtime-menu" },
                new[] { "-mc-game-runtime", "-mc-runtime-menu-cache" },
                new[] { "-mc-game-runtime", "-mc-runtime-media-cache" }
            };
            foreach (var arguments in cases)
            {
                var root = new GameObject("invalid startup argument owner");
                root.SetActive(false);
                var runtime = root.AddComponent<MagicCarpetRuntime>();
                try
                {
                    Assert.Throws<InvalidDataException>(() =>
                        runtime.QualificationInitializeGameRuntimeProductionFromArguments(arguments),
                        string.Join(" ", arguments));
                    Assert.That(runtime.GameRuntimeProductionStartupReady, Is.False);
                    Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions));
                    Assert.That(NativeCampaignSession.ActiveSessionCount, Is.EqualTo(campaignSessions));
                }
                finally
                {
                    runtime.QualificationDisposeGameRuntimeProduction();
                    UnityEngine.Object.DestroyImmediate(root);
                }
            }
        }

        [Test]
        [Timeout(180000)]
        public void ProductionPausedMenuReloadRebindsMusicBeforeInstallingPreferences()
        {
            var nativeSessions = NativeSession.ActiveSessionCount;
            var campaignSessions = NativeCampaignSession.ActiveSessionCount;
            var save = Path.Combine(Path.GetTempPath(), $"mc-paused-reload-{Guid.NewGuid():N}.json");
            var root = new GameObject("paused menu replay with live audio owner");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(new[] {
                    "-mc-game-runtime", "-mc-runtime-package", Path.Combine(PrivateRoot,
                        "generated/mc-bhq8-level1-collision-v5-night"),
                    "-mc-runtime-owner-cache", Path.Combine(PrivateRoot,
                        "generated/mc-b9s2-cursor-level1-20261029"),
                    "-mc-runtime-presentation-cache", Path.Combine(PrivateRoot,
                        "generated/mc2-level1-night-cache-v2"),
                    "-mc-runtime-loading-cache", Path.Combine(PrivateRoot,
                        "generated/mc2-level1-night-cache-v2"),
                    "-mc-runtime-replay-save", save,
                    "-mc-runtime-package", Path.Combine(PrivateRoot,
                        "generated/mc-level2-vault-successor-closure-20261025"),
                    "-mc-runtime-owner-cache", Path.Combine(PrivateRoot,
                        "generated/mc-b9s2-cursor-level2-vault-20261029"),
                    "-mc-runtime-presentation-cache", Path.Combine(PrivateRoot,
                        "generated/level2-vault-successor-owner-cache-20261025"),
                    "-mc-runtime-package", Path.Combine(PrivateRoot,
                        "generated/level3-day-block32-campaign-recipe-v2"),
                    "-mc-runtime-owner-cache", Path.Combine(PrivateRoot,
                        "generated/mc-b9s2-cursor-level3-20261029"),
                    "-mc-runtime-presentation-cache", Path.Combine(PrivateRoot,
                        "generated/level3-day-spell-owner-cache-audio-v4") });
                runtime.QualificationDriveGameRuntimeFrames(3);
                var saved = runtime.GameRuntimeProductionCampaignSnapshot;
                runtime.QualificationSaveGameRuntimeProductionReplayToPath(save);
                runtime.QualificationPauseGameRuntimeProduction();
                var previous = runtime.GameRuntimeProductionIdentity;
                var previousSurface = (CampaignPresentationSurface)typeof(MagicCarpetRuntime).GetField(
                    "gameRuntimeNightSurface", System.Reflection.BindingFlags.Instance |
                    System.Reflection.BindingFlags.NonPublic).GetValue(runtime);
                var previousRoot = previousSurface.Root;
                Assert.That(previousRoot != null, Is.True);

                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Load), Is.True,
                    "the real paused-menu load must install preferences with a current audio reader");
                Assert.That(previousRoot == null, Is.True,
                    "replacing a paused world must destroy its previous presentation root");
                var loaded = runtime.GameRuntimeProductionIdentity;
                Assert.That(loaded.runtime_id, Is.Not.EqualTo(previous.runtime_id));
                Assert.That(loaded.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(loaded.session.tick, Is.EqualTo(saved.identity.tick));
                Assert.That(loaded.player_preferences.installed, Is.True);
                Assert.That(runtime.GameRuntimeProductionStartupReady, Is.True);
                Assert.That(runtime.Level2ProductionDayMusic.generation,
                    Is.EqualTo(ulong.Parse(loaded.session.generation)));
                Assert.That(JsonUtility.ToJson(runtime.GameRuntimeProductionCampaignSnapshot.value.player_pose),
                    Is.EqualTo(JsonUtility.ToJson(saved.value.player_pose)));
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo("4"));
                Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions + 1));
                // Borrowed projections do not own or count an additional native handle.
                Assert.That(NativeCampaignSession.ActiveSessionCount, Is.EqualTo(campaignSessions));
                var borrowed = (NativeCampaignSession)typeof(MagicCarpetRuntime).GetField(
                    "campaignSession", System.Reflection.BindingFlags.Instance |
                    System.Reflection.BindingFlags.NonPublic).GetValue(runtime);
                Assert.That(borrowed.Handle.ToString(), Is.EqualTo(loaded.session.session_id));
                Assert.That(borrowed.Generation, Is.EqualTo(ulong.Parse(loaded.session.generation)));

                runtime.QualificationPauseGameRuntimeProduction();
                // Ordinary paused frames service audio generation changes before
                // the next UI action; direct native pause/resume alone skips that consumer.
                runtime.QualificationDriveGameRuntimeFrames(1);
                var beforeRejectedLoad = runtime.GameRuntimeProductionIdentity;
                var beforeRejectedPose = JsonUtility.ToJson(runtime.GameRuntimeProductionCampaignSnapshot.value.player_pose);
                // This disposable save belongs to this test. Rejection must occur
                // before any live owner or presentation is replaced.
                File.WriteAllText(save, "{}");
                Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.Load), Is.False);
                Assert.That(runtime.GameRuntimeProductionIdentity.runtime_id, Is.EqualTo(beforeRejectedLoad.runtime_id));
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo(beforeRejectedLoad.session.tick));
                Assert.That(runtime.GameRuntimeProductionStartupReady, Is.True);
                Assert.That(JsonUtility.ToJson(runtime.GameRuntimeProductionCampaignSnapshot.value.player_pose),
                    Is.EqualTo(beforeRejectedPose));
                runtime.QualificationResumeGameRuntimeProduction();
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo("5"));
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
                if (File.Exists(save)) File.Delete(save);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions));
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.EqualTo(campaignSessions));
        }

        [Test]
        [Timeout(180000)]
        public void ProductionMagicCarpetRuntimeRestoresItsReplayIntoOneFreshOwner()
        {
            var nativeSessions = NativeSession.ActiveSessionCount;
            var campaignSessions = NativeCampaignSession.ActiveSessionCount;
            Assert.That(campaignSessions, Is.Zero);
            var replayPath = Path.Combine(
                Path.GetTempPath(),
                $"magiccarpet-runtime-{Guid.NewGuid():N}.replay.json");
            GameRuntimeTransport.Identity sourceIdentity = null;
            EngineSessionTransport.CampaignSnapshotObservation
                preServicePausedSnapshot = null;
            EngineSessionTransport.CampaignSnapshotObservation
                savedPausedSnapshot = null;

            var sourceRoot = new GameObject("replay source production owner");
            sourceRoot.SetActive(false);
            var source = sourceRoot.AddComponent<MagicCarpetRuntime>();
            try
            {
                source.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeReplaySaveArguments(
                        replayPath,
                        LevelThreePackage,
                        LevelOnePackage,
                        LevelTwoPackage));
                source.QualificationDriveGameRuntimeFrames(3);
                sourceIdentity = source.QualificationPauseGameRuntimeProduction();
                preServicePausedSnapshot = source.GameRuntimeProductionCampaignSnapshot;
                Assert.That(sourceIdentity.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                Assert.That(sourceIdentity.session.tick, Is.EqualTo("3"));
                source.QualificationDriveGameRuntimeFrames(1);
                savedPausedSnapshot = source.GameRuntimeProductionCampaignSnapshot;
                Assert.That(
                    savedPausedSnapshot.value.state_hash,
                    Is.Not.EqualTo(preServicePausedSnapshot.value.state_hash),
                    "the save boundary includes one serviced Rust Paused frame");
                source.QualificationRequestGameRuntimeProductionReplaySave();
                source.QualificationRequestGameRuntimeProductionReplaySave(false);
                source.QualificationRequestGameRuntimeProductionReplaySave(false);
                Assert.That(File.Exists(replayPath), Is.False,
                    "render-frame input only queues the save until a fixed step");
                source.QualificationDriveGameRuntimeFrames(1);
                Assert.That(File.Exists(replayPath), Is.True);
                Assert.That(new FileInfo(replayPath).Length, Is.GreaterThan(0));
                Assert.That(source.GameRuntimeProductionIdentity.session.tick, Is.EqualTo("3"));
                var savedBytes = File.ReadAllBytes(replayPath);
                source.QualificationDriveGameRuntimeFrames(2);
                Assert.That(File.ReadAllBytes(replayPath), Is.EqualTo(savedBytes),
                    "later fixed steps must not repeat the consumed save edge");
                source.QualificationResumeGameRuntimeProduction();
                source.QualificationRequestGameRuntimeProductionReplaySave();
                source.QualificationResetGameRuntimeProduction();
                source.QualificationDriveGameRuntimeFrames(1);
                Assert.That(File.ReadAllBytes(replayPath), Is.EqualTo(savedBytes),
                    "reset discards a pending save from the previous generation");
            }
            finally
            {
                source.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(sourceRoot);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions));

            var restoredRoot = new GameObject("replay restored production owner");
            restoredRoot.SetActive(false);
            var restored = restoredRoot.AddComponent<MagicCarpetRuntime>();
            try
            {
                restored.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeReplayArguments(
                        replayPath,
                        LevelThreePackage,
                        LevelOnePackage,
                        LevelTwoPackage));
                Assert.That(restored.GameRuntimeProductionStartupReady, Is.True);
                var restoredIdentity = restored.GameRuntimeProductionIdentity;
                Assert.That(restoredIdentity.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                Assert.That(restoredIdentity.generation, Is.EqualTo("2"));
                Assert.That(restoredIdentity.session.generation, Is.EqualTo("1"));
                Assert.That(restoredIdentity.session.tick, Is.EqualTo("3"));
                Assert.That(
                    restoredIdentity.session.session_id,
                    Is.Not.EqualTo(sourceIdentity.session.session_id));
                Assert.That(
                    restoredIdentity.session.package_graph_hash,
                    Is.EqualTo(sourceIdentity.session.package_graph_hash));
                Assert.That(
                    restoredIdentity.session.definition_hash,
                    Is.EqualTo(sourceIdentity.session.definition_hash));
                Assert.That(
                    restoredIdentity.active_runtime_flow.bound_level_id,
                    Is.EqualTo(sourceIdentity.active_runtime_flow.bound_level_id));
                AssertSameCampaignState(
                    savedPausedSnapshot,
                    restored.GameRuntimeProductionCampaignSnapshot);

                restored.QualificationDriveGameRuntimeFrames(1);
                Assert.That(
                    restored.GameRuntimeProductionIdentity.session.tick,
                    Is.EqualTo("3"));
                Assert.That(
                    restored.QualificationResumeGameRuntimeProduction().mode,
                    Is.EqualTo(GameRuntimeTransport.PlayingMode));
                restored.QualificationDriveGameRuntimeFrames(1);
                Assert.That(
                    restored.GameRuntimeProductionIdentity.session.tick,
                    Is.EqualTo("4"));
                Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions + 1));
                Assert.That(
                    NativeCampaignSession.ActiveSessionCount,
                    Is.EqualTo(campaignSessions));
            }
            finally
            {
                restored.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(restoredRoot);
                if (File.Exists(replayPath)) File.Delete(replayPath);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions));
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.EqualTo(campaignSessions));

            // No-Claim: this proves the production runtime owner atomically
            // writes and restores one owner-local quiescent-Paused real-L1
            // replay file, remains frozen, and resumes in a fresh facade. It
            // does not provide paused-frame service, menu UI, cross-level
            // saves, migration, cloud sync, or completion handling.
        }

        [Test]
        [Timeout(180000)]
        public void ProductionNativeSessionFacadeOwnsOneResolverRuntimeHandle()
        {
            var nativeSessions = NativeSession.ActiveSessionCount;
            var campaignSessions = NativeCampaignSession.ActiveSessionCount;
            Assert.That(campaignSessions, Is.Zero);

            var incomplete = Assert.Throws<GameRuntimeTransport.TransitionException>(() =>
                MagicCarpetRuntime.OpenGameRuntimeSessionForPackages(
                    Package(LevelTwoPackage),
                    Package(LevelOnePackage)));
            Assert.That(incomplete.Status, Is.EqualTo(NativeSim.ValidationFailed));
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions));
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.EqualTo(campaignSessions));

            NativeSession session = null;
            ulong handle = 0;
            try
            {
                session = MagicCarpetRuntime.OpenGameRuntimeSessionForPackages(
                    ResolverPackages());
                handle = session.Handle;
                Assert.That(session.UsesGameRuntimeContract, Is.True);
                Assert.That(session.UsesEngineContract, Is.False);
                Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions + 1));
                Assert.That(NativeCampaignSession.ActiveSessionCount, Is.EqualTo(campaignSessions));

                var identity = session.GameRuntimeIdentity();
                Assert.That(identity.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(
                    identity.active_runtime_flow.bound_level_id,
                    Is.EqualTo("level.magic_carpet_2.night_spire"));
                Assert.That(identity.session.session_id, Is.EqualTo(handle.ToString()));
                Assert.That(identity.session.generation, Is.EqualTo("1"));
                Assert.That(identity.session.tick, Is.EqualTo("0"));

                var before = session.GameRuntimeCampaignSnapshot();
                Assert.That(before.identity.session_id, Is.EqualTo(handle.ToString()));
                var stepped = session.GameRuntimeCampaignStep(0, 0, 1, 0, 0);
                Assert.That(stepped.identity.tick, Is.EqualTo("1"));
                Assert.That(
                    session.GameRuntimeCampaignEvents("0").identity.session_id,
                    Is.EqualTo(handle.ToString()));
                var reset = session.GameRuntimeCampaignReset();
                Assert.That(reset.identity.generation, Is.EqualTo("2"));
                Assert.That(reset.identity.tick, Is.EqualTo("0"));
                Assert.That(reset.value.state_hash, Is.EqualTo(before.value.state_hash));

                Assert.That(
                    NativeSim.mc_game_session_destroy_v1(handle),
                    Is.EqualTo(NativeSim.InvalidArgument));
                Assert.That(
                    session.GameRuntimeIdentity().session.session_id,
                    Is.EqualTo(handle.ToString()));
            }
            finally
            {
                session?.Dispose();
            }

            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions));
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.EqualTo(campaignSessions));
            if (handle != 0)
            {
                Assert.That(
                    NativeSim.mc_game_runtime_identity_json_v1(
                        handle, IntPtr.Zero, 0, out _),
                    Is.EqualTo(NativeSim.UnknownHandle));
            }

            // No-Claim: this proves the production NativeSession facade owns
            // one resolver-derived runtime handle and its generic gameplay
            // facet. It does not migrate MagicCarpetRuntime presentation,
            // completion transitions, later-level rendering, or shell input.
        }

        [Test]
        [Timeout(180000)]
        public void ProductionMagicCarpetRuntimeOwnsAndStepsOneResolverRuntimeHandle()
        {
            var nativeSessions = NativeSession.ActiveSessionCount;
            var campaignSessions = NativeCampaignSession.ActiveSessionCount;
            Assert.That(campaignSessions, Is.Zero);
            var root = new GameObject("resolver runtime production owner");
            root.SetActive(false);
            MagicCarpetRuntime runtime = null;
            ulong handle = 0;
            try
            {
                runtime = root.AddComponent<MagicCarpetRuntime>();
                var incomplete = Assert.Throws<GameRuntimeTransport.TransitionException>(() =>
                    runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                        RuntimeArguments(LevelTwoPackage, LevelOnePackage)));
                Assert.That(incomplete.Status, Is.EqualTo(NativeSim.ValidationFailed));
                Assert.That(runtime.GameRuntimeProductionStartupReady, Is.False);
                Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions));
                Assert.That(
                    NativeCampaignSession.ActiveSessionCount,
                    Is.EqualTo(campaignSessions));

                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                Assert.That(runtime.GameRuntimeProductionStartupReady, Is.True);
                Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions + 1));
                Assert.That(NativeCampaignSession.ActiveSessionCount, Is.EqualTo(campaignSessions));

                var identity = runtime.GameRuntimeProductionIdentity;
                handle = ulong.Parse(identity.session.session_id);
                Assert.That(identity.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(
                    identity.active_runtime_flow.bound_level_id,
                    Is.EqualTo("level.magic_carpet_2.night_spire"));
                Assert.That(identity.session.session_id, Is.EqualTo(handle.ToString()));
                Assert.That(identity.session.generation, Is.EqualTo("1"));
                Assert.That(identity.session.tick, Is.EqualTo("0"));

                var before = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(before.identity.session_id, Is.EqualTo(handle.ToString()));
                runtime.QualificationDriveGameRuntimeFrames(1);
                var stepped = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(stepped.identity.tick, Is.EqualTo("1"));
                runtime.QualificationResetGameRuntimeProduction();
                var reset = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(reset.identity.generation, Is.EqualTo("2"));
                Assert.That(reset.identity.tick, Is.EqualTo("0"));
                Assert.That(reset.value.state_hash, Is.EqualTo(before.value.state_hash));

                Assert.That(
                    NativeSim.mc_game_session_destroy_v1(handle),
                    Is.EqualTo(NativeSim.InvalidArgument));
                Assert.That(
                    runtime.GameRuntimeProductionIdentity.session.session_id,
                    Is.EqualTo(handle.ToString()));
            }
            finally
            {
                runtime?.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }

            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions));
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.EqualTo(campaignSessions));
            if (handle != 0)
            {
                Assert.That(
                    NativeSim.mc_game_runtime_identity_json_v1(
                        handle, IntPtr.Zero, 0, out _),
                    Is.EqualTo(NativeSim.UnknownHandle));
            }

            // No-Claim: this proves MagicCarpetRuntime owns one resolver-derived
            // runtime handle and drives its generic fixed-step/reset facet. It
            // does not migrate terrain/HUD/audio/entity presentation, expose
            // completion transitions, discover packages, or render later levels.
        }

        [Test]
        [Timeout(180000)]
        public void ProductionGameRuntimePausedUpdateServicesRustAndDropsPausedFireInput()
        {
            var root = new GameObject("resolver runtime pause input");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                runtime.QualificationDriveGameRuntimeFrames(1);
                var playing = runtime.GameRuntimeProductionIdentity;
                var beforePause = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(playing.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(runtime.GameRuntimeProductionPauseOverlayText, Is.Null);
                Assert.That(beforePause.identity.tick, Is.EqualTo("1"));
                Assert.That(beforePause.value.presentation_entities, Is.Empty);

                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                var paused = runtime.GameRuntimeProductionIdentity;
                Assert.That(paused.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                Assert.That(paused.session.session_id, Is.EqualTo(playing.session.session_id));
                Assert.That(
                    runtime.GameRuntimeProductionPauseOverlayText,
                    Is.EqualTo("PAUSED\nP resume \u00b7 Escape close"));
                runtime.QualificationDriveGameRuntimeFrames(1);
                var afterFirstPausedUpdate = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(
                    afterFirstPausedUpdate.identity.tick,
                    Is.EqualTo(beforePause.identity.tick));
                Assert.That(
                    afterFirstPausedUpdate.value.state_hash,
                    Is.Not.EqualTo(beforePause.value.state_hash));
                Assert.That(
                    afterFirstPausedUpdate.value.terrain_hash,
                    Is.EqualTo(beforePause.value.terrain_hash));
                Assert.That(
                    afterFirstPausedUpdate.value.effect_journal_sha256,
                    Is.EqualTo(beforePause.value.effect_journal_sha256));
                Assert.That(afterFirstPausedUpdate.value.presentation_entities, Is.Empty);

                runtime.QualificationDriveGameRuntimeFrames(1);
                var afterSecondPausedUpdate = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(
                    afterSecondPausedUpdate.identity.tick,
                    Is.EqualTo(beforePause.identity.tick));
                Assert.That(
                    afterSecondPausedUpdate.value.state_hash,
                    Is.Not.EqualTo(afterFirstPausedUpdate.value.state_hash));
                Assert.That(
                    afterSecondPausedUpdate.value.player_pose.x,
                    Is.EqualTo(beforePause.value.player_pose.x));
                Assert.That(
                    afterSecondPausedUpdate.value.player_pose.y,
                    Is.EqualTo(beforePause.value.player_pose.y));
                Assert.That(
                    afterSecondPausedUpdate.value.player_pose.z,
                    Is.EqualTo(beforePause.value.player_pose.z));

                runtime.QualificationApplyGameRuntimeProductionInput(false, true);
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                Assert.That(
                    runtime.GameRuntimeProductionIdentity.mode,
                    Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(runtime.GameRuntimeProductionPauseOverlayText, Is.Null);
                runtime.QualificationDriveGameRuntimeFrames(1);
                var resumed = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(resumed.identity.tick, Is.EqualTo("2"));
                Assert.That(resumed.value.presentation_entities, Is.Empty);

                // This proves each production paused FixedUpdate services one
                // Rust-owned replay-visible pre-gate frame, freezes gameplay
                // tick/pose, presents Paused visibly, and drops a Fireball
                // click received while paused. It does not claim later-level
                // pause policy, pause-menu commands/audio, or completion.
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void ProductionCloseThenEnterRestartsResolverEntryOnSameHandleWithoutSessionAba()
        {
            var root = new GameObject("resolver runtime close input");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                runtime.QualificationDriveGameRuntimeFrames(1);
                var playing = runtime.GameRuntimeProductionIdentity;
                var handle = ulong.Parse(playing.runtime_id);
                var runtimeGeneration = ulong.Parse(playing.generation);
                var sessionGeneration = ulong.Parse(playing.session.generation);
                var tick = ulong.Parse(playing.session.tick);
                var activeSessionCount = NativeSession.ActiveSessionCount;
                Assert.That(playing.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(runtime.GameRuntimeProductionFireballTexture, Is.Not.Null);

                runtime.QualificationCloseGameRuntimeProduction();
                var title = runtime.GameRuntimeProductionIdentity;
                Assert.That(title.runtime_id, Is.EqualTo(playing.runtime_id));
                Assert.That(ulong.Parse(title.generation), Is.EqualTo(runtimeGeneration + 1));
                Assert.That(title.mode, Is.EqualTo(GameRuntimeTransport.TitleMode));
                Assert.That(title.session, Is.Null);
                Assert.That(title.pending_definition, Is.Null);
                Assert.That(title.active_runtime_flow, Is.Null);
                Assert.That(runtime.GameRuntimeProductionCampaignSnapshot, Is.Null);
                Assert.That(runtime.GameRuntimeProductionStartupReady, Is.False);
                Assert.That(runtime.GameRuntimeProductionFireballTexture, Is.Null);
                Assert.That(runtime.GameRuntimeProductionFireballMarkerCount, Is.Zero);
                StringAssert.Contains("TITLE", runtime.GameRuntimeProductionFlightHudText);

                Assert.That(
                    NativeSim.mc_game_runtime_close_v1(
                        handle,
                        runtimeGeneration,
                        sessionGeneration,
                        tick),
                    Is.EqualTo(NativeSim.GenerationMismatch));
                Assert.That(
                    NativeSim.mc_game_session_snapshot_json_v1(
                        handle,
                        IntPtr.Zero,
                        0,
                        out _),
                    Is.EqualTo(NativeSim.InvalidArgument));
                var command = Encoding.UTF8.GetBytes("{}");
                Assert.That(
                    NativeSim.mc_game_session_step_json_v1(
                        handle,
                        sessionGeneration,
                        tick,
                        command,
                        checked((uint)command.Length)),
                    Is.EqualTo(NativeSim.InvalidArgument));
                Assert.That(
                    runtime.GameRuntimeProductionIdentity.runtime_id,
                    Is.EqualTo(playing.runtime_id));

                runtime.QualificationRestartGameRuntimeProduction();
                var restarted = runtime.GameRuntimeProductionIdentity;
                Assert.That(restarted.runtime_id, Is.EqualTo(playing.runtime_id));
                Assert.That(ulong.Parse(restarted.generation), Is.EqualTo(runtimeGeneration + 3));
                Assert.That(restarted.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(
                    restarted.active_runtime_flow.bound_level_id,
                    Is.EqualTo(restarted.active_runtime_flow.entry_level_id));
                Assert.That(
                    restarted.active_runtime_flow.bound_level_id,
                    Is.EqualTo("level.magic_carpet_2.night_spire"));
                Assert.That(restarted.session.session_id, Is.EqualTo(playing.runtime_id));
                Assert.That(restarted.session.generation, Is.EqualTo("2"));
                Assert.That(restarted.session.tick, Is.EqualTo("0"));
                Assert.That(runtime.GameRuntimeProductionStartupReady, Is.True);
                Assert.That(runtime.GameRuntimeProductionFireballTexture, Is.Not.Null);
                Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeSessionCount));
                Assert.That(
                    NativeSim.mc_game_session_step_json_v1(
                        handle,
                        sessionGeneration,
                        tick,
                        command,
                        checked((uint)command.Length)),
                    Is.EqualTo(NativeSim.GenerationMismatch));
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(
                    runtime.GameRuntimeProductionCampaignSnapshot.identity.tick,
                    Is.EqualTo("1"));

                // No-Claim: Escape and Enter close and restart the retained
                // resolver entry on one Rust runtime without session-guard ABA.
                // They do not choose packages, accept completion, quick-load
                // after close, persist the closed state, or present a menu.
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void ProductionGameRuntimeSteeringChangesRustPoseAndSemanticCameraOnSameHandle()
        {
            var root = new GameObject("resolver runtime steering input");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                var beforeIdentity = runtime.GameRuntimeProductionIdentity;
                var before = runtime.GameRuntimeProductionCampaignSnapshot;
                var cameraBefore = runtime.GameRuntimeProductionCameraRotation;
                Assert.That(before.identity.tick, Is.EqualTo("0"));
                Assert.That(
                    Quaternion.Angle(
                        cameraBefore,
                        ExpectedRuntimeCameraRotation(before.value.player_pose)),
                    Is.LessThan(0.001f));

                runtime.QualificationApplyGameRuntimeProductionSteering(24f, -8f);
                runtime.QualificationApplyGameRuntimeProductionMovement(
                    true,
                    false,
                    false,
                    false);
                Assert.That(
                    Quaternion.Angle(
                        runtime.GameRuntimeProductionCameraRotation,
                        cameraBefore),
                    Is.LessThan(0.001f),
                    "Unity input alone must not move the semantic camera");
                runtime.QualificationDriveGameRuntimeFrames(3);

                var afterIdentity = runtime.GameRuntimeProductionIdentity;
                var after = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(afterIdentity.runtime_id, Is.EqualTo(beforeIdentity.runtime_id));
                Assert.That(
                    afterIdentity.session.session_id,
                    Is.EqualTo(beforeIdentity.session.session_id));
                Assert.That(after.identity.tick, Is.EqualTo("3"));
                Assert.That(
                    after.value.player_pose.roll != before.value.player_pose.roll ||
                    after.value.player_pose.yaw != before.value.player_pose.yaw ||
                    after.value.player_pose.display_pitch !=
                        before.value.player_pose.display_pitch,
                    Is.True,
                    "the steering command must change the Rust-owned player pose");
                Assert.That(
                    Quaternion.Angle(
                        runtime.GameRuntimeProductionCameraRotation,
                        ExpectedRuntimeCameraRotation(after.value.player_pose)),
                    Is.LessThan(0.001f));
                Assert.That(
                    Quaternion.Angle(
                        runtime.GameRuntimeProductionCameraRotation,
                        cameraBefore),
                    Is.GreaterThan(0.01f));

                // No-Claim: this proves production mouse/JLIK deflection reaches
                // the same Rust runtime handle and the existing semantic camera
                // follows the returned pose. It does not prove terrain/HUD/audio,
                // spell-menu input, controller rebinding, or later-level cameras.
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void ProductionGameRuntimeFlightHudReadsReturnedRustMotionOnSameHandle()
        {
            var root = new GameObject("resolver runtime flight HUD");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                var beforeIdentity = runtime.GameRuntimeProductionIdentity;
                var before = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(
                    runtime.GameRuntimeProductionFlightHudText,
                    Is.EqualTo(
                        "CARPET FLIGHT — RUST GAME RUNTIME · PLAYING\n" +
                        $"speed {before.value.player_motion.actual_speed,4} / " +
                        $"{before.value.player_motion.target_speed,4}  " +
                        $"strafe {before.value.player_motion.strafe_speed,4}  " +
                        $"altitude {before.value.player_pose.z,6}  " +
                        $"tick {before.identity.tick}\n" +
                        $"life {before.value.player_vitals.life,6} / " +
                        $"{before.value.player_vitals.max_life,6}  " +
                        $"mana {before.value.player_vitals.mana,6} / " +
                        $"{before.value.player_vitals.max_mana,6}\n" +
                        $"objective {before.value.objective_progress.current_objective} · " +
                        $"{before.value.objective_progress.completed_count}/" +
                        $"{before.value.objective_progress.total_count} complete\n" +
                        "event none\n" +
                        "W/S speed · A/D strafe · persistent mouse or I/J/K/L steer · " +
                        "C centre · Space fire · P pause · Escape close"));

                runtime.QualificationApplyGameRuntimeProductionMovement(
                    true,
                    false,
                    true,
                    false);
                runtime.QualificationDriveGameRuntimeFrames(3);

                var afterIdentity = runtime.GameRuntimeProductionIdentity;
                var after = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(afterIdentity.runtime_id, Is.EqualTo(beforeIdentity.runtime_id));
                Assert.That(
                    afterIdentity.session.session_id,
                    Is.EqualTo(beforeIdentity.session.session_id));
                Assert.That(after.identity.tick, Is.EqualTo("3"));
                Assert.That(
                    after.value.player_motion.actual_speed,
                    Is.Not.EqualTo(before.value.player_motion.actual_speed));
                var recentEvents = runtime.GameRuntimeProductionRecentSemanticEvents;
                var latestEvent = recentEvents.Length == 0
                    ? "none"
                    : $"{recentEvents[^1].sequence} {recentEvents[^1].event_id} " +
                      $"actor={(string.IsNullOrEmpty(recentEvents[^1].actor) ? "-" : recentEvents[^1].actor)}";
                Assert.That(
                    runtime.GameRuntimeProductionFlightHudText,
                    Is.EqualTo(
                        "CARPET FLIGHT — RUST GAME RUNTIME · PLAYING\n" +
                        $"speed {after.value.player_motion.actual_speed,4} / " +
                        $"{after.value.player_motion.target_speed,4}  " +
                        $"strafe {after.value.player_motion.strafe_speed,4}  " +
                        $"altitude {after.value.player_pose.z,6}  " +
                        $"tick {after.identity.tick}\n" +
                        $"life {after.value.player_vitals.life,6} / " +
                        $"{after.value.player_vitals.max_life,6}  " +
                        $"mana {after.value.player_vitals.mana,6} / " +
                        $"{after.value.player_vitals.max_mana,6}\n" +
                        $"objective {after.value.objective_progress.current_objective} · " +
                        $"{after.value.objective_progress.completed_count}/" +
                        $"{after.value.objective_progress.total_count} complete\n" +
                        $"event {latestEvent}\n" +
                        "W/S speed · A/D strafe · persistent mouse or I/J/K/L steer · " +
                        "C centre · Space fire · P pause · Escape close"));

                // No-Claim: this proves the production HUD text consumes the
                // same-handle Rust motion, altitude, mode, and tick returned by
                // the campaign snapshot. It does not prove source-art HUD assets,
                // source-art life/mana gauges, spells/objectives, minimap/terrain/audio,
                // or later levels.
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void ProductionGameRuntimeObjectiveHudConsumesRustSemanticProgressOnSameHandle()
        {
            var root = new GameObject("resolver runtime objective HUD");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                var beforeIdentity = runtime.GameRuntimeProductionIdentity;
                var before = runtime.GameRuntimeProductionCampaignSnapshot;
                var progress = before.value.objective_progress;
                Assert.That(progress, Is.Not.Null);
                Assert.That(
                    progress.current_objective,
                    Is.EqualTo("objective.reach.my_beacon"));
                Assert.That(progress.completed_count, Is.Zero);
                Assert.That(progress.total_count, Is.EqualTo(5));
                Assert.That(progress.all_objectives_satisfied, Is.False);
                Assert.That(runtime.GameRuntimeProductionObjectiveBarWidth, Is.Zero);
                Assert.That(
                    runtime.GameRuntimeProductionFlightHudText,
                    Does.Contain(
                        "objective objective.reach.my_beacon · 0/5 complete"));

                runtime.QualificationApplyGameRuntimeProductionMovement(
                    true,
                    false,
                    false,
                    false);
                var commandedRoll = 0f;
                var commandedPitch = 0f;
                for (var index = 0; index < 512; index++)
                {
                    var pose = runtime.GameRuntimeProductionCampaignSnapshot.value.player_pose;
                    var deltaX = unchecked((short)(39424 - pose.x));
                    var deltaY = unchecked((short)(58624 - pose.y));
                    var desired = Mathf.RoundToInt(
                        512f + Mathf.Atan2(deltaY, deltaX) * (1024f / Mathf.PI));
                    desired = (desired % 2048 + 2048) % 2048;
                    var difference = desired - (pose.yaw & 0x07ff);
                    if (difference > 1024) difference -= 2048;
                    if (difference < -1024) difference += 2048;
                    var nextRoll = Mathf.Clamp(difference, -24, 24);
                    const float nextPitch = -127f;
                    runtime.QualificationApplyGameRuntimeProductionSteering(
                        (nextRoll - commandedRoll) / 4.5f,
                        (commandedPitch - nextPitch) / 4.5f);
                    commandedRoll = nextRoll;
                    commandedPitch = nextPitch;
                    runtime.QualificationDriveGameRuntimeFrames(1);
                    if (runtime.GameRuntimeProductionCampaignSnapshot.value
                        .objective_progress.completed_count == 1)
                    {
                        break;
                    }
                }
                var steppedIdentity = runtime.GameRuntimeProductionIdentity;
                var stepped = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(
                    steppedIdentity.runtime_id,
                    Is.EqualTo(beforeIdentity.runtime_id));
                Assert.That(
                    steppedIdentity.session.session_id,
                    Is.EqualTo(beforeIdentity.session.session_id));
                Assert.That(
                    stepped.identity.session_id,
                    Is.EqualTo(before.identity.session_id));
                Assert.That(
                    stepped.value.objective_progress.current_objective,
                    Is.EqualTo("objective.reach.foot_of_spire"));
                Assert.That(stepped.value.objective_progress.completed_count, Is.EqualTo(1));
                Assert.That(stepped.value.objective_progress.total_count, Is.EqualTo(5));
                Assert.That(runtime.GameRuntimeProductionObjectiveBarWidth, Is.EqualTo(48));
                Assert.That(
                    runtime.GameRuntimeProductionFlightHudText,
                    Does.Contain(
                        "objective objective.reach.foot_of_spire · 1/5 complete"));

                runtime.QualificationResetGameRuntimeProduction();
                var reset = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(
                    reset.identity.session_id,
                    Is.EqualTo(before.identity.session_id));
                Assert.That(reset.identity.generation, Is.EqualTo("2"));
                Assert.That(
                    reset.value.objective_progress.current_objective,
                    Is.EqualTo("objective.reach.my_beacon"));
                Assert.That(reset.value.objective_progress.completed_count, Is.Zero);
                Assert.That(reset.value.objective_progress.total_count, Is.EqualTo(5));
                Assert.That(runtime.GameRuntimeProductionObjectiveBarWidth, Is.Zero);
                Assert.That(
                    runtime.GameRuntimeProductionFlightHudText,
                    Does.Contain(
                        "objective objective.reach.my_beacon · 0/5 complete"));

                // No-Claim: this proves the real Level 1 Rust snapshot's
                // semantic objective ID, counts, and directly derived generic
                // bar width are presented by Unity on one runtime/session
                // handle through ordinary commanded advancement and reset. It
                // does not localize objective text, use source-art gauges, or
                // prove later-level HUD presentation.
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void ProductionGameRuntimeSelectsAndCastsAnAcquiredSemanticSpell()
        {
            var root = new GameObject("resolver runtime semantic spell input");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                var initial = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(initial.identity.tick, Is.EqualTo("0"));
                Assert.That(initial.value.presentation_entities, Is.Empty);
                var fireball = CampaignSpellPresenter.FindSpell(initial.value, "spell.fireball");
                Assert.That(fireball.acquired, Is.True);
                Assert.That(fireball.tier, Is.Zero);
                Assert.That(fireball.mana_cost, Is.EqualTo(100));
                Assert.That(fireball.hand, Is.EqualTo("left"));
                Assert.That(fireball.availability, Is.EqualTo("ready"));
                Assert.That(runtime.GameRuntimeProductionFlightHudText, Does.Contain(fireball.display_name));

                Assert.That(
                    runtime.SubmitGameRuntimeSpellKey(1),
                    Is.True);
                Assert.That(
                    runtime.SubmitGameRuntimeSpellKey(3),
                    Is.False,
                    "Unity must not overwrite a pending semantic selection before Rust consumes it");
                runtime.QualificationDriveGameRuntimeFrames(1);
                var selected = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(selected.identity.tick, Is.EqualTo("1"));
                Assert.That(selected.value.presentation_entities, Is.Empty);
                Assert.That(CampaignSpellPresenter.FindSpell(selected.value, fireball.id).selected, Is.True);

                Assert.That(runtime.RequestGameRuntimeSelectedSpellCast(), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                var cast = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(cast.identity.tick, Is.EqualTo("2"));
                Assert.That(
                    Array.Exists(
                        cast.value.presentation_entities,
                        entity => entity.kind == "spell.fireball.projectile"),
                    Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.GameRuntimeProductionCampaignSnapshot.value.player_vitals.mana,
                    Is.EqualTo(initial.value.player_vitals.mana - fireball.mana_cost));
                runtime.QualificationResetGameRuntimeProduction();
                var reset = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(reset.identity.generation, Is.EqualTo("2"));
                Assert.That(reset.identity.tick, Is.EqualTo("0"));
                Assert.That(reset.value.state_hash, Is.EqualTo(initial.value.state_hash));
                Assert.That(JsonUtility.ToJson(CampaignSpellPresenter.FindSpell(reset.value, fireball.id)),
                    Is.EqualTo(JsonUtility.ToJson(fireball)));
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [TestCase("preferenceFullStopRequested")]
        [TestCase("preferenceRecenterRequested")]
        [Timeout(180000)]
        public void ProductionSpellSelectionFeedbackUsesObservedSelection(string inputFlag)
        {
            var root = new GameObject("spell selection and source control priority");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                var generated = Path.Combine(PrivateRoot, "generated");
                var night = Path.Combine(generated, "mc2-level1-night-cache-v2");
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    new[] {
                        "-mc-game-runtime", "-mc-runtime-loading-cache", night,
                        "-mc-runtime-package", Path.Combine(generated, "mc-bhq8-level1-collision-v5-night"),
                        "-mc-runtime-owner-cache", night, "-mc-runtime-presentation-cache", night,
                        "-mc-runtime-package", Path.Combine(generated, "level2-day-block32-campaign-v2"),
                        "-mc-runtime-owner-cache", Path.Combine(generated, "level2-day-spell-owner-cache-audio-v4/components/material"),
                        "-mc-runtime-presentation-cache", Path.Combine(generated, "level2-day-spell-owner-cache-audio-v4"),
                        "-mc-runtime-package", Path.Combine(generated, "level3-day-block32-campaign-recipe-v2"),
                        "-mc-runtime-owner-cache", Path.Combine(generated, "level3-day-spell-owner-cache-audio-v4/components/material"),
                        "-mc-runtime-presentation-cache", Path.Combine(generated, "level3-day-spell-owner-cache-audio-v4")
                    });
                Assert.That(CampaignSpellPresenter.FindSpell(
                    runtime.GameRuntimeProductionCampaignSnapshot.value, "spell.fireball").selected, Is.True);
                Assert.That(runtime.SubmitGameRuntimeSpellKey(2), Is.True);
                // Queue the same input bit as SampleInput, without changing
                // native player state or bypassing ordinary frame dispatch.
                var flags = System.Reflection.BindingFlags.Instance |
                    System.Reflection.BindingFlags.NonPublic;
                typeof(MagicCarpetRuntime).GetField(inputFlag, flags).SetValue(runtime, true);
                runtime.QualificationDriveGameRuntimeFrames(1);
                var observed = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(observed.identity.tick, Is.EqualTo("1"));
                Assert.That(CampaignSpellPresenter.FindSpell(observed.value, "spell.fireball").selected,
                    Is.True, "source recenter/full-stop takes priority over selection");
                Assert.That(CampaignSpellPresenter.FindSpell(observed.value, "spell.possession").selected,
                    Is.False);
                Assert.That(observed.value.presentation_entities, Is.Empty);
                var status = (string)typeof(MagicCarpetRuntime)
                    .GetField("gameRuntimeSpellInputStatus", flags).GetValue(runtime);
                Assert.That(status, Is.Null,
                    "Unity must not announce a requested selection that native authority did not apply");
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void LevelFourSpeechUsesOwnerPcmAndRejectsSegmentsOutsideTrack()
        {
            var root = new GameObject("ordinary Mission4 speech");
            try
            {
                using var speech = new CampaignSpeechPresenter(root.transform, CampaignSpeechCache);
                // Type_DB080_CdTrack.h fourth row, source floating conversion
                // truncated to milliseconds. Rust supplies these selected cues.
                var starts = new uint[] { 0, 7999, 12999, 19999 };
                var lengths = new uint[] { 6999, 3999, 5999, 7999 };
                for (var i = 0; i < starts.Length; i++)
                {
                    speech.Play(4, starts[i], lengths[i]);
                    Assert.That(speech.PlayCount, Is.EqualTo(i + 1));
                    Assert.That(speech.LastTrack, Is.EqualTo(4));
                    Assert.That(speech.LastStartMs, Is.EqualTo(starts[i]));
                    Assert.That(speech.LastLengthMs, Is.EqualTo(lengths[i]));
                }
                var channel = root.GetComponentInChildren<AudioSource>();
                Assert.That(root.GetComponentsInChildren<AudioSource>(), Has.Length.EqualTo(1),
                    "new speech interrupts the existing channel");
                Assert.That(channel.clip.samples, Is.EqualTo(1374156));
                Assert.That(channel.clip.frequency, Is.EqualTo(44100));
                Assert.That(channel.clip.channels, Is.EqualTo(2));
                var playingClip = channel.clip;
                var wasPlaying = channel.isPlaying;
                var cursor = channel.timeSamples;
                var prefs = new PlayerPreferences {
                    sound_enabled = true, speech_enabled = true,
                    sound_volume = 91, music_volume = 127
                };
                speech.ApplyPreferences(prefs);
                Assert.That(channel.volume, Is.EqualTo(91f / 127));
                prefs.speech_enabled = false;
                speech.ApplyPreferences(prefs);
                Assert.That(channel.volume, Is.EqualTo(91f / 127),
                    "source Speech Off gates future segments without muting the current CD segment");
                Assert.That(channel.clip, Is.SameAs(playingClip));
                Assert.That(channel.isPlaying, Is.EqualTo(wasPlaying));
                Assert.That(channel.timeSamples, Is.GreaterThanOrEqualTo(cursor));
                Assert.That(speech.PlayCount, Is.EqualTo(4), "preference changes cannot restart speech");
                Assert.Throws<InvalidDataException>(() => speech.Play(4, 31000, 161));
                Assert.Throws<InvalidDataException>(() => speech.Play(4, 32000, 1));
                Assert.Throws<InvalidDataException>(() => speech.Play(4, 0, 0));
                Assert.Throws<InvalidDataException>(() => speech.Play(28, 0, 1));
                Assert.That(speech.PlayCount, Is.EqualTo(4),
                    "rejected cues must not replace the current source segment");
                Assert.That(speech.LastStartMs, Is.EqualTo(19999));
                speech.Stop();
                Assert.That(channel.clip, Is.Null);
            }
            finally { UnityEngine.Object.DestroyImmediate(root); }
        }

        [Test]
        [Timeout(180000)]
        public void OpeningObjectiveSpeechReachesRealPcmConsumerOnceAndRestartsOnReset()
        {
            var root = new GameObject("ordinary Night objective speech");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var generated = Path.Combine(PrivateRoot, "generated");
            var night = Path.Combine(generated, "mc2-level1-night-cache-v2");
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(new[] {
                    "-mc-game-runtime", "-mc-runtime-loading-cache", night,
                    "-mc-runtime-package", Path.Combine(generated, "mc-bhq8-level1-collision-v5-night"),
                    "-mc-runtime-owner-cache", night, "-mc-runtime-presentation-cache", night,
                    "-mc-runtime-package", Path.Combine(generated, "level2-day-block32-campaign-v2"),
                    "-mc-runtime-owner-cache", Path.Combine(generated, "level2-day-spell-owner-cache-audio-v4/components/material"),
                    "-mc-runtime-presentation-cache", Path.Combine(generated, "level2-day-spell-owner-cache-audio-v4"),
                    "-mc-runtime-package", Path.Combine(generated, "level3-day-block32-campaign-recipe-v2")
                });
                var speech = (CampaignSpeechPresenter)typeof(MagicCarpetRuntime)
                    .GetField("campaignSpeech", System.Reflection.BindingFlags.Instance |
                        System.Reflection.BindingFlags.NonPublic).GetValue(runtime);
                Assert.That(speech.PlayCount, Is.Zero);
                runtime.QualificationDriveGameRuntimeFrames(10);
                Assert.That(speech.PlayCount, Is.EqualTo(1));
                Assert.That(speech.LastTrack, Is.EqualTo(1));
                Assert.That(speech.LastStartMs, Is.EqualTo(10999));
                Assert.That(speech.LastLengthMs, Is.EqualTo(3999));
                Assert.That(speech.SoundMix, Is.EqualTo(42f / 127));
                Assert.That(speech.MusicMix, Is.EqualTo(42f / 127));
                runtime.QualificationDriveGameRuntimeFrames(2);
                Assert.That(speech.PlayCount, Is.EqualTo(1), "polling must not restart a retained cue");
                var audio = (OwnerAudioPresentation)typeof(MagicCarpetRuntime)
                    .GetField("ownerAudio", System.Reflection.BindingFlags.Instance |
                        System.Reflection.BindingFlags.NonPublic).GetValue(runtime);
                var musicStarts = audio.QualificationMusicStartCount;
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                runtime.QualificationDriveGameRuntimeFrames(2);
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                runtime.QualificationDriveGameRuntimeFrames(2);
                Assert.That(speech.PlayCount, Is.EqualTo(1), "pause does not replay speech");
                Assert.That(audio.QualificationMusicStartCount, Is.EqualTo(musicStarts),
                    "pause generation changes do not restart the music device");
                runtime.QualificationResetGameRuntimeProduction();
                Assert.That(speech.IsPlaying, Is.False);
                Assert.That(speech.SoundMix, Is.EqualTo(1));
                runtime.QualificationDriveGameRuntimeFrames(10);
                Assert.That(speech.PlayCount, Is.EqualTo(2), "reset owns a fresh authored announcement");
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void ProductionSpellKeysRejectUnacquiredSelectionWithoutCastingOrStoppingFlight()
        {
            var root = new GameObject("campaign unavailable spell key");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                var before = runtime.GameRuntimeProductionIdentity;
                var mana = runtime.GameRuntimeProductionCampaignSnapshot.value.player_vitals.mana;
                Assert.That(runtime.SubmitGameRuntimeSpellKey(0), Is.False);
                Assert.That(runtime.SubmitGameRuntimeSpellKey(5), Is.False);
                Assert.That(runtime.SubmitGameRuntimeSpellKey(3), Is.False,
                    "number-key choices admit only advertised acquired spells");
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.GameRuntimeProductionIdentity.runtime_id, Is.EqualTo(before.runtime_id));
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo("1"));
                var after = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(after.value.presentation_entities, Is.Empty);
                Assert.That(after.value.player_vitals.mana, Is.EqualTo(mana));
                // The direct semantic command entry still reaches native
                // validation. Its failed select+cast must remain atomic and
                // leave the ordinary flight turn available.
                Assert.That(runtime.SubmitGameRuntimeSpellSelection("spell.castle"), Is.True);
                Assert.That(runtime.RequestGameRuntimeSelectedSpellCast(), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                var rejected = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(rejected.identity.tick, Is.EqualTo("2"));
                Assert.That(rejected.value.presentation_entities, Is.Empty);
                Assert.That(rejected.value.player_vitals.mana, Is.EqualTo(mana));
                var replay = JsonUtility.FromJson<RecordedInputReplay>(Encoding.UTF8.GetString(
                    runtime.QualificationSaveGameRuntimeProductionReplay()));
                Assert.That(replay.operations, Has.Length.EqualTo(2));
                Assert.That(replay.operations[0].command.cast_selected, Is.False);
                Assert.That(replay.operations[1].command.cast_selected, Is.False);
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                Assert.That(runtime.SubmitGameRuntimeSpellKey(1), Is.False);
                Assert.That(runtime.RequestGameRuntimeSelectedSpellCast(), Is.False);
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void DirectLevelThreePlayEntryCastsCarriedFireballAndSelectsCastleOnSameHandle()
        {
            var generated = Path.Combine(PrivateRoot, "generated");
            var replayPath = Path.Combine(Path.GetTempPath(), $"mc-level3-play-{Guid.NewGuid():N}.replay.json");
            var root = new GameObject("direct owner Level 3 spell controls");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                var arguments = new[]
                {
                    "-mc-game-runtime",
                    "-mc-runtime-package", LevelOnePackage,
                    "-mc-runtime-package", Path.Combine(generated, "level2-day-block32-campaign-v2"),
                    "-mc-runtime-package", Path.Combine(generated, "level3-day-block32-campaign-recipe-v2"),
                    "-mc-runtime-owner-cache", Path.Combine(generated, "level3-day-spell-owner-cache-audio-v4/components/material"),
                    "-mc-runtime-presentation-cache", Path.Combine(generated, "level3-day-spell-owner-cache-audio-v4"),
                    "-mc-runtime-loading-cache", LevelOneOwnerCache,
                    "-mc-runtime-replay-load", Path.Combine(generated, "opened-l3-block32-castle-boot-c330992-20261005.json"),
                    "-mc-runtime-replay-save", replayPath,
                };
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(arguments);
                Assert.That(runtime.GameRuntimeProductionStartupReady, Is.True);
                Assert.That(runtime.GameRuntimeProductionIdentity.active_runtime_flow.bound_level_id,
                    Is.EqualTo("level.magic_carpet_2.revealed_way"));
                Assert.That(runtime.GameRuntimeLevel3PresentationTick, Is.EqualTo(1));
                Assert.That(runtime.Level2ProductionDayHudPresenterReady, Is.True);
                var before = runtime.GameRuntimeProductionIdentity;
                Assert.That(runtime.SubmitGameRuntimeSpellKey(1), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.RequestGameRuntimeSelectedSpellCast(), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                // This recipe uses the qualified indexed world feed. Its
                // optional neutral billboard asset is absent, so read the
                // same native entity projection that Unity actually draws.
                var rows = new NativeSim.CampaignEntitySnapshot[1000];
                Assert.That(NativeSim.mc_native_campaign_entities_copy(
                    ulong.Parse(before.runtime_id), 0, rows, (uint)rows.Length,
                    (uint)Marshal.SizeOf<NativeSim.CampaignEntitySnapshot>(), out var written),
                    Is.EqualTo(NativeSim.Ok));
                Assert.That(written, Is.EqualTo(rows.Length));
                var player = runtime.Level2ProductionPlayers[0].entityIndex;
                var flyer = Array.Find(rows, row => row.active != 0 && row.classId == 9 &&
                    row.model == 0 && row.sourceId == player);
                Assert.That(flyer.active, Is.EqualTo(1), "the public cast creates a live Fireball");
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.GameRuntimeProductionCampaignSnapshot.value.player_vitals.mana,
                    Is.EqualTo(900), "the next source player visit debits the real cast");
                Assert.That(runtime.SubmitGameRuntimeSpellKey(3), Is.True);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(runtime.Level2ProductionPlayers[0].selectedSpellUiIndex, Is.EqualTo(2));
                StringAssert.Contains(
                    "\"semantic_spell_carry\":{\"acquired\":[\"fireball\",\"possession\",\"castle\",\"speed\"],\"selected\":\"fireball\"}",
                    Encoding.UTF8.GetString(runtime.QualificationSaveGameRuntimeProductionReplay()));
                Assert.That(runtime.GameRuntimeProductionIdentity.runtime_id, Is.EqualTo(before.runtime_id));
                Assert.That(runtime.GameRuntimeProductionIdentity.session.tick, Is.EqualTo("5"));
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                runtime.QualificationDriveGameRuntimeFrames(1);
                var paused = runtime.GameRuntimeProductionCampaignSnapshot;
                runtime.QualificationRequestGameRuntimeProductionReplaySave();
                Assert.That(File.Exists(replayPath), Is.False);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(File.Exists(replayPath), Is.True);
                arguments[Array.IndexOf(arguments, "-mc-runtime-replay-load") + 1] = replayPath;
                var restoredRoot = new GameObject("restored Level 3 play entry");
                restoredRoot.SetActive(false);
                var restored = restoredRoot.AddComponent<MagicCarpetRuntime>();
                try
                {
                    restored.QualificationInitializeGameRuntimeProductionFromArguments(arguments);
                    Assert.That(restored.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                    AssertSameCampaignState(paused, restored.GameRuntimeProductionCampaignSnapshot);
                    Assert.That(restored.Level2ProductionPlayers[0].selectedSpellUiIndex, Is.EqualTo(2));
                    StringAssert.Contains(
                        "\"semantic_spell_carry\":{\"acquired\":[\"fireball\",\"possession\",\"castle\",\"speed\"],\"selected\":\"fireball\"}",
                        Encoding.UTF8.GetString(restored.QualificationSaveGameRuntimeProductionReplay()));
                    restored.QualificationApplyGameRuntimeProductionInput(true, false);
                    restored.QualificationDriveGameRuntimeFrames(1);
                    Assert.That(restored.GameRuntimeProductionIdentity.session.tick, Is.EqualTo("6"));
                    Assert.That(restored.SubmitGameRuntimeSpellKey(2), Is.True);
                    restored.QualificationDriveGameRuntimeFrames(1);
                    Assert.That(restored.Level2ProductionPlayers[0].selectedSpellUiIndex, Is.EqualTo(1));
                    Assert.That(restored.SubmitGameRuntimeSpellKey(4), Is.True);
                    restored.QualificationDriveGameRuntimeFrames(1);
                    Assert.That(restored.Level2ProductionPlayers[0].selectedSpellUiIndex, Is.EqualTo(3));
                }
                finally
                {
                    restored.QualificationDisposeNativeOwners();
                    UnityEngine.Object.DestroyImmediate(restoredRoot);
                }
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void ProductionGameRuntimeVitalsHudConsumesRustFireballManaAndResetOnSameHandle()
        {
            var root = new GameObject("resolver runtime vitals HUD");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                var beforeIdentity = runtime.GameRuntimeProductionIdentity;
                var before = runtime.GameRuntimeProductionCampaignSnapshot;
                var beforeVitals = before.value.player_vitals;
                Assert.That(beforeVitals, Is.Not.Null);
                Assert.That(beforeVitals.life, Is.Positive);
                Assert.That(beforeVitals.max_life, Is.GreaterThanOrEqualTo(beforeVitals.life));
                Assert.That(beforeVitals.mana, Is.Positive);
                Assert.That(beforeVitals.max_mana, Is.GreaterThanOrEqualTo(beforeVitals.mana));
                Assert.That(
                    runtime.GameRuntimeProductionVitalsBarWidths,
                    Is.EqualTo(new Vector2Int(240, 240)));
                Assert.That(
                    runtime.GameRuntimeProductionFlightHudText,
                    Does.Contain(
                        $"life {beforeVitals.life,6} / {beforeVitals.max_life,6}  " +
                        $"mana {beforeVitals.mana,6} / {beforeVitals.max_mana,6}"));

                runtime.QualificationRequestGameRuntimeFireball();
                runtime.QualificationDriveGameRuntimeFrames(2);

                var afterIdentity = runtime.GameRuntimeProductionIdentity;
                var after = runtime.GameRuntimeProductionCampaignSnapshot;
                var afterVitals = after.value.player_vitals;
                Assert.That(afterIdentity.runtime_id, Is.EqualTo(beforeIdentity.runtime_id));
                Assert.That(
                    afterIdentity.session.session_id,
                    Is.EqualTo(beforeIdentity.session.session_id));
                Assert.That(after.identity.tick, Is.EqualTo("2"));
                Assert.That(afterVitals.life, Is.EqualTo(beforeVitals.life));
                Assert.That(afterVitals.max_life, Is.EqualTo(beforeVitals.max_life));
                Assert.That(afterVitals.max_mana, Is.EqualTo(beforeVitals.max_mana));
                Assert.That(afterVitals.mana, Is.EqualTo(beforeVitals.mana - 100));
                Assert.That(
                    runtime.GameRuntimeProductionVitalsBarWidths,
                    Is.EqualTo(new Vector2Int(
                        240,
                        (int)((long)afterVitals.mana * 240 / afterVitals.max_mana))));
                Assert.That(
                    runtime.GameRuntimeProductionFlightHudText,
                    Does.Contain(
                        $"life {afterVitals.life,6} / {afterVitals.max_life,6}  " +
                        $"mana {afterVitals.mana,6} / {afterVitals.max_mana,6}"));

                runtime.QualificationResetGameRuntimeProduction();
                var resetIdentity = runtime.GameRuntimeProductionIdentity;
                var reset = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(resetIdentity.runtime_id, Is.EqualTo(beforeIdentity.runtime_id));
                Assert.That(
                    resetIdentity.session.session_id,
                    Is.EqualTo(beforeIdentity.session.session_id));
                Assert.That(reset.identity.generation, Is.EqualTo("2"));
                Assert.That(reset.identity.tick, Is.EqualTo("0"));
                Assert.That(reset.value.player_vitals.life, Is.EqualTo(beforeVitals.life));
                Assert.That(reset.value.player_vitals.max_life, Is.EqualTo(beforeVitals.max_life));
                Assert.That(reset.value.player_vitals.mana, Is.EqualTo(beforeVitals.mana));
                Assert.That(reset.value.player_vitals.max_mana, Is.EqualTo(beforeVitals.max_mana));
                Assert.That(
                    runtime.GameRuntimeProductionVitalsBarWidths,
                    Is.EqualTo(new Vector2Int(240, 240)));
                Assert.That(
                    runtime.GameRuntimeProductionFlightHudText,
                    Does.Contain(
                        $"life {beforeVitals.life,6} / {beforeVitals.max_life,6}  " +
                        $"mana {beforeVitals.mana,6} / {beforeVitals.max_mana,6}"));

                // No-Claim: this proves generic numeric local-player life/mana
                // projection, generic bars, one real Fireball charge, reset,
                // and same-handle Unity consumption. It does not prove source-art
                // HUD, damage or death presentation, spell inventory, objectives,
                // L2/L3, audio, or completion transitions.
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void ProductionGameRuntimeHudConsumesOrderedSemanticFireballEventsOnceOnSameHandle()
        {
            var root = new GameObject("resolver runtime semantic event HUD");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                var beforeIdentity = runtime.GameRuntimeProductionIdentity;
                var before = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(
                    runtime.GameRuntimeProductionSemanticEventCursor,
                    Is.EqualTo(before.value.effect_head));
                Assert.That(runtime.GameRuntimeProductionRecentSemanticEvents, Is.Empty);

                runtime.QualificationApplyGameRuntimeProductionInput(false, true);
                runtime.QualificationDriveGameRuntimeFrames(1);

                var afterIdentity = runtime.GameRuntimeProductionIdentity;
                var after = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(afterIdentity.runtime_id, Is.EqualTo(beforeIdentity.runtime_id));
                Assert.That(
                    afterIdentity.session.session_id,
                    Is.EqualTo(beforeIdentity.session.session_id));
                Assert.That(after.identity.tick, Is.EqualTo("1"));
                Assert.That(
                    runtime.GameRuntimeProductionSemanticEventCursor,
                    Is.EqualTo(after.value.effect_head));

                var events = runtime.GameRuntimeProductionRecentSemanticEvents;
                Assert.That(events, Is.Not.Empty);
                var expectedSequence = ulong.Parse(before.value.effect_head);
                EngineSessionTransport.GameEvent activation = null;
                foreach (var gameEvent in events)
                {
                    expectedSequence++;
                    Assert.That(gameEvent.sequence, Is.EqualTo(expectedSequence.ToString()));
                    if (gameEvent.event_id == "spell.activation_requested")
                    {
                        activation = gameEvent;
                    }
                }
                Assert.That(expectedSequence.ToString(), Is.EqualTo(after.value.effect_head));
                Assert.That(activation, Is.Not.Null);
                Assert.That(ulong.TryParse(activation.actor, out var actor), Is.True);
                Assert.That(actor, Is.Not.Zero);
                var latest = events[^1];
                Assert.That(
                    runtime.GameRuntimeProductionFlightHudText,
                    Does.Contain(
                        $"event {latest.sequence} {latest.event_id} " +
                        $"actor={(string.IsNullOrEmpty(latest.actor) ? "-" : latest.actor)}"));

                var cursor = runtime.GameRuntimeProductionSemanticEventCursor;
                runtime.QualificationRefreshGameRuntimeProductionEvents();
                var repeated = runtime.GameRuntimeProductionRecentSemanticEvents;
                Assert.That(runtime.GameRuntimeProductionSemanticEventCursor, Is.EqualTo(cursor));
                Assert.That(repeated.Length, Is.EqualTo(events.Length));
                for (var index = 0; index < events.Length; index++)
                {
                    Assert.That(repeated[index].sequence, Is.EqualTo(events[index].sequence));
                    Assert.That(repeated[index].event_id, Is.EqualTo(events[index].event_id));
                    Assert.That(repeated[index].actor, Is.EqualTo(events[index].actor));
                }

                // No-Claim: this proves the production HUD drains one ordered,
                // same-session semantic Fireball event stream exactly once. It
                // does not play audio, localize hints, place impact VFX, retain
                // UI history across reset, or prove later-level presentation.
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void ProductionMagicCarpetRuntimeRendersTheSemanticFireballAssetWithoutOwnerCache()
        {
            const string content =
                "sha256:331df08c5fa8483d5dd75dc10abd9dd76cc26918d6447403b3f3199bc31f521d";
            var nativeSessions = NativeSession.ActiveSessionCount;
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
            var root = new GameObject("semantic Fireball runtime owner");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                var texture = runtime.GameRuntimeProductionFireballTexture;
                Assert.That(texture, Is.Not.Null);
                Assert.That(texture.width, Is.EqualTo(34));
                Assert.That(texture.height, Is.EqualTo(35));
                Assert.That(
                    runtime.GameRuntimeProductionFireballContentAddress,
                    Is.EqualTo(content));
                Assert.That(runtime.GameRuntimeProductionFireballMarkerCount, Is.Zero);
                Assert.That(
                    runtime.GameRuntimeProductionCampaignSnapshot.value.presentation_entities,
                    Is.Empty);

                runtime.QualificationRequestGameRuntimeFireball();
                runtime.QualificationDriveGameRuntimeFrames(1);
                var snapshot = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(snapshot.identity.tick, Is.EqualTo("1"));
                Assert.That(snapshot.value.presentation_entities, Has.Length.EqualTo(1));
                var fireball = snapshot.value.presentation_entities[0];
                Assert.That(fireball.kind, Is.EqualTo("spell.fireball.projectile"));
                Assert.That(fireball.logical_height, Is.EqualTo(100));
                Assert.That(
                    fireball.asset.id,
                    Is.EqualTo("presentation.spell.fireball.projectile_asset"));
                Assert.That(fireball.asset.content_address, Is.EqualTo(content));
                Assert.That(ulong.Parse(fireball.actor), Is.Not.Zero);
                Assert.That(runtime.GameRuntimeProductionFireballMarkerCount, Is.EqualTo(1));
                Assert.That(
                    runtime.GameRuntimeProductionRenderedFireballTexture,
                    Is.SameAs(texture));
                Assert.That(runtime.GameRuntimeProductionFireballTexture, Is.SameAs(texture));

                runtime.QualificationResetGameRuntimeProduction();
                Assert.That(
                    runtime.GameRuntimeProductionCampaignSnapshot.identity.generation,
                    Is.EqualTo("2"));
                Assert.That(
                    runtime.GameRuntimeProductionCampaignSnapshot.value.presentation_entities,
                    Is.Empty);
                Assert.That(runtime.GameRuntimeProductionFireballMarkerCount, Is.Zero);
                Assert.That(runtime.GameRuntimeProductionFireballTexture, Is.SameAs(texture));
                Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions + 1));
                Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions));
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);

            // No-Claim: this proves one real L1 one-frame Fireball asset is
            // queried and rendered from the resolver-owned runtime snapshot
            // without an owner-cache/TMAP selector path. It does not prove
            // terrain, other entities, impacts, animation, L2/L3 assets,
            // completion transitions, or a complete playable scene.
        }

        [Test]
        [Timeout(180000)]
        public void RustGameRuntimePausedFrameServicesTheSameLevelOneSession()
        {
            using var runtime = new GameRuntimeTransport();
            runtime.BootstrapPackageSet(ResolverPackages());
            var playing = runtime.OpenLevel();
            var before = runtime.CampaignSnapshot();

            var paused = runtime.Pause();
            Assert.That(paused.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
            Assert.That(paused.generation, Is.EqualTo("4"));
            Assert.That(paused.session.session_id, Is.EqualTo(playing.session.session_id));
            Assert.That(paused.session.generation, Is.EqualTo("2"));
            Assert.That(paused.session.tick, Is.EqualTo("0"));
            Assert.That(
                paused.active_runtime_flow.bound_level_id,
                Is.EqualTo(playing.active_runtime_flow.bound_level_id));
            Assert.That(
                paused.session.definition_hash,
                Is.EqualTo(playing.session.definition_hash));

            paused.mode = GameRuntimeTransport.PlayingMode;
            Assert.That(
                runtime.IdentitySnapshot().mode,
                Is.EqualTo(GameRuntimeTransport.PausedMode));

            var command = Encoding.UTF8.GetBytes("{}");
            Assert.That(
                NativeSim.mc_game_session_step_json_v1(
                    runtime.Handle,
                    2,
                    0,
                    command,
                    checked((uint)command.Length)),
                Is.EqualTo(NativeSim.InvalidArgument));
            Assert.That(
                NativeSim.mc_game_session_reset_v1(runtime.Handle, 2, 0),
                Is.EqualTo(NativeSim.InvalidArgument));
            var afterFirstPausedFrame = runtime.CampaignPausedFrame();
            Assert.That(afterFirstPausedFrame.identity.tick, Is.EqualTo(before.identity.tick));
            Assert.That(
                afterFirstPausedFrame.value.state_hash,
                Is.Not.EqualTo(before.value.state_hash));
            Assert.That(
                afterFirstPausedFrame.value.terrain_hash,
                Is.EqualTo(before.value.terrain_hash));
            Assert.That(
                afterFirstPausedFrame.value.effect_journal_sha256,
                Is.EqualTo(before.value.effect_journal_sha256));
            Assert.That(
                afterFirstPausedFrame.value.player_pose.x,
                Is.EqualTo(before.value.player_pose.x));
            Assert.That(
                afterFirstPausedFrame.value.player_pose.y,
                Is.EqualTo(before.value.player_pose.y));
            Assert.That(
                afterFirstPausedFrame.value.player_pose.z,
                Is.EqualTo(before.value.player_pose.z));
            var afterSecondPausedFrame = runtime.CampaignPausedFrame();
            Assert.That(afterSecondPausedFrame.identity.tick, Is.EqualTo(before.identity.tick));
            Assert.That(
                afterSecondPausedFrame.value.state_hash,
                Is.Not.EqualTo(afterFirstPausedFrame.value.state_hash));

            var noticesBefore = runtime.ParticipantNotifications();
            Assert.That(noticesBefore.value, Is.Empty);
            var hiddenFrame = runtime.PresentationFrame(false);
            var visibleFrame = runtime.PresentationFrame(true);
            Assert.That(hiddenFrame.identity.tick, Is.EqualTo(before.identity.tick));
            Assert.That(visibleFrame.identity.generation, Is.EqualTo(paused.session.generation));
            Assert.That(visibleFrame.value, Is.Empty);
            Assert.That(runtime.CampaignSnapshot().value.state_hash,
                Is.EqualTo(afterSecondPausedFrame.value.state_hash),
                "a participant presentation frame does not run the paused RNG/objective prefix");

            var resumed = runtime.Resume();
            Assert.That(resumed.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
            Assert.That(resumed.generation, Is.EqualTo("5"));
            Assert.That(resumed.session.generation, Is.EqualTo("3"));
            Assert.That(resumed.session.tick, Is.EqualTo("0"));
            Assert.That(
                resumed.active_runtime_flow.bound_level_id,
                Is.EqualTo(playing.active_runtime_flow.bound_level_id));
            Assert.That(
                NativeSim.mc_game_session_step_json_v1(
                    runtime.Handle,
                    2,
                    0,
                    command,
                    checked((uint)command.Length)),
                Is.EqualTo(NativeSim.GenerationMismatch));
            Assert.That(
                NativeSim.mc_game_session_step_json_v1(
                    runtime.Handle,
                    3,
                    0,
                    command,
                    checked((uint)command.Length)),
                Is.EqualTo(NativeSim.Ok));
            Assert.That(runtime.IdentitySnapshot().session.tick, Is.EqualTo("1"));

            // This proves the managed transport services two real-L1 native
            // paused frames while gameplay identity/tick/pose remain frozen,
            // then resumes ordinary stepping. Production consumption is owned
            // by the dedicated MagicCarpetRuntime paused-Update test.
        }

        [Test]
        [Timeout(180000)]
        public void RustGameRuntimeOwnsTheBorrowedCampaignStepEventAndResetFacet()
        {
            var runtime = new GameRuntimeTransport();
            var handle = runtime.Handle;
            try
            {
                runtime.BootstrapPackageSet(ResolverPackages());
                var playing = runtime.OpenLevel();
                Assert.That(playing.session.session_id, Is.EqualTo(handle.ToString()));

                var initial = runtime.CampaignSnapshot();
                using (var other = new GameRuntimeTransport())
                {
                    other.BootstrapPackageSet(ResolverPackages());
                    var otherPlaying = other.OpenLevel();
                    var otherInitial = other.CampaignSnapshot();
                    Assert.That(otherPlaying.session.generation, Is.EqualTo("1"));
                    Assert.That(otherPlaying.session.tick, Is.EqualTo("0"));
                    Assert.That(other.Handle, Is.Not.EqualTo(handle));
                    Assert.Throws<InvalidOperationException>(() =>
                        EngineSessionTransport.StepHandle(
                            handle,
                            otherPlaying.session,
                            0,
                            0,
                            1,
                            0,
                            0,
                            runtime.NegotiatedSessionContract.limits.maximum_command_json_bytes));
                    var unchanged = runtime.CampaignSnapshot();
                    var otherUnchanged = other.CampaignSnapshot();
                    Assert.That(unchanged.identity.generation, Is.EqualTo("1"));
                    Assert.That(unchanged.identity.tick, Is.EqualTo("0"));
                    Assert.That(otherUnchanged.identity.generation, Is.EqualTo("1"));
                    Assert.That(otherUnchanged.identity.tick, Is.EqualTo("0"));
                    AssertSameCampaignState(initial, unchanged);
                    AssertSameCampaignState(otherInitial, otherUnchanged);
                }

                var stepped = runtime.CampaignStep(0, 0, 1, 0, 0);
                Assert.That(stepped.identity.session_id, Is.EqualTo(handle.ToString()));
                Assert.That(stepped.identity.generation, Is.EqualTo("1"));
                Assert.That(stepped.identity.tick, Is.EqualTo("1"));
                Assert.That(runtime.Current.generation, Is.EqualTo("3"));
                Assert.That(runtime.Current.session.tick, Is.EqualTo("1"));

                var head = ulong.Parse(stepped.value.effect_head);
                Assert.That(head, Is.GreaterThan(0UL));
                var cursor = "0";
                var drained = 0UL;
                while (true)
                {
                    var events = runtime.CampaignEvents(cursor);
                    Assert.That(events.identity.session_id, Is.EqualTo(handle.ToString()));
                    Assert.That(events.identity.tick, Is.EqualTo("1"));
                    Assert.That(events.value.requested_cursor, Is.EqualTo(cursor));
                    Assert.That(events.value.event_head, Is.EqualTo(stepped.value.effect_head));
                    foreach (var gameEvent in events.value.events)
                    {
                        drained++;
                        Assert.That(gameEvent.sequence, Is.EqualTo(drained.ToString()));
                    }
                    if (!events.value.has_more)
                    {
                        break;
                    }
                    Assert.That(events.value.next_cursor, Is.Not.EqualTo(cursor));
                    cursor = events.value.next_cursor;
                }
                Assert.That(drained, Is.EqualTo(head));

                Assert.That(
                    NativeSim.mc_game_session_destroy_v1(handle),
                    Is.EqualTo(NativeSim.InvalidArgument));
                Assert.That(runtime.IdentitySnapshot().session.tick, Is.EqualTo("1"));

                var reset = runtime.CampaignReset();
                Assert.That(reset.identity.session_id, Is.EqualTo(handle.ToString()));
                Assert.That(reset.identity.generation, Is.EqualTo("2"));
                Assert.That(reset.identity.tick, Is.EqualTo("0"));
                Assert.That(runtime.Current.generation, Is.EqualTo("3"));
                AssertSameCampaignState(initial, reset);

                var command = Encoding.UTF8.GetBytes("{}");
                Assert.That(
                    NativeSim.mc_game_session_step_json_v1(
                        handle,
                        1,
                        1,
                        command,
                        checked((uint)command.Length)),
                    Is.EqualTo(NativeSim.GenerationMismatch));

                runtime.Dispose();
                runtime.Dispose();
                Assert.That(
                    NativeSim.mc_game_session_snapshot_json_v1(
                        handle,
                        IntPtr.Zero,
                        0,
                        out _),
                    Is.EqualTo(NativeSim.UnknownHandle));

                // No-Claim: this is a borrow-only managed gameplay facet on
                // the Rust runtime handle. It does not replace the production
                // MagicCarpetRuntime bootstrap or prove campaign presentation,
                // HUD/minimap/audio, later-level policy, or paused-frame work.
            }
            finally
            {
                runtime.Dispose();
            }
        }
    }
}
