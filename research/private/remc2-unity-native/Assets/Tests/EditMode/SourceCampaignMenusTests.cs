using System;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class SourceCampaignMenusTests
    {
        [Test]
        public void SourceMapPathRetainsItsIntegerAsymmetryAndFollowingMoveArrival()
        {
            var path = new SourceMapMotion.Path(10, 3, 0, 0, 2);
            var expected = new[] { (2, 0), (4, 0), (6, 2), (8, 2), (10, 2) };
            foreach (var point in expected)
            {
                Assert.That(path.Move(), Is.False);
                Assert.That(((int)path.X, (int)path.Y), Is.EqualTo(point));
            }
            Assert.That(path.Move(), Is.True,
                "source arrival checks the dominant axis before moving, without inventing a final minor-axis step");
            var reverse = new SourceMapMotion.Path(0, 0, 10, 3, 2);
            for (var pass = 0; pass < 5; pass++) Assert.That(reverse.Move(), Is.False);
            Assert.That(((int)reverse.X, (int)reverse.Y), Is.EqualTo((0, 1)));
            Assert.That(reverse.Move(), Is.True);
        }

        [Test]
        public void SourceMapEdgePanUsesPriorAccelerationAndExactSourceBounds()
        {
            var map = new SourceMapMotion(Array.ConvertAll(new int[15], _ => new SourceMapMotion.BackgroundBinding()),
                Array.ConvertAll(new int[11], _ => new SourceMapMotion.DecorationBinding()));
            map.Pan(638, 478);
            Assert.That((map.X, map.Y), Is.EqualTo((0, 0)));
            map.Pan(638, 478);
            Assert.That((map.X, map.Y), Is.EqualTo((4, 4)));
            map.Pan(638, 478);
            Assert.That((map.X, map.Y), Is.EqualTo((12, 12)));
            map.Pan(320, 240); map.Pan(638, 478);
            Assert.That((map.X, map.Y), Is.EqualTo((12, 12)), "leaving the edge resets the next acceleration");
            for (var pass = 0; pass < 40; pass++) map.Pan(638, 478);
            Assert.That((map.X, map.Y), Is.EqualTo((638, 478)));
        }

        [Test]
        public void SaveSlotNameUsesSourceReplacementUppercaseAndFifteenCharacterLimit()
        {
            var edited = false;
            var label = SourceScrollDialog.EditName("OLD SLOT", ref edited, KeyCode.A, 'a');
            Assert.That(label, Is.EqualTo("A"), "first printable key replaces the selected slot label");
            foreach (var value in "bc 12345678901234567890")
                label = SourceScrollDialog.EditName(label, ref edited, KeyCode.None, value);
            Assert.That(label, Is.EqualTo("ABC 12345678901"));
            Assert.That(SourceScrollDialog.EditName(label, ref edited, KeyCode.None, '!'), Is.EqualTo(label));
            Assert.That(SourceScrollDialog.EditName(label, ref edited, KeyCode.Backspace, '\0'), Is.EqualTo("ABC 1234567890"));
        }

        [Test]
        public void SourceConfirmationUsesBothButtonsAndReturnEscapeWithinItsExactHitRegions()
        {
            var okay = new Rect(270, 104, 50, 32);
            var cancel = new Rect(320, 104, 50, 32);
            Assert.That(SourcePauseMenu.ConfirmationInput(EventType.KeyDown, KeyCode.Return,
                0, Vector2.zero, okay, cancel), Is.EqualTo(SourcePauseMenu.ConfirmationResult.Confirm));
            Assert.That(SourcePauseMenu.ConfirmationInput(EventType.KeyDown, KeyCode.Escape,
                0, Vector2.zero, okay, cancel), Is.EqualTo(SourcePauseMenu.ConfirmationResult.Cancel));
            foreach (var button in new[] { 0, 1 })
            {
                Assert.That(SourcePauseMenu.ConfirmationInput(EventType.MouseDown, KeyCode.None,
                    button, new Vector2(319, 104), okay, cancel), Is.EqualTo(SourcePauseMenu.ConfirmationResult.Confirm));
                Assert.That(SourcePauseMenu.ConfirmationInput(EventType.MouseDown, KeyCode.None,
                    button, new Vector2(320, 104), okay, cancel), Is.EqualTo(SourcePauseMenu.ConfirmationResult.Cancel));
                Assert.That(SourcePauseMenu.ConfirmationInput(EventType.MouseDown, KeyCode.None,
                    button, new Vector2(270, 136), okay, cancel), Is.EqualTo(SourcePauseMenu.ConfirmationResult.None));
            }
            Assert.That(SourcePauseMenu.ConfirmationInput(EventType.MouseDown, KeyCode.None,
                2, new Vector2(270, 104), okay, cancel), Is.EqualTo(SourcePauseMenu.ConfirmationResult.None));
            // Unity marks synthetic mouse events Ignore outside OnGUI. The
            // device adapter must continue to reject Ignore and Used; only
            // the pure source predicate receives the intended event type here.
            foreach (var type in new[] { EventType.Ignore, EventType.Used })
                Assert.That(SourcePauseMenu.ConfirmationInput(new Event {
                    button = 0, mousePosition = new Vector2(270, 104), type = type
                }, okay, cancel), Is.EqualTo(SourcePauseMenu.ConfirmationResult.None));
        }

        [Test]
        public void EnterMapAdmitsFlightAndCtrlDragReturnsToTheSameMap()
        {
            var menus = new SourceCampaignMenus();
            var opened = menus.Step(true, false, false, false, false, true);
            Assert.That(opened.MapChanged, Is.True);
            Assert.That(menus.MapOpen && menus.AdmitFlightInput && menus.CaptureMouse, Is.True);
            menus.Step(false, true, false, false, false, true);
            Assert.That(menus.Current, Is.EqualTo(SourceCampaignMenus.Mode.MapSpells));
            Assert.That(menus.AdmitFlightInput || menus.CaptureMouse, Is.False);
            menus.Step(false, false, false, true, false, true);
            Assert.That(menus.SpellOpen, Is.True, "Ctrl release cannot cancel a held assignment drag");
            menus.Step(false, false, false, false, false, true);
            Assert.That(menus.Current, Is.EqualTo(SourceCampaignMenus.Mode.Map));
            menus.Step(true, false, false, false, false, true);
            Assert.That(menus.Current, Is.EqualTo(SourceCampaignMenus.Mode.Flight));
        }

        [Test]
        public void MenuTransitionsEmitAuthorityActionsAndRejectedTransitionsRestoreTheirOrigin()
        {
            var menus = new SourceCampaignMenus();
            var spell = menus.Step(false, true, false, false, false, true);
            Assert.That(spell.Action, Is.EqualTo("open_spell_menu"));
            menus.RestoreRejectedTransition(spell);
            Assert.That(menus.Current, Is.EqualTo(SourceCampaignMenus.Mode.Flight));
            spell = menus.Step(false, true, false, false, false, true);
            Assert.That(spell.Action, Is.EqualTo("open_spell_menu"));
            Assert.That(menus.Step(false, false, false, false, false, true).Action,
                Is.EqualTo("close_gameplay_menus"));

            var map = menus.Step(true, false, false, false, false, true);
            Assert.That(map.Action, Is.EqualTo("toggle_gameplay_map"));
            var info = menus.Step(false, false, false, false, false, true, informationHeld: true);
            Assert.That(info.Action, Is.EqualTo("show_player_information"));
            menus.RestoreRejectedTransition(info);
            Assert.That(menus.Current, Is.EqualTo(SourceCampaignMenus.Mode.Map));
            info = menus.Step(false, false, false, false, false, true, informationHeld: true);
            Assert.That(info.Action, Is.EqualTo("show_player_information"));
            var mapSpell = menus.Step(false, true, false, false, false, true);
            Assert.That(mapSpell.Action, Is.EqualTo("open_spell_menu"));
            var closeSpell = menus.Step(false, false, false, false, false, true);
            Assert.That(closeSpell.Action, Is.EqualTo("close_spell_menu"));
            Assert.That(menus.Current, Is.EqualTo(SourceCampaignMenus.Mode.Map));
            Assert.That(menus.Step(false, false, false, false, false, true).Action, Is.Null);
        }

        [Test]
        public void PendingMenuActionReconcilesAuthorityAndConsumesOnlyMatchingReceipts()
        {
            var menus = new SourceCampaignMenus();
            var open = menus.Step(false, true, false, false, false, true);
            Assert.That(menus.QueueAction(open.Action, false, true, false, open), Is.True);
            Assert.That(menus.PendingAction, Is.EqualTo("open_spell_menu"));
            Assert.That(menus.QueueAction("toggle_gameplay_map", true, false, false), Is.False);
            menus.CompleteAction(new EngineSessionTransport.SourceMenuAdmission {
                action = "toggle_gameplay_map", admitted = true, map_open = true
            });
            Assert.That(menus.HasPendingAction, Is.True, "a different receipt cannot consume the intent");
            menus.CompleteAction(new EngineSessionTransport.SourceMenuAdmission {
                action = open.Action, admitted = true, player_information_open = true
            });
            Assert.That(menus.HasPendingAction, Is.True, "impossible flags cannot authorize a local state");
            menus.CompleteAction(new EngineSessionTransport.SourceMenuAdmission {
                action = open.Action, admitted = false, map_open = true
            });
            Assert.That(menus.HasPendingAction, Is.False);
            Assert.That(menus.Current, Is.EqualTo(SourceCampaignMenus.Mode.Map),
                "even a rejected action reconciles to the authoritative state");
            menus.CompleteAction(new EngineSessionTransport.SourceMenuAdmission {
                action = open.Action, admitted = true, spells_open = true
            });
            Assert.That(menus.Current, Is.EqualTo(SourceCampaignMenus.Mode.Map), "a receipt is consumed once");

            var info = menus.Step(false, false, false, false, false, true, informationHeld: true);
            Assert.That(menus.QueueAction(info.Action, true, false, true, info), Is.True);
            menus.CancelUnsubmittedAction();
            Assert.That(menus.Current, Is.EqualTo(SourceCampaignMenus.Mode.Map));
            Assert.That(menus.HasPendingAction, Is.False);

            var escape = menus.Step(false, false, true, false, false, true);
            Assert.That(escape.RequestAbandonConfirmation && menus.AbandonConfirmationPending, Is.True);
            Assert.That(menus.QueueAction("open_abandon_confirmation", true, false, false), Is.True);
            menus.CompleteAction(new EngineSessionTransport.SourceMenuAdmission {
                action = "open_abandon_confirmation", admitted = false, map_open = true
            });
            Assert.That(menus.AbandonConfirmationPending, Is.False);
            menus.Step(false, false, true, false, false, true);
            Assert.That(menus.QueueAction("open_abandon_confirmation", true, false, false), Is.True);
            menus.CompleteAction(new EngineSessionTransport.SourceMenuAdmission {
                action = "open_abandon_confirmation", admitted = true, map_open = true
            });
            Assert.That(menus.AbandonConfirmationPending, Is.True);
            Assert.That(menus.QueueAction("cancel_abandon_confirmation", true, false, false), Is.True);
            menus.CompleteAction(new EngineSessionTransport.SourceMenuAdmission {
                action = "cancel_abandon_confirmation", admitted = false, map_open = true
            });
            Assert.That(menus.AbandonConfirmationPending, Is.True);
            Assert.That(menus.QueueAction("cancel_abandon_confirmation", true, false, false), Is.True);
            menus.CompleteAction(new EngineSessionTransport.SourceMenuAdmission {
                action = "cancel_abandon_confirmation", admitted = true, map_open = true
            });
            Assert.That(menus.AbandonConfirmationPending, Is.False);
        }

        [Test]
        public void SpellDragUsesCurrentAcquiredHolderAndTierBinding()
        {
            var original = new EngineSessionTransport.SpellObservation {
                id = "spell.fireball", menu_slot = 3, acquired = true, unlocked_tier = 2
            };
            var current = new EngineSessionTransport.SpellObservation {
                id = original.id, menu_slot = original.menu_slot, acquired = true, unlocked_tier = 1,
                menu_artwork = new EngineSessionTransport.SpellMenuArtwork {
                    grid_icon = 97, tier_icons = new ushort[] { 179, 180, 181 }
                }
            };
            Assert.That(SourceSpellMenu.CurrentChoice(original, new[] { current }), Is.SameAs(current));
            Assert.That(SourceSpellMenu.HasRequiredBitmaps(new[] { current }, _ => true), Is.True);
            Assert.That(SourceSpellMenu.HasRequiredBitmaps(new[] { current }, selector => selector != 180), Is.False,
                "a missing retained tier bitmap must not reach the GUI lookup");
            Assert.That(SourceSpellMenu.HasRequiredBitmaps(new[] { current }, selector => selector != 88), Is.False,
                "a missing menu background must not reach the GUI lookup");
            original.id = current.id = "spell.new_family";
            Assert.That(SourceSpellMenu.CurrentChoice(original, new[] { current }), Is.SameAs(current),
                "artwork comes from the observation rather than a Unity ID switch");
            current.acquired = false;
            Assert.That(SourceSpellMenu.CurrentChoice(original, new[] { current }), Is.Null);
            current.acquired = true;
            current.menu_slot++;
            Assert.That(SourceSpellMenu.CurrentChoice(original, new[] { current }), Is.Null);
            current.menu_slot = original.menu_slot;
            current.id = "spell.shield";
            Assert.That(SourceSpellMenu.CurrentChoice(original, new[] { current }), Is.Null);
            Assert.That(SourceSpellMenu.CurrentChoice(original, null), Is.Null);
            current.id = original.id;
            current.menu_artwork.tier_icons = new ushort[] { 179, 180 };
            Assert.That(SourceSpellMenu.CurrentChoice(original, new[] { current }), Is.Null,
                "an incomplete tier binding cannot keep a drag active");
            current.menu_artwork.tier_icons = new ushort[] { 179, 180, 257 };
            Assert.That(SourceSpellMenu.HasArtwork(current), Is.False);
            current.menu_artwork.tier_icons[2] = 181;
            current.menu_artwork.grid_icon = 123;
            Assert.That(SourceSpellMenu.HasArtwork(current), Is.False);
            current.menu_artwork = null;
            Assert.That(SourceSpellMenu.HasArtwork(current), Is.False);
        }

        [Test]
        public void BothButtonsAreConsumedOnceAndEscapeRequestsTheSourceConfirmation()
        {
            var menus = new SourceCampaignMenus();
            Assert.That(menus.Step(false, false, false, true, true, true).SuppressCasting, Is.True);
            Assert.That(menus.Step(false, false, false, true, true, true).SuppressCasting, Is.True,
                "a consumed map gesture cannot begin casting on its following held frame");
            Assert.That(menus.MapOpen, Is.True, "holding both buttons cannot oscillate the map");
            var escape = menus.Step(false, false, true, false, false, true);
            Assert.That(escape.RequestAbandonConfirmation, Is.True);
            Assert.That(menus.MapOpen, Is.True, "confirmation returns to its source map origin");
            menus.Step(false, false, false, true, true, true);
            Assert.That(menus.MapOpen, Is.False);
            menus.Step(false, true, false, true, true, true);
            Assert.That(menus.Current, Is.EqualTo(SourceCampaignMenus.Mode.Spells), "Ctrl precedes the normal both-button map gate");
            Assert.That(menus.Step(false, true, true, false, false, true).RequestAbandonConfirmation, Is.False);
            menus.Step(false, true, false, false, false, false);
            Assert.That(menus.Current, Is.EqualTo(SourceCampaignMenus.Mode.Flight), "death closes source menus");
        }

        [Test]
        public void PlayerInformationIsAMapHoldAndRestoresTheMapOnRelease()
        {
            var menus = new SourceCampaignMenus();
            menus.Step(false, false, false, false, false, true, informationHeld: true);
            Assert.That(menus.Current, Is.EqualTo(SourceCampaignMenus.Mode.Flight));
            menus.Step(true, false, false, false, false, true);
            menus.Step(false, false, false, false, false, true, informationHeld: true);
            Assert.That(menus.PlayerInformationOpen && menus.MapOpen && menus.AdmitFlightInput, Is.True);
            menus.Step(false, true, false, false, false, true, informationHeld: true);
            Assert.That(menus.Current, Is.EqualTo(SourceCampaignMenus.Mode.MapSpells));
            menus.Step(false, false, false, false, false, true);
            Assert.That(menus.Current, Is.EqualTo(SourceCampaignMenus.Mode.Map));
            menus.Step(false, false, false, false, false, true, previousMarkedHeld: true);
            Assert.That(menus.PlayerInformationOpen, Is.True, "source Alt/reverse modifier also shows the scores");
            menus.Step(false, false, false, false, false, true);
            Assert.That(menus.Current, Is.EqualTo(SourceCampaignMenus.Mode.Map));
        }

        [Test]
        public void MarkedSpellModifiersConsumeOneHandPulseAndNeverCastOnFollowingHeldFrames()
        {
            var menus = new SourceCampaignMenus();
            var next = menus.Step(false, false, false, true, false, true, nextMarkedHeld: true);
            Assert.That((next.CycleHand, next.CycleDirection), Is.EqualTo(((byte)1, 1)));
            var held = menus.Step(false, false, false, true, false, true, nextMarkedHeld: true);
            Assert.That(held.CycleHand, Is.Zero);
            Assert.That(held.SuppressCasting, Is.True);
            menus.Step(false, false, false, false, false, true);
            var previous = menus.Step(false, false, false, false, true, true,
                nextMarkedHeld: true, previousMarkedHeld: true);
            Assert.That((previous.CycleHand, previous.CycleDirection), Is.EqualTo(((byte)2, -1)),
                "reverse has source precedence when both modifiers are held");
            menus.Step(false, false, false, false, false, true);
            var spell = menus.Step(false, true, false, true, false, true, nextMarkedHeld: true);
            Assert.That(spell.CycleHand, Is.Zero, "spell menu owns its marking/assignment click before cycling");
            Assert.That(menus.SpellOpen, Is.True);
        }

        [Test]
        public void InformationModifierCyclesBackwardsOnlyWithinTheMap()
        {
            var menus = new SourceCampaignMenus();
            menus.Step(true, false, false, false, false, true);
            var cycle = menus.Step(false, false, false, false, true, true, informationHeld: true);
            Assert.That((cycle.CycleHand, cycle.CycleDirection), Is.EqualTo(((byte)2, -1)));
            Assert.That(cycle.SuppressCasting, Is.True);
            menus.Step(false, false, false, false, false, true, informationHeld: true);
            Assert.That(menus.PlayerInformationOpen, Is.True);
            menus.Step(false, false, false, false, false, false, informationHeld: true);
            Assert.That(menus.Current, Is.EqualTo(SourceCampaignMenus.Mode.Flight));
        }

        [Test]
        public void TerminalVolcanoRequiresTheEarnedFinalOrdinaryMissionBinding()
        {
            const string finalId = "level.final";
            var binding = new SourceCampaignTerminalEffects.FinalMissionBinding(finalId, 24, false);
            var results = new GameRuntimeTransport.CampaignMenuSnapshot {
                phase = "results", completed_level_id = finalId,
                available_levels = new[] { finalId }, completed_levels = new[] { finalId }
            };
            var missionSelect = new GameRuntimeTransport.CampaignMenuSnapshot {
                phase = "mission_select", available_levels = new[] { finalId },
                completed_levels = new[] { finalId }
            };

            Assert.That(SourceCampaignTerminalEffects.IsEarnedFinalMission(results, binding), Is.True);
            Assert.That(SourceCampaignTerminalEffects.IsEarnedFinalMission(missionSelect, binding), Is.True,
                "the earned binding remains authoritative after dismissing results");
            Assert.That(SourceCampaignTerminalEffects.IsEarnedFinalMission(results, default), Is.False,
                "an absent owner binding fails closed");
            Assert.That(SourceCampaignTerminalEffects.IsEarnedFinalMission(results,
                new SourceCampaignTerminalEffects.FinalMissionBinding(finalId, 23, false)), Is.False,
                "a different ordinary ordinal cannot enable the terminal row");
            Assert.That(SourceCampaignTerminalEffects.IsEarnedFinalMission(results,
                new SourceCampaignTerminalEffects.FinalMissionBinding(finalId, 24, true)), Is.False,
                "a secret mission cannot enable the ordinary ending");

            var unrelatedCatalog = new string[25];
            for (var i = 0; i < unrelatedCatalog.Length; i++) unrelatedCatalog[i] = "other." + i;
            Assert.That(SourceCampaignTerminalEffects.IsEarnedFinalMission(new GameRuntimeTransport.CampaignMenuSnapshot {
                phase = "mission_select", available_levels = unrelatedCatalog,
                completed_levels = (string[])unrelatedCatalog.Clone()
            }, binding), Is.False, "25 completed unrelated entries do not substitute for the exact final ID");
        }

        [Test]
        public void EarnedTerminalVolcanoAnimationContinuesAcrossResultsDismissalAndMapReentry()
        {
            const string finalId = "level.final";
            var binding = new SourceCampaignTerminalEffects.FinalMissionBinding(finalId, 24, false);
            var results = new GameRuntimeTransport.CampaignMenuSnapshot {
                phase = "results", completed_level_id = finalId,
                available_levels = new[] { finalId }, completed_levels = new[] { finalId }
            };
            var missionSelect = new GameRuntimeTransport.CampaignMenuSnapshot {
                phase = "mission_select", selected_level_id = finalId,
                available_levels = new[] { finalId }, completed_levels = new[] { finalId }
            };
            var decorations = Array.ConvertAll(new int[11], _ => new SourceMapMotion.DecorationBinding());
            decorations[0] = new SourceMapMotion.DecorationBinding {
                kind = 3, x = 10, y = 12, target_x = 20, target_y = 12,
                first = 100, last = 102
            };
            var motion = new SourceMapMotion(
                Array.ConvertAll(new int[15], _ => new SourceMapMotion.BackgroundBinding()), decorations);
            var effects = new SourceCampaignTerminalEffects();
            OwnerCampaignMenuCache.Mission Lookup(string id) => new() {
                level_id = id, viewport_x = 0, viewport_y = 0, portal_x = 0, portal_y = 0
            };
            void Step(GameRuntimeTransport.CampaignMenuSnapshot menu, uint now, uint pass)
            {
                var enabled = effects.Step(menu, binding, pass);
                Assert.That(enabled, Is.True, "the test binding is present and authoritatively earned");
                motion.Step(menu, Lookup, now, 0, 0, false, false, 0, 0,
                    _ => (1, 1), (_, _) => { }, (_, _, _) => { }, enabled);
            }

            motion.Enter(results, Lookup, finalId, (_, _) => { }, 0, 0);
            Step(results, 0, 1);  // Starts the source animation row.
            Step(results, 8, 2);  // Creates its persistent path.
            Step(results, 16, 3); // Advances the source frame and position.
            Assert.That(motion.DecorationSprites[0].Selector, Is.EqualTo(101));
            Assert.That((motion.DecorationSprites[0].X, motion.DecorationSprites[0].Y), Is.EqualTo((18, 12)));

            effects.Reset(); // SourceCampaignMap.Enter resets eligibility, not the motion row.
            motion.Enter(missionSelect, Lookup, finalId, (_, _) => { }, 0, 0);
            Step(missionSelect, 24, 4);
            Assert.That(motion.DecorationSprites[0].Selector, Is.EqualTo(102),
                "reentry keeps the already-running kind-3 selector instead of restarting phase zero");
            Assert.That((motion.DecorationSprites[0].X, motion.DecorationSprites[0].Y), Is.EqualTo((20, 12)));
        }
    }
}
