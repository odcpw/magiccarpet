using UnityEngine;
using UnityEngine.UI;

namespace MagicCarpet.PresentationLab
{
    /// <summary>
    /// Asset-free Enhanced HUD. It consumes only the immutable Rust-owned
    /// mission snapshot and renders through the Enhanced camera, so the HUD
    /// is visible in both the player route and resolved qualification frames.
    /// It never sends input or state back to the simulation.
    /// </summary>
    [DisallowMultipleComponent]
    public sealed class UnityEnhancedHudPresentation : MonoBehaviour
    {
        private Canvas canvas;
        private RawImage panel;
        private RawImage lifeFill;
        private RawImage manaFill;
        private Text titleText;
        private Text lifeText;
        private Text manaText;
        private Text speedText;
        private Text spellText;
        private Text statusText;
        private Text objectiveText;
        private float lifeFillAmount;
        private float manaFillAmount;
        private ulong generation;
        private ulong lastTick;

        public ulong Generation { get { return generation; } }
        public ulong LastTick { get { return lastTick; } }
        public float LifeFillAmount { get { return lifeFillAmount; } }
        public float ManaFillAmount { get { return manaFillAmount; } }
        public string StatusText { get { return statusText == null ? string.Empty : statusText.text; } }
        public string SpellText { get { return spellText == null ? string.Empty : spellText.text; } }
        public bool IsVisible { get { return canvas != null && canvas.enabled; } }

        public bool ApplySnapshot(UnityEnhancedHudSnapshot snapshot, Camera targetCamera)
        {
            if (snapshot == null || targetCamera == null || snapshot.Generation == 0)
            {
                return false;
            }

            if (generation == snapshot.Generation && snapshot.Tick < lastTick)
            {
                return false;
            }

            EnsureHierarchy(targetCamera);
            generation = snapshot.Generation;
            lastTick = snapshot.Tick;
            canvas.enabled = true;

            float lifeRatio = Ratio(snapshot.PlayerLife, snapshot.PlayerMaxLife);
            float manaRatio = Ratio(snapshot.PlayerMana, snapshot.PlayerMaxMana);
            lifeFillAmount = lifeRatio;
            manaFillAmount = manaRatio;
            SetBarAmount(lifeFill, lifeRatio);
            SetBarAmount(manaFill, manaRatio);
            lifeFill.color = Color.Lerp(
                new Color(0.72f, 0.12f, 0.18f, 1f),
                new Color(0.25f, 0.95f, 0.58f, 1f),
                lifeRatio);
            manaFill.color = new Color(0.25f, 0.72f, 1f, 1f);
            lifeText.text = "LIFE  " + snapshot.PlayerLife + " / " + snapshot.PlayerMaxLife;
            manaText.text = "MANA  " + snapshot.PlayerMana + " / " + snapshot.PlayerMaxMana;
            speedText.text = "SPEED  " + snapshot.ActualSpeed + "  /  " + snapshot.TargetSpeed;
            spellText.text = SpellSummary(snapshot);
            titleText.text = "MAGIC CARPET  //  LEVEL 1 NIGHT";
            objectiveText.text = ObjectiveSummary(snapshot);
            statusText.text = StatusSummary(snapshot);
            statusText.color = snapshot.PlayerDead
                ? new Color(1f, 0.35f, 0.38f, 1f)
                : snapshot.ExitSequenceComplete
                    ? new Color(0.45f, 1f, 0.72f, 1f)
                    : Color.white;
            return true;
        }

        public void ResetForGeneration(ulong nextGeneration)
        {
            generation = nextGeneration;
            lastTick = 0;
            if (canvas != null)
            {
                canvas.enabled = false;
            }
        }

        public void ResetForSession()
        {
            ResetForGeneration(0);
        }

        public void HideForSnapshot()
        {
            if (canvas != null)
            {
                canvas.enabled = false;
            }
        }

