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
        public void Dispose() { voxels?.Dispose(); character.Dispose(); }
    }
}
