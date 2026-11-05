using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using UnityEngine;

namespace MagicCarpet.Native
{
    // Composes cosmetic weather and water once per main frame. Reflection cameras only
    // consume the resulting fields. All gameplay and prepared audio remain Rust-owned.
    internal sealed class ModernWaterSystem : IDisposable
    {
        private readonly ModernTerrainMesh terrain;
        private readonly ModernWaterCoast coast;
        private readonly ModernWind wind;
        private readonly ModernWaterSpectrum spectrum;
        private readonly ModernWaterInteractions interactions;
        private readonly ModernWaterEffects effects;
        private readonly Texture2D pattern;
        private readonly NativeSim.NativeEvent[] pendingSplashes = new NativeSim.NativeEvent[16];
        private int splashCount;
        private readonly Dictionary<ushort, Vector3> lastFlyer = new();
        private readonly HashSet<ushort> liveFlyers = new();
        private readonly List<ushort> removed = new();
        private readonly bool review;
        private readonly float reviewDelay = 8;
        private ulong generation = ulong.MaxValue, lastTick = ulong.MaxValue, coastRevision = ulong.MaxValue;
        private float time, bornAt = -1, lastObservationTime, nextShoreSpray, lastReviewTime;
        private Vector3 reviewCenter, lastReviewWake, viewPosition;
        private int lastShot = -1;
        private bool disposed;

        internal ModernWaterSystem(Transform parent, ModernTerrainMesh terrain)
        {
            this.terrain = terrain;
            var args = Environment.GetCommandLineArgs();
            review = args.Contains("-mc-water-review");
            var delay = Array.IndexOf(args, "-mc-water-review-start-delay");
            if (delay >= 0 && delay + 1 < args.Length)
                reviewDelay = Mathf.Clamp(float.Parse(args[delay + 1], CultureInfo.InvariantCulture), 0, 60);
            try
            {
                coast = new ModernWaterCoast(); wind = new ModernWind();
                spectrum = new ModernWaterSpectrum(); interactions = new ModernWaterInteractions();
                pattern = CreatePattern(); effects = new ModernWaterEffects(parent, pattern);
                Shader.SetGlobalTexture("_WaterPattern", pattern);
                Shader.SetGlobalTexture("_WaterCoast", coast.Texture);
            }
            catch { Dispose(); throw; }
        }

        internal void Observe(NativeCampaignSession session, EngineSessionTransport.CampaignSnapshotValue snapshot)
        {
            if (session.Generation != generation)
            {
                generation = session.Generation; lastTick = session.Tick; splashCount = 0;
                lastFlyer.Clear(); effects.Clear(); interactions.Reset();
                lastObservationTime = Time.unscaledTime;
                return; // Restores begin at the head; historical impacts are not replayed.
            }
            if (lastTick == session.Tick) return;
            lastTick = session.Tick;
            // Swept stamps bridge these bounded source observations; avoid serializing
            // the full actor arena every rendered/simulation frame just for downwash.
            if (splashCount == 0 && Time.unscaledTime - lastObservationTime < .15f) return;
            var dt = Mathf.Clamp(Time.unscaledTime - lastObservationTime, 0, .25f);
            lastObservationTime = Time.unscaledTime;
            // Source class3/model0 is the player, model1 the rival wizard family. The source
            // presentation still owns identities and positions; this only adds downwash.
            var actors = session.Entities(); liveFlyers.Clear();
            foreach (var actor in actors)
            {
                if (actor.active == 0 || actor.classId != 3 || actor.model > 1) continue;
                var position = new Vector3(actor.x / 64f, actor.z / 64f, actor.y / 64f);
                liveFlyers.Add(actor.entityIndex);
                if (lastFlyer.TryGetValue(actor.entityIndex, out var previous))
                {
                    var altitude = Mathf.Max(0, position.y);
                    var influence = Mathf.Exp(-altitude * .28f);
                    if (influence > .02f && terrain.IsWater(position.x, position.z))
                        interactions.AddWake(previous, position, 2.2f + altitude * .15f, dt * influence * 5);
                }
                lastFlyer[actor.entityIndex] = position;
            }
            removed.Clear(); foreach (var key in lastFlyer.Keys) if (!liveFlyers.Contains(key)) removed.Add(key);
            foreach (var key in removed) lastFlyer.Remove(key);
            // Reuse the already drained, validated events. Sound27 on a live model5
            // splash is a real source splash update, not a guessed projectile disappearance.
            // This adds droplets/ripples only; a dedicated contact event is still needed
            // to distinguish hot fireball condensation from ordinary water contacts.
            for (var i = 0; i < splashCount; i++)
            {
                var e = pendingSplashes[i];
                foreach (var actor in actors)
                {
                    if (actor.entityIndex != e.entityIndex || actor.classId != 10 || actor.model != 5) continue;
                    var p = new Vector3(actor.x / 64f, .1f, actor.y / 64f);
                    p.x = viewPosition.x + Mathf.Repeat(p.x - viewPosition.x + 512, 1024) - 512;
                    p.z = viewPosition.z + Mathf.Repeat(p.z - viewPosition.z + 512, 1024) - 512;
                    if (terrain.IsWater(p.x, p.z)) { interactions.AddImpulse(p, 2, 1); effects.Splash(p, wind.Velocity, false, .3f); }
                    break;
                }
            }
            splashCount = 0;
        }

