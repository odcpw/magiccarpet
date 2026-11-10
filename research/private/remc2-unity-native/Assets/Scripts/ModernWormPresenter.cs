using System;
using System.Collections.Generic;
using UnityEngine;
using MagicCarpet.Native.Voxels;

namespace MagicCarpet.Native
{
    // One native head lifetime owns up to sixteen independently placed source
    // children. The caller supplies interpolated Unity transforms; this class
    // never estimates a spline, follows a predecessor, or advances gameplay.
    internal sealed class ModernWormPresenter : IDisposable
    {
        internal readonly struct SegmentPose
        {
            internal readonly ushort slot, sourceId, parentSlot;
            internal readonly uint generation, parentGeneration;
            internal readonly byte partIndex, action;
            internal readonly Vector3 nativePosition;
            internal readonly float yaw, pitch, roll, width, height;
            internal readonly bool active, drawing;
            internal readonly int life;

            internal SegmentPose(ushort slot, ushort sourceId, uint generation,
                ushort parentSlot, uint parentGeneration, byte partIndex,
                Vector3 nativePosition, float yaw, float pitch, float roll,
                float width, float height, bool active, bool drawing, int life, byte action)
            {
                this.slot = slot; this.sourceId = sourceId; this.generation = generation;
                this.parentSlot = parentSlot; this.parentGeneration = parentGeneration;
                this.partIndex = partIndex; this.nativePosition = nativePosition;
                this.yaw = yaw; this.pitch = pitch; this.roll = roll;
                this.width = width; this.height = height; this.active = active;
                this.drawing = drawing; this.life = life; this.action = action;
            }
        }

        private readonly struct LocalPose
        {
            private readonly Vector3 position, scale;
            private readonly Quaternion rotation;
            internal LocalPose(Transform t) { position = t.localPosition; rotation = t.localRotation; scale = t.localScale; }
            internal void Apply(Transform t) { t.localPosition = position; t.localRotation = rotation; t.localScale = scale; }
        }

        private readonly struct WorldPose
        {
            private readonly Vector3 position, scale;
            private readonly Quaternion rotation;
            internal WorldPose(Transform t) { position = t.position; rotation = t.rotation; scale = t.localScale; }
            internal Vector3 Position => position;
            internal void Apply(Transform t) { t.SetPositionAndRotation(position, rotation); t.localScale = scale; }
        }

        private readonly struct ArtSize
        {
            internal readonly float width, height, depth, bottom;
            internal ArtSize(float width, float height, float depth, float bottom)
            { this.width = width; this.height = height; this.depth = depth; this.bottom = bottom; }
        }

        private sealed class Segment : IDisposable
        {
            internal readonly ushort slot, sourceId;
            internal readonly uint generation;
            internal readonly byte partIndex;
            internal readonly GameObject root;
            internal readonly ArtSize modelSize;
            internal Renderer[] renderers;
            internal VoxelCharacterSkin voxels;
            internal WorldPose lastLive;
            internal bool placed, drawing;

            internal Segment(SegmentPose pose, ModernCharacterAsset asset, ArtSize size, Transform parent, VoxelGoatAsset voxel)
            {
                slot = pose.slot; sourceId = pose.sourceId; generation = pose.generation; partIndex = pose.partIndex;
                root = new GameObject("Worm segment " + partIndex);
                try
                {
                    root.transform.SetParent(parent, false);
                    var visual = UnityEngine.Object.Instantiate(asset.model, root.transform);
                    visual.transform.localPosition = Vector3.zero;
                    visual.transform.localRotation = Quaternion.Euler(0, asset.forwardYaw, 0);
                    foreach (var animator in visual.GetComponentsInChildren<Animator>(true)) animator.enabled = false;
                    // The imported segment is a single rigid skinned mesh. A static
                    // sample supplies its bind pose without a graph for each child.
                    asset.Clip("Idle").SampleAnimation(visual, 0);
                    modelSize = size;
                    if (voxel != null) voxels = new VoxelCharacterSkin(root, voxel);
                    renderers = root.GetComponentsInChildren<Renderer>(true);
                    root.SetActive(false);
                }
                catch { voxels?.Dispose(); Destroy(root); throw; }
            }

