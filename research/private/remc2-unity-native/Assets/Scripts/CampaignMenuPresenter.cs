using System;
using UnityEngine;

namespace MagicCarpet.Native
{
    // UI only: every action is dispatched to the existing Rust runtime owner.
    // The original indexed loading artwork is shared; buttons are Unity UI,
    // not a claim that the original menu layout/animation has been ported.
    internal sealed class CampaignMenuPresenter : IDisposable
    {
        internal enum Action { None, NewGame, Resume, Save, Load, Restart, Continue, Title, Quit }
        private readonly Texture2D background;
        private GUIStyle heading;
        private GUIStyle button;
        private GUIStyle message;

        internal CampaignMenuPresenter(OwnerAssetCache cache)
        {
            var indices = cache.ReadOutput(CampaignTransitionPresenter.IndicesPath);
            var palette = cache.ReadOutput(CampaignTransitionPresenter.PalettePath);
            if (indices.Length != 320 * 200 || palette.Length != 256 * 3)
                throw new InvalidOperationException("Menu background requires the qualified indexed frame");
            var pixels = new Color32[indices.Length];
            for (var y = 0; y < 200; y++)
                for (var x = 0; x < 320; x++)
                {
                    var offset = indices[y * 320 + x] * 3;
                    pixels[(199 - y) * 320 + x] = new Color32(
                        checked((byte)(palette[offset] * 4)),
                        checked((byte)(palette[offset + 1] * 4)),
                        checked((byte)(palette[offset + 2] * 4)), 255);
                }
            background = new Texture2D(320, 200, TextureFormat.RGBA32, false)
            { name = "Original campaign menu backdrop", filterMode = FilterMode.Point };
            background.SetPixels32(pixels);
            background.Apply(false, true);
        }

        internal static bool Allows(string mode, Action action, bool canSave, bool canLoad, bool objectivesSatisfied)
        {
            if (mode == GameRuntimeTransport.LoadingMode) return false;
            return action switch
            {
                Action.NewGame => mode == GameRuntimeTransport.TitleMode,
                Action.Resume => mode == GameRuntimeTransport.PausedMode,
                Action.Save => mode == GameRuntimeTransport.PausedMode && canSave,
                Action.Load => mode == GameRuntimeTransport.TitleMode && canLoad,
                Action.Restart or Action.Title => mode == GameRuntimeTransport.PausedMode,
                Action.Continue => mode == GameRuntimeTransport.PausedMode && objectivesSatisfied,
                Action.Quit => mode == GameRuntimeTransport.TitleMode || mode == GameRuntimeTransport.PausedMode,
                _ => false
            };
        }

        internal Action Draw(string mode, bool canSave, bool canLoad, bool objectivesSatisfied, string status)
        {
            if (mode != GameRuntimeTransport.TitleMode && mode != GameRuntimeTransport.PausedMode)
                return Action.None;
            heading ??= new GUIStyle(GUI.skin.label) { alignment = TextAnchor.MiddleCenter, fontSize = 28 };
            button ??= new GUIStyle(GUI.skin.button) { fontSize = 19 };
            message ??= new GUIStyle(GUI.skin.label) { alignment = TextAnchor.MiddleCenter, wordWrap = true };
            var oldColor = GUI.color;
            var oldEnabled = GUI.enabled;
            try
            {
                GUI.color = Color.white;
                GUI.DrawTexture(new Rect(0, 0, Screen.width, Screen.height), background, ScaleMode.ScaleToFit);
                var width = Mathf.Min(360, Screen.width - 32);
                var x = (Screen.width - width) * .5f;
                var y = Mathf.Max(16, (Screen.height - 480) * .5f);
                GUI.color = new Color(0, 0, 0, .85f);
                GUI.DrawTexture(new Rect(x - 16, y, width + 32, 480), Texture2D.whiteTexture);
                GUI.color = Color.white;
                GUI.Label(new Rect(x, y + 12, width, 44), mode == GameRuntimeTransport.TitleMode ? "MAGIC CARPET II" : "PAUSED", heading);
                y += 66;
                var choices = mode == GameRuntimeTransport.TitleMode
                    ? new[] { (Action.NewGame, "New game"), (Action.Load, "Load saved game"), (Action.Quit, "Quit") }
                    : new[] { (Action.Resume, "Resume"), (Action.Save, "Save game"), (Action.Continue, "Continue to next level"), (Action.Restart, "Restart level"), (Action.Title, "Main menu"), (Action.Quit, "Quit") };
                foreach (var (action, label) in choices)
                {
                    GUI.enabled = Allows(mode, action, canSave, canLoad, objectivesSatisfied);
                    if (GUI.Button(new Rect(x, y, width, 42), label, button)) return action;
                    y += 50;
                }
                GUI.enabled = true;
                var help = mode == GameRuntimeTransport.PausedMode
                    ? "W/S fly · A/D strafe · Mouse steer\n" +
                      "1 Fireball · 2 Possession · 3 Castle · 4 Speed\n" +
                      "Space cast · F5 save · R restart"
                    : "";
                GUI.Label(new Rect(x, y + 8, width, 80), status ?? help, message);
                return Action.None;
            }
            finally { GUI.color = oldColor; GUI.enabled = oldEnabled; }
        }

        public void Dispose()
        {
            if (Application.isPlaying) UnityEngine.Object.Destroy(background);
            else UnityEngine.Object.DestroyImmediate(background);
        }
    }
}