        internal void ObserveEffect(NativeSim.NativeEvent effect)
        {
            if (effect.generation == generation && effect.kind == NativeSim.CampaignEffectFlight
                && effect.code == 1 && effect.value0 == 27 && splashCount < pendingSplashes.Length)
                pendingSplashes[splashCount++] = effect;
        }

        internal void Update(Camera camera)
        {
            var dt = Mathf.Clamp(Time.unscaledDeltaTime, 0, .25f);
            time += dt;
            if (coastRevision != terrain.Revision)
            {
                coast.Update(terrain); coastRevision = terrain.Revision; interactions.Reset();
            }
            if (bornAt < 0)
            {
                bornAt = time;
                reviewCenter = FindReviewWater();
                Debug.Log($"MC_WATER_READY fft=2x256 local=256 coast=256 review={review} center={reviewCenter} presentation_only=True");
            }
            if (review) Review(camera, Mathf.Max(0, time - bornAt - reviewDelay), dt);
            viewPosition = camera.transform.position;
            wind.Update(dt, time);
            Shader.SetGlobalVector("_ModernWind", new Vector4(wind.Velocity.x, wind.Velocity.y, wind.Velocity.magnitude / 8f, time));
            Shader.SetGlobalVector("_ModernWindOffset", new Vector4(wind.Offset.x, wind.Offset.y, 0, 0));
            Shader.SetGlobalVector("_ModernCloudWindOffset", new Vector4(wind.CloudOffset.x, wind.CloudOffset.y, 0, 0));
            Shader.SetGlobalFloat("_WaterTime", time);
            var started = ModernCapture.BeginMeasurement();
            spectrum.Update(time, wind.Velocity, dt);
            interactions.Update(dt, camera.transform.position, wind.Velocity, coast.Texture, spectrum.NormalFoam, time);
            ModernCapture.EndMeasurement("water_simulation_submit", started);
            Shader.SetGlobalTexture("_WaterDisplacement", spectrum.Displacement);
            Shader.SetGlobalTexture("_WaterNormalFoam", spectrum.NormalFoam);
            Shader.SetGlobalTexture("_WaterDetailNormal", spectrum.DetailNormalFoam);
            Shader.SetGlobalTexture("_WaterLocal", interactions.State);
            Shader.SetGlobalVector("_WaterLocalRect", interactions.WorldRect);
            effects.Update(wind.Velocity);
            // A bounded near-coast spray sample, masked by exposure and incoming phase.
            if (time >= nextShoreSpray)
            {
                nextShoreSpray = time + .18f;
                var angle = time * 2.399963f;
                var p = camera.transform.position + new Vector3(Mathf.Cos(angle), 0, Mathf.Sin(angle)) * 28;
                var distance = coast.SampleDistance(p.x, p.z);
                var gradient = coast.SampleGradient(p.x, p.z);
                if (distance > .2f && distance < 5 && Vector2.Dot(wind.Velocity.normalized, gradient) < -.15f
                    && Mathf.Sin(distance * .8f + time * 1.25f) > .7f)
                    effects.Splash(new Vector3(p.x, .1f, p.z), wind.Velocity, false, .2f);
            }
        }

        private Vector3 FindReviewWater()
        {
            // Actual source lake near the established NightSpire review camera.
            var best = new Vector3(125, 0, 842); var score = float.NegativeInfinity;
            for (var z = 814; z <= 870; z += 2)
            for (var x = 100; x <= 156; x += 2)
            {
                if (!terrain.IsWater(x, z)) continue;
                var d = coast.SampleDistance(x, z);
                var candidate = Mathf.Min(d, 16) - new Vector2(x - 125, z - 842).magnitude * .3f;
                if (candidate > score) { score = candidate; best = new Vector3(x, 0, z); }
            }
            return best;
        }