            public void Dispose()
            {
                voxels?.Dispose();
                if (root != null) root.SetActive(false);
                Destroy(root);
            }
        }

        private const int MaxParts = 16;
        internal const int MaximumSnapshotFragments = 8192;
        private const float DeathHold = .25f;
        private readonly ushort headSlot, headSourceId;
        private readonly uint headGeneration;
        private readonly ModernCharacterInstance head;
        private readonly ModernCharacterAsset segmentAsset;
        private readonly Transform parent;
        private readonly Segment[] parts = new Segment[MaxParts];
        private readonly ArtSize headSize, segmentSize;
        private VoxelGoatAsset segmentVoxel;
        private VoxelCharacterSkin headVoxel;
        private Renderer[] headRenderers;
        private readonly List<Renderer> familyRenderers = new List<Renderer>();
        private bool familyDirty = true, eligible = true;
        private Transform[] liveTransforms;
        private LocalPose[] livePose;
        private WorldPose lastLiveHead;
        private List<VoxelFragment> deathSnapshot;
        private int snapshotLimit;
        private bool observed, removed, placedHead, restartHit, attackPending, disposed;
        private ulong lastTick;
        private int lastLife;
        private byte lastAction;
        private float hitRemaining, attackRemaining, deathElapsed;

        internal GameObject Root => head.Root;
        internal bool IsDying { get; private set; }
        internal bool DeathFinished => IsDying && deathElapsed >= head.Asset.Clip("Death").length + DeathHold;
        internal string CurrentClip => head.CurrentClip;
        internal float ClipTime => head.ClipTime;
        internal bool HasRetainedLivePose => placedHead;
        internal ushort HeadSlot => headSlot;
        internal ushort HeadSourceId => headSourceId;
        internal uint HeadGeneration => headGeneration;
        internal float HeadWidthToHeight => headSize.width / headSize.height;
        internal float HeadDepthToHeight => headSize.depth / headSize.height;
        internal float HeadBottomToHeight => headSize.bottom / headSize.height;
        internal float SegmentWidthToHeight => segmentSize.width / segmentSize.height;
        internal float SegmentDepthToHeight => segmentSize.depth / segmentSize.height;
        internal float SegmentBottomToHeight => segmentSize.bottom / segmentSize.height;
        internal int SegmentCount { get { var count = 0; foreach (var part in parts) if (part != null) count++; return count; } }

        internal ModernWormPresenter(ModernCharacterAsset headAsset, ModernCharacterAsset segmentAsset,
            Transform parent, ushort headSlot, ushort headSourceId, uint headGeneration)
        {
            if (headAsset == null || segmentAsset == null) throw new ArgumentNullException(headAsset == null ? nameof(headAsset) : nameof(segmentAsset));
            if (headAsset.model == null || segmentAsset.model == null || !Positive(headAsset.height) || !Positive(segmentAsset.height))
                throw new InvalidOperationException("Worm assets require models and measured positive heights");
            foreach (var name in new[] { "Idle", "Attack", "Hit", "Death" }) headAsset.Clip(name);
            segmentAsset.Clip("Idle");
            this.headSlot = headSlot; this.headSourceId = headSourceId; this.headGeneration = headGeneration;
            this.segmentAsset = segmentAsset; this.parent = parent;
            head = new ModernCharacterInstance(headAsset, parent);
            try { headSize = ModelSize(head.Root); segmentSize = MeasureSegment(segmentAsset); }
            catch { head.Dispose(); throw; }
            headRenderers = head.Root.GetComponentsInChildren<Renderer>(true);
            lastLiveHead = new WorldPose(head.Root.transform);
        }

        // Membership is independent of view eligibility so hidden actors can
        // be evaluated for re-entry without activating their game objects.
        internal IReadOnlyList<Renderer> FamilyRenderers
        {
            get
            {
                if (!familyDirty) return familyRenderers;
                familyRenderers.Clear();
                if (!removed || IsDying)
                {
                    familyRenderers.AddRange(headRenderers);
                    foreach (var part in parts)
                        if (part != null && part.placed && part.drawing) familyRenderers.AddRange(part.renderers);
                }
                familyDirty = false;
                return familyRenderers;
            }
        }

