using System;
using System.Collections.Generic;
using UnityEngine;

namespace MagicCarpet.Native
{
    // DrawBottomSpellsMenu_2ECC0 / spell choice press, move and release.
    // This owner retains only the pointer choice. Rust owns unlocks, tier
    // admission, assignment/mark state and the resulting holder configuration.
    internal sealed class SourceSpellMenu
    {
        private EngineSessionTransport.SpellObservation choice;
        private int button, tier;
        private float virtualX, physicalX;

        internal static bool UsesBitmap(int selector) =>
            selector >= 88 && selector <= 122 || selector == 149 || selector == 150 ||
            selector >= 161 && selector <= 256;

        // Only selectors from the retained source MSPR spell banks may be
        // passed to the bitmap cache. A missing or malformed binding leaves
        // the cell unavailable instead of aborting the gameplay GUI.
        internal static bool HasArtwork(EngineSessionTransport.SpellObservation spell)
        {
            var art = spell?.menu_artwork;
            if (art == null || art.grid_icon < 97 || art.grid_icon > 122 ||
                art.tier_icons == null || art.tier_icons.Length != 3) return false;
            foreach (var icon in art.tier_icons)
                if (icon < 179 || icon > 256) return false;
            return true;
        }

        internal void Close() { choice = null; }

        // A drag can span simulation turns. Keep the choice tied to the live
        // acquired holder and slot, not the observation from the initial click.
        internal static EngineSessionTransport.SpellObservation CurrentChoice(
            EngineSessionTransport.SpellObservation selected,
            EngineSessionTransport.SpellObservation[] spells) =>
            selected == null || spells == null ? null : Array.Find(spells,
                row => row != null && row.acquired && HasArtwork(row) && row.id == selected.id &&
                    row.menu_slot == selected.menu_slot);

        internal static bool HasRequiredBitmaps(EngineSessionTransport.SpellObservation[] spells,
            Func<int, bool> available)
        {
            foreach (var selector in new[] { 88, 89, 90, 149, 150, 161, 163, 164, 165, 166, 167 })
                if (!available(selector)) return false;
            foreach (var spell in spells)
            {
                if (spell == null || !spell.acquired || !HasArtwork(spell)) continue;
                if (!available(spell.menu_artwork.grid_icon)) return false;
                foreach (var icon in spell.menu_artwork.tier_icons)
                    if (!available(icon)) return false;
            }
            return true;
        }

        internal EngineSessionTransport.SpellAssignment Draw(
            EngineSessionTransport.CampaignSnapshotValue snapshot,
            Func<int, Texture2D> bitmap,
            Func<int, Vector2Int> size,
            bool markHeld,
            out EngineSessionTransport.SpellMark mark)
        {
            mark = null;
            if (snapshot?.player_spells == null) { Close(); return null; }
            bool Available(int selector)
            {
                try
                {
                    var extent = size(selector);
                    return extent.x > 0 && extent.y > 0 && bitmap(selector) != null;
                }
                catch (KeyNotFoundException) { return false; }
            }
            if (!HasRequiredBitmaps(snapshot.player_spells, Available)) { Close(); return null; }
            if (choice != null)
            {
                choice = CurrentChoice(choice, snapshot.player_spells);
                if (choice != null) tier = Mathf.Clamp(tier, 0, choice.unlocked_tier);
            }
            const int columns = 13, edge = 8, panelWidth = 48, panelHeight = 36, top = 408;
            var scale = Mathf.Min(Screen.width / 640f, Screen.height / 480f);
            var view = new Rect((Screen.width - 640 * scale) / 2, (Screen.height - 480 * scale) / 2, 640 * scale, 480 * scale);
            var pointer = (Event.current.mousePosition - view.position) / scale;
            void Sprite(int selector, int x, int y)
            {
                var extent = size(selector);
                GUI.DrawTexture(new Rect(view.x + x * scale, view.y + y * scale,
                    extent.x * scale, extent.y * scale), bitmap(selector), ScaleMode.StretchToFill, true);
            }
            Sprite(88, 0, top); Sprite(88, 640 - edge, top);
            var hovered = -1;
            for (var slot = 0; slot < columns * 2; slot++)
            {
                var x = edge + slot % columns * panelWidth;
                var y = top + slot / columns * panelHeight;
                var spell = Array.Find(snapshot.player_spells, row => row != null &&
                    row.menu_slot == slot && row.acquired && HasArtwork(row));
                Sprite(89, x, y);
                if (spell == null) continue;
                Sprite(spell.menu_artwork.grid_icon, x, y);
                if (spell.marked_hand == "left") Sprite(149, x, y);
                if (spell.marked_hand == "right") Sprite(150, x + panelWidth - size(149).x, y);
                if (new Rect(x, y, panelWidth, panelHeight).Contains(pointer))
                {
                    hovered = slot;
                    Sprite(90, x, y);
                }
            }
            if (choice != null)
            {
                var extent = size(163);
                var start = MagicCarpetRuntime.SpellTierMenuStartX(choice.menu_slot, columns, edge, panelWidth, 3, extent.x);
                if (Event.current.type == EventType.MouseMove || Event.current.type == EventType.MouseDrag)
                {
                    virtualX += pointer.x - physicalX;
                    physicalX = pointer.x;
                    tier = Mathf.Clamp(Mathf.FloorToInt((virtualX - start) / extent.x), 0, choice.unlocked_tier);
                }
                for (var value = 0; value < 3; value++)
                {
                    var x = start + value * extent.x;
                    var y = top - extent.y;
                    Sprite(value > choice.unlocked_tier ? 163 : 161, x, y);
                    if (value > choice.unlocked_tier) continue;
                    Sprite(165 + value, x + 6, y + 10);
                    Sprite(choice.menu_artwork.tier_icons[value], x + 18, y + 6);
                    if (value == tier) Sprite(164, x, y);
                }
                if (Event.current.type == EventType.MouseUp && Event.current.button == button)
                {
                    var result = new EngineSessionTransport.SpellAssignment {
                        spell_id = choice.id, tier = (byte)tier, hand = button == 0 ? "left" : "right"
                    };
                    Close(); Event.current.Use(); return result;
                }
            }
            else if (hovered >= 0 && Event.current.type == EventType.MouseDown && Event.current.button <= 1)
            {
                var spell = Array.Find(snapshot.player_spells, row => row != null &&
                    row.menu_slot == hovered && row.acquired && HasArtwork(row));
                if (markHeld)
                {
                    var hand = Event.current.button == 0 ? "left" : "right";
                    mark = new EngineSessionTransport.SpellMark {
                        spell_id = spell.id,
                        hand = spell.marked_hand == hand ? "none" : hand
                    };
                }
                else
                {
                    choice = spell;
                    button = Event.current.button;
                    tier = Mathf.Clamp(choice.tier, 0, choice.unlocked_tier);
                    var extent = size(163);
                    var start = MagicCarpetRuntime.SpellTierMenuStartX(hovered, columns, edge, panelWidth, 3, extent.x);
                    virtualX = start + (tier + .5f) * extent.x;
                    physicalX = pointer.x;
                }
                Event.current.Use();
            }
            return null;
        }
    }
}
