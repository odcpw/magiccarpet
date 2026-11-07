using System;
using System.Collections;
using System.IO;
using System.Linq;
using System.Globalization;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.Native
{
    // Interactive playback of the imported rigs, materials and named clips.
    // This scene never opens a campaign, advances Rust or selects a game save.
    public sealed class ModernCharacterReview : MonoBehaviour
    {
        private ModernCharacterInstance goat, archer, current;
        private Camera reviewCamera;
        private Material floorMaterial;
        private Texture2D floorTexture;
        private Renderer[] goatRenderers, archerRenderers;
        private Vector3 framingCenter;
        private float framingDistance;
        private bool cameraPositioned;
        private float elapsed, phaseTime, orbitYaw = 25;
        private int phase = -1;
        private bool automatic = true, paused;
        private string caption = "";
        private string captureDirectory;
        private int captureFrame;
        private int framingFailures;
        private int reportedFramingPhase = int.MinValue;
        private bool singleFrame;
        private GUIStyle title, body, button;
        private readonly string[] clips = { "Idle", "Walk", "StandingShot", "KneelEnter", "KneelingShot", "KneelExit" };
        private ReviewStep[] steps;
        private float duration;
        private sealed class ReviewStep {
            internal ModernCharacterInstance actor;
            internal string clip, label;
            internal float duration;
        }

        private void Awake()
        {
            GraphicsSettings.defaultRenderPipeline = Resources.Load<UniversalRenderPipelineAsset>("ModernPipeline");
            QualitySettings.renderPipeline = null;
            QualitySettings.vSyncCount = 0;
            Application.targetFrameRate = 60;
            RenderSettings.fog = false;
            RenderSettings.ambientMode = AmbientMode.Trilight;
            RenderSettings.ambientSkyColor = new Color(.43f, .49f, .59f);
            RenderSettings.ambientEquatorColor = new Color(.29f, .30f, .31f);
            RenderSettings.ambientGroundColor = new Color(.13f, .12f, .10f);
            var key = new GameObject("Character daylight").AddComponent<Light>();
            key.type = LightType.Directional; key.intensity = 1.45f;
            key.color = new Color(1, .94f, .84f); key.shadows = LightShadows.Soft;
            key.transform.rotation = Quaternion.Euler(42, -32, 0);
            RenderSettings.sun = key;
            var fill = new GameObject("Soft character fill").AddComponent<Light>();
            fill.type = LightType.Directional; fill.intensity = .7f;
            fill.color = new Color(.64f, .76f, 1); fill.transform.rotation = Quaternion.Euler(28, 135, 0);
            var floor = GameObject.CreatePrimitive(PrimitiveType.Plane);
            floor.name = "Review ground"; floor.transform.localScale = Vector3.one * 30;
            floorMaterial = new Material(Shader.Find("Universal Render Pipeline/Lit"));
            floorMaterial.SetColor("_BaseColor", new Color(.25f, .28f, .25f));
            floorMaterial.SetFloat("_Smoothness", .12f);
            floorTexture = new Texture2D(128, 128, TextureFormat.RGB24, true) { name = "Metre reference tiles", filterMode = FilterMode.Trilinear, anisoLevel = 4 };
            var pixels = new Color32[128 * 128];
            for (var y = 0; y < 128; y++) for (var x = 0; x < 128; x++)
            { var tone = (byte)((x / 64 + y / 64) % 2 == 0 ? 230 : 208); pixels[y * 128 + x] = new Color32(tone, tone, tone, 255); }
            floorTexture.SetPixels32(pixels); floorTexture.Apply(true, true);
            floorMaterial.SetTexture("_BaseMap", floorTexture); floorMaterial.SetTextureScale("_BaseMap", Vector2.one * 150);
            floor.GetComponent<Renderer>().sharedMaterial = floorMaterial;
            reviewCamera = new GameObject("Character review camera").AddComponent<Camera>();
            reviewCamera.backgroundColor = new Color(.18f, .23f, .29f);
            reviewCamera.clearFlags = CameraClearFlags.SolidColor;
            reviewCamera.nearClipPlane = .05f; reviewCamera.farClipPlane = 80;
            reviewCamera.fieldOfView = 36;
            reviewCamera.rect = new Rect(0, .15f, 1, .69f);
            reviewCamera.GetUniversalAdditionalCameraData().renderPostProcessing = false;
            goat = Create("Goat"); archer = Create("Archer");
            goatRenderers = goat.Root.GetComponentsInChildren<Renderer>(true);
            archerRenderers = archer.Root.GetComponentsInChildren<Renderer>(true);
            SetCharacter(goat);
            steps = new[] {
                Step(goat, "Idle", "GOAT · idle", 3),
                Step(goat, "Walk", "GOAT · walk, turn and stop", 7),
                Step(goat, "Idle", "GOAT · settled stance", 3),
                Step(archer, "Idle", "ARCHER · ready stance", 3),
                Step(archer, "Walk", "ARCHER · walk and stop", 4),
                Step(archer, "Idle", "ARCHER · settle", 1),
                Step(archer, "StandingShot", "ARCHER · nock, draw, loose, recover"),
                Step(archer, "KneelEnter", "ARCHER · prepare and kneel"),
                Step(archer, "KneelingShot", "ARCHER · kneeling shot"),
                Step(archer, "KneelExit", "ARCHER · rise"),
                Step(archer, "Idle", "ARCHER · recovered stance", 3)
            };
            duration = steps.Sum(step => step.duration);
            var args = Environment.GetCommandLineArgs();
            var still = Array.IndexOf(args, "-mc-character-still");
            if (still >= 0)
            {
                if (still + 1 >= args.Length) throw new ArgumentException("Still requires character,clip,time,yaw");
                var values = args[still + 1].Split(',');
                if (values.Length != 4) throw new ArgumentException("Still requires character,clip,time,yaw");
                SetCharacter(values[0] == "Goat" ? goat : values[0] == "Archer" ? archer : throw new ArgumentException("Unknown character"));
                current.Sample(values[1], float.Parse(values[2], CultureInfo.InvariantCulture));
                orbitYaw = float.Parse(values[3], CultureInfo.InvariantCulture);
                automatic = false; paused = true; singleFrame = true;
                caption = values[0] + " · " + ClipLabel(values[1]);
            }
            var option = Array.IndexOf(args, "-mc-character-capture");
            if (option >= 0)
            {
                if (option + 1 >= args.Length) throw new ArgumentException("Character capture requires a new directory");
                captureDirectory = args[option + 1];
                if (Directory.Exists(captureDirectory)) throw new IOException("Capture directory already exists");
                Directory.CreateDirectory(captureDirectory);
                Time.captureFramerate = 24;
                StartCoroutine(Capture());
            }
            Debug.Log($"MC_CHARACTER_REVIEW_READY goat={goat.Asset.height} archer={archer.Asset.height} capture={captureDirectory != null} gameplay=False");
        }

        private ModernCharacterInstance Create(string name)
        {
            var asset = Resources.Load<ModernCharacterAsset>("ModernCharacters/" + name + "/Character");
            return new ModernCharacterInstance(asset, transform);
        }

        private void SetCharacter(ModernCharacterInstance value)
        {
            current = value;
            goat.Root.SetActive(current == goat); archer.Root.SetActive(current == archer);
            current.Root.transform.SetPositionAndRotation(Vector3.zero, Quaternion.identity);
            current.Play("Idle", 0); current.Tick(0);
            cameraPositioned = false;
        }

        private static ReviewStep Step(ModernCharacterInstance actor, string clip, string label, float seconds = 0)
            => new ReviewStep { actor = actor, clip = clip, label = label,
                duration = seconds > 0 ? seconds : actor.Asset.Clip(clip).length + .35f };

        private void Enter(int next, ModernCharacterInstance actor, string clip, string label)
        {
            if (phase == next) return;
            if (current != actor) SetCharacter(actor);
            phase = next; phaseTime = elapsed;
            caption = label; current.Play(clip, .16f);
            Debug.Log($"MC_CHARACTER_PHASE phase={phase} character={actor.Asset.name} clip={clip} time={elapsed:F3}");
        }

        private void Update()
        {
            if (Input.GetKeyDown(KeyCode.Escape)) Application.Quit();
            if (Input.GetKeyDown(KeyCode.Space)) paused = !paused;
            if (Input.GetKeyDown(KeyCode.Alpha1)) { automatic = false; SetCharacter(goat); caption = "Goat"; }
            if (Input.GetKeyDown(KeyCode.Alpha2)) { automatic = false; SetCharacter(archer); caption = "Archer"; }
            if (Input.GetMouseButton(0) && Input.mousePosition.y > Screen.height * .2f)
                orbitYaw += Input.GetAxisRaw("Mouse X") * 2;
            var dt = paused ? 0 : Time.deltaTime;
            if (automatic)
            {
                elapsed += dt;
                var end = 0f;
                for (var i = 0; i < steps.Length; i++)
                {
                    end += steps[i].duration;
                    if (elapsed < end || i == steps.Length - 1)
                    { Enter(i, steps[i].actor, steps[i].clip, steps[i].label); break; }
                }
            }
            current.Tick(dt);
            if (automatic)
            {
                if (current.CurrentClip == "Walk")
                {
                    var local = elapsed - phaseTime;
                    current.Root.transform.rotation = Quaternion.Euler(0, local * (current == goat ? 9 : 6), 0);
                }
                current.Root.transform.position += current.Root.transform.forward * (current.Asset.walkSpeed * current.WalkWeight * dt);
            }
            var angleView = automatic ? current.Root.transform.eulerAngles.y + (current == goat ? 38 : 50) : orbitYaw;
            var rotation = Quaternion.Euler(9, angleView + 180, 0);
            FrameCharacter(rotation, dt);
            if (captureDirectory == null && automatic && elapsed > duration) { automatic = false; paused = true; }
        }

        private void FrameCharacter(Quaternion rotation, float dt)
        {
            var renderers = current == goat ? goatRenderers : archerRenderers;
            var bounds = new Bounds(); var found = false;
            foreach (var renderer in renderers)
            {
                // Follow a held arrow, while allowing the released projectile to leave view.
                var arrow = renderer.transform;
                while (arrow != null && arrow.name != "ArrowControl") arrow = arrow.parent;
                if (arrow != null)
                {
                    var held = current.CurrentClip == "KneelEnter" ||
                        current.CurrentClip == "StandingShot" && current.ClipTime < 3.17f ||
                        current.CurrentClip == "KneelingShot" && current.ClipTime < 1.09f;
                    if (!held || arrow.lossyScale.sqrMagnitude < .01f) continue;
                }
                // Unity already maintains world culling bounds, including animated props.
                // Baking differently scaled body/prop skins into a shared snapshot made
                // the bow bounds grow 100x after the first frame in the real player.
                var worldBounds = renderer.bounds;
                if (!Finite(worldBounds.center) || !Finite(worldBounds.extents))
                { framingFailures++; throw new InvalidOperationException("Non-finite character bounds: " + renderer.name); }
                for (var i = 0; i < 8; i++)
                {
                    var world = worldBounds.center + Vector3.Scale(worldBounds.extents,
                        new Vector3((i & 1) == 0 ? -1 : 1, (i & 2) == 0 ? -1 : 1, (i & 4) == 0 ? -1 : 1));
                    if (!found) { bounds = new Bounds(world, Vector3.zero); found = true; }
                    else bounds.Encapsulate(world);
                }
            }
            if (!found) throw new InvalidOperationException("Character has no visible framing geometry");
            var vertical = Mathf.Tan(reviewCamera.fieldOfView * Mathf.Deg2Rad * .5f);
            var horizontal = vertical * reviewCamera.aspect;
            var blend = cameraPositioned && !singleFrame ? 1 - Mathf.Exp(-dt * 8) : 1;
            framingCenter = Vector3.Lerp(framingCenter, bounds.center, blend);
            var distance = 0f;
            for (var i = 0; i < 8; i++)
            {
                var corner = Vector3.Scale(bounds.extents,
                    new Vector3((i & 1) == 0 ? -1 : 1, (i & 2) == 0 ? -1 : 1, (i & 4) == 0 ? -1 : 1));
                var view = Quaternion.Inverse(rotation) * (bounds.center - framingCenter + corner);
                distance = Mathf.Max(distance, Mathf.Max(Mathf.Abs(view.x) / horizontal, Mathf.Abs(view.y) / vertical) - view.z);
            }
            var required = distance * 1.15f + .1f;
            framingDistance = Mathf.Max(required, Mathf.Lerp(framingDistance, required, blend));
            reviewCamera.transform.position = framingCenter + rotation * new Vector3(0, 0, -framingDistance);
            reviewCamera.transform.LookAt(framingCenter);
            cameraPositioned = true;
            for (var i = 0; i < 8; i++)
            {
                var corner = bounds.center + Vector3.Scale(bounds.extents,
                    new Vector3((i & 1) == 0 ? -1 : 1, (i & 2) == 0 ? -1 : 1, (i & 4) == 0 ? -1 : 1));
                var view = reviewCamera.WorldToViewportPoint(corner);
                if (!Finite(view) || view.z < reviewCamera.nearClipPlane || view.z > reviewCamera.farClipPlane || view.x < 0 || view.x > 1 || view.y < 0 || view.y > 1)
                    framingFailures++;
            }
            if (reportedFramingPhase != phase)
            {
                reportedFramingPhase = phase;
                Debug.Log($"MC_CHARACTER_FRAMING actor={current.Asset.name} phase={phase} bounds={bounds} center={framingCenter} distance={framingDistance} camera={reviewCamera.transform.position} aspect={reviewCamera.aspect}");
            }
        }

        private static bool Finite(Vector3 value) => float.IsFinite(value.x) && float.IsFinite(value.y) && float.IsFinite(value.z);

        private IEnumerator Capture()
        {
            while (elapsed < duration)
            {
                yield return new WaitForEndOfFrame();
                ScreenCapture.CaptureScreenshot(Path.Combine(captureDirectory, $"frame-{captureFrame++:D5}.png"));
                if (singleFrame) break;
            }
            yield return new WaitForSecondsRealtime(.25f);
            File.WriteAllText(Path.Combine(captureDirectory, "capture.json"),
                $"{{\"frames\":{captureFrame},\"fps\":24,\"width\":{Screen.width},\"height\":{Screen.height},\"framing_failures\":{framingFailures},\"engine\":\"Unity\",\"performance_measurement\":false}}");
            Debug.Log("MC_CHARACTER_CAPTURE_COMPLETE frames=" + captureFrame);
            Application.Quit();
        }

        private void OnGUI()
        {
            var scale = Screen.height / 1920f;
            var previousColor = GUI.color;
            GUI.color = reviewCamera.backgroundColor;
            GUI.DrawTexture(new Rect(0, 0, Screen.width, Screen.height * .165f), Texture2D.whiteTexture);
            GUI.DrawTexture(new Rect(0, Screen.height * .85f, Screen.width, Screen.height * .15f), Texture2D.whiteTexture);
            GUI.color = previousColor;
            title ??= new GUIStyle(GUI.skin.label) { alignment = TextAnchor.MiddleCenter, wordWrap = true };
            body ??= new GUIStyle(GUI.skin.label) { alignment = TextAnchor.MiddleCenter, wordWrap = true };
            button ??= new GUIStyle(GUI.skin.button) { wordWrap = true };
            title.fontSize = Mathf.RoundToInt(50 * scale); title.normal.textColor = Color.white;
            body.fontSize = Mathf.RoundToInt(32 * scale); body.normal.textColor = new Color(.83f, .88f, .91f);
            button.fontSize = Mathf.RoundToInt(30 * scale);
            GUI.Label(new Rect(35 * scale, 90 * scale, Screen.width - 70 * scale, 155 * scale), caption, title);
            GUI.Label(new Rect(35 * scale, 240 * scale, Screen.width - 70 * scale, 70 * scale),
                "Original sprite identity · modern 3D animation", body);
            if (captureDirectory == null)
            {
                var y = Screen.height - 210 * scale;
                var clip = current.Asset.Clip(current.CurrentClip);
                GUI.Label(new Rect(35 * scale, Screen.height - 280 * scale, Screen.width - 70 * scale, 40 * scale),
                    $"{ClipLabel(current.CurrentClip)} · {current.ClipTime:F2} / {clip.length:F2} s", body);
                var time = GUI.HorizontalSlider(new Rect(35 * scale, Screen.height - 238 * scale,
                    Screen.width - 70 * scale, 25 * scale), current.ClipTime, 0, clip.length);
                if (Mathf.Abs(time - current.ClipTime) > .00001f)
                { automatic = false; paused = true; current.Sample(current.CurrentClip, time); }
                if (GUI.Button(new Rect(25 * scale, y, 210 * scale, 70 * scale), "Goat", button)) { automatic = false; SetCharacter(goat); caption = "GOAT"; }
                if (GUI.Button(new Rect(250 * scale, y, 210 * scale, 70 * scale), "Archer", button)) { automatic = false; SetCharacter(archer); caption = "ARCHER"; }
                if (GUI.Button(new Rect(475 * scale, y, 240 * scale, 70 * scale), paused ? "Play" : "Pause", button)) paused = !paused;
                if (GUI.Button(new Rect(730 * scale, y, 280 * scale, 70 * scale), "Full sequence", button)) { automatic = true; paused = false; elapsed = 0; phase = -1; }
                var visible = clips.Where(name => current.Asset.clips.Any(clip => clip.name == name)).ToArray();
                for (var i = 0; i < visible.Length; i++)
                    if (GUI.Button(new Rect(25 * scale + i * (Screen.width - 50 * scale) / visible.Length, y + 85 * scale,
                        (Screen.width - 65 * scale) / visible.Length, 70 * scale), ClipLabel(visible[i]), button))
                    { automatic = false; paused = false; current.Play(visible[i]); caption = current.Asset.name + " · " + visible[i]; }
            }
            else GUI.Label(new Rect(35 * scale, Screen.height - 150 * scale, Screen.width - 70 * scale, 75 * scale),
                "Unity playback · complete actions", body);
        }

        private static string ClipLabel(string name) => name switch {
            "StandingShot" => "Shoot", "KneelEnter" => "Kneel", "KneelingShot" => "Kneeling shot",
            "KneelExit" => "Rise", _ => name
        };

        private void OnDestroy()
        {
            goat?.Dispose(); archer?.Dispose();
            if (floorMaterial != null) Destroy(floorMaterial);
            if (floorTexture != null) Destroy(floorTexture);
            if (captureDirectory != null) Time.captureFramerate = 0;
        }
    }
}