        internal bool TryGetFamilyBounds(out Bounds bounds)
        {
            bounds = default; var havePoint = false;
            foreach (var renderer in FamilyRenderers)
            {
                if (renderer == null) continue;
                // localBounds and transform are usable even on culled objects;
                // Renderer.bounds can otherwise be stale for inactive skins.
                var localBounds = renderer.localBounds;
                var matrix = renderer.transform.localToWorldMatrix;
                for (var x = -1; x <= 1; x += 2) for (var y = -1; y <= 1; y += 2) for (var z = -1; z <= 1; z += 2)
                {
                    var point = matrix.MultiplyPoint3x4(localBounds.center + Vector3.Scale(localBounds.extents, new Vector3(x, y, z)));
                    if (!havePoint) { bounds = new Bounds(point, Vector3.zero); havePoint = true; }
                    else bounds.Encapsulate(point);
                }
            }
            return havePoint;
        }

        private void ApplyVisibility()
        {
            Root.SetActive(eligible && (!removed || IsDying));
            foreach (var part in parts)
                if (part != null) part.root.SetActive(eligible && (!removed || IsDying) && part.placed && part.drawing);
        }

        internal void SetVisible(bool value) { eligible = value; ApplyVisibility(); }

        internal static bool SourceDeath(byte action, int life) => life < 0 || action == 28 || action == 29;

        // Model-3 head roll is a steering target and its initial pitch copies
        // that heading. Child action232 uses pitch to calculate predecessor
        // spacing, but the original TMAP30/66 renderer does not tilt either
        // sprite from those fields. Source yaw remains each part's own heading.
        internal static Quaternion SourceRotation(short yaw) => Quaternion.Euler(0, 180f - yaw * 360f / 2048f, 0);

        private static bool Positive(float value) => float.IsFinite(value) && value > 0;
        private static bool Finite(Vector3 value) => float.IsFinite(value.x) && float.IsFinite(value.y) && float.IsFinite(value.z);
        private static bool Finite(Quaternion value) => float.IsFinite(value.x) && float.IsFinite(value.y) && float.IsFinite(value.z) && float.IsFinite(value.w)
            && value.x * value.x + value.y * value.y + value.z * value.z + value.w * value.w > 1e-12f;

        private static ArtSize ModelSize(GameObject root)
        {
            var renderers = root.GetComponentsInChildren<SkinnedMeshRenderer>(true);
            if (renderers.Length != 1) throw new InvalidOperationException("Worm part requires exactly one skinned mesh");
            var renderer = renderers[0]; var bounds = renderer.localBounds;
            var low = new Vector3(float.PositiveInfinity, float.PositiveInfinity, float.PositiveInfinity);
            var high = new Vector3(float.NegativeInfinity, float.NegativeInfinity, float.NegativeInfinity);
            for (var x = -1; x <= 1; x += 2) for (var y = -1; y <= 1; y += 2) for (var z = -1; z <= 1; z += 2)
            {
                var corner = bounds.center + Vector3.Scale(bounds.extents, new Vector3(x, y, z));
                var local = root.transform.InverseTransformPoint(renderer.transform.TransformPoint(corner));
                low = Vector3.Min(low, local); high = Vector3.Max(high, local);
            }
            var size = high - low;
            if (!Positive(size.x) || !Positive(size.y) || !Positive(size.z))
                throw new InvalidOperationException("Worm mesh has invalid imported bounds");
            return new ArtSize(size.x, size.y, size.z, low.y);
        }

        private static ArtSize MeasureSegment(ModernCharacterAsset asset)
        {
            var root = new GameObject("Worm segment measurement");
            try
            {
                var visual = UnityEngine.Object.Instantiate(asset.model, root.transform);
                visual.transform.localPosition = Vector3.zero;
                visual.transform.localRotation = Quaternion.Euler(0, asset.forwardYaw, 0);
                foreach (var animator in visual.GetComponentsInChildren<Animator>(true)) animator.enabled = false;
                asset.Clip("Idle").SampleAnimation(visual, 0);
                return ModelSize(root);
            }
            finally { root.SetActive(false); Destroy(root); }
        }

