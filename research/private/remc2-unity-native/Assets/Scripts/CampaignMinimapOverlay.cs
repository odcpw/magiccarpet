using System;
using System.Collections.Generic;
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
        private bool iconsAndMapVisible = true;
        private bool fullMap;
        internal bool MapVisible => iconsAndMapVisible || fullMap;
        private readonly Color32[] pixels = new Color32[64 * 64];
        private Color32[] labelPixels;
        private bool panelsOpaque, cave, indexedFrame, lookupDirty;
        private readonly byte[] terrain = new byte[4096], terrainAdmission = new byte[4096];
        private readonly byte[] lookup = new byte[256 * 4096];
        private readonly byte[] terrainFunctions = new byte[65536], constants = new byte[65536];
        private readonly Color32[] outside = new Color32[320 * 200];
        private readonly List<(int offset, byte value, bool blend)> operations = new();
        private byte[] sourceTables;
        private Texture2D lookupTexture, outsideTexture, paletteTexture, webIndexed;
        private Material indexedMaterial;
        private RenderTexture indexedScene, visibleScene;
        private Texture priorScene;
        private ulong priorGeneration, priorRevision, programRevision, composedProgramRevision;
        private bool priorVisible, priorTopDown;
        internal Func<RenderTexture> SceneProvider;

        // Each map pixel is a unary function of its underlying world index.
        // A 256-entry lookup retains arbitrary source blend/overwrite order,
        // without a blend-depth cap or a GPU-to-CPU world readback.
        internal void BeginIndexedFrame(byte[] frame, byte[] tables, byte[] admission, bool fullMap, bool cave)
        {
            if (frame == null || frame.Length != 64000 || tables == null || tables.Length != 83456 ||
                !fullMap && (admission == null || admission.Length != 4096))
                throw new InvalidOperationException("source minimap indexed inputs are incomplete");
            this.fullMap = fullMap; this.cave = cave; indexedFrame = true;
            if (!ReferenceEquals(sourceTables, tables))
            {
                sourceTables = tables;
                for (var color = 0; color < 256; color++)
                for (var world = 0; world < 256; world++)
                {
                    terrainFunctions[color * 256 + world] = tables[0x4000 + (world << 8) + color];
                    constants[color * 256 + world] = (byte)color;
                }
            }
            for (var y = 0; y < 64; y++) Buffer.BlockCopy(frame, y * 320, terrain, y * 64, 64);
            if (!fullMap) Buffer.BlockCopy(admission, 0, terrainAdmission, 0, 4096);
            operations.Clear(); Array.Clear(outside, 0, outside.Length);
            InvalidateLookup();
        }

        internal void RecordPixel(int offset, byte value, bool blend)
        {
            if (!indexedFrame || fullMap) return;
            if (offset < 0 || offset >= 64000) throw new InvalidOperationException("source map write is outside its frame");
            if (offset % 320 < 64 && offset / 320 < 64) operations.Add((offset, value, blend));
            else
            {
                if (blend) throw new InvalidOperationException("source minimap blend escaped its circular aperture");
                outside[offset] = new Color32(value, 0, 0, 255);
            }
        }

        internal void RecordLabels(byte[] frame, bool[] coverage)
        {
            if (coverage == null) return;
            if (frame.Length != 64000 || coverage.Length != frame.Length)
                throw new InvalidOperationException("source map label coverage has invalid extent");
            for (var pixel = 0; pixel < coverage.Length; pixel++)
                if (coverage[pixel]) RecordPixel(pixel, frame[pixel], false);
        }

        internal void SetPanelsOpaque(bool value)
        {
            if (panelsOpaque == value) return;
            panelsOpaque = value; InvalidateLookup();
        }

        private void InvalidateLookup() { lookupDirty = true; programRevision++; }

        internal void SetIndexedWeb(byte[] indices, byte[] mask, int width, int height)
        {
            if (indices == null || mask == null || indices.Length != width * height || mask.Length != indices.Length)
                throw new InvalidOperationException("source WEB indices are incomplete");
            if (webIndexed != null) DestroyResource(webIndexed);
            webIndexed = new Texture2D(width, height, TextureFormat.RGBA32, false, true) {
                filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp
            };
            var data = new Color32[indices.Length];
            for (var i = 0; i < data.Length; i++) data[i] = new Color32(indices[i], 0, 0, mask[i]);
            webIndexed.SetPixels32(data); webIndexed.Apply(false, true);
            programRevision++;
        }

        internal RenderTexture Compose(Texture scene, bool sceneTopDown, ulong generation, ulong revision)
        {
            if (scene == null) return null;
            EnsureIndexedResources();
            if (lookupDirty && indexedFrame)
            {
                for (var pixel = 0; pixel < 4096; pixel++)
                {
                    var y = pixel / 64; var x = pixel % 64;
                    var phase = y * 2 < 64 ? 63 - y * 2 : y * 2 - 64;
                    var halfSpan = (64 * sourceTables[0x14300 + phase * 4]) >> 8;
                    var aperture = x * 2 >= 64 - halfSpan && x * 2 < 64 + halfSpan;
                    var row = pixel * 256;
                    if (aperture && (panelsOpaque || cave))
                        Buffer.BlockCopy(constants, terrain[pixel] * 256, lookup, row, 256);
                    else if (aperture && terrainAdmission[pixel] != 0)
                        Buffer.BlockCopy(terrainFunctions, terrain[pixel] * 256, lookup, row, 256);
                    else for (var world = 0; world < 256; world++) lookup[row + world] = (byte)world;
                }
                foreach (var op in operations)
                {
                    var row = (op.offset / 320 * 64 + op.offset % 320) * 256;
                    if (!op.blend) Buffer.BlockCopy(constants, op.value * 256, lookup, row, 256);
                    else for (var world = 0; world < 256; world++)
                        lookup[row + world] = sourceTables[0x4000 + (lookup[row + world] << 8) + op.value];
                }
                lookupTexture.LoadRawTextureData(lookup); lookupTexture.Apply(false, false);
                outsideTexture.SetPixels32(outside); outsideTexture.Apply(false, false);
                lookupDirty = false;
            }
            if (ReferenceEquals(scene, priorScene) && generation == priorGeneration && revision == priorRevision &&
                composedProgramRevision == programRevision && priorVisible == MapVisible && priorTopDown == sceneTopDown)
                return indexedScene;
            indexedMaterial.SetTexture("_MapFunctions", lookupTexture);
            indexedMaterial.SetTexture("_MapOutside", outsideTexture);
            indexedMaterial.SetTexture("_Web", webIndexed != null ? webIndexed : Texture2D.blackTexture);
            indexedMaterial.SetFloat("_WebVisible", !fullMap && immobilized && webIndexed != null ? 1 : 0);
            indexedMaterial.SetFloat("_SceneTopDown", sceneTopDown ? 1 : 0);
            indexedMaterial.SetFloat("_MapVisible", indexedFrame && MapVisible && !fullMap ? 1 : 0);
            var previous = RenderTexture.active; var srgb = GL.sRGBWrite;
            try { GL.sRGBWrite = false; Graphics.Blit(scene, indexedScene, indexedMaterial, 0); }
            finally { GL.sRGBWrite = srgb; RenderTexture.active = previous; }
            priorScene = scene; priorGeneration = generation; priorRevision = revision;
            composedProgramRevision = programRevision; priorVisible = MapVisible; priorTopDown = sceneTopDown;
            return indexedScene;
        }

        private void EnsureIndexedResources()
        {
            if (indexedMaterial != null) return;
            var shader = Resources.Load<Shader>("CampaignIndexedMinimap");
            if (shader == null) throw new InvalidOperationException("source indexed minimap shader is missing");
            indexedMaterial = new Material(shader);
            lookupTexture = new Texture2D(256, 4096, TextureFormat.R8, false, true);
            outsideTexture = new Texture2D(320, 200, TextureFormat.RGBA32, false, true);
            paletteTexture = new Texture2D(256, 1, TextureFormat.RGB24, false, false);
            foreach (var value in new[] { lookupTexture, outsideTexture, paletteTexture }) {
                value.filterMode = FilterMode.Point; value.wrapMode = TextureWrapMode.Clamp;
            }
            indexedScene = Target("MC2 minimap-inclusive indices");
            visibleScene = Target("MC2 minimap-inclusive display");
        }

        private static RenderTexture Target(string name)
        {
            var value = new RenderTexture(320, 200, 0, RenderTextureFormat.ARGB32, RenderTextureReadWrite.Linear) {
                name = name, filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp
            };
            if (!value.Create())
            {
                DestroyResource(value);
                throw new InvalidOperationException("source minimap target creation failed");
            }
            return value;
        }

        internal void SetPalette(byte[] palette)
        {
            if (palette == null || palette.Length != 768) throw new InvalidOperationException("source minimap palette is invalid");
            EnsureIndexedResources(); paletteTexture.LoadRawTextureData(palette); paletteTexture.Apply(false, false);
        }

        internal void PresentWeb(Texture2D retainedWeb, bool visible)
        {
            web = retainedWeb;
            if (immobilized != visible) programRevision++;
            immobilized = visible;
        }

        internal void Present(byte[] indices, byte[] palette, byte[] tables, bool[] labelCoverage = null, bool fullMap = false)
        {
            if (indices.Length != 320 * 200 || palette.Length != 768 || tables.Length != 83456 ||
                labelCoverage != null && labelCoverage.Length != indices.Length)
                throw new InvalidOperationException("Campaign map overlay has invalid input extents");
            this.fullMap = fullMap;
            if (indexedFrame) SetPalette(palette);
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

        internal void SetIconsAndMapVisible(bool visible) => iconsAndMapVisible = visible;

        private void OnGUI()
        {
            if (texture == null || Event.current.type != EventType.Repaint) return;
            if (indexedFrame && !MapVisible && !immobilized) return;
            if (indexedFrame && SceneProvider?.Invoke() == null) return;
            var view = Level1ExitPresentationMath.SourceDisplayRect(Screen.width, Screen.height);
            var rectangle = new Rect(view.x * Screen.width, (1f - view.yMax) * Screen.height,
                view.width * Screen.width * (texture.width / 320f), view.height * Screen.height * (texture.height / 200f));
            var previousDepth = GUI.depth;
            var previousColor = GUI.color;
            if (indexedFrame && priorScene != null)
            {
                var previous = RenderTexture.active; var srgb = GL.sRGBWrite;
                try
                {
                    indexedMaterial.SetTexture("_Palette", paletteTexture);
                    GL.sRGBWrite = false;
                    Graphics.Blit(indexedScene, visibleScene, indexedMaterial, 1);
                }
                finally { GL.sRGBWrite = srgb; RenderTexture.active = previous; }
                GUI.depth = 1; GUI.color = Color.white;
                GUI.DrawTexture(new Rect(view.x * Screen.width, (1f - view.yMax) * Screen.height,
                    view.width * Screen.width, view.height * Screen.height), visibleScene, ScaleMode.StretchToFill, true);
                GUI.color = previousColor; GUI.depth = previousDepth;
                return;
            }
            if (immobilized && web != null)
            {
                GUI.depth = 2; // Source WEB is after world sprites, before map/HUD.
                GUI.color = Color.white;
                GUI.DrawTexture(new Rect(view.x * Screen.width, (1f - view.yMax) * Screen.height,
                    view.width * Screen.width, view.height * Screen.height), web, ScaleMode.StretchToFill, true);
            }
            if (MapVisible)
            {
                GUI.depth = 1; // Existing HUD depth 0 remains above the map.
                GUI.color = Color.white;
                GUI.DrawTexture(rectangle, texture, ScaleMode.StretchToFill, true);
            }
            GUI.color = previousColor;
            GUI.depth = previousDepth;
        }

        private void OnDestroy() => ReleaseResources();

        internal void ReleaseResources()
        {
            foreach (var value in new UnityEngine.Object[] { texture, lookupTexture, outsideTexture, paletteTexture,
                webIndexed, indexedMaterial, indexedScene, visibleScene }) DestroyResource(value);
            texture = lookupTexture = outsideTexture = paletteTexture = webIndexed = null;
            indexedMaterial = null; indexedScene = visibleScene = null;
            priorScene = null; SceneProvider = null; indexedFrame = false;
        }

        private static void DestroyResource(UnityEngine.Object value)
        {
            if (value == null) return;
            if (value is RenderTexture target) target.Release();
            if (Application.isPlaying) Destroy(value); else DestroyImmediate(value);
        }
    }
}
