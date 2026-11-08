using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.InputSystem;

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
        private OwnerAssetCache cursorCache;
        private Texture2D cursorTexture;
        private bool cursorActive, previousCursorVisible, cursorFailed, warpPending, releasePending;
        private int warpDeadline;
        private Vector2 warpTarget;

        internal static bool UsesBitmap(int selector) =>
            selector >= 88 && selector <= 122 || selector == 149 || selector == 150 ||
            selector >= 161 && selector <= 256;

        internal static Vector2 GridPointer(int slot) =>
            new Vector2(8 + slot % 13 * 48 + 24, 408 + slot / 13 * 36 + 18);

        internal static Vector2 TierPointer(int start, int width, int height, int tier) =>
            new Vector2(start + width * tier + width / 2 + 8, 408 - height + 18);

        internal static Vector2 WindowPointer(Vector2 source, Rect view, float scale, int screenHeight) =>
            new Vector2(view.x + source.x * scale, screenHeight - (view.y + source.y * scale));

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

        internal void Close()
        {
            choice = null;
            releasePending = false;
            if (cursorActive) Cursor.visible = previousCursorVisible;
            cursorActive = false;
            cursorFailed = warpPending = false;
        }

        private void PrepareCursor(OwnerAssetCache cache, EngineSessionTransport.SpellObservation[] spells,
            Rect view, float scale)
        {
            if (!ReferenceEquals(cache, cursorCache))
            {
                Close();
                if (cursorTexture != null) UnityEngine.Object.Destroy(cursorTexture);
                cursorTexture = null;
                cursorCache = cache;
                var material = cache?.EnvironmentMaterials;
                var cursor = material?.cursor;
                if (cursor != null && cursor.width > 0 && cursor.height > 0)
                {
                    var indices = cache.ReadOutput(cursor.indices_path);
                    var mask = cache.ReadOutput(cursor.mask_path);
                    var palette = cache.ReadOutput(material.palette.rgb8_path);
                    var pixels = new Color32[indices.Length];
                    for (var y = 0; y < cursor.height; y++)
                    for (var x = 0; x < cursor.width; x++)
                    {
                        var source = y * cursor.width + x;
                        if (mask[source] == 0) continue;
                        var color = indices[source] * 3;
                        pixels[(cursor.height - 1 - y) * cursor.width + x] =
                            new Color32(palette[color], palette[color + 1], palette[color + 2], 255);
                    }
                    cursorTexture = new Texture2D(cursor.width, cursor.height, TextureFormat.RGBA32, false) {
                        filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp
                    };
                    cursorTexture.SetPixels32(pixels);
                    cursorTexture.Apply();
                }
            }
            if (cursorActive || warpPending || cursorFailed || cursorTexture == null) return;
            var selected = Array.Find(spells, row => row != null && row.acquired && row.selected && row.menu_slot < 26 &&
                !string.IsNullOrEmpty(row.holder_actor) && HasArtwork(row));
            if (selected == null) return;
            RequestWarp(GridPointer(selected.menu_slot), view, scale);
        }

        private void RequestWarp(Vector2 source, Rect view, float scale)
        {
            var mouse = Mouse.current;
            if (!Application.isFocused || mouse == null)
            {
                if (cursorActive) Cursor.visible = previousCursorVisible;
                cursorActive = false;
                cursorFailed = true;
                Debug.LogWarning("Original spell-menu cursor transport unavailable: no focused mouse device.");
                return;
            }
            warpTarget = WindowPointer(source, view, scale, Screen.height);
            mouse.WarpCursorPosition(warpTarget);
            warpPending = true;
            warpDeadline = Time.frameCount + 8;
        }

        private void CompleteWarp()
        {
            if (cursorActive && !Application.isFocused)
            {
                Cursor.visible = previousCursorVisible;
                cursorActive = warpPending = false;
                cursorFailed = true;
                Debug.LogWarning("Original spell-menu cursor transport lost focus.");
                return;
            }
            if (!warpPending) return;
            var mouse = Mouse.current;
            if (Application.isFocused && mouse != null &&
                Vector2.Distance(mouse.position.ReadValue(), warpTarget) <= 2f)
            {
                warpPending = false;
                if (!cursorActive)
                {
                    previousCursorVisible = Cursor.visible;
                    Cursor.visible = false;
                    cursorActive = true;
                }
            }
            else if (!Application.isFocused || mouse == null || Time.frameCount > warpDeadline)
            {
                if (cursorActive) Cursor.visible = previousCursorVisible;
                cursorActive = warpPending = false;
                cursorFailed = true;
                Debug.LogWarning("Original spell-menu cursor warp was not observed on the mouse device.");
            }
        }

        // A drag can span simulation turns. Keep the choice tied to the live
        // acquired holder and slot, not the observation from the initial click.
        internal static EngineSessionTransport.SpellObservation CurrentChoice(
            EngineSessionTransport.SpellObservation selected,
            EngineSessionTransport.SpellObservation[] spells) =>
            selected == null || string.IsNullOrEmpty(selected.holder_actor) || spells == null ? null : Array.Find(spells,
                row => row != null && row.acquired && HasArtwork(row) && row.id == selected.id &&
                    row.menu_slot == selected.menu_slot && row.holder_actor == selected.holder_actor);

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
            EngineSessionTransport.CampaignRenderValue snapshot,
            Func<int, Texture2D> bitmap,
            Func<int, Vector2Int> size,
            bool markHeld,
            out EngineSessionTransport.SpellMark mark,
            OwnerAssetCache environment = null)
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
                else releasePending = false;
            }
            const int columns = 13, edge = 8, panelWidth = 48, panelHeight = 36, top = 408;
            var scale = Mathf.Min(Screen.width / 640f, Screen.height / 480f);
            var view = new Rect((Screen.width - 640 * scale) / 2, (Screen.height - 480 * scale) / 2, 640 * scale, 480 * scale);
            PrepareCursor(environment, snapshot.player_spells, view, scale);
            CompleteWarp();
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
                    row.menu_slot == slot && row.acquired && !string.IsNullOrEmpty(row.holder_actor) && HasArtwork(row));
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
                if (!warpPending && (Event.current.type == EventType.MouseMove || Event.current.type == EventType.MouseDrag))
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
                    releasePending = true;
                    Event.current.Use();
                }
                if (!warpPending && releasePending)
                {
                    var result = new EngineSessionTransport.SpellAssignment {
                        spell_id = choice.id, tier = (byte)tier, hand = button == 0 ? "left" : "right",
                        holder_actor = choice.holder_actor
                    };
                    choice = null; releasePending = false; return result;
                }
            }
            else if (!warpPending && hovered >= 0 && Event.current.type == EventType.MouseDown && Event.current.button <= 1)
            {
                var spell = Array.Find(snapshot.player_spells, row => row != null &&
                    row.menu_slot == hovered && row.acquired && !string.IsNullOrEmpty(row.holder_actor) && HasArtwork(row));
                if (markHeld)
                {
                    var hand = Event.current.button == 0 ? "left" : "right";
                    mark = new EngineSessionTransport.SpellMark {
                        spell_id = spell.id,
                        hand = spell.marked_hand == hand ? "none" : hand,
                        holder_actor = spell.holder_actor
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
                    if (cursorActive)
                    {
                        var target = TierPointer(start, extent.x, extent.y, tier);
                        RequestWarp(target, view, scale);
                        physicalX = target.x;
                        virtualX = target.x;
                    }
                }
                Event.current.Use();
            }
            if (cursorActive && Event.current.type == EventType.Repaint)
                GUI.DrawTexture(new Rect(view.x + pointer.x * scale, view.y + pointer.y * scale,
                    cursorTexture.width * scale, cursorTexture.height * scale), cursorTexture, ScaleMode.StretchToFill, true);
            return null;
        }
    }
}
