using UnityEngine;

namespace MagicCarpet.Native.Voxels
{
    // World-space cell shared by the sandbox's geometry and physical fragments.
    internal readonly struct VoxelFragment
    {
        internal readonly Vector3 Position;
        internal readonly float Size;
        internal readonly Color32 Color;
        internal VoxelFragment(Vector3 position, float size, Color32 color)
        { Position = position; Size = size; Color = color; }
    }
}
