using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;
using MagicCarpet.Native.Voxels;

namespace MagicCarpet.Native
{
    // The native resident owns movement and lifetime. Playback follows travel.
    internal sealed class ModernVillagerPresenter : IDisposable
    {
        internal const ushort Appearance = 241;
        private readonly ModernCharacterInstance character;
        private readonly float scale;
        private readonly Transform[] recoilBones;
        private readonly Quaternion[][] recoil;
        private VoxelCharacterSkin voxels;
        private bool observed, removed, hasRenderPosition, restartHit;
        private ulong lastTick;
        private int lastLife;
        private byte lastAction;
        private Vector3 lastRenderPosition;
        private float deathElapsed, hitElapsed = float.PositiveInfinity;
        private float movingGrace, recentSpeed;
        private SkinnedMeshRenderer groundRenderer;
        private Mesh groundMesh;
        private readonly List<Vector3> groundVertices = new();
        private int contactStep = -1;
        private ulong contactRevision;
        private float corpseLift;
        internal GameObject Root => character.Root;
        internal bool IsDying { get; private set; }
        internal bool DeathFinished => IsDying && deathElapsed >= character.Asset.Clip("Death").length + .25f;
        internal string CurrentClip => character.CurrentClip;
        internal float ClipTime => character.ClipTime;
        internal int VoxelResolution => voxels != null ? voxels.Resolution : 0;
        internal bool MovingHit { get; private set; }

        internal ModernVillagerPresenter(ModernCharacterAsset asset, Transform parent, float sourceHeight)
        {
            if (asset == null) throw new ArgumentNullException(nameof(asset));
            if (!float.IsFinite(sourceHeight) || sourceHeight <= 0 || !float.IsFinite(asset.height) || asset.height <= 0 ||
                !float.IsFinite(asset.walkSpeed) || asset.walkSpeed <= 0 || !float.IsFinite(asset.runSpeed) || asset.runSpeed <= asset.walkSpeed)
                throw new ArgumentException("Villager requires measured height and Walk/Run travel speeds");
            foreach (var name in new[] { "Idle", "Walk", "Run", "Hit", "Death" }) asset.Clip(name);
            asset.ResolveHitOverlayBones(asset.model != null ? asset.model.transform : null);
            character = new ModernCharacterInstance(asset, parent);
            scale = sourceHeight / asset.height;
            Root.transform.localScale = Vector3.one * scale;
            recoilBones = asset.ResolveHitOverlayBones(Root.transform);
            character.Sample("Idle", 0);
            var neutral = recoilBones.Select(t => t.localRotation).ToArray();
            recoil = new Quaternion[25][];
            for (var i = 0; i < recoil.Length; i++)
            {
                character.Sample("Hit", asset.Clip("Hit").length * i / (recoil.Length - 1));
                recoil[i] = recoilBones.Select((t, bone) => Quaternion.Inverse(neutral[bone]) * t.localRotation).ToArray();
            }
            character.Sample("Idle", 0);
        }

        internal static bool Supports(NativeSim.CampaignCharacterState state) =>
            state.classId == 5 && state.model == 13 && state.renderParameter == Appearance;
        internal static bool SourceDeath(byte action, int life) => life < 0 || action == 108 || action == 109;

        internal void Observe(Vector3 position, float yaw, int life, byte action, ulong tick)
        {
            if (removed) throw new InvalidOperationException("Removed villager cannot accept observations");
            if (observed && tick < lastTick) throw new ArgumentOutOfRangeException(nameof(tick));
            if (observed && tick == lastTick) return;
            if (!float.IsFinite(position.x) || !float.IsFinite(position.y) || !float.IsFinite(position.z) || !float.IsFinite(yaw))
                throw new ArgumentOutOfRangeException(nameof(position));
            if (SourceDeath(action, life)) BeginDeath();
            else if (!IsDying && (observed && life < lastLife || action == 110 && (!observed || lastAction != 110)))
            {
                hitElapsed = 0;
                restartHit = true;
            }
            observed = true;
            lastTick = tick;
            lastLife = life;
            lastAction = action;
        }

        private void BeginDeath()
        {
            if (IsDying) return;
            IsDying = true;
            deathElapsed = 0;
            character.Play("Death", .1f);
        }

        internal void MarkRemoved() { removed = true; BeginDeath(); }
        internal void RebaseTravel() { hasRenderPosition = false; movingGrace = 0; recentSpeed = 0; }

