using System;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    // Bounded cosmetic pools. No sound, damage, collisions or simulation RNG are emitted here.
    internal sealed class ModernWaterEffects : IDisposable
    {
        private readonly GameObject root;
        private readonly ParticleSystem spray, ember;
        private readonly ModernSteamVolumePool steam;
        private readonly Material sprayMaterial, emberMaterial;
        private uint seed = 314159;
        internal Vector3? LatestCloudCenter => steam.LatestCenter;

        internal ModernWaterEffects(Transform parent, Texture pattern)
        {
            root = new GameObject("Water spray and condensation");
            root.transform.SetParent(parent, false);
            var shader = Resources.Load<Shader>("ModernWaterMist");
            if (shader == null) throw new InvalidOperationException("Missing water particle shader");
            sprayMaterial = new Material(shader); sprayMaterial.SetTexture("_Pattern", pattern);
            emberMaterial = new Material(shader); emberMaterial.SetTexture("_Pattern", pattern); emberMaterial.SetFloat("_Glow", 1);
            spray = Create("Ballistic droplets", sprayMaterial, 1200, .75f, false);
            var args = Environment.GetCommandLineArgs();
            var grid = 48;
            for (var i = 0; i + 1 < args.Length; i++)
                if (args[i] == "-mc-steam-resolution" && !int.TryParse(args[i + 1], out grid))
                    throw new ArgumentException("Steam resolution must be 32, 48 or 64");
            steam = new ModernSteamVolumePool(parent, grid);
            ember = Create("Review projectile light", emberMaterial, 96, 0f, false);
        }

        private ParticleSystem Create(string name, Material material, int capacity, float gravity, bool expands)
        {
            var node = new GameObject(name); node.transform.SetParent(root.transform, false);
            var system = node.AddComponent<ParticleSystem>();
            system.Stop(true, ParticleSystemStopBehavior.StopEmittingAndClear);
            system.useAutoRandomSeed = false; system.randomSeed = seed++;
            var main = system.main;
            main.playOnAwake = false; main.loop = true; main.maxParticles = capacity;
            main.simulationSpace = ParticleSystemSimulationSpace.World;
            main.gravityModifier = gravity; main.useUnscaledTime = true;
            main.cullingMode = ParticleSystemCullingMode.AlwaysSimulate;
            var emission = system.emission; emission.enabled = false;
            var shape = system.shape; shape.enabled = false;
            var color = system.colorOverLifetime; color.enabled = true;
            var gradient = new Gradient();
            gradient.SetKeys(new[] { new GradientColorKey(Color.white, 0), new GradientColorKey(Color.white, 1) },
                new[] { new GradientAlphaKey(0, 0), new GradientAlphaKey(1, .08f), new GradientAlphaKey(.65f, .5f), new GradientAlphaKey(0, 1) });
            color.color = gradient;
            if (expands)
            {
                var size = system.sizeOverLifetime; size.enabled = true;
                size.size = new ParticleSystem.MinMaxCurve(1f, AnimationCurve.Linear(0, .4f, 1, 1.8f));
            }
            var renderer = node.GetComponent<ParticleSystemRenderer>();
            renderer.sharedMaterial = material; renderer.shadowCastingMode = ShadowCastingMode.Off;
            renderer.receiveShadows = false; renderer.sortMode = ParticleSystemSortMode.Distance;
            renderer.maxParticleSize = .18f;
            system.Play();
            return system;
        }

        internal void Update(Vector2 wind, Vector3 viewer)
        {
            steam.Update(Mathf.Clamp(Time.unscaledDeltaTime, 0, .25f), wind);
            steam.PlaceForView(viewer);
            var sprayForce = spray.forceOverLifetime; sprayForce.enabled = true;
            sprayForce.space = ParticleSystemSimulationSpace.World;
            sprayForce.x = wind.x * .05f; sprayForce.z = wind.y * .05f;
        }

        private float Random01()
        {
            seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5;
            return (seed & 0xffffff) / 16777216f;
        }

        internal void Splash(Vector3 position, Vector2 wind, bool condensation, float strength = 1f)
        {
            strength = Mathf.Clamp(strength, .2f, 2f);
            for (var i = 0; i < 52 * strength; i++)
            {
                var angle = Random01() * Mathf.PI * 2;
                var radial = 1.5f + Random01() * 3;
                var velocity = new Vector3(Mathf.Cos(angle) * radial, 2.2f + Random01() * 4, Mathf.Sin(angle) * radial) * strength;
                spray.Emit(new ParticleSystem.EmitParams {
                    position = position + new Vector3(0, .12f, 0), velocity = velocity,
                    startLifetime = .7f + Random01() * .65f, startSize = .035f + Random01() * .09f,
                    startColor = new Color(.65f, .78f, .84f, .8f), randomSeed = ++seed
                }, 1);
            }
            if (condensation) Condensation(position, wind, ModernFireballEnergy.Standard);
        }

        internal void Quench(Vector3 position, Vector2 wind, ModernFireballEnergy energy)
        {
            var scale = Mathf.Clamp((float)Math.Pow(energy.Joules / ModernFireballEnergy.ReferenceJoules, 1.0 / 3), .2f, 2f);
            Splash(position, wind, false, scale);
            Condensation(position, wind, energy);
        }

        private void Condensation(Vector3 position, Vector2 wind, ModernFireballEnergy energy)
            => steam.Quench(position, energy);

        internal void DisturbClouds(Vector3 from, Vector3 to, float dt) => steam.Disturb(from, to, dt);

        internal void ReviewFireball(Vector3 position, Vector3 velocity, float lifetime)
        {
            ember.Emit(new ParticleSystem.EmitParams { position = position, velocity = velocity,
                startLifetime = lifetime, startSize = .65f, startColor = new Color(1, .24f, .024f, 1), randomSeed = ++seed }, 1);
        }

        internal void Clear() { spray.Clear(); ember.Clear(); steam.Reset(); }
        public void Dispose()
        {
            steam?.Dispose();
            UnityEngine.Object.Destroy(root);
            UnityEngine.Object.Destroy(sprayMaterial); UnityEngine.Object.Destroy(emberMaterial);
        }
    }
}
