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
        private readonly ModernWaterHeightProbe heightProbe;
        private readonly ModernWaterEffects effects;
        private readonly ModernWaterBed bed;
        private readonly Texture2D pattern;
        private readonly ModernNightLighting lighting;
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
        private Vector3 previousReviewFlight;
        private bool reviewFlying;
        private readonly bool steamReview;
        private bool disposed;
        private int sandboxWind = -1;
        internal Vector2 WindVelocity => wind.Velocity;
        internal Vector3? CloudCenter => effects.LatestCloudCenter;

        // Sandbox flight stays above the observed neighborhood; unavailable,
        // stale, distant or revision-mismatched readbacks use the old full bound.
        // Broad-wave evolution padding is empirical, not a physics collision guarantee.
        internal float CameraClearance(float x,float z)
        {
            var d=Mathf.Max(0,coast.SampleDistance(x,z));
            var broad=Mathf.SmoothStep(0,1,Mathf.Clamp01(d/12));
            var local=Mathf.SmoothStep(0,1,Mathf.Clamp01(d/2));
            var fallback=Mathf.Max(1.2f,.02f+.65f+ModernWaterSpectrum.MaximumDisplacement*broad
                +2*local+.15f*local*Mathf.Exp(-d*.18f));
            if (heightProbe != null && heightProbe.TryClearance(new Vector2(x,z),Time.realtimeSinceStartup,terrain.Revision,out var sampled))
                return Mathf.Min(fallback,sampled);
            return fallback;
        }

        internal void SetSandboxWind(int mode)
        {
            sandboxWind = Mathf.Clamp(mode, 0, 2);
            if (sandboxWind == 1) wind.ResumeWeather();
        }

        internal bool SandboxImpact(Vector3 position, bool hot)
        {
            if (!terrain.IsWater(position.x, position.z)) return false;
            position.y = .15f;
            interactions.AddImpulse(position, hot ? 3 : 2, hot ? 6 : 2);
            if (hot) effects.Quench(position, wind.Velocity, ModernFireballEnergy.Standard);
            else effects.Splash(position, wind.Velocity, false, .5f);
            Debug.Log($"MC_SANDBOX_IMPACT hot={hot} position={position} presentation_only=True");
            return true;
        }

        internal void SandboxSweep(Vector3 previous, Vector3 position, float dt)
        {
            effects.DisturbClouds(previous, position, dt);
            if ((position-previous).sqrMagnitude > 32*32 || !terrain.IsWater(position.x,position.z)) return;
            var influence = Mathf.Exp(-Mathf.Max(0,position.y)*.28f);
            if (influence > .02f) interactions.AddWake(previous,position,2.2f+position.y*.15f,dt*influence*5);
        }

        internal void ClearSandboxEffects() { heightProbe?.Invalidate(); effects.Clear(); interactions.Reset(); }

        internal ModernWaterSystem(Transform parent, ModernTerrainMesh terrain, ModernNightLighting lighting, Material bedMaterial)
        {
            this.terrain = terrain;
            this.lighting = lighting;
            var args = Environment.GetCommandLineArgs();
            steamReview = args.Contains("-mc-steam-review");
            review = args.Contains("-mc-water-review") && !ModernEnvironmentSandbox.Requested;
            var delay = Array.IndexOf(args, "-mc-water-review-start-delay");
            if (delay >= 0 && delay + 1 < args.Length)
                reviewDelay = Mathf.Clamp(float.Parse(args[delay + 1], CultureInfo.InvariantCulture), 0, 60);
            try
            {
                coast = new ModernWaterCoast(); wind = new ModernWind();
                spectrum = new ModernWaterSpectrum(); interactions = new ModernWaterInteractions();
                if (ModernEnvironmentSandbox.Requested)
                {
                    try { heightProbe = new ModernWaterHeightProbe(); }
                    catch (Exception error) { Debug.LogWarning($"MC_WATER_HEIGHT_PROBE fallback=True reason={error.Message}"); }
                }
                pattern = CreatePattern(); effects = new ModernWaterEffects(parent, pattern);
                bed = new ModernWaterBed(parent, bedMaterial);
                Shader.SetGlobalTexture("_WaterPattern", pattern);
                Shader.SetGlobalTexture("_WaterCoast", coast.Texture);
                Shader.SetGlobalFloat("_ModernCoastReady",1);
            }
            catch { Dispose(); throw; }
        }

        internal void Observe(NativeCampaignSession session, EngineSessionTransport.CampaignSnapshotValue snapshot)
        {
            if (session.Generation != generation)
            {
                generation = session.Generation; lastTick = session.Tick; splashCount = 0;
                heightProbe?.Invalidate(); lastFlyer.Clear(); effects.Clear(); interactions.Reset();
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
                    effects.DisturbClouds(previous, position, dt);
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
            var coastStart = ModernCapture.BeginMeasurement();
            if (coastRevision != terrain.Revision)
            {
                heightProbe?.Invalidate(); coast.Update(terrain); coastRevision = terrain.Revision; interactions.Reset();
            }
            ModernCapture.EndMeasurement("water_coast_update", coastStart);
            if (bornAt < 0)
            {
                bornAt = time;
                reviewCenter = FindReviewWater();
                if (review && !steamReview) lighting.SetReviewTorches(FindReviewTorches());
                Debug.Log($"MC_WATER_READY fft=2x256 local=256 coast=256 review={review} center={reviewCenter} presentation_only=True");
            }
            if (review) Review(camera, Mathf.Max(0, time - bornAt - reviewDelay), dt);
            viewPosition = camera.transform.position;
            var bedStart = ModernCapture.BeginMeasurement();
            bed.Update(terrain,coast,viewPosition);
            ModernCapture.EndMeasurement("water_bed_update", bedStart);
            if (sandboxWind == 0) wind.SetTarget(new Vector2(.7f+.3f*Mathf.Sin(time*.2f),.3f+.2f*Mathf.Cos(time*.17f)));
            else if (sandboxWind == 2) wind.SetTarget(new Vector2(13+4*Mathf.Sin(time*.18f),4+5*Mathf.Cos(time*.13f)));
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
            heightProbe?.Submit(new Vector2(camera.transform.position.x,camera.transform.position.z),
                Time.realtimeSinceStartup,coast.Revision,spectrum.Displacement,coast.Texture,interactions.State,
                interactions.WorldRect,new Vector4(wind.Velocity.x,wind.Velocity.y,wind.Velocity.magnitude/8f,time),time);
            effects.Update(wind.Velocity, camera.transform.position);
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
            if (steamReview)
            {
                var center = new Vector3(124, 0, 842); var bestPoint = center; var bestScore = float.NegativeInfinity;
                for (var z = 740; z <= 940; z += 4) for (var x = 20; x <= 220; x += 4)
                {
                    if (!terrain.IsWater(x,z)) continue;
                    var p = new Vector3(x,0,z);
                    var candidate = Mathf.Min(40,coast.SampleDistance(x,z)) - Vector3.Distance(p,center)*.08f;
                    if (candidate > bestScore) { bestScore = candidate; bestPoint = p; }
                }
                return bestPoint;
            }
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

        private Vector3[] FindReviewTorches()
        {
            var result=new List<Vector3>();
            // This replay has no active houses: explicitly staged exterior-light
            // study on genuine dry terrain, never added to the Rust world.
            var best=Vector3.zero; var score=float.PositiveInfinity;
            for (var z=790; z<900; z+=2) for (var x=70; x<190; x+=2)
            {
                if (terrain.IsWater(x,z)) continue;
                var h=terrain.SampleHeight(x,z);
                var slope=Mathf.Abs(terrain.SampleHeight(x+2,z)-h)+Mathf.Abs(terrain.SampleHeight(x,z+2)-h);
                var candidate=Vector2.Distance(new Vector2(x,z),new Vector2(reviewCenter.x,reviewCenter.z))+slope*8+h*.5f;
                if (candidate < score) { score=candidate; best=new Vector3(x,h,z); }
            }
            if (float.IsPositiveInfinity(score)) return result.ToArray();
            foreach (var offset in new[] { Vector3.zero,new Vector3(3,0,2),new Vector3(-3,0,3) })
            {
                var p=best+offset;
                if (!terrain.IsWater(p.x,p.z)) result.Add(new Vector3(p.x,terrain.SampleHeight(p.x,p.z)+1.6f,p.z));
            }
            return result.ToArray();
        }

        private void Review(Camera camera, float elapsed, float dt)
        {
            if (steamReview) { ReviewSteam(camera, elapsed, dt); return; }
            // Review uses the same autonomous weather as ordinary play.
            var angle = (-145f + Mathf.Min(elapsed, 28) * 1.3f) * Mathf.Deg2Rad;
            var approach = Mathf.SmoothStep(0, 1, Mathf.InverseLerp(14, 22, elapsed));
            var radius = Mathf.Lerp(25, 15, approach);
            var position = reviewCenter + new Vector3(Mathf.Cos(angle) * radius, Mathf.Lerp(6.5f, 3.6f, approach), Mathf.Sin(angle) * radius);
            position.y = Mathf.Max(position.y, terrain.SampleHeight(position.x, position.z) + 2);
            camera.transform.SetPositionAndRotation(position, Quaternion.LookRotation(reviewCenter + Vector3.up * .3f - position));
            if (elapsed >= 27 && elapsed < 33 && effects.LatestCloudCenter.HasValue)
            {
                var center=effects.LatestCloudCenter.Value;
                var direction=new Vector3(.8f,0,.6f);
                var travel=Mathf.SmoothStep(-12,6,Mathf.InverseLerp(27,32,elapsed));
                var p=center+direction*travel;
                p.y=Mathf.Max(p.y,terrain.SampleHeight(p.x,p.z)+1.2f);
                camera.transform.SetPositionAndRotation(p,Quaternion.LookRotation(direction));
                if (reviewFlying) effects.DisturbClouds(previousReviewFlight,p,dt);
                previousReviewFlight=p; reviewFlying=true;
            }
            else reviewFlying=false;
            if (elapsed >= 35 && elapsed < 47)
            {
                var moon = new Vector3(-.38f,.48f,.79f).normalized;
                camera.transform.rotation = Quaternion.LookRotation(Quaternion.Euler(0,(elapsed-41)*2,0)*moon);
            }
            else if (elapsed >= 47)
            {
                var fire = lighting.NearestFire(reviewCenter);
                if (fire.HasValue)
                {
                    var orbit = (elapsed-47)*.09f;
                    var p = fire.Value + new Vector3(Mathf.Cos(orbit)*12,3.5f,Mathf.Sin(orbit)*12);
                    p.y = Mathf.Max(p.y,terrain.SampleHeight(p.x,p.z)+2);
                    camera.transform.SetPositionAndRotation(p,Quaternion.LookRotation(fire.Value+Vector3.up-p));
                }
            }
            if (elapsed > 2)
            {
                var phase = (elapsed - 2) * .45f;
                var wake = reviewCenter + new Vector3(Mathf.Sin(phase) * 14, 1.5f, Mathf.Cos(phase) * 5);
                if (lastReviewTime > 0 && terrain.IsWater(wake.x, wake.z))
                    interactions.AddWake(lastReviewWake, wake, 2.5f, dt * 7);
                lastReviewWake = wake;
            }
            var shot = Mathf.FloorToInt((elapsed - 10) / 7);
            if (shot >= 0 && shot != lastShot && elapsed < 30)
            {
                lastShot = shot;
                var impact = reviewCenter + new Vector3((shot % 3 - 1) * 5, .2f, 0);
                effects.ReviewFireball(impact + new Vector3(-8, 14, -5), new Vector3(8, -14, 5), 1);
                lighting.AddTransient(impact + new Vector3(-8,14,-5),new Vector3(8,-14,5),1,8);
            }
            if (shot >= 0 && elapsed < 30 && elapsed - (10 + shot * 7) >= 1 && lastReviewTime - (10 + shot * 7) < 1)
            {
                var impact = reviewCenter + new Vector3((shot % 3 - 1) * 5, .2f, 0);
                if (terrain.IsWater(impact.x, impact.z))
                {
                    interactions.AddImpulse(impact, 3, 6);
                    var energy = ModernFireballEnergy.Standard;
                    effects.Quench(impact, wind.SampleVelocity(new Vector2(impact.x,impact.z),time), energy);
                    lighting.AddTransient(impact+Vector3.up*.6f,Vector3.zero,.18f,6,14);
                    Debug.Log($"MC_WATER_REVIEW_IMPACT staged=True time={elapsed:F2} position={impact} joules={energy.Joules:F0} vapor_kg_max={energy.VaporizedKilograms:F3} ambient_c=12 humidity=.75 equilibrium_plume=True");
                }
            }
            lastReviewTime = elapsed;
        }

        private void ReviewSteam(Camera camera, float elapsed, float dt)
        {
            // Explicit controlled study; ordinary flight retains autonomous weather.
            wind.SetTarget(new Vector2(.65f + .2f*Mathf.Sin(elapsed*.23f), .25f + .15f*Mathf.Cos(elapsed*.31f)));
            var shot = elapsed >= 4 ? 1 : elapsed >= 2 ? 0 : -1;
            if (shot >= 0 && shot != lastShot)
            {
                lastShot = shot;
                var impact = reviewCenter + new Vector3(shot*.8f,.2f,0);
                interactions.AddImpulse(impact,3,6);
                effects.Quench(impact,wind.Velocity,ModernFireballEnergy.Standard);
                Debug.Log($"MC_STEAM_REVIEW_QUENCH staged=True time={elapsed:F2} position={impact} joules=2000000");
            }
            var center = effects.LatestCloudCenter ?? (reviewCenter + Vector3.up*1.2f);
            var direction = new Vector3(.8f,0,.6f);
            Vector3 position, target;
            if (elapsed >= 11 && elapsed < 16)
            {
                position = center + direction*Mathf.Lerp(-7,7,Mathf.SmoothStep(0,1,(elapsed-11)/5));
                target = position + direction;
                if (reviewFlying) effects.DisturbClouds(previousReviewFlight,position,dt);
                previousReviewFlight=position; reviewFlying=true;
            }
            else
            {
                reviewFlying=false;
                var angle=(elapsed < 11 ? -145+elapsed*4 : 37+(elapsed-16)*8)*Mathf.Deg2Rad;
                var distance=elapsed < 11?8f:6f;
                position=center+new Vector3(Mathf.Cos(angle)*distance,1.5f,Mathf.Sin(angle)*distance);
                target=center;
            }
            position.y=Mathf.Max(position.y,terrain.SampleHeight(position.x,position.z)+.6f);
            camera.transform.SetPositionAndRotation(position,Quaternion.LookRotation(target-position));
            lastReviewTime=elapsed;
        }

        internal void DrawReviewLabel()
        {
            if (!review) return;
            var style = new GUIStyle(GUI.skin.label) { fontSize = 16 };
            style.normal.textColor = new Color(.8f, .9f, 1);
            if (steamReview)
            {
                GUI.Label(new Rect(25,20,1300,32), "STEAM STUDY  /  two staged 2 MJ quenches  /  3D density  /  fly-through and persistent wake", style);
                GUI.Label(new Rect(25,45,1300,32), $"CONTROLLED VARIABLE BREEZE {wind.Velocity.magnitude:F1} m/s  /  12 C, 75% RH  /  moonlit condensation", style);
                return;
            }
            GUI.Label(new Rect(25, 20, 1300, 32), "NIGHTSPIRE STUDY  /  variable wind  /  staged quench, flight pass and torch study", style);
            GUI.Label(new Rect(25, 45, 1300, 32), $"WIND {wind.Velocity.magnitude:F1} m/s   /   vapor upper bound 0.77 kg   /   cooling and condensation in 12 C, 75% RH air", style);
        }

        private static Texture2D CreatePattern()
        {
            const int n = 512, cellPixels = n / 32; var pixels = new Color[n * n];
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
                // The same 32-cell period, with enough texels to resolve rounded
                // bubble walls instead of exposing the source grid in highlights.
                var cellX = x / cellPixels; var cellZ = z / cellPixels;
                var first = 100f; var second = 100f;
                for (var dz = -1; dz <= 1; dz++) for (var dx = -1; dx <= 1; dx++)
                {
                    uint h = (uint)(((cellX + dx + 32) % 32) * 73856093 ^ ((cellZ + dz + 32) % 32) * 19349663);
                    h ^= h >> 13; h *= 0x7feb352d; h ^= h >> 16;
                    var q = new Vector2(cellX + dx + .2f + (h & 255) / 425f - x / (float)cellPixels,
                        cellZ + dz + .2f + ((h >> 8) & 255) / 425f - z / (float)cellPixels);
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
            heightProbe?.Dispose(); effects?.Dispose(); interactions?.Dispose(); spectrum?.Dispose(); coast?.Dispose();
            bed?.Dispose();
            Shader.SetGlobalFloat("_ModernCoastReady",0);
            UnityEngine.Object.Destroy(pattern);
            Shader.SetGlobalVector("_ModernWind", Vector4.zero);
            Shader.SetGlobalVector("_ModernWindOffset", Vector4.zero);
            Shader.SetGlobalVector("_ModernCloudWindOffset", Vector4.zero);
        }
    }
}
