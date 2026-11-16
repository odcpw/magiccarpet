using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class SemanticFlightEditModeTests
    {
        private static string PrivateRoot =>
            System.IO.Path.GetFullPath(System.IO.Path.Combine(UnityEngine.Application.dataPath, "../.."));

        private static string PackagePath =>
            System.IO.Path.Combine(PrivateRoot, "generated/level1-export-m");

        private static string ClosurePath =>
            System.IO.Path.Combine(PrivateRoot, "generated/level1-asset-closure.json");

        private static string AssetCachePath =>
            System.IO.Path.Combine(PrivateRoot, "generated/level1-unity-asset-cache-plan29-final");

        [Test]
        public void PreferencesCloseBlocksOnlyHeldControlsUntilTheirIndividualRelease()
        {
            var profile = Path.Combine(Path.GetTempPath(), $"mc-key-release-{Guid.NewGuid():N}");
            var keys = new HashSet<KeyCode>();
            var downKeys = new HashSet<KeyCode>();
            var buttons = new HashSet<int>();
            var downButtons = new HashSet<int>();
            try
            {
                using var owner = new GameRuntimeTransport();
                var preferences = new PlayerPreferencesPresenter(json =>
                    GameRuntimeTransport.PreferencesCommand(owner.Handle, owner.IdentitySnapshot(), json), profile,
                    keyHeld: keys.Contains, keyPressed: downKeys.Contains,
                    mouseHeld: buttons.Contains, mousePressed: downButtons.Contains);
                Assert.That(preferences.ApplySource(value => value.input_device = "keyboard"), Is.True);
                preferences.Open();
                preferences.BeginKeyCapture(0);
                Assert.That(preferences.CaptureKey(Event.KeyboardEvent("w")), Is.True);
                // The old and new forward keys, map key, and confirming mouse
                // button are down when the draft is committed.
                keys.UnionWith(new[] { KeyCode.UpArrow, KeyCode.W, KeyCode.Return, KeyCode.LeftShift,
                    KeyCode.RightShift, KeyCode.RightAlt, KeyCode.F3, KeyCode.F5, KeyCode.F6, KeyCode.F7, KeyCode.F10 });
                downKeys.Add(KeyCode.Return);
                downKeys.Add(KeyCode.F5);
                downKeys.Add(KeyCode.F6);
                downKeys.Add(KeyCode.F7);
                downKeys.Add(KeyCode.F3);
                buttons.Add(0);
                Assert.That(preferences.Apply(), Is.True);
                Assert.That(preferences.Current.preferences.keys[0], Is.EqualTo("W"));
                Assert.That(preferences.Held(0), Is.False);
                Assert.That(preferences.KeyHeld(KeyCode.UpArrow), Is.False, "old held binding remains suppressed");
                Assert.That(preferences.KeyHeld(KeyCode.LeftShift), Is.False, "source fixed modifier is suppressed");
                Assert.That(preferences.KeyHeld(KeyCode.RightShift), Is.False);
                Assert.That(preferences.KeyHeld(KeyCode.RightAlt), Is.False, "both physical Alt keys share source modifier precedence");
                Assert.That(preferences.KeyPressed(KeyCode.F5), Is.False, "held F5 cannot leak a save or display toggle");
                Assert.That(preferences.KeyPressed(KeyCode.F6), Is.False, "held F6 cannot leak the transparency toggle");
                Assert.That(preferences.KeyPressed(KeyCode.F7), Is.False, "held F7 cannot leak the flat shading toggle");
                Assert.That(preferences.KeyPressed(KeyCode.F3), Is.False, "held F3 cannot leak the speed action");
                Assert.That(preferences.KeyHeld(KeyCode.F10), Is.True, "reserved recovery controls stay available");
                Assert.That(preferences.Held(4), Is.False);
                Assert.That(preferences.Pressed(4), Is.False, "same-frame confirmation cannot open the map");
                Assert.That(preferences.MouseHeld(0), Is.False);
                var blocked = preferences.SampleInput(0, true);
                Assert.That(blocked.Forward || blocked.CastLeft, Is.False);
                var menus = new SourceCampaignMenus();
                var suppressedMenu = menus.Step(preferences.Pressed(4), preferences.Held(5), false,
                    preferences.MouseHeld(0), preferences.MouseHeld(1), true, preferences.Held(6),
                    preferences.KeyHeld(KeyCode.LeftShift) || preferences.Held(8),
                    preferences.KeyHeld(KeyCode.LeftAlt) || preferences.Held(9));
                Assert.That(suppressedMenu.Changed, Is.False, "closing input cannot open a source menu");
                Assert.That(suppressedMenu.CycleHand, Is.Zero);

                // A different key and button remain independently usable.
                keys.Add(KeyCode.LeftArrow);
                buttons.Add(1);
                var independent = preferences.SampleInput(0, true);
                Assert.That(independent.Left, Is.True);
                Assert.That(independent.CastRight, Is.True);
                Assert.That(independent.Forward || independent.CastLeft, Is.False);

                // Observe release while no flight input is sampled, then a
                // subsequent press is fresh rather than permanently latched.
                keys.ExceptWith(new[] { KeyCode.W, KeyCode.Return, KeyCode.LeftShift, KeyCode.RightShift,
                    KeyCode.RightAlt, KeyCode.F3, KeyCode.F5, KeyCode.F6, KeyCode.F7 });
                downKeys.Clear();
                buttons.Remove(0);
                preferences.AdvanceInputReleaseBarrier();
                keys.UnionWith(new[] { KeyCode.W, KeyCode.Return, KeyCode.RightShift, KeyCode.RightAlt, KeyCode.F3, KeyCode.F5, KeyCode.F6, KeyCode.F7 });
                downKeys.Add(KeyCode.Return);
                downKeys.Add(KeyCode.F5);
                downKeys.Add(KeyCode.F6);
                downKeys.Add(KeyCode.F7);
                downKeys.Add(KeyCode.F3);
                buttons.Add(0);
                Assert.That(preferences.Held(0), Is.True);
                Assert.That(preferences.Pressed(4), Is.True);
                var freshMenu = menus.Step(preferences.Pressed(4), preferences.Held(5), false,
                    preferences.MouseHeld(0), false, true);
                Assert.That(freshMenu.Action, Is.EqualTo("toggle_gameplay_map"),
                    "a fresh rebound map press reaches the source menu consumer");
                keys.Add(KeyCode.LeftShift);
                Assert.That(preferences.KeyHeld(KeyCode.LeftShift), Is.True);
                Assert.That(preferences.KeyHeld(KeyCode.RightShift), Is.True);
                Assert.That(preferences.KeyHeld(KeyCode.RightAlt), Is.True);
                Assert.That(preferences.KeyPressed(KeyCode.F5), Is.True, "release and repress admits the fresh F5 edge");
                Assert.That(preferences.KeyPressed(KeyCode.F6), Is.True, "release and repress admits the fresh F6 edge");
                Assert.That(preferences.KeyPressed(KeyCode.F7), Is.True, "release and repress admits the fresh F7 edge");
                Assert.That(preferences.KeyPressed(KeyCode.F3), Is.True, "release and repress admits the fresh F3 edge");
                Assert.That(preferences.MouseHeld(0), Is.True);

                // Cancelling another draft preserves the committed mapping
                // and blocks both the old and draft keys while held.
                preferences.Open();
                preferences.BeginKeyCapture(0);
                Assert.That(preferences.CaptureKey(Event.KeyboardEvent("s")), Is.True);
                keys.Add(KeyCode.S);
                preferences.Cancel();
                Assert.That(preferences.Current.preferences.keys[0], Is.EqualTo("W"));
                Assert.That(preferences.Held(0), Is.False);
                Assert.That(preferences.KeyHeld(KeyCode.S), Is.False, "cancelled draft key remains suppressed");
                Assert.That(preferences.MouseHeld(1), Is.False);
                keys.Remove(KeyCode.W);
                buttons.Remove(1);
                preferences.AdvanceInputReleaseBarrier();
                keys.Add(KeyCode.W);
                buttons.Add(1);
                Assert.That(preferences.Held(0), Is.True);
                Assert.That(preferences.MouseHeld(1), Is.True);
                Assert.That(preferences.Current.preferences.keys[0], Is.EqualTo("W"),
                    "cancel keeps the previously committed key active");
            }
            finally
            {
                if (Directory.Exists(profile)) Directory.Delete(profile, true);
            }
        }

        [Test]
        public void PreferenceKeyCaptureConsumesTextInputAndPersistsOnlyConfirmedBindings()
        {
            var profile = Path.Combine(Path.GetTempPath(), $"mc-key-capture-{Guid.NewGuid():N}");
            try
            {
                using (var owner = new GameRuntimeTransport())
                {
                    var preferences = new PlayerPreferencesPresenter(json =>
                        GameRuntimeTransport.PreferencesCommand(owner.Handle, owner.IdentitySnapshot(), json), profile);
                    preferences.Open();
                    preferences.BeginKeyCapture(0);
                    var key = Event.KeyboardEvent("w");
                    Assert.That(preferences.CaptureKey(key), Is.True);
                    Assert.That(key.type, Is.EqualTo(EventType.Used),
                        "the name field must not receive a captured character");
                    Assert.That(preferences.Current.preferences.keys[0], Is.EqualTo("UpArrow"),
                        "capture edits the pending preferences, not live controls");
                    preferences.BeginKeyCapture(1);
                    Assert.That(preferences.CaptureKey(new Event { type = EventType.KeyDown, keyCode = KeyCode.Minus }), Is.True);
                    var menuKeys = new[] { KeyCode.Q, KeyCode.E, KeyCode.T, KeyCode.Y, KeyCode.U, KeyCode.I };
                    for (var slot = 4; slot < 10; slot++)
                    {
                        preferences.BeginKeyCapture(slot);
                        Assert.That(preferences.CaptureKey(new Event { type = EventType.KeyDown, keyCode = menuKeys[slot - 4] }), Is.True);
                    }
                    Assert.That(preferences.Apply(), Is.True);
                    Assert.That(preferences.Current.preferences.keys[0], Is.EqualTo("W"));
                    Assert.That(preferences.Current.preferences.keys[1], Is.EqualTo("Minus"));
                    Assert.That(preferences.Current.preferences.player_name, Is.EqualTo("PLAYER"));
                    var confirmed = File.ReadAllBytes(Path.Combine(profile, "player-preferences.json"));

                    preferences.Open();
                    preferences.BeginKeyCapture(0);
                    Assert.That(preferences.CaptureKey(Event.KeyboardEvent("s")), Is.True);
                    preferences.Cancel();
                    Assert.That(preferences.Current.preferences.keys[0], Is.EqualTo("W"));
                    Assert.That(File.ReadAllBytes(Path.Combine(profile, "player-preferences.json")),
                        Is.EqualTo(confirmed));

                    preferences.Open();
                    preferences.BeginKeyCapture(0);
                    var recoveryKey = Event.KeyboardEvent("f10");
                    Assert.That(recoveryKey.keyCode, Is.EqualTo(KeyCode.F10));
                    Assert.That(preferences.CaptureKey(recoveryKey), Is.True);
                    Assert.That(preferences.Apply(), Is.False, "reserved recovery keys remain unavailable");
                    Assert.That(preferences.IsOpen, Is.True, "an invalid edit can still be cancelled");
                    preferences.Cancel();
                    Assert.That(preferences.Current.preferences.keys[0], Is.EqualTo("W"));
                }
                using var coldOwner = new GameRuntimeTransport();
                var held = new HashSet<KeyCode>();
                var down = new HashSet<KeyCode>();
                var buttons = new HashSet<int>();
                var coldPreferences = new PlayerPreferencesPresenter(json =>
                    GameRuntimeTransport.PreferencesCommand(coldOwner.Handle, coldOwner.IdentitySnapshot(), json), profile,
                    keyHeld: held.Contains, keyPressed: down.Contains, mouseHeld: buttons.Contains);
                Assert.That(coldPreferences.Current.load_status, Is.EqualTo("loaded"));
                Assert.That(coldPreferences.Current.preferences.keys[0], Is.EqualTo("W"));
                Assert.That(coldPreferences.Current.preferences.keys[1], Is.EqualTo("Minus"));
                Assert.That(coldPreferences.Current.preferences.player_name, Is.EqualTo("PLAYER"));
                var coldMenus = new SourceCampaignMenus();
                held.Add(KeyCode.W);
                Assert.That(coldPreferences.Held(0), Is.True, "reloaded forward key reaches the flight adapter");
                held.Add(KeyCode.Q); down.Add(KeyCode.Q);
                Assert.That(coldMenus.Step(coldPreferences.Pressed(4), coldPreferences.Held(5), false,
                    coldPreferences.MouseHeld(0), coldPreferences.MouseHeld(1), true).Action,
                    Is.EqualTo("toggle_gameplay_map"));
                held.Remove(KeyCode.Q); down.Clear(); held.Add(KeyCode.T);
                Assert.That(coldMenus.Step(false, coldPreferences.Held(5), false, false, false, true,
                    informationHeld: coldPreferences.Held(6)).Action, Is.EqualTo("show_player_information"));
                held.Remove(KeyCode.T); held.Add(KeyCode.E);
                Assert.That(coldMenus.Step(false, coldPreferences.Held(5), false, false, false, true).Action,
                    Is.EqualTo("open_spell_menu"));
                held.Add(KeyCode.Y);
                Assert.That(coldPreferences.Held(7), Is.True, "reloaded mark modifier reaches the spell-menu adapter");
                held.Remove(KeyCode.E); held.Remove(KeyCode.Y);
                Assert.That(coldMenus.Step(false, coldPreferences.Held(5), false, false, false, true).Action,
                    Is.EqualTo("close_spell_menu"));
                held.Add(KeyCode.Q); down.Add(KeyCode.Q);
                Assert.That(coldMenus.Step(coldPreferences.Pressed(4), false, false, false, false, true).Action,
                    Is.EqualTo("toggle_gameplay_map"));
                held.Remove(KeyCode.Q); down.Clear(); held.Add(KeyCode.U); buttons.Add(0);
                var next = coldMenus.Step(false, false, false, coldPreferences.MouseHeld(0), false, true,
                    nextMarkedHeld: coldPreferences.Held(8));
                Assert.That((next.CycleHand, next.CycleDirection), Is.EqualTo(((byte)1, 1)));
                held.Remove(KeyCode.U); buttons.Clear();
                coldMenus.Step(false, false, false, false, false, true);
                held.Add(KeyCode.I); buttons.Add(1);
                var previous = coldMenus.Step(false, false, false, false, coldPreferences.MouseHeld(1), true,
                    previousMarkedHeld: coldPreferences.Held(9));
                Assert.That((previous.CycleHand, previous.CycleDirection), Is.EqualTo(((byte)2, -1)));
            }
            finally
            {
                if (Directory.Exists(profile)) Directory.Delete(profile, true);
            }
        }

        [TestCase("Minus", "- ")]
        [TestCase("Equals", "- ")]
        [TestCase("Semicolon", "; ")]
        [TestCase("Backslash", "# ")]
        [TestCase("Comma", ", ")]
        [TestCase("Period", ". ")]
        [TestCase("Slash", "/ ")]
        public void SourceSettingsUsesOriginalNamesForSupportedPunctuation(string key, string label)
        {
            Assert.That(SourceSettingsDialog.SourceKeyName(key), Is.EqualTo(label));
        }

        [Test]
        [Timeout(120000)]
        public void PublishedFlightLimitChangesTheOrdinaryRustObservationOnly()
        {
            var workspace = Environment.GetEnvironmentVariable("MC_SEMANTIC_VARIANT_WORKSPACE");
            Assert.That(workspace, Is.Not.Null.And.Not.Empty,
                "F6 qualification must provide the published definition workspace");
            using var baseline = NativeSession.OpenEngine(
                PackagePath, ClosurePath, AssetCachePath);
            using var authored = NativeSession.OpenEngine(
                PackagePath, ClosurePath, AssetCachePath, workspace);

            for (var tick = 0; tick < 2; tick++)
            {
                baseline.Step(0, 0, 1);
                authored.Step(0, 0, 1);
            }

            var baselineSnapshot = baseline.EngineSnapshot();
            var authoredSnapshot = authored.EngineSnapshot();
            Assert.That(baselineSnapshot.value.player_motion.target_speed, Is.EqualTo(32));
            Assert.That(authoredSnapshot.value.player_motion.target_speed, Is.EqualTo(16));
            Assert.That(
                authoredSnapshot.value.player_motion.actual_speed,
                Is.Not.EqualTo(baselineSnapshot.value.player_motion.actual_speed));
            Assert.That(
                new[]
                {
                    authoredSnapshot.value.player_pose.x,
                    authoredSnapshot.value.player_pose.y,
                    unchecked((ushort)authoredSnapshot.value.player_pose.z),
                },
                Is.Not.EqualTo(new[]
                {
                    baselineSnapshot.value.player_pose.x,
                    baselineSnapshot.value.player_pose.y,
                    unchecked((ushort)baselineSnapshot.value.player_pose.z),
                }));
            Assert.That(authoredSnapshot.value.tick, Is.EqualTo(baselineSnapshot.value.tick));
            Assert.That(authoredSnapshot.value.caster_mana,
                Is.EqualTo(baselineSnapshot.value.caster_mana));
            Assert.That(authoredSnapshot.value.objective_count,
                Is.EqualTo(baselineSnapshot.value.objective_count));
            Assert.That(authoredSnapshot.value.action_sequence,
                Is.EqualTo(baselineSnapshot.value.action_sequence));
            Assert.That(
                authored.EngineEvents("0").value.events.Select(value => value.event_id),
                Is.EqualTo(baseline.EngineEvents("0").value.events.Select(value => value.event_id)));

            var firstValue = authoredSnapshot.value;
            authored.Reset();
            for (var tick = 0; tick < 2; tick++)
                authored.Step(0, 0, 1);
            var replay = authored.EngineSnapshot().value;
            Assert.That(replay.player_motion.target_speed,
                Is.EqualTo(firstValue.player_motion.target_speed));
            Assert.That(replay.player_motion.actual_speed,
                Is.EqualTo(firstValue.player_motion.actual_speed));
            Assert.That(replay.player_pose.x, Is.EqualTo(firstValue.player_pose.x));
            Assert.That(replay.player_pose.y, Is.EqualTo(firstValue.player_pose.y));
            Assert.That(replay.player_pose.z, Is.EqualTo(firstValue.player_pose.z));
            Assert.That(replay.state_hash, Is.EqualTo(firstValue.state_hash));
            Assert.That(replay.conformance_hash, Is.EqualTo(firstValue.conformance_hash));
        }
    }
}