        private void Review(Camera camera, float elapsed, float dt)
        {
            wind.SetTarget(elapsed < 8 ? new Vector2(5, 2) : new Vector2(13, -6));
            var angle = (-145f + Mathf.Min(elapsed, 28) * 1.3f) * Mathf.Deg2Rad;
            var approach = Mathf.SmoothStep(0, 1, Mathf.InverseLerp(14, 22, elapsed));
            var radius = Mathf.Lerp(25, 15, approach);
            var position = reviewCenter + new Vector3(Mathf.Cos(angle) * radius, Mathf.Lerp(6.5f, 3.6f, approach), Mathf.Sin(angle) * radius);
            position.y = Mathf.Max(position.y, terrain.SampleHeight(position.x, position.z) + 2);
            camera.transform.SetPositionAndRotation(position, Quaternion.LookRotation(reviewCenter + Vector3.up * .3f - position));
            if (elapsed > 2)
            {
                var phase = (elapsed - 2) * .45f;
                var wake = reviewCenter + new Vector3(Mathf.Sin(phase) * 14, 1.5f, Mathf.Cos(phase) * 5);
                if (lastReviewTime > 0 && terrain.IsWater(wake.x, wake.z))
                    interactions.AddWake(lastReviewWake, wake, 2.5f, dt * 7);
                lastReviewWake = wake;
            }
            var shot = Mathf.FloorToInt((elapsed - 10) / 7);
            if (shot >= 0 && shot != lastShot)
            {
                lastShot = shot;
                var impact = reviewCenter + new Vector3((shot % 3 - 1) * 5, .2f, 0);
                effects.ReviewFireball(impact + new Vector3(-8, 14, -5), new Vector3(8, -14, 5), 1);
            }
            if (shot >= 0 && elapsed - (10 + shot * 7) >= 1 && lastReviewTime - (10 + shot * 7) < 1)
            {
                var impact = reviewCenter + new Vector3((shot % 3 - 1) * 5, .2f, 0);
                if (terrain.IsWater(impact.x, impact.z))
                {
                    interactions.AddImpulse(impact, 3, 6);
                    effects.Splash(impact, wind.Velocity, true, 1.3f);
                    Debug.Log($"MC_WATER_REVIEW_IMPACT staged=True time={elapsed:F2} position={impact}");
                }
            }
            lastReviewTime = elapsed;
        }

        internal void DrawReviewLabel()
        {
            if (!review) return;
            var style = new GUIStyle(GUI.skin.label) { fontSize = 16 };
            style.normal.textColor = new Color(.8f, .9f, 1);
            GUI.Label(new Rect(25, 20, 900, 32), "WATER STUDY  /  scripted wind, downwash and quench effects  /  live NightSpire terrain", style);
        }

        private static Texture2D CreatePattern()
        {
            const int n = 128; var pixels = new Color[n * n];
            // Offline-in-memory periodic filtered texture: no per-fragment hash octaves.
            for (var z = 0; z < n; z++) for (var x = 0; x < n; x++)
            {
                float Noise(int frequency, uint seed)
                {
                    var px = x * frequency / (float)n; var pz = z * frequency / (float)n;
                    var ix = Mathf.FloorToInt(px); var iz = Mathf.FloorToInt(pz);
                    var fx = Mathf.SmoothStep(0, 1, px - ix); var fz = Mathf.SmoothStep(0, 1, pz - iz);
                    float Hash(int a, int b) {
                        var h = (uint)((a % frequency) * 73856093 ^ (b % frequency) * 19349663) ^ seed;
                        h ^= h >> 16; h *= 0x7feb352d; h ^= h >> 15;
                        return (h & 65535) / 65535f;
                    }
                    return Mathf.Lerp(Mathf.Lerp(Hash(ix, iz), Hash(ix + 1, iz), fx), Mathf.Lerp(Hash(ix, iz + 1), Hash(ix + 1, iz + 1), fx), fz);
                }
                // Periodic Voronoi walls for fine bubbles (four texels per cell).
                var cellX = x / 4; var cellZ = z / 4;
                var first = 100f; var second = 100f;
                for (var dz = -1; dz <= 1; dz++) for (var dx = -1; dx <= 1; dx++)
                {
                    uint h = (uint)(((cellX + dx + 32) % 32) * 73856093 ^ ((cellZ + dz + 32) % 32) * 19349663);
                    h ^= h >> 13; h *= 0x7feb352d; h ^= h >> 16;
                    var q = new Vector2(cellX + dx + .2f + (h & 255) / 425f - x / 4f,
                        cellZ + dz + .2f + ((h >> 8) & 255) / 425f - z / 4f);
                    var distance = q.magnitude;
                    if (distance < first) { second = first; first = distance; }
                    else if (distance < second) second = distance;
                }
                var walls = 1 - Mathf.SmoothStep(0, 1, Mathf.InverseLerp(.035f, .16f, second - first));
                pixels[x + z * n] = new Color(Noise(16, 71), walls, Noise(8, 131) * .7f + Noise(32, 51) * .3f, 1);
            }
            var texture = new Texture2D(n, n, TextureFormat.RGBA32, true, true) { name = "Periodic water foam and mist", wrapMode = TextureWrapMode.Repeat, filterMode = FilterMode.Trilinear };
            texture.SetPixels(pixels); texture.Apply(true, true); return texture;
        }

        public void Dispose()
        {
            if (disposed) return; disposed = true;
            effects?.Dispose(); interactions?.Dispose(); spectrum?.Dispose(); coast?.Dispose();
            UnityEngine.Object.Destroy(pattern);
            Shader.SetGlobalVector("_ModernWind", Vector4.zero);
            Shader.SetGlobalVector("_ModernWindOffset", Vector4.zero);
            Shader.SetGlobalVector("_ModernCloudWindOffset", Vector4.zero);
        }
    }
}
