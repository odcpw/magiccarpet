using System;
using System.Collections.Generic;
using UnityEngine;
using MagicCarpet.Native.Voxels;

namespace MagicCarpet.Native
{
    // Native simulation owns the flight transform, damage and lifetime. The
    // caller places the interpolated root once; this class only poses its rig.
    internal sealed class ModernFireflyPresenter : IDisposable
    {
        private readonly ModernCharacterInstance character;
        private VoxelCharacterSkin voxels;
        private bool observed, removed, hasRenderPosition, restartHit, attackPending;
        private ulong lastTick;
        private int lastLife;
        private byte lastAction;
        private Vector3 lastRenderPosition;
        private float movingGrace, hitRemaining, attackRemaining, fadeElapsed;
        private Transform[] liveTransforms;
        private LocalPose[] livePose;
        private readonly List<DeathMaterial> deathMaterials = new List<DeathMaterial>();
        private const float DeathFadeDuration = .18f;

        private sealed class DeathMaterial
        {
            internal Material material;
            internal Color baseColor, emission;
        }

        private struct LocalPose
        {
            internal Vector3 position, scale;
            internal Quaternion rotation;
            internal LocalPose(Transform value)
            {
                position = value.localPosition;
                rotation = value.localRotation;
                scale = value.localScale;
            }
            internal void Apply(Transform value)
            {
                value.localPosition = position;
                value.localRotation = rotation;
                value.localScale = scale;
            }
        }

        internal GameObject Root => character.Root;
        internal bool IsDying { get; private set; }
        internal bool DeathFinished => IsDying && removed && fadeElapsed >= DeathFadeDuration;
        internal string CurrentClip => character.CurrentClip;
        internal float ClipTime => character.ClipTime;
        internal int VoxelResolution => voxels != null ? voxels.Resolution : 0;
        // Native position is the source sprite base; the authored FBX origin
        // is mid-body. The caller adds this vertical offset to its visual root.
        internal float PivotAboveBase { get; }

        internal ModernFireflyPresenter(ModernCharacterAsset asset, Transform parent, float sourceHeight)
        {
            if (asset == null) throw new ArgumentNullException(nameof(asset));
            if (!float.IsFinite(sourceHeight) || sourceHeight <= 0 || !float.IsFinite(asset.height) || asset.height <= 0)
                throw new ArgumentOutOfRangeException(nameof(sourceHeight));
            foreach (var clip in new[] { "Idle", "Flight", "Attack", "Hit", "Death" }) asset.Clip(clip);
            character = new ModernCharacterInstance(asset, parent);
            try
            {
                var scale = sourceHeight / asset.height;
                PivotAboveBase = MeasureIdlePivotAboveBase() * scale;
                Root.transform.localScale = Vector3.one * scale;
            }
            catch
            {
                character.Dispose();
                throw;
            }
        }

        private float MeasureIdlePivotAboveBase()
        {
            // ModernCharacterInstance has already evaluated imported Idle at
            // frame zero. Sample actual skinned surfaces in the root frame,
            // before source scale or world placement, rather than renderer
            // bounds (which can include conservative animated padding).
            var renderers = Root.GetComponentsInChildren<SkinnedMeshRenderer>(true);
            if (renderers.Length == 0) throw new InvalidOperationException("Firefly has no imported skinned surface");
            var mesh = new Mesh { name = "Firefly idle base measurement" };
            var minimum = float.PositiveInfinity;
            try
            {
                foreach (var renderer in renderers)
                {
                    renderer.BakeMesh(mesh, true);
                    foreach (var point in mesh.vertices)
                    {
                        var local = Root.transform.InverseTransformPoint(renderer.transform.TransformPoint(point));
                        if (!float.IsFinite(local.y)) throw new InvalidOperationException("Firefly idle surface is non-finite");
                        minimum = Mathf.Min(minimum, local.y);
                    }
                }
            }
            finally
            {
                if (Application.isPlaying) UnityEngine.Object.Destroy(mesh);
                else UnityEngine.Object.DestroyImmediate(mesh);
            }
            if (!float.IsFinite(minimum) || minimum >= 0)
                throw new InvalidOperationException("Firefly idle surface does not extend below its authored origin");
            return -minimum;
        }

