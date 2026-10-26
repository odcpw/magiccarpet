using System;
using UnityEngine;

namespace MagicCarpet.Native
{
    // Re-present the completed indexed map after world cameras, before the HUD.
    // No entities, objectives or pulse timers are simulated by this component.
    internal sealed class CampaignMinimapOverlay : MonoBehaviour
    {
        private Texture2D texture;
        private Texture2D web;
        private bool immobilized;
        private readonly Color32[] pixels = new Color32[64 * 64];
        private Color32[] labelPixels;

        internal void PresentWeb(Texture2D retainedWeb, bool visible)
        {
            web = retainedWeb;
            immobilized = visible;
        }

        internal void Present(byte[] indices, byte[] palette, byte[] tables, bool[] labelCoverage = null, bool fullMap = false)
        {
            if (indices.Length != 320 * 200 || palette.Length != 768 || tables.Length != 83456 ||
                labelCoverage != null && labelCoverage.Length != indices.Length)
                throw new InvalidOperationException("Campaign map overlay has invalid input extents");
            var width = !fullMap && labelCoverage == null ? 64 : 320;
            var height = !fullMap && labelCoverage == null ? 64 : 200;
            if (texture == null || texture.width != width || texture.height != height)
            {
                if (texture != null)
                {
                    if (Application.isPlaying) Destroy(texture);
                    else DestroyImmediate(texture);
                }
                texture = new Texture2D(width, height, TextureFormat.RGBA32, false, false) {
                    filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp
                };
            }
            var target = pixels;
            if (fullMap || labelCoverage != null)
            {
                labelPixels ??= new Color32[320 * 200];
                target = labelPixels;
            }
            for (var y = 0; y < height; y++)
            {
                var halfSpan = 0;
                if (y < 64)
                {
                    var sourceY = y * 2;
                    var phase = sourceY < 64 ? 63 - sourceY : sourceY - 64;
                    halfSpan = (64 * tables[0x14300 + phase * 4]) >> 8;
                }
                for (var x = 0; x < width; x++)
                {
                    var color = indices[y * 320 + x] * 3;
                    var visible = fullMap || x < 64 && y < 64 && x * 2 >= 64 - halfSpan && x * 2 < 64 + halfSpan;
                    visible |= labelCoverage != null && labelCoverage[y * 320 + x];
                    target[(height - 1 - y) * width + x] = new Color32(palette[color], palette[color + 1],
                        palette[color + 2], visible ? (byte)255 : (byte)0);
                }
            }
            texture.SetPixels32(target);
            texture.Apply(false, false);
        }

        private void OnGUI()
        {
            if (texture == null || Event.current.type != EventType.Repaint) return;
            var view = Level1ExitPresentationMath.SourceDisplayRect(Screen.width, Screen.height);
            var rectangle = new Rect(view.x * Screen.width, (1f - view.yMax) * Screen.height,
                view.width * Screen.width * (texture.width / 320f), view.height * Screen.height * (texture.height / 200f));
            var previousDepth = GUI.depth;
            var previousColor = GUI.color;
            if (immobilized && web != null)
            {
                GUI.depth = 2; // Source WEB is after world sprites, before map/HUD.
                GUI.color = Color.white;
                GUI.DrawTexture(new Rect(view.x * Screen.width, (1f - view.yMax) * Screen.height,
                    view.width * Screen.width, view.height * Screen.height), web, ScaleMode.StretchToFill, true);
            }
            GUI.depth = 1; // Existing HUD depth 0 remains above the map.
            GUI.color = Color.white;
            GUI.DrawTexture(rectangle, texture, ScaleMode.StretchToFill, true);
            GUI.color = previousColor;
            GUI.depth = previousDepth;
        }

        private void OnDestroy()
        {
            if (texture == null) return;
            if (Application.isPlaying) Destroy(texture);
            else DestroyImmediate(texture);
        }
    }
}
