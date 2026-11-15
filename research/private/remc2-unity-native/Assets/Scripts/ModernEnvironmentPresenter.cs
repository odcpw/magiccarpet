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
        internal bool SandboxActive => Ready && sandbox != null;
        private readonly ModernEnvironmentSandbox sandbox;
        private readonly ModernBuildings buildings;
        private readonly Voxels.VoxelModernWorld voxelWorld;
        private readonly GameObject root;
        private readonly Camera camera;
        private readonly Light moon;
        private readonly ModernTerrainMesh terrain;
        private readonly ModernVegetation vegetation;
        private readonly ModernShoreMask sourceShore;
        private readonly ModernWaterReflection waterReflection;
        private readonly ModernWaterSystem waterSystem;
        private readonly ModernNightLighting nightLighting;
        private readonly ModernEntityPresenter entities;
        private readonly Material landMaterial, waterMaterial, skyMaterial;
        private readonly Texture3D cloudNoise;
        private readonly VolumeProfile volumeProfile;
        private readonly RenderPipelineAsset previousPipeline, previousQuality;
        private readonly Material previousSky;
        private readonly Light previousSun;
        private readonly bool previousFog;
        private readonly Color previousFogColor, previousAmbientSky, previousAmbientEquator, previousAmbientGround;
        private readonly float previousFogDensity;
        private readonly FogMode previousFogMode;
        private readonly AmbientMode previousAmbientMode;
        private EngineSessionTransport.CampaignRenderValue snapshot;
        private Vector3 targetPosition;
        private Quaternion targetRotation;
        private bool positioned, disposed;
        private ulong generation = ulong.MaxValue, terrainSequence = ulong.MaxValue;
        private byte[] nativeHeightPlane, visualHeightPlane;
        private int buildingVoxelSlot = int.MinValue, buildingLightMode = -1;
        private int snapshotCount;
        private bool reportedGeometry;
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
            var unpackedTerrain = Environment.GetCommandLineArgs().Contains("-mc-modern-unpacked-terrain");
            foreach (var layer in new[] { "Rock", "Ground" })
                foreach (var channel in unpackedTerrain ? new[] { "Albedo", "Normal", "Roughness" } : new[] { "AlbedoRoughness", "Normal" })
                {
                    var texture = Resources.Load<Texture2D>("ModernTextures/" + layer + channel);
                    if (texture == null) throw new InvalidOperationException("Missing terrain material map: " + layer + channel);
                    landMaterial.SetTexture("_" + layer + channel, texture);
                }
            landMaterial.SetFloat("_UseTextures", 1f);
            landMaterial.SetFloat("_UsePackedRoughness", unpackedTerrain ? 0f : 1f);
            landMaterial.SetFloat("_UseSparseProjection", Environment.GetCommandLineArgs().Contains("-mc-modern-full-terrain-projection") ? 0f : 1f);
            landMaterial.SetFloat("_TextureScale", .20f);
            landMaterial.SetFloat("_UseMaterialEndpoints", ModernCapture.FullTerrainMaterial ? 0f : 1f);
            waterMaterial = Material("ModernWater");
            skyMaterial = Material("ModernSky");
            Shader.SetGlobalFloat("_ModernDensityDetail",0);
            if (!ModernCapture.ProceduralClouds)
            {
                cloudNoise = ModernCloudNoise.Create();
                skyMaterial.SetTexture("_CloudNoise", cloudNoise);
                skyMaterial.SetFloat("_UseCloudNoise", 1f);
                Shader.SetGlobalTexture("_ModernDetailNoise",cloudNoise);
                Shader.SetGlobalFloat("_ModernDensityDetail",Environment.GetCommandLineArgs().Contains("-mc-steam-no-detail")?0:1);
            }
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
            cameraData.requiresColorTexture = false;
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
            sourceShore=new ModernShoreMask(cache.ReadOutput(cache.EnvironmentMaterials.blocks.atlas_path));
            Shader.SetGlobalTexture("_ModernShoreMask",sourceShore.Texture);
            Shader.SetGlobalFloat("_ModernShoreMaskReady",1);
            terrain = new ModernTerrainMesh(root.transform, landMaterial, waterMaterial,sourceShore,
                voxel: Voxels.VoxelModernWorld.Requested) { ViewDistance = 650f };
            vegetation = new ModernVegetation(terrain, voxel: Voxels.VoxelModernWorld.Requested);
            entities = new ModernEntityPresenter(root.transform, cache, terrain);
            waterReflection = new ModernWaterReflection(root.transform, waterMaterial);
            nightLighting = new ModernNightLighting(root.transform);
            waterSystem = new ModernWaterSystem(root.transform, terrain, nightLighting, landMaterial);
            ModernEnvironmentSandbox.ApplyLighting(2, moon, skyMaterial, ModernEnvironmentSandbox.DefaultCloudCover);
            if (Voxels.VoxelModernWorld.Requested)
            {
                voxelWorld=root.AddComponent<Voxels.VoxelModernWorld>();
                voxelWorld.Configure(terrain,vegetation,camera,landMaterial,waterSystem,nightLighting,moon,skyMaterial);
                vegetation.ExcludedFootprint=voxelWorld.ExcludesVegetation;
            }
            if (ModernEnvironmentSandbox.Requested)
            {
                buildings=new ModernBuildings(root.transform,terrain,voxel: Voxels.VoxelModernWorld.Requested);
                vegetation.ExcludedFootprint=buildings.ExcludesVegetation;
                sandbox = new ModernEnvironmentSandbox(camera, terrain, waterSystem, nightLighting, moon, skyMaterial,buildings);
            }
            else
            {
                var layout = ModernBuildingLayout.LoadRequested();
                if (layout == null) throw new InvalidOperationException("The installed Level 1 building layout is missing");
                buildings = new ModernBuildings(root.transform, terrain, layout, voxel: Voxels.VoxelModernWorld.Requested);
                vegetation.ExcludedFootprint = (x, z) => buildings.ExcludesVegetation(x, z) ||
                    (voxelWorld?.ExcludesVegetation(x, z) ?? false);
            }
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

        internal void Present(NativeCampaignSession session, EngineSessionTransport.CampaignRenderObservation observation, bool showMap)
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
            var terrainStart = ModernCapture.BeginMeasurement();
            var observedGeneration = session.Generation;
            var mutationSequence = observation.terrain_mutation_sequence == null
                ? session.TerrainState().mutationSequence
                : ulong.Parse(observation.terrain_mutation_sequence, System.Globalization.CultureInfo.InvariantCulture);
            if (generation != observedGeneration) positioned = false;
            var terrainChanged = nativeHeightPlane == null || generation != observedGeneration || terrainSequence != mutationSequence;
            if (terrainChanged) nativeHeightPlane = session.TerrainPlane(NativeSim.TerrainHeight);
            var architectureChanged = false;
            if (sandbox == null && buildings != null)
            {
                var live = session.BuildingPresentations(out var ownersChanged);
                var selectedSlot = voxelWorld?.SelectedBuildingSlot ?? -1;
                if (terrainChanged || ownersChanged || selectedSlot != buildingVoxelSlot)
                {
                    visualHeightPlane = buildings.Reconcile(nativeHeightPlane, live, selectedSlot, out architectureChanged);
                    buildingVoxelSlot = selectedSlot;
                    if (architectureChanged)
                    {
                        nightLighting.SetReviewWindows(buildings.WindowCenters, buildings.WindowNormals);
                        foreach (var footprint in buildings.ChangedFootprints)
                        {
                            var nearest = footprint;
                            nearest.center = new Vector3(camera.transform.position.x + ModernBuildings.WrappedDelta(footprint.center.x, camera.transform.position.x),
                                footprint.center.y, camera.transform.position.z + ModernBuildings.WrappedDelta(footprint.center.z, camera.transform.position.z));
                            for (var dz = -1; dz <= 1; dz++) for (var dx = -1; dx <= 1; dx++) {
                                var copy = nearest; copy.center += new Vector3(dx * 1024, 0, dz * 1024);
                                vegetation.InvalidateFootprint(copy);
                            }
                        }
                        Debug.Log($"MC_BUILDINGS_LIVE generation={observedGeneration} tick={session.Tick} known={buildings.Count} visible={buildings.VisibleCount} voxel_slot={selectedSlot} source_cells=True");
                    }
                }
            }
            else if (terrainChanged)
                visualHeightPlane = buildings != null ? buildings.PrepareTerrain(nativeHeightPlane) : nativeHeightPlane;
            if (terrainChanged || architectureChanged)
            {
                terrain.Update(visualHeightPlane,
                    session.TerrainPlane(NativeSim.TerrainTypes), session.TerrainPlane(NativeSim.TerrainAngle),
                    observedGeneration, session.Tick);
                generation = observedGeneration;
                terrainSequence = mutationSequence;
            }
            ModernCapture.EndMeasurement("modern_terrain_observe", terrainStart);
            voxelWorld?.Observe(session);
            if (sandbox == null && buildings != null)
            {
                var lightMode = voxelWorld?.LightingMode ?? 2;
                if (lightMode != buildingLightMode) {
                    buildings.SetLighting(lightMode);
                    nightLighting.SetWindowLighting(lightMode == 0 ? 0 : lightMode == 1 ? .45f : .8f);
                    buildingLightMode = lightMode;
                }
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
            var actorsStart = ModernCapture.BeginMeasurement();
            if (sandbox == null)
            {
                entities.Present(session, pose.yaw);
                nightLighting.Observe(entities.VisibleRows, session.Generation);
            }
            waterSystem.Observe(session, snapshot);
            ModernCapture.EndMeasurement("modern_actors_present", actorsStart);
            Ready = true;
            if (snapshotCount++ == 0)
                Debug.Log($"MC_MODERN_LIVE generation={generation} tick={session.Tick} terrain_revision={terrainSequence} position={wrapped} native_session={session.Handle}");
            if (measuredStart != 0)
            {
                presentationMilliseconds += (System.Diagnostics.Stopwatch.GetTimestamp() - measuredStart) * 1000.0 / System.Diagnostics.Stopwatch.Frequency;
                if (snapshotCount == 120)
                    Debug.Log($"MC_MODERN_PRESENT_TIMES samples={snapshotCount} mean_ms={presentationMilliseconds / snapshotCount:F2}");
            }
        }

        internal void ObserveEffect(NativeSim.NativeEvent effect) => waterSystem.ObserveEffect(effect);

        internal void Render()
        {
            if (!Ready || disposed) return;
            var renderStart = ModernCapture.BeginMeasurement();
            var blend = 1f - Mathf.Exp(-Time.unscaledDeltaTime * 22f);
            if (sandbox != null) sandbox.Update();
            else camera.transform.SetPositionAndRotation(ModernCapture.ReviewPosition ?? Vector3.Lerp(camera.transform.position, targetPosition, blend),
                    ModernCapture.ReviewPosition.HasValue ? ModernCapture.ReviewRotation : Quaternion.Slerp(camera.transform.rotation, targetRotation, blend));
            if (sandbox == null) entities.ReviewCamera(camera);
            voxelWorld?.Render();
            waterSystem.Update(camera);
            nightLighting.Render(camera);
            terrain.Present(camera.transform.position);
            buildings?.Render(camera.transform.position);
            var vegetationStart=ModernCapture.BeginMeasurement();
            vegetation.Render(camera);
            ModernCapture.EndMeasurement("vegetation_render",vegetationStart);
            var characterRenderStart=ModernCapture.BeginMeasurement();
            if (sandbox == null) entities.Render(camera);
            ModernCapture.EndMeasurement("entities_render",characterRenderStart);
            var reflectionStart = ModernCapture.BeginMeasurement();
            waterReflection.Render(camera, vegetation, terrain);
            ModernCapture.EndMeasurement("water_reflection", reflectionStart);
            ModernCapture.EndMeasurement("modern_render", renderStart);
            if (ModernCapture.Measure && !reportedGeometry && reflectionStart != 0)
            {
                Debug.Log($"MC_MODERN_GEOMETRY range_triangles={terrain.VisibleTriangleCount} detail_copies={terrain.VisibleDetailCopies} far_copies={terrain.VisibleFarCopies} nature_triangles={vegetation.SubmittedTriangleCount} nature_shadow_only_triangles={vegetation.ShadowOnlyTriangleCount} nature_draws={vegetation.DrawCount} nature_detail={vegetation.DetailInstanceCount} nature_intermediate={vegetation.IntermediateInstanceCount} nature_distant={vegetation.DistantInstanceCount} reflections={waterReflection.RenderedFrames} reflection_reuse={waterReflection.ReusedFrames} reflection_invisible={waterReflection.InvisibleFrames}");
                reportedGeometry = true;
            }
        }

        internal void DrawHud(string instruction)
        {
            if (voxelWorld != null) { voxelWorld.Draw(); return; }
            if (sandbox != null) { sandbox.Draw(); return; }
            waterSystem?.DrawReviewLabel();
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
            sandbox?.Dispose();
            buildings?.Dispose();
            waterReflection?.Dispose();
            waterSystem?.Dispose();
            nightLighting?.Dispose();
            vegetation?.Dispose();
            entities?.Dispose();
            terrain?.Dispose();
            sourceShore?.Dispose();
            Shader.SetGlobalFloat("_ModernShoreMaskReady",0);
            UnityEngine.Object.Destroy(root);
            foreach (var material in new[] { landMaterial, waterMaterial, skyMaterial }) UnityEngine.Object.Destroy(material);
            UnityEngine.Object.Destroy(volumeProfile);
            UnityEngine.Object.Destroy(cloudNoise);
            Shader.SetGlobalFloat("_ModernDensityDetail",0);
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
