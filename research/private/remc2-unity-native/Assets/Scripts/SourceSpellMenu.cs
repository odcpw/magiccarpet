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
        internal int HoveredSlot { get; private set; } = -1;
        internal int HoveredTier { get; private set; }
        internal Vector2Int PanelExtent { get; private set; }
        internal Vector2Int TierExtent { get; private set; }
        // mode 0 = opaque sprite, 1 = source TABLES blend, 2 = solid rectangle,
        // 3 = sprite mask colourized to a fixed source palette index.
        private readonly List<(int selector, int x, int y, int width, int height, int mode, byte color)> draws = new();
        private readonly List<(int selector, int x, int y, int width, int height, int mode, byte color)> cachedDraws = new();
        private object priorAssets;
        private RectInt priorRect;
        private Color32[] priorPixels;
        private Rect cursorRect;

        internal static bool UsesBitmap(int selector) =>
            selector >= 87 && selector <= 122 || selector == 149 || selector == 150 ||
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
            HoveredSlot = -1;
            HoveredTier = 0;
            PanelExtent = TierExtent = default;
            draws.Clear();
            cachedDraws.Clear();
            priorAssets = null;
            priorPixels = null;
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
            foreach (var selector in new[] { 87, 88, 89, 90, 91, 149, 150, 161, 162, 163, 164, 165, 166, 167 })
                if (!available(selector)) return false;
            foreach (var spell in spells)
            {
                if (spell == null || spell.menu_help == null) return false;
                var help = spell.menu_help;
                if (!help.definition_enabled || (!help.is_standard && !help.discovered)) continue;
                if (!HasArtwork(spell) || !available(spell.menu_artwork.grid_icon)) return false;
                if (help.is_standard)
                    foreach (var icon in spell.menu_artwork.tier_icons)
                        if (!available(icon)) return false;
            }
            return true;
        }

        // Preserve the exact indexed layers that this menu drew before the
        // source popup. The source shader samples them over the current world,
        // so transparent sprite pixels still reveal the live underlying scene.
        internal bool TryIndexedPrior(object assets, Func<int, ObjectiveTextboxCompositor.Sprite> sprite,
            byte[] tables, int tableOffset, out RectInt rect, out Color32[] pixels)
        {
            rect = default; pixels = null;
            if (assets == null || sprite == null || draws.Count == 0) return false;
            if (tables == null || tableOffset < 0 || tableOffset > tables.Length - 65536)
                throw new InvalidOperationException("unbound indexed spell blend table");
            var unchanged = ReferenceEquals(assets, priorAssets) && priorPixels != null && cachedDraws.Count == draws.Count;
            for (var i = 0; unchanged && i < draws.Count; i++) unchanged = cachedDraws[i] == draws[i];
            if (!unchanged)
            {
                var minY = 480;
                var maxY = 0;
                foreach (var draw in draws)
                {
                    minY = Math.Min(minY, Math.Max(0, draw.y));
                    maxY = Math.Max(maxY, Math.Min(480, draw.y + draw.height));
                }
                if (minY >= maxY) return false;
                var nextRect = new RectInt(0, minY, 640, maxY - minY);
                var next = new Color32[checked(nextRect.width * nextRect.height)];
                foreach (var draw in draws)
                {
                    var source = draw.mode == 2 ? null : sprite(draw.selector);
                    if (draw.mode != 2 && (source == null || source.Width <= 0 || source.Height <= 0 ||
                        source.Indices == null || source.Mask == null ||
                        source.Indices.Length != source.Width * source.Height || source.Mask.Length != source.Indices.Length))
                        throw new InvalidOperationException("invalid indexed spell sprite");
                    for (var y = 0; y < draw.height; y++)
                    for (var x = 0; x < draw.width; x++)
                    {
                        var dx = draw.x + x; var dy = draw.y + y;
                        if (dx < 0 || dx >= 640 || dy < minY || dy >= maxY) continue;
                        var sourceIndex = draw.mode == 2 ? 0 :
                            (y * source.Height / draw.height) * source.Width + x * source.Width / draw.width;
                        if (draw.mode != 2 && source.Mask[sourceIndex] == 0) continue;
                        var value = draw.mode == 2 || draw.mode == 3 ? draw.color : source.Indices[sourceIndex];
                        var target = (dy - minY) * 640 + dx;
                        var previous = next[target];
                        if (draw.mode != 1)
                            next[target] = new Color32(value, 0, 0, 5); // one opaque operation
                        else if ((previous.a & 4) != 0)
                            next[target] = new Color32(tables[tableOffset + (previous.r << 8) + value], 0, 0, 5);
                        else
                        {
                            var count = previous.a & 3;
                            if (count == 3) throw new InvalidOperationException("spell underlay exceeds source blend depth");
                            if (count == 0) previous.r = value;
                            else if (count == 1) previous.g = value;
                            else previous.b = value;
                            previous.a = (byte)(count + 1);
                            next[target] = previous;
                        }
                    }
                }
                priorAssets = assets; priorRect = nextRect; priorPixels = next;
                cachedDraws.Clear(); cachedDraws.AddRange(draws);
            }
            rect = priorRect; pixels = priorPixels; return true;
        }

        internal EngineSessionTransport.SpellAssignment Draw(
            EngineSessionTransport.CampaignRenderValue snapshot,
            Func<int, Texture2D> bitmap,
            Func<int, Vector2Int> size,
            bool markHeld,
            OwnerCampaignMenuCache.ObjectiveAssets assets,
            out EngineSessionTransport.SpellMark mark,
            OwnerAssetCache environment = null)
        {
            draws.Clear();
            mark = null;
            if (assets == null || snapshot?.player_spells == null || snapshot.player_vitals == null ||
                snapshot.spell_menu_wizard_colors == null || snapshot.spell_menu_wizard_colors.Length != 2)
            { Close(); return null; }
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
            PanelExtent = size(89);
            TierExtent = size(163);
            var pointer = (Event.current.mousePosition - view.position) / scale;
            if (cursorTexture != null)
                cursorRect = new Rect(view.x + pointer.x * scale, view.y + pointer.y * scale,
                    cursorTexture.width * scale, cursorTexture.height * scale);
            void Sprite(int selector, int x, int y, bool blended = false, int tint = -1)
            {
                var extent = size(selector);
                draws.Add((selector, x, y, extent.x, extent.y, tint >= 0 ? 3 : blended ? 1 : 0,
                    tint >= 0 ? checked((byte)tint) : (byte)0));
            }
            void Fill(int x, int y, int width, int height, byte color)
            {
                if (width > 0 && height > 0) draws.Add((-1, x, y, width, height, 2, color));
            }
            var transparentPanel = !snapshot.spell_menu_panels_opaque;
            Sprite(88, 0, top, transparentPanel); Sprite(88, 640 - edge, top, transparentPanel);
            // SelectSpellCategory_6D420 clamps the pointer to the 26 source
            // cells even when that cell is not currently acquired.
            var hovered = choice != null ? choice.menu_slot :
                Mathf.Clamp(((int)pointer.y - top) / panelHeight, 0, 1) * columns +
                Mathf.Clamp(((int)pointer.x - edge) / panelWidth, 0, columns - 1);
            var hoveredSpell = Array.Find(snapshot.player_spells, row => row != null && row.menu_slot == hovered);
            HoveredSlot = hovered;
            HoveredTier = choice != null ? tier : Mathf.Clamp(hoveredSpell?.menu_help?.selected_tier ?? 0, 0, 2);
            if (choice != null && !warpPending &&
                (Event.current.type == EventType.MouseMove || Event.current.type == EventType.MouseDrag))
            {
                var start = MagicCarpetRuntime.SpellTierMenuStartX(choice.menu_slot, columns, edge, panelWidth, 3, TierExtent.x);
                virtualX += pointer.x - physicalX;
                physicalX = pointer.x;
                tier = Mathf.Clamp(Mathf.FloorToInt((virtualX - start) / TierExtent.x), 0, choice.unlocked_tier);
                HoveredTier = tier;
            }
            for (var slot = 0; slot < columns * 2; slot++)
            {
                var x = edge + slot % columns * panelWidth;
                var y = top + slot / columns * panelHeight;
                var spell = Array.Find(snapshot.player_spells, row => row != null && row.menu_slot == slot);
                if (spell?.menu_help == null) { Close(); return null; }
                var help = spell.menu_help;
                if (help.title_records?.Length != 3 || help.castle_blocked?.Length != 3 ||
                    help.tier_mana_costs?.Length != 3 || help.experience_thresholds?.Length != 3 ||
                    help.selected_tier < 0 || help.selected_tier > 2)
                { Close(); return null; }
                var currentTier = slot == hovered ? HoveredTier : help.selected_tier;
                if (!help.definition_enabled)
                {
                    Sprite(89, x, y, transparentPanel);
                }
                else if (help.is_standard)
                {
                    if (!help.blink_hidden)
                    {
                        var canSummon = !help.castle_blocked[currentTier];
                        if (canSummon && slot == hovered)
                        {
                            var cost = help.tier_mana_costs[currentTier];
                            if (cost > 0)
                            {
                                Sprite(87, x, y);
                                Fill(x + 6, y + 28, 36 * (snapshot.player_vitals.mana % cost) / cost, 4,
                                    snapshot.spell_menu_wizard_colors[1]);
                                var shots = snapshot.player_vitals.mana / cost;
                                for (var sx = 0; sx < 36 && shots > 0; sx += 2)
                                    for (var sy = 0; sy < 4 && shots > 0; sy += 2, shots--)
                                        Fill(x + 6 + sx, y + 28 + sy, 2, 2, snapshot.spell_menu_wizard_colors[0]);
                            }
                        }
                        else Sprite(canSummon ? 89 : 91, x, y, transparentPanel);
                        Sprite(spell.menu_artwork.grid_icon, x, y, !canSummon);
                    }
                    if (help.blink_hidden || slot == hovered)
                    {
                        if (spell.marked_hand == "left") Sprite(149, x, y, true);
                        if (spell.marked_hand == "right") Sprite(150, x + panelWidth - size(149).x, y, true);
                    }
                }
                else
                {
                    Sprite(89, x, y, transparentPanel);
                    if (help.discovered) Sprite(spell.menu_artwork.grid_icon, x, y, tint: 0xa6);
                }
            }
            Sprite(90, edge + hovered % columns * panelWidth, top + hovered / columns * panelHeight);
            if (hoveredSpell?.menu_help?.is_standard == true)
            {
                var extent = size(163);
                var start = MagicCarpetRuntime.SpellTierMenuStartX(hovered, columns, edge, panelWidth, 3, extent.x);
                var help = hoveredSpell.menu_help;
                for (var value = 0; value < 3; value++)
                {
                    var x = start + value * extent.x;
                    var y = top - extent.y;
                    var unlocked = value <= hoveredSpell.unlocked_tier;
                    var cost = help.tier_mana_costs[value];
                    var canSummon = !help.castle_blocked[value];
                    if (canSummon && cost <= 0)
                        throw new InvalidOperationException("source spell menu has a nonpositive mana cost");
                    var canPay = cost > 0 && snapshot.player_vitals.mana / cost != 0;
                    Sprite(!unlocked ? 163 : canSummon && canPay ? 161 : 162, x, y);
                    if (!unlocked)
                    {
                        if (value == HoveredTier) Sprite(164, x, y);
                        continue;
                    }
                    Sprite(165 + value, x + 6, y + 10);
                    Sprite(hoveredSpell.menu_artwork.tier_icons[value], x + 18, y + 6, !canSummon);
                    if (value == HoveredTier) Sprite(164, x, y);
                    if (value < 2)
                    {
                        var filled = 54;
                        if (value == hoveredSpell.unlocked_tier)
                        {
                            var delta = unchecked(help.experience_thresholds[value + 1] - help.experience_thresholds[value]);
                            var progress = unchecked(help.experience - help.experience_thresholds[value]);
                            filled = delta > 0
                                ? Mathf.Clamp(unchecked((short)(unchecked(54 * progress) / delta)), 0, 54)
                                : 0;
                        }
                        Fill(x + 6, y + 28, 54, 2, assets.BlendSelector);
                        Fill(x + 6, y + 28, filled, 2, assets.TextColor);
                    }
                }
            }
            if (choice != null)
            {
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
                if (spell == null) return null;
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
                    tier = Mathf.Clamp(choice.menu_help.selected_tier, 0, choice.unlocked_tier);
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
            return null;
        }

        internal void DrawCursor()
        {
            if (cursorActive && cursorTexture != null && Event.current?.type == EventType.Repaint)
                GUI.DrawTexture(cursorRect, cursorTexture, ScaleMode.StretchToFill, true);
        }
    }
}
