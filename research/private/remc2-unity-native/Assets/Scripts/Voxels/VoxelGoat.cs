using System;
using System.Collections.Generic;
using UnityEngine;

namespace MagicCarpet.Native.Voxels
{
    // Compatibility presenter for the standalone sandbox. Campaign uses the
    // same skin on the source-driven ModernGoatPresenter instead.
    internal sealed class VoxelGoat : IDisposable
    {
        private readonly ModernCharacterInstance character;
        private readonly VoxelCharacterSkin skin;
        private readonly float scale;
        private bool disposed;
        internal GameObject Root => character.Root;
        internal string CurrentClip => character.CurrentClip;
        internal float ClipTime => character.ClipTime;

        internal VoxelGoat(ModernCharacterAsset source, VoxelGoatAsset voxels, Transform parent, float height)
        {
            if (source == null || voxels == null) throw new ArgumentNullException(source == null ? nameof(source) : nameof(voxels));
            if (!float.IsFinite(height) || height <= 0 || !float.IsFinite(source.height) || source.height <= 0)
                throw new ArgumentOutOfRangeException(nameof(height));
            source.Clip("Idle"); source.Clip("Walk"); source.Clip("Sprint");
            scale = height / source.height;
            character = new ModernCharacterInstance(source, parent);
            try
            {
                Root.transform.localScale = Vector3.one * scale;
                skin = new VoxelCharacterSkin(Root, voxels);
            }
            catch { character.Dispose(); throw; }
        }

        internal void Render(float dt, float distance, bool sprint = false)
        {
            if (disposed) throw new ObjectDisposedException(nameof(VoxelGoat));
            if (!float.IsFinite(dt) || dt < 0 || !float.IsFinite(distance) || distance < 0)
                throw new ArgumentOutOfRangeException(nameof(distance));
            if (distance <= .00001f)
            {
                character.Play("Idle", .12f, false);
                character.Tick(dt, dt);
                return;
            }
            var clip = sprint ? "Sprint" : "Walk";
            character.Play(clip, .08f, false);
            var cycleTravel = .16f / (sprint ? .075f : .18f) * scale;
            character.Tick(dt, distance / cycleTravel * character.Asset.Clip(clip).length);
        }

        internal List<VoxelFragment> SnapshotFragments() => skin.SnapshotFragments();

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            skin.Dispose(); character.Dispose();
        }
    }
}
