using System;
using UnityEngine;

namespace MagicCarpet.Native
{
    // References Unity's imported clips, so the reviewed and played data agree.
    public sealed class ModernCharacterAsset : ScriptableObject
    {
        public GameObject model;
        public AnimationClip[] clips;
        public float forwardYaw = 180;
        public float walkSpeed = .15f;
        public float height;

        public AnimationClip Clip(string name)
        {
            foreach (var clip in clips)
                if (clip != null && clip.name == name) return clip;
            throw new InvalidOperationException($"Character {this.name} lacks clip {name}");
        }
    }
}
