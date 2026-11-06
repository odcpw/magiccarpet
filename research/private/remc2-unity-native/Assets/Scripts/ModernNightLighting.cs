using System;
using System.Collections.Generic;
using UnityEngine;

namespace MagicCarpet.Native
{
    // Source-presented fires plus explicitly labeled review emitters. Fixed global
    // light budget, reused by every camera; no per-emitter GameObject or shadow pass.
    internal sealed class ModernNightLighting : IDisposable
    {
        private struct Emitter
        {
            internal Vector3 Position, Velocity;
            internal float Range, Strength, Born, Life;
            internal int Seed;
            internal bool Settlement;
        }
        private readonly List<Emitter> sources = new();
        private readonly List<Emitter> reviewSources = new();
        private readonly Emitter[] transient = new Emitter[16];
        private readonly Vector4[] positions = new Vector4[8], colors = new Vector4[8];
        private readonly Vector4[] glowPositions = new Vector4[8], glowColors = new Vector4[8];
        private readonly float[] scores = new float[8];
        private readonly bool[] houseTorches = new bool[8];
        private readonly int[] seeds = new int[8];
        private int nextTransient, count, glowCount;
        private bool logged;
        private readonly ModernTorchVisuals torches;
        private ulong generation = ulong.MaxValue;

        internal ModernNightLighting(Transform parent) { torches = new ModernTorchVisuals(parent); }

        internal static int SourceKind(NativeSim.CampaignEntityPresentation row)
        {
            // Exact pairs from NightSpire presentation.bin + native sprite bindings.
            if ((row.renderParameterIndex == 340 && row.baseTmap == 463)
                || (row.renderParameterIndex == 42 && row.baseTmap == 55)) return 1; // Fireball
            if (row.renderParameterIndex == 41 && row.baseTmap == 54) return 2; // Village/ambient fire
            if (row.renderParameterIndex >= 228 && row.renderParameterIndex <= 235 && row.baseTmap == 77) return 3;
            if (row.renderParameterIndex == 7 && row.baseTmap == 38) return 4; // Brief flame/impact
            if (row.renderParameterIndex >= 177 && row.renderParameterIndex <= 184
                && row.baseTmap == row.renderParameterIndex - 81) return 5; // Authored exterior house torch
            return 0;
        }

        internal void Observe(IEnumerable<NativeSim.CampaignEntityPresentation> rows, ulong currentGeneration)
        {
            if (generation != currentGeneration)
            {
                generation = currentGeneration; Array.Clear(transient, 0, transient.Length);
            }
            sources.Clear();
            foreach (var row in rows)
            {
                var kind = SourceKind(row);
                if (kind == 0) continue;
                sources.Add(new Emitter {
                    Position = new Vector3(row.x / 64f + (kind == 5 ? 2 : 0),
                        row.z / 64f + (kind == 5 ? 1.6f : row.logicalHeight / 128f), row.y / 64f),
                    Range = kind == 1 ? 20 : kind == 2 ? 24 : kind == 5 ? 10 : 14,
                    Strength = kind == 1 ? 5 : kind == 5 ? 1.6f : 3,
                    Seed = row.entityIndex, Settlement = kind == 5
                });
            }
        }

        internal void AddTransient(Vector3 position, Vector3 velocity, float lifetime, float strength, float range = 20)
        {
            transient[nextTransient] = new Emitter { Position = position, Velocity = velocity,
                Born = Time.unscaledTime, Life = Mathf.Max(.01f, lifetime), Strength = strength,
                Range = range, Seed = nextTransient * 31 };
            nextTransient = (nextTransient + 1) % transient.Length;
        }

        internal void SetReviewTorches(Vector3[] positions)
        {
            reviewSources.Clear();
            for (var i=0; i<Mathf.Min(positions.Length,8); i++)
                reviewSources.Add(new Emitter { Position=positions[i],Range=10,Strength=1.6f,
                    Seed=1000+i*31,Settlement=true });
            Debug.Log($"MC_NIGHT_REVIEW_TORCHES count={reviewSources.Count} staged=True");
        }

        internal Vector3? NearestFire(Vector3 camera)
        {
            Vector3? found = null; var best = float.PositiveInfinity;
            for (var i=0; i<sources.Count+reviewSources.Count; i++)
            {
                var source=i<sources.Count ? sources[i] : reviewSources[i-sources.Count];
                if (!source.Settlement) continue;
                var p = Wrapped(source.Position, camera); var d = (p-camera).sqrMagnitude;
                if (d < best) { best = d; found = p; }
            }
            return found;
        }

