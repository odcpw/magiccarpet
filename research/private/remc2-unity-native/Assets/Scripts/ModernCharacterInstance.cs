using System;
using UnityEngine;
using UnityEngine.Animations;
using UnityEngine.Playables;

namespace MagicCarpet.Native
{
    // Owns playback of the imported rig and its clips, including crossfades.
    internal sealed class ModernCharacterInstance : IDisposable
    {
        internal readonly GameObject Root;
        internal readonly ModernCharacterAsset Asset;
        internal string CurrentClip { get; private set; }
        internal float ClipTime => (float)times[target];
        internal float WalkWeight {
            get { var index = Array.FindIndex(Asset.clips, clip => clip.name == "Walk"); return index < 0 ? 0 : weights[index]; }
        }
        private readonly PlayableGraph graph;
        private readonly AnimationMixerPlayable mixer;
        private readonly AnimationClipPlayable[] players;
        private readonly double[] times;
        private readonly float[] weights, blendFrom;
        private int target;
        private float blendDuration, blendElapsed;
        private bool disposed;

        internal ModernCharacterInstance(ModernCharacterAsset asset, Transform parent)
        {
            Asset = asset != null ? asset : throw new ArgumentNullException(nameof(asset));
            if (asset.model == null || asset.clips == null || asset.clips.Length == 0)
                throw new InvalidOperationException("Character has no imported model/clips");
            Root = new GameObject(asset.name);
            Root.transform.SetParent(parent, false);
            var visual = UnityEngine.Object.Instantiate(asset.model, Root.transform);
            visual.transform.localPosition = Vector3.zero;
            visual.transform.localRotation = Quaternion.Euler(0, asset.forwardYaw, 0);
            var animator = visual.GetComponent<Animator>();
            if (animator == null) animator = visual.AddComponent<Animator>();
            animator.applyRootMotion = false;
            animator.cullingMode = AnimatorCullingMode.AlwaysAnimate;
            graph = PlayableGraph.Create("Character " + asset.name);
            graph.SetTimeUpdateMode(DirectorUpdateMode.Manual);
            players = new AnimationClipPlayable[asset.clips.Length];
            times = new double[players.Length];
            weights = new float[players.Length];
            blendFrom = new float[players.Length];
            mixer = AnimationMixerPlayable.Create(graph, players.Length);
            for (var i = 0; i < players.Length; i++)
            {
                players[i] = AnimationClipPlayable.Create(graph, asset.clips[i]);
                players[i].SetApplyFootIK(false);
                players[i].SetSpeed(0);
                graph.Connect(players[i], 0, mixer, i);
            }
            AnimationPlayableOutput.Create(graph, "Character", animator).SetSourcePlayable(mixer);
            graph.Play();
            Play("Idle", 0);
            Tick(0);
        }

        internal void Play(string name, float transition = .18f, bool restart = true)
        {
            var index = Array.FindIndex(Asset.clips, clip => clip.name == name);
            if (index < 0) throw new ArgumentException("Absent character clip " + name);
            if (CurrentClip == name && !restart) return;
            Array.Copy(weights, blendFrom, weights.Length);
            target = index;
            CurrentClip = name;
            if (restart) times[target] = 0;
            blendDuration = Mathf.Max(0, transition);
            blendElapsed = 0;
        }

        internal void Tick(float dt) => Tick(dt, dt);

        // Locomotion phase follows distance; crossfades retain real-time duration.
        internal void Tick(float dt, float targetAdvance)
        {
            if (disposed) throw new ObjectDisposedException(nameof(ModernCharacterInstance));
            if (float.IsNaN(dt) || float.IsInfinity(dt) || dt < 0) throw new ArgumentOutOfRangeException(nameof(dt));
            if (!float.IsFinite(targetAdvance) || targetAdvance < 0) throw new ArgumentOutOfRangeException(nameof(targetAdvance));
            blendElapsed += dt;
            var blend = blendDuration == 0 ? 1 : Mathf.SmoothStep(0, 1, Mathf.Clamp01(blendElapsed / blendDuration));
            for (var i = 0; i < players.Length; i++)
            {
                weights[i] = Mathf.Lerp(blendFrom[i], i == target ? 1 : 0, blend);
                if (weights[i] > 0 || i == target)
                {
                    times[i] += i == target ? targetAdvance : dt;
                    var length = Asset.clips[i].length;
                    if (Asset.clips[i].isLooping && length > 0) times[i] %= length;
                    else times[i] = Math.Min(times[i], length);
                }
                mixer.SetInputWeight(i, weights[i]);
                players[i].SetTime(times[i]);
            }
            graph.Evaluate(0);
        }

        internal void Sample(string clip, float time)
        {
            Play(clip, 0);
            times[target] = Mathf.Clamp(time, 0, Asset.clips[target].length);
            Tick(0);
        }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            if (graph.IsValid()) graph.Destroy();
            if (Application.isPlaying) UnityEngine.Object.Destroy(Root);
            else UnityEngine.Object.DestroyImmediate(Root);
        }
    }
}