        // Source Firefly pre-kill 156 and kill 157 are distinct from its
        // living actions 153/154/155/159. Exactly zero life is still alive.
        internal static bool SourceDeath(byte action, int life) => life < 0 || action == 156 || action == 157;

        internal void Observe(Vector3 position, float yaw, float pitch, float roll, int life, byte action, byte phase, ulong tick)
        {
            if (removed) throw new InvalidOperationException("A removed Firefly cannot accept observations");
            if (!float.IsFinite(position.x) || !float.IsFinite(position.y) || !float.IsFinite(position.z) ||
                !float.IsFinite(yaw) || !float.IsFinite(pitch) || !float.IsFinite(roll))
                throw new ArgumentOutOfRangeException(nameof(position));
            if (observed && tick < lastTick) throw new ArgumentOutOfRangeException(nameof(tick));
            if (observed && tick == lastTick) return;
            if (SourceDeath(action, life)) BeginDeath();
            else if (!IsDying)
            {
                if (observed && life < lastLife)
                {
                    hitRemaining = character.Asset.Clip("Hit").length;
                    restartHit = true;
                }
                // 154 covers orbit and charge as well as fire. Play a bounded
                // transition once on entry; do not invent a projectile release
                // or loop a firing gesture throughout the source attack state.
                if (action == 154 && (!observed || lastAction != 154)) attackPending = true;
                if (action != 154) { attackPending = false; attackRemaining = 0; }
            }
            // phase is a source animation clock, not an attack or hit event.
            observed = true;
            lastTick = tick;
            lastLife = life;
            lastAction = action;
        }

        private void BeginDeath()
        {
            if (IsDying) return;
            // Capture local rig transforms before Death advances. Restore these
            // only around a Voxel snapshot so fracture uses the last live pose,
            // while the root keeps its current native world placement.
            var all = Root.GetComponentsInChildren<Transform>(true);
            liveTransforms = new Transform[all.Length - 1];
            livePose = new LocalPose[liveTransforms.Length];
            var index = 0;
            foreach (var t in all)
            {
                if (t == Root.transform) continue;
                liveTransforms[index] = t;
                livePose[index] = new LocalPose(t);
                index++;
            }
            IsDying = true;
            if (voxels == null) CloneDeathMaterials();
            hitRemaining = attackRemaining = 0;
            attackPending = false;
            character.Play("Death", .08f);
            if (removed) EnableDeathTransparency();
        }

        private void CloneDeathMaterials()
        {
            var copies = new Dictionary<Material, Material>();
            foreach (var renderer in Root.GetComponentsInChildren<Renderer>(true))
            {
                var materials = renderer.sharedMaterials;
                for (var i = 0; i < materials.Length; i++)
                {
                    var original = materials[i];
                    if (original == null) continue;
                    if (!copies.TryGetValue(original, out var copy))
                    {
                        copy = new Material(original) { name = original.name + " (Firefly death)" };
                        copies.Add(original, copy);
                        deathMaterials.Add(new DeathMaterial {
                            material = copy,
                            baseColor = copy.HasProperty("_BaseColor") ? copy.GetColor("_BaseColor") : Color.white,
                            emission = copy.HasProperty("_EmissionColor") ? copy.GetColor("_EmissionColor") : Color.black
                        });
                    }
                    materials[i] = copy;
                }
                renderer.sharedMaterials = materials;
            }
        }

        private void EnableDeathTransparency()
        {
            foreach (var surface in deathMaterials)
            {
                var material = surface.material;
                material.SetFloat("_Surface", 1);
                material.SetFloat("_Blend", 0);
                material.SetInt("_SrcBlend", (int)UnityEngine.Rendering.BlendMode.SrcAlpha);
                material.SetInt("_DstBlend", (int)UnityEngine.Rendering.BlendMode.OneMinusSrcAlpha);
                material.SetInt("_ZWrite", 0);
                material.SetOverrideTag("RenderType", "Transparent");
                material.renderQueue = (int)UnityEngine.Rendering.RenderQueue.Transparent;
                material.SetShaderPassEnabled("ShadowCaster", false);
                material.EnableKeyword("_SURFACE_TYPE_TRANSPARENT");
            }
        }

