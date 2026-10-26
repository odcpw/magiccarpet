using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class SourceCampaignMenusTests
    {
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