        internal void Render(Camera camera)
        {
            count = 0; glowCount = 0; Array.Clear(scores, 0, scores.Length);
            var cameraPosition = camera.transform.position; var now = Time.unscaledTime;
            torches.BeginFrame();
            for (var sourceIndex=0; sourceIndex<sources.Count+reviewSources.Count; sourceIndex++)
            {
                var source=sourceIndex<sources.Count ? sources[sourceIndex] : reviewSources[sourceIndex-sources.Count];
                var p = Wrapped(source.Position, cameraPosition);
                Add(source, p, cameraPosition, now, 1);
                if (!source.Settlement || (p-cameraPosition).sqrMagnitude > 600*600) continue;
                // Nearby fires form a locality, not one giant global orange horizon.
                var merged = false;
                for (var k=0; k<glowCount; k++)
                    if ((new Vector3(glowPositions[k].x, glowPositions[k].y, glowPositions[k].z)-p).sqrMagnitude < 50*50)
                    {
                        glowColors[k] = Vector4.Min(new Vector4(.035f,.009f,.0015f,0),glowColors[k]+new Vector4(.004f,.001f,.00015f,0));
                        merged=true; break;
                    }
                if (merged) continue;
                var glowSlot = glowCount;
                if (glowCount == 8)
                {
                    var farthest = (p-cameraPosition).sqrMagnitude;
                    glowSlot = -1;
                    for (var k=0; k<8; k++)
                    {
                        var distance=(new Vector3(glowPositions[k].x,glowPositions[k].y,glowPositions[k].z)-cameraPosition).sqrMagnitude;
                        if (distance > farthest) { farthest=distance; glowSlot=k; }
                    }
                    if (glowSlot < 0) continue;
                }
                else glowCount++;
                glowPositions[glowSlot] = new Vector4(p.x,p.y,p.z,35);
                glowColors[glowSlot] = new Vector4(.004f,.001f,.00015f,0);
            }
            foreach (var source in transient)
            {
                var age = now-source.Born;
                if (source.Life <= 0 || age < 0 || age >= source.Life) continue;
                var fade = Mathf.Clamp01((source.Life-age)*8);
                Add(source, source.Position+source.Velocity*age, cameraPosition, now, fade);
            }
            for (var i=0; i<count; i++)
                if (houseTorches[i]) torches.Add(new Vector3(positions[i].x,positions[i].y,positions[i].z),seeds[i]);
            Shader.SetGlobalInt("_ModernLightCount", count);
            torches.EndFrame();
            Shader.SetGlobalVectorArray("_ModernLightPosition", positions);
            Shader.SetGlobalVectorArray("_ModernLightColor", colors);
            Shader.SetGlobalInt("_ModernGlowCount", glowCount);
            Shader.SetGlobalVectorArray("_ModernGlowPosition", glowPositions);
            Shader.SetGlobalVectorArray("_ModernGlowColor", glowColors);
            if (!logged && sources.Count+reviewSources.Count > 0)
            { Debug.Log($"MC_NIGHT_LIGHTS source_emitters={sources.Count} review_emitters={reviewSources.Count} selected={count} fire_localities={glowCount} shadowless=True"); logged=true; }
        }

        private void Add(Emitter source, Vector3 p, Vector3 camera, float time, float fade)
        {
            var distance = Vector3.Distance(p, camera);
            if (distance > source.Range + 160) return;
            var score = source.Strength*fade/(4+distance*distance);
            var slot = count < 8 ? count++ : 0;
            if (count == 8)
            {
                for (var i=1; i<8; i++) if (scores[i] < scores[slot]) slot=i;
                if (score < scores[slot]) return;
            }
            scores[slot]=score;
            houseTorches[slot]=source.Settlement; seeds[slot]=source.Seed;
            var flicker=1 + .08f*Mathf.Sin(time*9.1f+source.Seed) + .045f*Mathf.Sin(time*17.3f+source.Seed*.73f);
            var strength=source.Strength*fade*flicker;
            positions[slot]=new Vector4(p.x,p.y,p.z,source.Range);
            // Unflickered strength lets volume light selection stay stable even
            // when two nearby lamps have similar instantaneous intensities.
            colors[slot]=new Vector4(strength,strength*.40f,strength*.10f,source.Strength*fade);
        }

        private static Vector3 Wrapped(Vector3 p, Vector3 camera) => new Vector3(
            camera.x+Mathf.Repeat(p.x-camera.x+512,1024)-512,p.y,
            camera.z+Mathf.Repeat(p.z-camera.z+512,1024)-512);

        public void Dispose()
        { torches.Dispose(); Shader.SetGlobalInt("_ModernLightCount",0); Shader.SetGlobalInt("_ModernGlowCount",0); }
    }
}
