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
        public float runSpeed;
        public float height;
        public string[] hitOverlayBones = Array.Empty<string>();

        // Bone identities belong to the authored asset, rather than one rig's
        // naming convention. Reject missing or ambiguous bindings at admission.
        public Transform[] ResolveHitOverlayBones(Transform root)
        {
            if (root == null) throw new ArgumentNullException(nameof(root));
            if (hitOverlayBones == null || hitOverlayBones.Length == 0)
                throw new InvalidOperationException("Character lacks authored moving-hit bone bindings");
            var hierarchy = root.GetComponentsInChildren<Transform>(true);
            var result = new Transform[hitOverlayBones.Length];
            for (var i = 0; i < hitOverlayBones.Length; i++)
            {
                var bone = hitOverlayBones[i];
                if (string.IsNullOrWhiteSpace(bone) || Array.IndexOf(hitOverlayBones, bone) != i)
                    throw new InvalidOperationException("Character moving-hit bone binding is empty or repeated: " + bone);
                var matches = Array.FindAll(hierarchy, transform => transform.name == bone);
                if (matches.Length != 1)
                    throw new InvalidOperationException("Character moving-hit bone binding must identify exactly one transform: " + bone);
                result[i] = matches[0];
            }
            return result;
        }

        public AnimationClip Clip(string name)
        {
            foreach (var clip in clips)
                if (clip != null && clip.name == name) return clip;
            throw new InvalidOperationException($"Character {this.name} lacks clip {name}");
        }
    }
}