        internal void Render(float dt)
        {
            if (!float.IsFinite(dt) || dt < 0) throw new ArgumentOutOfRangeException(nameof(dt));
            var position = Root.transform.position;
            var delta = hasRenderPosition ? HorizontalDelta(lastRenderPosition, position) : Vector3.zero;
            lastRenderPosition = position;
            hasRenderPosition = true;
            if (IsDying)
            {
                deathElapsed += dt;
                character.Tick(dt);
                return;
            }
            var localTravel = delta.magnitude / scale;
            var localSpeed = dt > .000001f ? localTravel / dt : 0;
            if (localSpeed > .03f) { movingGrace = .08f; recentSpeed = localSpeed; }
            else movingGrace = Mathf.Max(0, movingGrace - dt);
            // Interpolation can hold its endpoint for a render frame before
            // the next native turn. Retain the gait, without advancing feet
            // over distance that the source did not travel.
            var moving = movingGrace > 0;
            var hit = hitElapsed < character.Asset.Clip("Hit").length;
            var clip = moving ? (recentSpeed > character.Asset.walkSpeed * 1.35f ? "Run" : "Walk") : hit ? "Hit" : "Idle";
            if (CurrentClip != clip || clip == "Hit" && restartHit) character.Play(clip, .12f);
            restartHit = false;
            var advance = moving ? localTravel / (clip == "Run" ? character.Asset.runSpeed : character.Asset.walkSpeed) : dt;
            character.Tick(dt, advance);
            MovingHit = moving && hit;
            if (MovingHit)
            {
                var sample = Mathf.Clamp01(hitElapsed / character.Asset.Clip("Hit").length) * (recoil.Length - 1);
                var a = Mathf.FloorToInt(sample);
                var b = Mathf.Min(a + 1, recoil.Length - 1);
                for (var bone = 0; bone < recoilBones.Length; bone++)
                    recoilBones[bone].localRotation *= Quaternion.Slerp(recoil[a][bone], recoil[b][bone], sample - a);
            }
            hitElapsed += dt;
        }

        private static Vector3 HorizontalDelta(Vector3 a, Vector3 b) => new Vector3(
            Mathf.Repeat(b.x - a.x + 512, 1024) - 512, 0, Mathf.Repeat(b.z - a.z + 512, 1024) - 512);

        internal void AttachVoxelSkin(VoxelGoatAsset skin)
        {
            if (voxels != null) throw new InvalidOperationException("Villager voxel skin already attached");
            voxels = new VoxelCharacterSkin(Root, skin);
        }
        internal void SetVoxelDetail(VoxelGoatAsset skin) => voxels.SetDetail(skin);
        internal List<VoxelFragment> SnapshotVoxelFragments() => voxels.SnapshotFragments();
        // Whole-costume support is only needed during the short authored fall.
        // Sample at 30Hz, reuse buffers, and retain the result after settling.
        // The caller places the visual root on terrain before this correction.
        internal void GroundCorpseToTerrain(Func<float, float, float> terrainHeight, ulong terrainRevision)
        {
            if (!IsDying) return;
            var position = Root.transform.position;
            position.y = terrainHeight(position.x, position.z);
            Root.transform.position = position;
            var step = Mathf.FloorToInt(Mathf.Min(deathElapsed, character.Asset.Clip("Death").length) * 30);
            if (step != contactStep || terrainRevision != contactRevision)
            {
                var started = ModernCapture.BeginMeasurement();
                corpseLift = Mathf.Max(0, .002f - SurfaceGroundClearance(terrainHeight));
                contactStep = step;
                contactRevision = terrainRevision;
                ModernCapture.EndMeasurement("villager_corpse_ground", started);
            }
            Root.transform.position = position + Vector3.up * corpseLift;
        }

        // Actual posed surface measurement, including the separate robe/shoes.
        // Living characters only use this in an opt-in diagnostic capture.
        internal float SurfaceGroundClearance(Func<float, float, float> terrainHeight)
        {
            groundRenderer ??= voxels != null ? voxels.Source : Root.GetComponentsInChildren<SkinnedMeshRenderer>(true).Single();
            groundMesh ??= new Mesh { name = "Villager costume ground sample" };
            groundRenderer.BakeMesh(groundMesh, true);
            groundMesh.GetVertices(groundVertices);
            var minimum = float.PositiveInfinity;
            foreach (var point in groundVertices)
            {
                var world = groundRenderer.transform.TransformPoint(point);
                var ground = terrainHeight(world.x, world.z);
                if (!float.IsFinite(ground)) throw new InvalidOperationException("Non-finite villager terrain height");
                minimum = Mathf.Min(minimum, world.y - ground);
            }
            return minimum;
        }
        public void Dispose()
        {
            if (groundMesh != null)
            {
                if (Application.isPlaying) UnityEngine.Object.Destroy(groundMesh);
                else UnityEngine.Object.DestroyImmediate(groundMesh);
            }
            voxels?.Dispose(); character.Dispose();
        }
    }
}
