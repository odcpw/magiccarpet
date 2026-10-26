using System;
using UnityEngine;

namespace MagicCarpet.Native
{
    // DrawBottomSpellsMenu_2ECC0 / spell choice press, move and release.
    // This owner retains only the pointer choice. Rust owns unlocks, tier
    // admission, assignments and the resulting holder configuration.
    internal sealed class SourceSpellMenu
    {
        private EngineSessionTransport.SpellObservation choice;
        private int button, tier;
        private float virtualX, physicalX;

        internal static bool UsesBitmap(int selector) =>
            selector >= 88 && selector <= 122 || selector == 149 || selector == 150 ||
            selector >= 161 && selector <= 256;

        // Original MSPR artwork is indexed by family, while grid positions
        // use spellIndex_D94FF's different order.
        private static int Family(string id) => id switch {
            "spell.fireball" => 0, "spell.possession" => 1, "spell.castle" => 2,
            "spell.speed" => 3, "spell.morph" => 4, "spell.heal" => 5,
            "spell.shield" => 6, "spell.rebound" => 8, "spell.lightning" => 9,
            "spell.teleport" => 10, "spell.invisibility" => 11,
            "spell.beyond_sight" => 12, "spell.cave_in" => 25,
            _ => throw new InvalidOperationException("unbound source spell artwork: " + id)
        };

        internal void Close() { choice = null; }

        internal EngineSessionTransport.SpellAssignment Draw(
            EngineSessionTransport.CampaignSnapshotValue snapshot,
            Func<int, Texture2D> bitmap, Func<int, Vector2Int> size)
        {
            if (snapshot?.player_spells == null) return null;
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
                var spell = Array.Find(snapshot.player_spells, row => row.menu_slot == slot && row.acquired);
                Sprite(89, x, y);
                if (spell == null) continue;
                Sprite(97 + Family(spell.id), x, y);
                if (spell.hand == "left" || spell.hand == "both") Sprite(149, x, y);
                if (spell.hand == "right" || spell.hand == "both") Sprite(150, x + panelWidth - 12, y);
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
                    Sprite(179 + Family(choice.id) * 3 + value, x + 18, y + 6);
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
                choice = Array.Find(snapshot.player_spells, row => row.menu_slot == hovered && row.acquired);
                button = Event.current.button;
                tier = Mathf.Clamp(choice.tier, 0, choice.unlocked_tier);
                var extent = size(163);
                var start = MagicCarpetRuntime.SpellTierMenuStartX(hovered, columns, edge, panelWidth, 3, extent.x);
                virtualX = start + (tier + .5f) * extent.x;
                physicalX = pointer.x;
                Event.current.Use();
            }
            return null;
        }
    }
}