        private static void ValidatePlacement(Vector3 position, Quaternion rotation, float width, float height, float depth)
        {
            if (!Finite(position) || !Finite(rotation) || !Positive(width) || !Positive(height) || !Positive(depth))
                throw new ArgumentOutOfRangeException(nameof(position), "Worm placement requires finite pose and positive dimensions");
        }

        private static void Place(Transform root, Vector3 position, Quaternion rotation, float width, float height, float depth,
            ArtSize artSize)
        {
            ValidatePlacement(position, rotation, width, height, depth);
            root.SetPositionAndRotation(position, rotation);
            root.localScale = new Vector3(width / artSize.width, height / artSize.height, depth / artSize.depth);
        }

        internal void PlaceHead(Vector3 position, Quaternion rotation, float width, float height, float depth)
        {
            Place(Root.transform, position, rotation, width, height, depth, headSize);
            if (observed && !IsDying && !removed) { lastLiveHead = new WorldPose(Root.transform); placedHead = true; }
        }

        // Returns false for a stale/reclaimed slot or an unadmitted part.
        internal bool PlaceSegment(ushort slot, uint generation, Vector3 position, Quaternion rotation, float width, float height, float depth)
        {
            ValidatePlacement(position, rotation, width, height, depth);
            foreach (var part in parts)
            {
                if (part == null || part.slot != slot || part.generation != generation || !part.drawing || IsDying || removed) continue;
                Place(part.root.transform, position, rotation, width, height, depth, part.modelSize);
                if (!part.placed) familyDirty = true;
                part.lastLive = new WorldPose(part.root.transform); part.placed = true;
                part.root.SetActive(eligible && !removed && part.drawing);
                return true;
            }
            return false;
        }

        internal GameObject SegmentRoot(ushort slot, uint generation)
        {
            foreach (var part in parts) if (part != null && part.slot == slot && part.generation == generation) return part.root;
            return null;
        }

        internal void Observe(int life, byte action, ulong tick, IReadOnlyList<SegmentPose> segments)
        {
            if (removed) throw new InvalidOperationException("A removed Worm cannot accept observations");
            if (segments == null) throw new ArgumentNullException(nameof(segments));
            if (observed && tick < lastTick) throw new ArgumentOutOfRangeException(nameof(tick));
            if (observed && tick == lastTick) return;
            if (segments.Count > MaxParts) throw new ArgumentOutOfRangeException(nameof(segments));
            var seen = new bool[MaxParts]; var slots = new HashSet<ushort>();
            foreach (var pose in segments)
            {
                if (pose.partIndex >= MaxParts || pose.slot == headSlot || pose.parentSlot != headSlot || pose.parentGeneration != headGeneration ||
                    seen[pose.partIndex] || !slots.Add(pose.slot))
                    throw new ArgumentException("Worm part has a duplicate or foreign native identity", nameof(segments));
                if (!Finite(pose.nativePosition) || !float.IsFinite(pose.yaw) || !float.IsFinite(pose.pitch) || !float.IsFinite(pose.roll) ||
                    !float.IsFinite(pose.width) || !float.IsFinite(pose.height) ||
                    (pose.active && pose.drawing && pose.action != 29 && pose.life >= 0 && (!Positive(pose.width) || !Positive(pose.height))))
                    throw new ArgumentException("Worm part has invalid native pose or dimensions", nameof(segments));
                seen[pose.partIndex] = true;
            }
            if (SourceDeath(action, life)) BeginDeath();
            else if (!IsDying)
            {
                if (observed && life < lastLife) { hitRemaining = head.Asset.Clip("Hit").length; restartHit = true; }
                if (action == 26 && (!observed || lastAction != 26)) attackPending = true;
                if (action != 26) { attackPending = false; attackRemaining = 0; }
            }
            if (!IsDying)
            {
                var beforeMask = 0; var membershipReplaced = false;
                for (var i = 0; i < parts.Length; i++)
                    if (parts[i] != null && parts[i].placed && parts[i].drawing) beforeMask |= 1 << i;
                foreach (var part in parts) if (part != null) part.drawing = false;
                foreach (var pose in segments)
                {
                    // A live source slot cannot remain retained under an older
                    // part index after native slot reuse.
                    for (var i = 0; i < parts.Length; i++)
                    {
                        var old = parts[i];
                        if (old != null && old.slot == pose.slot && i != pose.partIndex)
                        { old.Dispose(); parts[i] = null; membershipReplaced = true; }
                    }
                    var part = parts[pose.partIndex];
                    if (part != null && (part.slot != pose.slot || part.generation != pose.generation || part.sourceId != pose.sourceId))
                    { part.Dispose(); parts[pose.partIndex] = part = null; membershipReplaced = true; }
                    // A copied action-29 child is not a head death and cannot
                    // replace the last living visual or trigger its fracture.
                    if (!pose.active || !pose.drawing || pose.action == 29 || pose.life < 0) continue;
                    if (part == null) parts[pose.partIndex] = part = new Segment(pose, segmentAsset, segmentSize, parent, segmentVoxel);
                    part.drawing = true;
                }
                var afterMask = 0;
                for (var i = 0; i < parts.Length; i++)
                    if (parts[i] != null && parts[i].placed && parts[i].drawing) afterMask |= 1 << i;
                if (membershipReplaced || beforeMask != afterMask) familyDirty = true;
                ApplyVisibility();
            }
            observed = true; lastTick = tick; lastLife = life; lastAction = action;
        }