        // Removal alone can be culling or positive-life cleanup. Only a real
        // source death observation starts Death or authorizes a fracture.
        internal void MarkRemoved()
        {
            if (removed) return;
            removed = true;
            if (IsDying) EnableDeathTransparency();
        }

        // Caller supplies a matching lifetime-qualified native death journal
        // when the negative-life/pre-kill row was not observed directly.
        internal void ConfirmDeath() => BeginDeath();

        internal void Render(float dt)
        {
            if (!float.IsFinite(dt) || dt < 0) throw new ArgumentOutOfRangeException(nameof(dt));
            var position = Root.transform.position;
            var moved = hasRenderPosition && WrappedDelta(lastRenderPosition, position).sqrMagnitude > 1e-10f;
            lastRenderPosition = position;
            hasRenderPosition = true;
            movingGrace = moved ? .08f : Mathf.Max(0, movingGrace - dt);
            if (IsDying)
            {
                if (removed)
                {
                    fadeElapsed = Mathf.Min(DeathFadeDuration, fadeElapsed + dt);
                    var opacity = 1 - fadeElapsed / DeathFadeDuration;
                    foreach (var surface in deathMaterials)
                    {
                        var color = surface.baseColor;
                        color.a *= opacity;
                        if (surface.material.HasProperty("_BaseColor")) surface.material.SetColor("_BaseColor", color);
                        if (surface.material.HasProperty("_EmissionColor")) surface.material.SetColor("_EmissionColor", surface.emission * opacity);
                    }
                }
                character.Tick(dt);
                return;
            }
            if (hitRemaining > 0)
            {
                character.Play("Hit", .06f, restartHit);
                restartHit = false;
                character.Tick(dt);
                hitRemaining = Mathf.Max(0, hitRemaining - dt);
                return;
            }
            if (attackPending)
            {
                attackPending = false;
                attackRemaining = character.Asset.Clip("Attack").length;
                character.Play("Attack", .06f);
            }
            if (attackRemaining > 0)
            {
                character.Play("Attack", .06f, false);
                character.Tick(dt);
                attackRemaining = Mathf.Max(0, attackRemaining - dt);
                return;
            }
            // Both clips animate the wings on real time; travel detection only
            // chooses the flight attitude and never changes the world pose.
            character.Play(movingGrace > 0 ? "Flight" : "Idle", .08f, false);
            character.Tick(dt);
        }

        private static Vector3 WrappedDelta(Vector3 a, Vector3 b) => new Vector3(
            Mathf.Repeat(b.x - a.x + 512, 1024) - 512, b.y - a.y,
            Mathf.Repeat(b.z - a.z + 512, 1024) - 512);

        internal void AttachVoxelSkin(VoxelGoatAsset skin)
        {
            if (voxels != null) throw new InvalidOperationException("Firefly voxel skin already attached");
            voxels = new VoxelCharacterSkin(Root, skin);
        }
        internal void SetVoxelDetail(VoxelGoatAsset skin)
        {
            if (voxels == null) throw new InvalidOperationException("Firefly voxel skin not attached");
            voxels.SetDetail(skin);
        }
        internal List<VoxelFragment> SnapshotVoxelFragments()
        {
            if (voxels == null) throw new InvalidOperationException("Firefly voxel skin not attached");
            if (!IsDying) return voxels.SnapshotFragments();
            var current = new LocalPose[liveTransforms.Length];
            for (var i = 0; i < liveTransforms.Length; i++) current[i] = new LocalPose(liveTransforms[i]);
            try
            {
                for (var i = 0; i < liveTransforms.Length; i++) livePose[i].Apply(liveTransforms[i]);
                return voxels.SnapshotFragments();
            }
            finally
            {
                for (var i = 0; i < liveTransforms.Length; i++) current[i].Apply(liveTransforms[i]);
            }
        }
        public void Dispose()
        {
            voxels?.Dispose();
            character.Dispose();
            foreach (var surface in deathMaterials)
            {
                if (Application.isPlaying) UnityEngine.Object.Destroy(surface.material);
                else UnityEngine.Object.DestroyImmediate(surface.material);
            }
            deathMaterials.Clear();
        }
    }
}
