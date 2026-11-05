using System;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    // Bounded cosmetic pools. No sound, damage, collisions or simulation RNG are emitted here.
    internal sealed class ModernWaterEffects : IDisposable
    {
        private readonly GameObject root;
        private readonly ParticleSystem spray, mist, ember;
        private readonly Material sprayMaterial, mistMaterial, emberMaterial;
        private uint seed = 314159;

        internal ModernWaterEffects(Transform parent, Texture pattern)
        {
            root = new GameObject("Water spray and condensation");
            root.transform.SetParent(parent, false);
            var shader = Resources.Load<Shader>("ModernWaterMist");
            if (shader == null) throw new InvalidOperationException("Missing water particle shader");
            sprayMaterial = new Material(shader); sprayMaterial.SetTexture("_Pattern", pattern);
            mistMaterial = new Material(shader); mistMaterial.SetTexture("_Pattern", pattern); mistMaterial.SetFloat("_Mist", 1);
            emberMaterial = new Material(shader); emberMaterial.SetTexture("_Pattern", pattern); emberMaterial.SetFloat("_Glow", 1);
            spray = Create("Ballistic droplets", sprayMaterial, 1200, .75f, false);
            mist = Create("Wind advected condensation", mistMaterial, 240, -.025f, true);
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

        internal void Update(Vector2 wind)
        {
            var force = mist.forceOverLifetime; force.enabled = true; force.space = ParticleSystemSimulationSpace.World;
            force.x = wind.x * .12f; force.z = wind.y * .12f;
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
            if (!condensation) return;
            for (var i = 0; i < 22; i++)
            {
                var angle = Random01() * Mathf.PI * 2; var radius = Random01() * 1.2f;
                mist.Emit(new ParticleSystem.EmitParams {
                    position = position + new Vector3(Mathf.Cos(angle) * radius, .3f + Random01() * .35f, Mathf.Sin(angle) * radius),
                    velocity = new Vector3(wind.x * .1f, 1 + Random01() * 1.1f, wind.y * .1f),
                    startLifetime = 2.2f + Random01() * 1.8f, startSize = 1.2f + Random01() * 1.6f,
                    rotation = Random01() * 360f,
                    startColor = new Color(.7f, .78f, .82f, .3f), randomSeed = ++seed
                }, 1);
            }
        }

        internal void ReviewFireball(Vector3 position, Vector3 velocity, float lifetime)
        {
            ember.Emit(new ParticleSystem.EmitParams { position = position, velocity = velocity,
                startLifetime = lifetime, startSize = .65f, startColor = new Color(1, .24f, .024f, 1), randomSeed = ++seed }, 1);
        }

        internal void Clear() { spray.Clear(); mist.Clear(); ember.Clear(); }
        public void Dispose()
        {
            UnityEngine.Object.Destroy(root);
            UnityEngine.Object.Destroy(sprayMaterial); UnityEngine.Object.Destroy(mistMaterial); UnityEngine.Object.Destroy(emberMaterial);
        }
    }
}
