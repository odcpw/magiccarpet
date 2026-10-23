using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using UnityEngine;

namespace MagicCarpet.Native
{
    // Private owner presentation assets. Mission availability and selection
    // always come from Rust; this cache supplies images, text and hit regions.
    internal sealed class OwnerCampaignMenuCache : IDisposable
    {
        [Serializable] internal sealed class Mission
        {
            public string level_id, name, briefing;
            public int viewport_x, viewport_y, portal_x, portal_y, portal_width, portal_height;
        }
        [Serializable] private sealed class Output { public string path, sha256; public int bytes; }
        [Serializable] private sealed class Portal { public string kind, path; public int width, height; }
        [Serializable] private sealed class Manifest
        {
            public string schema, source_commit;
            public int width, height, view_width, view_height;
            public Mission[] missions;
            public Portal[] portals;
            public Output[] outputs;
        }

        private readonly Dictionary<string, Mission> missions;
        private readonly Dictionary<string, Texture2D> portals = new();
        internal Texture2D World { get; private set; }

        internal OwnerCampaignMenuCache(string path)
        {
            var root = Path.GetFullPath(path);
            var manifestPath = Path.Combine(root, "manifest.json");
            if ((File.GetAttributes(root) & FileAttributes.ReparsePoint) != 0 ||
                (File.GetAttributes(manifestPath) & FileAttributes.ReparsePoint) != 0)
                throw new InvalidDataException("campaign menu cache must use regular owner files");
            var manifest = JsonUtility.FromJson<Manifest>(File.ReadAllText(manifestPath));
            if (manifest?.schema != "mc2-owner-campaign-menu-cache/v1" ||
                manifest.source_commit != "6806c7204d17220ecc0430759f784aea8ee2ddb2" ||
                manifest.width != 1280 || manifest.height != 960 || manifest.view_width != 640 || manifest.view_height != 480 ||
                manifest.missions?.Length != 3 || manifest.portals?.Length != 2 || manifest.outputs?.Length != 4)
                throw new InvalidDataException("campaign menu cache has an unsupported source binding");
            missions = manifest.missions.ToDictionary(mission => mission.level_id, StringComparer.Ordinal);
            foreach (var mission in missions.Values)
                if (string.IsNullOrWhiteSpace(mission.name) || string.IsNullOrWhiteSpace(mission.briefing) ||
                    !mission.level_id.StartsWith("level.", StringComparison.Ordinal) ||
                    mission.viewport_x < 0 || mission.viewport_x > 640 || mission.viewport_y < 0 || mission.viewport_y > 480 ||
                    mission.portal_x < 0 || mission.portal_y < 0 || mission.portal_width <= 0 || mission.portal_height <= 0)
                    throw new InvalidDataException("campaign mission presentation is incomplete");
            byte[] Read(string relative, int expected)
            {
                var row = manifest.outputs.Single(output => output.path == relative);
                if (relative != Path.GetFileName(relative)) throw new InvalidDataException("campaign cache path is not confined");
                var file = Path.Combine(root, relative);
                if ((File.GetAttributes(file) & FileAttributes.ReparsePoint) != 0) throw new InvalidDataException("campaign cache output is a symlink");
                var bytes = File.ReadAllBytes(file);
                using var sha = SHA256.Create();
                var digest = BitConverter.ToString(sha.ComputeHash(bytes)).Replace("-", "").ToLowerInvariant();
                if (bytes.Length != expected || bytes.Length != row.bytes || digest != row.sha256)
                    throw new InvalidDataException("campaign cache output receipt mismatch: " + relative);
                return bytes;
            }
            try
            {
                var indices = Read("world-map.indices.u8", 1280 * 960);
                var palette = Read("palette.rgb6", 768);
                if (palette.Any(channel => channel > 63)) throw new InvalidDataException("campaign palette is not RGB6");
                var rgba = new byte[indices.Length * 4];
                for (var i = 0; i < indices.Length; i++)
                {
                    var source = indices[i] * 3;
                    rgba[i * 4] = (byte)(palette[source] * 4);
                    rgba[i * 4 + 1] = (byte)(palette[source + 1] * 4);
                    rgba[i * 4 + 2] = (byte)(palette[source + 2] * 4);
                    rgba[i * 4 + 3] = 255;
                }
                World = Texture(rgba, 1280, 960, "MC2 owner campaign world map");
                foreach (var portal in manifest.portals)
                {
                    if ((portal.kind != "completed" && portal.kind != "available") ||
                        portal.width <= 0 || portal.width > 256 || portal.height <= 0 || portal.height > 256)
                        throw new InvalidDataException("campaign portal presentation is invalid");
                    portals.Add(portal.kind, Texture(Read(portal.path, checked(portal.width * portal.height * 4)),
                        portal.width, portal.height, "MC2 owner " + portal.kind + " portal"));
                }
            }
            catch { Dispose(); throw; }
        }

        internal Mission ForMission(string level) => missions.TryGetValue(level, out var mission) ? mission :
            throw new InvalidDataException("campaign menu has no owner presentation for " + level);

        internal void DrawMap(GameRuntimeTransport.CampaignMenuSnapshot menu)
        {
            var focus = ForMission(menu.phase == "results" ? menu.completed_level_id :
                menu.selected_level_id ?? menu.briefing_level_id ?? menu.completed_level_id);
            var scale = Mathf.Min(Screen.width / 640f, Screen.height / 480f);
            var view = new Rect((Screen.width - 640 * scale) / 2, (Screen.height - 480 * scale) / 2, 640 * scale, 480 * scale);
            GUI.DrawTextureWithTexCoords(view, World,
                new Rect(focus.viewport_x / 1280f, (960 - focus.viewport_y - 480) / 960f, .5f, .5f));
            foreach (var level in menu.available_levels)
            {
                var mission = ForMission(level);
                var texture = portals[menu.completed_levels.Contains(level) ? "completed" : "available"];
                var x = mission.portal_x - focus.viewport_x;
                var y = mission.portal_y - focus.viewport_y;
                if (x < 0 || y < 0 || x + texture.width > 640 || y + texture.height > 480) continue;
                GUI.DrawTexture(new Rect(view.x + x * scale, view.y + y * scale, texture.width * scale, texture.height * scale), texture);
            }
        }

        private static Texture2D Texture(byte[] topDown, int width, int height, string name)
        {
            var bottomUp = new byte[topDown.Length];
            for (var y = 0; y < height; y++) Buffer.BlockCopy(topDown, y * width * 4, bottomUp, (height - 1 - y) * width * 4, width * 4);
            var texture = new Texture2D(width, height, TextureFormat.RGBA32, false) { name = name, filterMode = FilterMode.Point };
            texture.LoadRawTextureData(bottomUp);
            texture.Apply(false, true);
            return texture;
        }

        public void Dispose()
        {
            foreach (var texture in portals.Values) Destroy(texture);
            portals.Clear();
            if (World != null) Destroy(World);
            World = null;
        }

        private static void Destroy(Texture2D texture)
        {
            if (Application.isPlaying) UnityEngine.Object.Destroy(texture);
            else UnityEngine.Object.DestroyImmediate(texture);
        }
    }
}
