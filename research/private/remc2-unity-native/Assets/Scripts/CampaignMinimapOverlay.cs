using System;
using UnityEngine;

namespace MagicCarpet.Native
{
    // Re-present the completed indexed map after world cameras, before the HUD.
    // No entities, objectives or pulse timers are simulated by this component.
    internal sealed class CampaignMinimapOverlay : MonoBehaviour
    {
        private Texture2D texture;
        private readonly Color32[] pixels = new Color32[64 * 64];

        internal void Present(byte[] indices, byte[] palette, byte[] tables)
        {
            if (indices.Length != 320 * 200 || palette.Length != 768 || tables.Length != 83456)
                throw new InvalidOperationException("Campaign map overlay has invalid input extents");
            if (texture == null)
                texture = new Texture2D(64, 64, TextureFormat.RGBA32, false, false) {
                    filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp
                };
            for (var y = 0; y < 64; y++)
            {
                var sourceY = y * 2;
                var phase = sourceY < 64 ? 63 - sourceY : sourceY - 64;
                var halfSpan = (64 * tables[0x14300 + phase * 4]) >> 8;
                for (var x = 0; x < 64; x++)
                {
                    var color = indices[y * 320 + x] * 3;
                    var visible = x * 2 >= 64 - halfSpan && x * 2 < 64 + halfSpan;
                    pixels[(63 - y) * 64 + x] = new Color32(palette[color], palette[color + 1],
                        palette[color + 2], visible ? (byte)255 : (byte)0);
                }
            }
            texture.SetPixels32(pixels);
            texture.Apply(false, false);
        }

        private void OnGUI()
        {
            if (texture == null || Event.current.type != EventType.Repaint) return;
            var view = Level1ExitPresentationMath.SourceDisplayRect(Screen.width, Screen.height);
            var rectangle = new Rect(view.x * Screen.width, (1f - view.yMax) * Screen.height,
                view.width * Screen.width * (64f / 320f), view.height * Screen.height * (64f / 200f));
            var previousDepth = GUI.depth;
            var previousColor = GUI.color;
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