        private void EnsureHierarchy(Camera targetCamera)
        {
            if (canvas != null)
            {
                canvas.worldCamera = targetCamera;
                return;
            }

            GameObject canvasObject = new GameObject("EnhancedHudCanvas");
            canvasObject.transform.SetParent(transform, false);
            canvasObject.hideFlags = HideFlags.DontSave;
            canvas = canvasObject.AddComponent<Canvas>();
            canvas.renderMode = RenderMode.ScreenSpaceCamera;
            canvas.worldCamera = targetCamera;
            canvas.planeDistance = 1f;
            canvas.sortingOrder = 100;

            CanvasScaler scaler = canvasObject.AddComponent<CanvasScaler>();
            scaler.uiScaleMode = CanvasScaler.ScaleMode.ScaleWithScreenSize;
            scaler.referenceResolution = new Vector2(1280f, 720f);
            scaler.screenMatchMode = CanvasScaler.ScreenMatchMode.MatchWidthOrHeight;
            scaler.matchWidthOrHeight = 0.5f;

            RectTransform root = canvasObject.GetComponent<RectTransform>();
            if (root == null)
            {
                root = canvasObject.AddComponent<RectTransform>();
            }
            root.anchorMin = Vector2.zero;
            root.anchorMax = Vector2.one;
            root.offsetMin = Vector2.zero;
            root.offsetMax = Vector2.zero;

            panel = CreateImage(root, "EnhancedHudPanel", new Color(0.015f, 0.04f, 0.10f, 0.84f));
            ConfigureBox(panel.rectTransform, new Vector2(0f, 0f), new Vector2(0f, 0f),
                new Vector2(26f, 24f), new Vector2(366f, 158f));

            titleText = CreateText(panel.rectTransform, "EnhancedHudTitle", 16, FontStyle.Bold);
            ConfigureBox(titleText.rectTransform, new Vector2(0f, 1f), new Vector2(0f, 1f),
                new Vector2(18f, -14f), new Vector2(330f, 24f));

            lifeFill = CreateBar(panel.rectTransform, "Life", new Color(0.25f, 0.95f, 0.58f, 1f), 70f);
            manaFill = CreateBar(panel.rectTransform, "Mana", new Color(0.25f, 0.72f, 1f, 1f), 42f);
            lifeText = CreateText(panel.rectTransform, "EnhancedHudLife", 12, FontStyle.Normal);
            manaText = CreateText(panel.rectTransform, "EnhancedHudMana", 12, FontStyle.Normal);
            ConfigureBox(lifeText.rectTransform, new Vector2(0f, 1f), new Vector2(0f, 1f),
                new Vector2(18f, -48f), new Vector2(326f, 20f));
            ConfigureBox(manaText.rectTransform, new Vector2(0f, 1f), new Vector2(0f, 1f),
                new Vector2(18f, -78f), new Vector2(326f, 20f));

            spellText = CreateText(panel.rectTransform, "EnhancedHudSpells", 11, FontStyle.Normal);
            ConfigureBox(spellText.rectTransform, new Vector2(0f, 0f), new Vector2(0f, 0f),
                new Vector2(18f, 16f), new Vector2(330f, 22f));

            speedText = CreateText(root, "EnhancedHudSpeed", 15, FontStyle.Bold);
            ConfigureBox(speedText.rectTransform, new Vector2(1f, 0f), new Vector2(1f, 0f),
                new Vector2(-28f, 28f), new Vector2(220f, 30f));
            speedText.alignment = TextAnchor.MiddleRight;

            objectiveText = CreateText(root, "EnhancedHudObjective", 15, FontStyle.Bold);
            ConfigureBox(objectiveText.rectTransform, new Vector2(0.5f, 1f), new Vector2(0.5f, 1f),
                new Vector2(0f, -28f), new Vector2(440f, 28f));
            objectiveText.alignment = TextAnchor.MiddleCenter;

            statusText = CreateText(root, "EnhancedHudStatus", 13, FontStyle.Normal);
            ConfigureBox(statusText.rectTransform, new Vector2(1f, 1f), new Vector2(1f, 1f),
                new Vector2(-28f, -26f), new Vector2(360f, 24f));
            statusText.alignment = TextAnchor.MiddleRight;
        }