        private void BeginDeath()
        {
            if (IsDying) return;
            var all = Root.GetComponentsInChildren<Transform>(true);
            liveTransforms = new Transform[all.Length - 1]; livePose = new LocalPose[liveTransforms.Length];
            var index = 0;
            foreach (var transform in all)
            {
                if (transform == Root.transform) continue;
                liveTransforms[index] = transform; livePose[index] = new LocalPose(transform); index++;
            }
            IsDying = true; deathElapsed = 0; hitRemaining = attackRemaining = 0;
            attackPending = false; head.Play("Death", .08f);
            familyDirty = true; ApplyVisibility();
        }

        internal void ConfirmDeath() => BeginDeath();
        internal void MarkRemoved()
        {
            if (removed) return;
            removed = true;
            familyDirty = true; ApplyVisibility();
        }

        internal void Render(float dt)
        {
            if (!float.IsFinite(dt) || dt < 0) throw new ArgumentOutOfRangeException(nameof(dt));
            if (IsDying) { deathElapsed += dt; head.Tick(dt); return; }
            if (removed) return;
            if (hitRemaining > 0)
            {
                head.Play("Hit", .06f, restartHit); restartHit = false; head.Tick(dt);
                hitRemaining = Mathf.Max(0, hitRemaining - dt); return;
            }
            if (attackPending) { attackPending = false; attackRemaining = head.Asset.Clip("Attack").length; head.Play("Attack", .06f); }
            if (attackRemaining > 0)
            {
                head.Play("Attack", .06f, false); head.Tick(dt); attackRemaining = Mathf.Max(0, attackRemaining - dt); return;
            }
            head.Play("Idle", .08f, false); head.Tick(dt);
        }

        internal void AttachVoxelSkins(VoxelGoatAsset headSkin, VoxelGoatAsset bodySkin)
        {
            if (headVoxel != null || segmentVoxel != null) throw new InvalidOperationException("Worm voxel skins already attached");
            if (headSkin == null || bodySkin == null) throw new ArgumentNullException(headSkin == null ? nameof(headSkin) : nameof(bodySkin));
            try
            {
                headVoxel = new VoxelCharacterSkin(Root, headSkin);
                foreach (var part in parts) if (part != null)
                {
                    part.voxels = new VoxelCharacterSkin(part.root, bodySkin);
                    part.renderers = part.root.GetComponentsInChildren<Renderer>(true);
                }
                segmentVoxel = bodySkin;
                headRenderers = Root.GetComponentsInChildren<Renderer>(true); familyDirty = true;
            }
            catch
            {
                foreach (var part in parts) if (part != null)
                { part.voxels?.Dispose(); part.voxels = null; part.renderers = part.root.GetComponentsInChildren<Renderer>(true); }
                headVoxel?.Dispose(); headVoxel = null;
                headRenderers = Root.GetComponentsInChildren<Renderer>(true); familyDirty = true; throw;
            }
        }

