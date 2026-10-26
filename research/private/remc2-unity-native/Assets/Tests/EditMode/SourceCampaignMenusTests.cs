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
    }
}