        private static RawImage CreateBar(Transform parent, string name, Color color, float y)
        {
            RawImage background = CreateImage(parent, name + "BarBackground", new Color(0f, 0f, 0f, 0.62f));
            ConfigureBox(background.rectTransform, new Vector2(0f, 1f), new Vector2(0f, 1f),
                new Vector2(18f, -y), new Vector2(326f, 14f));
            RawImage fill = CreateImage(background.rectTransform, name + "BarFill", color);
            fill.rectTransform.anchorMin = new Vector2(0f, 0f);
            fill.rectTransform.anchorMax = new Vector2(0f, 1f);
            fill.rectTransform.pivot = new Vector2(0f, 0.5f);
            fill.rectTransform.anchoredPosition = Vector2.zero;
            fill.rectTransform.sizeDelta = new Vector2(0f, 0f);
            return fill;
        }

        private static RawImage CreateImage(Transform parent, string name, Color color)
        {
            GameObject imageObject = new GameObject(name);
            imageObject.transform.SetParent(parent, false);
            imageObject.hideFlags = HideFlags.DontSave;
            RawImage image = imageObject.AddComponent<RawImage>();
            image.texture = Texture2D.whiteTexture;
            image.color = color;
            image.raycastTarget = false;
            return image;
        }

        private static void SetBarAmount(RawImage fill, float amount)
        {
            if (fill == null)
            {
                return;
            }

            fill.rectTransform.anchorMin = new Vector2(0f, 0f);
            fill.rectTransform.anchorMax = new Vector2(0f, 1f);
            fill.rectTransform.pivot = new Vector2(0f, 0.5f);
            fill.rectTransform.anchoredPosition = Vector2.zero;
            fill.rectTransform.sizeDelta = new Vector2(326f * Mathf.Clamp01(amount), 0f);
        }

        private static Text CreateText(Transform parent, string name, int size, FontStyle style)
        {
            GameObject textObject = new GameObject(name);
            textObject.transform.SetParent(parent, false);
            textObject.hideFlags = HideFlags.DontSave;
            Text text = textObject.AddComponent<Text>();
            text.font = Resources.GetBuiltinResource<Font>("LegacyRuntime.ttf");
            text.fontSize = size;
            text.fontStyle = style;
            text.color = new Color(0.88f, 0.95f, 1f, 1f);
            text.alignment = TextAnchor.MiddleLeft;
            text.raycastTarget = false;
            return text;
        }

        private static void ConfigureBox(
            RectTransform rect,
            Vector2 anchor,
            Vector2 pivot,
            Vector2 position,
            Vector2 size)
        {
            rect.anchorMin = anchor;
            rect.anchorMax = anchor;
            rect.pivot = pivot;
            rect.anchoredPosition = position;
            rect.sizeDelta = size;
        }

        private static float Ratio(int value, int maximum)
        {
            if (maximum <= 0)
            {
                return 0f;
            }

            return Mathf.Clamp01((float)value / maximum);
        }

        private static string SpellSummary(UnityEnhancedHudSnapshot snapshot)
        {
            return "SPELLS  " +
                   (snapshot.HasFireball ? "FIRE" : "----") + "  " +
                   (snapshot.HasPossession ? "POS" : "---") + "  " +
                   (snapshot.HasCastleSpell ? "CASTLE" : "------") + "  " +
                   (snapshot.HasSpeedSpell ? "SPEED" : "-----");
        }

        private static string ObjectiveSummary(UnityEnhancedHudSnapshot snapshot)
        {
            string prefix = snapshot.ObjectiveTextboxTicks > 0 ? "OBJECTIVE ACTIVE" : "LEVEL 1 // NIGHT";
            return prefix + "   STAGE " + snapshot.CurrentStage + "   TASKS " + snapshot.TasksCompleted;
        }

        private static string StatusSummary(UnityEnhancedHudSnapshot snapshot)
        {
            if (snapshot.PlayerDead)
            {
                return "DEFEATED  //  RESTART AVAILABLE";
            }

            if (snapshot.ExitSequenceComplete)
            {
                return "MISSION COMPLETE  //  RESULT " + snapshot.ExitResultCode;
            }

            if (snapshot.ExitSequenceActive)
            {
                return "EXIT SEQUENCE ACTIVE";
            }

            return "CASTER READY  //  MANA " + snapshot.SecuredManaPercent + "%";
        }
    }
}