        // One cached, bounded snapshot of the last live head and child poses.
        // The shared death lifecycle owns exactly-once emission and retries.
        internal List<VoxelFragment> SnapshotVoxelFragments(int maxFragments)
        {
            if (!IsDying) throw new InvalidOperationException("A live Worm cannot fracture");
            if (!placedHead) throw new InvalidOperationException("Worm death has no retained live head placement");
            if (maxFragments <= 0 || maxFragments > MaximumSnapshotFragments) throw new ArgumentOutOfRangeException(nameof(maxFragments));
            if (headVoxel == null || segmentVoxel == null) throw new InvalidOperationException("Worm voxel skins are not attached");
            if (deathSnapshot != null)
            {
                if (maxFragments != snapshotLimit) throw new InvalidOperationException("A retained Worm snapshot cannot change its bound");
                return deathSnapshot;
            }
            var currentHead = new WorldPose(Root.transform); var current = new LocalPose[liveTransforms.Length];
            // The source map wraps every 1024 Unity units. Live observations
            // and the current camera can choose different periodic images of
            // the same family. Rebase one common shift after sampling, so
            // joints stay together and voxel rounding is unchanged.
            var displacement = currentHead.Position - lastLiveHead.Position;
            var rebase = new Vector3(Mathf.Round(displacement.x / 1024f) * 1024f, 0,
                Mathf.Round(displacement.z / 1024f) * 1024f);
            for (var i = 0; i < current.Length; i++) current[i] = new LocalPose(liveTransforms[i]);
            var retainedParts = 1; foreach (var part in parts) if (part != null && part.placed && part.drawing && part.voxels != null) retainedParts++;
            if (maxFragments < retainedParts) throw new ArgumentOutOfRangeException(nameof(maxFragments), "Bound must admit at least one cell per retained part");
            var result = new List<VoxelFragment>(maxFragments); var remainingParts = retainedParts;
            void Append(VoxelCharacterSkin skin)
            {
                var allowance = (maxFragments - result.Count) / remainingParts--;
                result.AddRange(skin.SnapshotFragments(allowance));
            }
            try
            {
                lastLiveHead.Apply(Root.transform);
                for (var i = 0; i < liveTransforms.Length; i++) livePose[i].Apply(liveTransforms[i]);
                Append(headVoxel);
                foreach (var part in parts)
                {
                    if (part == null || !part.placed || !part.drawing || part.voxels == null) continue;
                    var previous = new WorldPose(part.root.transform);
                    try { part.lastLive.Apply(part.root.transform); Append(part.voxels); }
                    finally { previous.Apply(part.root.transform); }
                }
            }
            finally
            {
                currentHead.Apply(Root.transform);
                for (var i = 0; i < liveTransforms.Length; i++) current[i].Apply(liveTransforms[i]);
            }
            if (rebase != Vector3.zero)
                for (var i = 0; i < result.Count; i++)
                {
                    var cell = result[i];
                    result[i] = new VoxelFragment(cell.Position + rebase, cell.Size, cell.Color);
                }
            deathSnapshot = result; snapshotLimit = maxFragments; return deathSnapshot;
        }

        private static void Destroy(UnityEngine.Object value)
        {
            if (Application.isPlaying) UnityEngine.Object.Destroy(value); else UnityEngine.Object.DestroyImmediate(value);
        }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            foreach (var part in parts) part?.Dispose();
            headVoxel?.Dispose();
            if (Root != null) Root.SetActive(false);
            head.Dispose();
        }
    }
}
