using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.Native
{
    // Owns only presentation. Terrain, camera, actors and all interactions come from Rust.
    internal sealed class ModernEnvironmentPresenter : IDisposable
    {
        internal static bool Requested => Environment.GetCommandLineArgs().Contains("-mc-modern");
        private static RenderPipelineGlobalSettings modernGlobalSettings;
        internal bool Ready { get; private set; }
        private readonly GameObject root;
        private readonly Camera camera;
        private readonly Light moon;
        private readonly ModernTerrainMesh terrain;
        private readonly ModernVegetation vegetation;
        private readonly ModernWaterReflection waterReflection;
        private readonly ModernEntityPresenter entities;
        private readonly Material landMaterial, waterMaterial, skyMaterial;
        private readonly VolumeProfile volumeProfile;
        private readonly RenderPipelineAsset previousPipeline, previousQuality;
        private readonly Material previousSky;
        private readonly Light previousSun;
        private readonly bool previousFog;
        private readonly Color previousFogColor, previousAmbientSky, previousAmbientEquator, previousAmbientGround;
        private readonly float previousFogDensity;
        private readonly FogMode previousFogMode;
        private readonly AmbientMode previousAmbientMode;
        private EngineSessionTransport.CampaignSnapshotValue snapshot;
        private Vector3 targetPosition;
        private Quaternion targetRotation;
        private bool positioned, disposed;
        private ulong generation = ulong.MaxValue, terrainSequence = ulong.MaxValue;
        private int snapshotCount;
        private double presentationMilliseconds;
        private Texture2D worldMap;
        private bool mapOpen;
        private readonly OwnerAssetCache mapAssets;
        private readonly byte[] mapTables;
        private readonly byte mapCrossColor;
        private readonly Dictionary<int, CampaignPresentationSurface.MinimapSymbolPixels> mapSymbols = new();
        private readonly SourceMinimapLabels mapLabels = new();

        [RuntimeInitializeOnLoadMethod(RuntimeInitializeLoadType.BeforeSceneLoad)]
        private static void OriginalFrontendPipeline()
        {
            // The menu/loading compositor is the original indexed presentation. The modern
            // pipeline is activated only once the live NightSpire snapshot has arrived.
            GraphicsSettings.TryGetCurrentRenderPipelineGlobalSettings(out modernGlobalSettings);
            if (modernGlobalSettings == null)
                modernGlobalSettings = Resources.Load<RenderPipelineGlobalSettings>("ModernPipelineGlobalSettings");
            GraphicsSettings.defaultRenderPipeline = null;
            QualitySettings.renderPipeline = null;
        }

        internal ModernEnvironmentPresenter(OwnerAssetCache cache)
        {
            var pipeline = Resources.Load<UniversalRenderPipelineAsset>("ModernPipeline");
            if (pipeline == null) throw new InvalidOperationException("Modern pipeline asset is missing; build with BuildModernPlayer");
            previousPipeline = GraphicsSettings.defaultRenderPipeline;
            previousQuality = QualitySettings.renderPipeline;
            previousSky = RenderSettings.skybox;
            previousSun = RenderSettings.sun;
            previousFog = RenderSettings.fog;
            previousFogColor = RenderSettings.fogColor;
            previousFogDensity = RenderSettings.fogDensity;
            previousFogMode = RenderSettings.fogMode;
            previousAmbientMode = RenderSettings.ambientMode;
            previousAmbientSky = RenderSettings.ambientSkyColor;
            previousAmbientEquator = RenderSettings.ambientEquatorColor;
            previousAmbientGround = RenderSettings.ambientGroundColor;
            try
            {
            mapAssets = cache;
            mapTables = cache.ReadOutput(cache.EnvironmentMaterials.tables.path);
            mapCrossColor = cache.ReadOutput(cache.EnvironmentMaterials.colors.path)[4095];
            foreach (var bitmap in cache.EnvironmentMaterials.bitmap_banks.Single(
                bank => bank.bank == cache.EnvironmentMaterials.profile.sprites).bitmaps)
                if (!bitmap.source_empty)
                    mapSymbols.Add(bitmap.selector, new CampaignPresentationSurface.MinimapSymbolPixels {
                        Width = bitmap.width, Height = bitmap.height,
                        Indices = cache.ReadOutput(bitmap.indices_path), Mask = cache.ReadOutput(bitmap.mask_path)
                    });
            if (!Application.isEditor && !GraphicsSettings.TrySetCurrentRenderPipelineGlobalSettings(modernGlobalSettings))
                throw new InvalidOperationException("Modern global render settings were not included in this player");
            GraphicsSettings.defaultRenderPipeline = pipeline;
            QualitySettings.renderPipeline = null;
            root = new GameObject("NightSpire modern environment");
            landMaterial = Material("ModernTerrain");
            foreach (var layer in new[] { "Rock", "Ground" })
                foreach (var channel in new[] { "Albedo", "Normal", "Roughness" })
                    landMaterial.SetTexture("_" + layer + channel, Resources.Load<Texture2D>("ModernTextures/" + layer + channel));
            landMaterial.SetFloat("_UseTextures", 1f);
            landMaterial.SetFloat("_TextureScale", .20f);
            waterMaterial = Material("ModernWater");
            skyMaterial = Material("ModernSky");
            RenderSettings.skybox = skyMaterial;
            RenderSettings.fog = true;
            RenderSettings.fogMode = FogMode.ExponentialSquared;
            RenderSettings.fogColor = new Color(.24f, .29f, .32f);
            RenderSettings.fogDensity = .0038f;
            RenderSettings.ambientMode = AmbientMode.Trilight;
            RenderSettings.ambientSkyColor = new Color(.18f, .24f, .34f);
            RenderSettings.ambientEquatorColor = new Color(.12f, .16f, .20f);
            RenderSettings.ambientGroundColor = new Color(.07f, .09f, .085f);
            moon = new GameObject("Moonlight").AddComponent<Light>();
            moon.transform.SetParent(root.transform, false);
            var moonDirection = new Vector3(-.38f, .48f, .79f).normalized;
            moon.transform.rotation = Quaternion.LookRotation(-moonDirection);
            moon.type = LightType.Directional;
            moon.color = new Color(.76f, .86f, 1f);
            moon.intensity = .80f;
            moon.shadows = LightShadows.Soft;
            moon.shadowStrength = .86f;
            RenderSettings.sun = moon;
            camera = new GameObject("Modern flight camera").AddComponent<Camera>();
            camera.transform.SetParent(root.transform, false);
            camera.nearClipPlane = .10f;
            camera.farClipPlane = 900f;
            camera.fieldOfView = 67f;
            camera.allowHDR = true;
            camera.allowMSAA = true;
            camera.clearFlags = CameraClearFlags.Skybox;
            camera.cullingMask = 1; // Modern world owns layer zero; no indexed-frame overlays.
            camera.depth = 0;
            var cameraData = camera.GetUniversalAdditionalCameraData();
            cameraData.renderPostProcessing = true;
            cameraData.requiresDepthTexture = true;
            cameraData.antialiasing = AntialiasingMode.SubpixelMorphologicalAntiAliasing;
            cameraData.antialiasingQuality = AntialiasingQuality.High;
            volumeProfile = ScriptableObject.CreateInstance<VolumeProfile>();
            var tone = volumeProfile.Add<Tonemapping>(true);
            tone.mode.Override(TonemappingMode.ACES);
            var color = volumeProfile.Add<ColorAdjustments>(true);
            color.postExposure.Override(.15f);
            color.contrast.Override(10f);
            color.saturation.Override(-8f);
            var bloom = volumeProfile.Add<Bloom>(true);
            bloom.threshold.Override(1.15f);
            bloom.intensity.Override(.18f);
            bloom.scatter.Override(.6f);
            var volume = root.AddComponent<Volume>();
            volume.isGlobal = true;
            volume.sharedProfile = volumeProfile;
            terrain = new ModernTerrainMesh(root.transform, landMaterial, waterMaterial) { ViewDistance = 650f };
            vegetation = new ModernVegetation(terrain);
            entities = new ModernEntityPresenter(root.transform, cache);
            waterReflection = new ModernWaterReflection(root.transform, waterMaterial);
            camera.gameObject.AddComponent<ModernCapture>();
            Debug.Log($"MC_MODERN_CREATED graphics={SystemInfo.graphicsDeviceName} api={SystemInfo.graphicsDeviceType}");
            }
            catch
            {
                Dispose();
                throw;
            }
        }

        private static Material Material(string name)
        {
            var shader = Resources.Load<Shader>(name);
            if (shader == null) throw new InvalidOperationException($"Missing modern shader {name}");
            return new Material(shader) { name = name };
        }

        internal void Present(NativeCampaignSession session, EngineSessionTransport.CampaignSnapshotObservation observation, bool showMap)
        {
            var measuredStart = ModernCapture.Measure ? System.Diagnostics.Stopwatch.GetTimestamp() : 0;
            snapshot = observation.value;
            mapOpen = showMap;
            if (showMap)
            {
                var map = session.WorldMap(session.Tick);
                CampaignPresentationSurface.MaterializeMinimapPixels(map.indices, map.entities, mapTables,
                    session.Generation, session.Tick, mapSymbols, true);
                mapLabels.Materialize(map.indices, session, mapAssets, mapTables, session.Generation, session.Tick, true);
                CampaignPresentationSurface.MaterializeMapCross(map.indices, mapTables, mapCrossColor);
                CampaignPresentationSurface.MaterializeMinimapMarks(map.indices, map.marks, mapTables,
                    session.Generation, session.Tick, true);
                worldMap ??= new Texture2D(320, 200, TextureFormat.RGBA32, false) {
                    name = "Live campaign map", filterMode = FilterMode.Point
                };
                var pixels = new Color32[320 * 200];
                for (var y = 0; y < 200; y++)
                for (var x = 0; x < 320; x++)
                {
                    var index = map.indices[y * 320 + x] * 3;
                    pixels[(199 - y) * 320 + x] = new Color32(map.palette[index], map.palette[index + 1], map.palette[index + 2], 255);
                }
                worldMap.SetPixels32(pixels);
                worldMap.Apply(false);
            }
            var state = session.TerrainState();
            if (generation != state.generation) positioned = false;
            if (generation != state.generation || terrainSequence != state.mutationSequence)
            {
                terrain.Update(session.TerrainPlane(NativeSim.TerrainHeight),
                    session.TerrainPlane(NativeSim.TerrainTypes), session.TerrainPlane(NativeSim.TerrainAngle),
                    state.generation, state.tick);
                generation = state.generation;
                terrainSequence = state.mutationSequence;
            }
            var pose = snapshot.source_camera_pose;
            var wrapped = new Vector3(pose.x / 64f, pose.z / 64f, pose.y / 64f);
            if (positioned)
            {
                wrapped.x = camera.transform.position.x + Mathf.DeltaAngle(camera.transform.position.x * 360f / 1024f, wrapped.x * 360f / 1024f) * 1024f / 360f;
                wrapped.z = camera.transform.position.z + Mathf.DeltaAngle(camera.transform.position.z * 360f / 1024f, wrapped.z * 360f / 1024f) * 1024f / 360f;
            }
            targetPosition = wrapped;
            // Discontinuous authoritative relocation (reset, portal, possession) is a cut.
            if (positioned && Vector3.Distance(camera.transform.position, wrapped) > 48f) positioned = false;
            camera.fieldOfView = Level1ExitPresentationMath.UnityVerticalFov(snapshot.source_camera_fov);
            var projectionScale = 7 * 377 * snapshot.source_camera_fov >> 11;
            targetRotation = Quaternion.Euler(-Mathf.Atan2(pose.pitch * 1.25f, projectionScale) * Mathf.Rad2Deg,
                180f - pose.yaw * 360f / 2048f, -pose.roll * 360f / 2048f);
            if (!positioned)
            {
                camera.transform.SetPositionAndRotation(targetPosition, targetRotation);
                positioned = true;
            }
            entities.Present(session, pose.yaw);
            Ready = true;
            if (snapshotCount++ == 0)
                Debug.Log($"MC_MODERN_LIVE generation={generation} tick={state.tick} terrain={state.terrainHash:x16} position={wrapped} native_session={session.Handle}");
            if (measuredStart != 0)
            {
                presentationMilliseconds += (System.Diagnostics.Stopwatch.GetTimestamp() - measuredStart) * 1000.0 / System.Diagnostics.Stopwatch.Frequency;
                if (snapshotCount == 120)
                    Debug.Log($"MC_MODERN_PRESENT_TIMES samples={snapshotCount} mean_ms={presentationMilliseconds / snapshotCount:F2}");
            }
        }

        internal void Render()
        {
            if (!Ready || disposed) return;
            var renderStart = ModernCapture.BeginMeasurement();
            var blend = 1f - Mathf.Exp(-Time.unscaledDeltaTime * 22f);
            camera.transform.SetPositionAndRotation(ModernCapture.ReviewPosition ?? Vector3.Lerp(camera.transform.position, targetPosition, blend),
                ModernCapture.ReviewPosition.HasValue ? ModernCapture.ReviewRotation : Quaternion.Slerp(camera.transform.rotation, targetRotation, blend));
            terrain.Present(camera.transform.position);
            vegetation.Render(camera);
            entities.Render(camera);
            var reflectionStart = ModernCapture.BeginMeasurement();
            waterReflection.Render(camera, vegetation);
            ModernCapture.EndMeasurement("water_reflection", reflectionStart);
            ModernCapture.EndMeasurement("modern_render", renderStart);
        }

        internal void DrawHud(string instruction)
        {
            if (snapshot?.player_vitals == null || ModernCapture.HideHud) return;
            if (mapOpen && worldMap != null)
            {
                var rect = Level1ExitPresentationMath.SourceDisplayRect(Screen.width, Screen.height);
                GUI.DrawTexture(new Rect(0, 0, Screen.width, Screen.height), Texture2D.blackTexture);
                GUI.DrawTexture(new Rect(rect.x * Screen.width, rect.y * Screen.height,
                    rect.width * Screen.width, rect.height * Screen.height), worldMap);
                return;
            }
            var v = snapshot.player_vitals;
            var style = new GUIStyle(GUI.skin.label) { fontSize = Mathf.Max(14, Screen.height / 65) };
            style.normal.textColor = new Color(.82f, .89f, .89f);
            GUI.Label(new Rect(30, 24, 420, 30), "NIGHTSPIRE", style);
            if (snapshot.objective_textbox_ticks > 0 && !string.IsNullOrEmpty(instruction))
                GUI.Label(new Rect(Screen.width * .25f, Screen.height - 100, Screen.width * .5f, 70), instruction, style);
            GUI.Label(new Rect(30, Screen.height - 54, 500, 40), $"LIFE {v.life} / {v.max_life}    MANA {v.mana} / {v.max_mana}", style);
            GUI.color = new Color(.8f, .9f, 1f, .65f);
            GUI.DrawTexture(new Rect(Screen.width / 2f - 1, Screen.height / 2f - 4, 2, 8), Texture2D.whiteTexture);
            GUI.DrawTexture(new Rect(Screen.width / 2f - 4, Screen.height / 2f - 1, 8, 2), Texture2D.whiteTexture);
            GUI.color = Color.white;
        }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            waterReflection?.Dispose();
            vegetation?.Dispose();
            entities?.Dispose();
            terrain?.Dispose();
            UnityEngine.Object.Destroy(root);
            foreach (var material in new[] { landMaterial, waterMaterial, skyMaterial }) UnityEngine.Object.Destroy(material);
            UnityEngine.Object.Destroy(volumeProfile);
            UnityEngine.Object.Destroy(worldMap);
            RenderSettings.skybox = previousSky;
            RenderSettings.sun = previousSun;
            RenderSettings.fog = previousFog;
            RenderSettings.fogColor = previousFogColor;
            RenderSettings.fogDensity = previousFogDensity;
            RenderSettings.fogMode = previousFogMode;
            RenderSettings.ambientMode = previousAmbientMode;
            RenderSettings.ambientSkyColor = previousAmbientSky;
            RenderSettings.ambientEquatorColor = previousAmbientEquator;
            RenderSettings.ambientGroundColor = previousAmbientGround;
            GraphicsSettings.defaultRenderPipeline = previousPipeline;
            QualitySettings.renderPipeline = previousQuality;
        }
    }
}
