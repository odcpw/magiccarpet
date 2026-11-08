using System;
using System.Collections;
using System.Collections.Generic;
using UnityEngine;

namespace MagicCarpet.Native.Voxels
{
    // Experimental projectile: the parent sphere supplies continuous collision,
    // and its constituent cells become separate PhysX clusters on first contact.
    internal sealed class VoxelFireball : MonoBehaviour
    {
        internal VoxelRubble Rubble { get; set; }
        internal const int MaxProjectiles = 8;
        private int active;
        internal int ActiveProjectiles => active;
        internal int PeakProjectiles { get; private set; }

        internal bool Launch(Vector3 origin, Vector3 velocity, float radius, float cellSize, Action<Vector3, Vector3, float> onImpact)
        {
            if (Rubble == null) throw new InvalidOperationException("VoxelFireball requires a VoxelRubble manager");
            if (!(radius > 0f) || !(cellSize > 0f) || float.IsInfinity(radius) || float.IsInfinity(cellSize))
                throw new ArgumentOutOfRangeException(nameof(radius), "Radius and cell size must be positive and finite");
            if (active >= MaxProjectiles) return false;
            var cells = SphereCells(radius, cellSize);
            var go = new GameObject("Voxel fireball");
            go.transform.SetParent(transform, false); go.transform.position = origin;
            go.transform.rotation = Quaternion.identity;
            var shot = go.AddComponent<Projectile>();
            active++; PeakProjectiles = Mathf.Max(PeakProjectiles, active);
            shot.Initialize(this, Rubble, cells, radius, velocity, onImpact);
            return true;
        }

        // Cell size is coarsened only if necessary to bound sphere geometry.
        internal static List<VoxelFragment> SphereCells(float radius, float requestedCellSize)
        {
            float size = Mathf.Clamp(requestedCellSize, radius / 8f, radius);
            int n = Mathf.CeilToInt(radius / size);
            var cells = new List<VoxelFragment>();
            for (int x = -n; x <= n; x++) for (int y = -n; y <= n; y++) for (int z = -n; z <= n; z++)
            {
                Vector3 p = new Vector3(x, y, z) * size;
                // Every cube fits inside the swept collision sphere, including
                // its corners, so it does not start through the struck surface.
                if (p.magnitude + .866026f * size > radius + .00001f * size) continue;
                float t = Mathf.Clamp01(p.magnitude / radius);
                cells.Add(new VoxelFragment(p, size, Color32.Lerp(new Color32(255, 214, 38, 255), new Color32(220, 38, 7, 255), t)));
            }
            return cells;
        }

        internal static float ImpactEnergy(float mass, Vector3 velocity) => .5f * mass * velocity.sqrMagnitude;

        private sealed class Projectile : MonoBehaviour
        {
            private VoxelFireball owner;
            private VoxelRubble rubble;
            private List<VoxelFragment> cells;
            private Rigidbody body;
            private Mesh mesh;
            private Material material;
            private Action<Vector3, Vector3, float> callback;
            private Vector3 incoming;
            private bool impacted;
            private float born;

            internal void Initialize(VoxelFireball owner, VoxelRubble rubble, List<VoxelFragment> cells, float radius,
                Vector3 velocity, Action<Vector3, Vector3, float> callback)
            {
                this.owner = owner; this.rubble = rubble; this.cells = cells; this.callback = callback;
                mesh = VoxelRubble.BuildMesh(cells, Vector3.zero);
                gameObject.AddComponent<MeshFilter>().sharedMesh = mesh;
                var shader = Shader.Find("MagicCarpet/VoxelSurface");
                if (shader == null) throw new InvalidOperationException("Missing MagicCarpet/VoxelSurface shader");
                material = new Material(shader) { name = "Voxel fireball" };
                material.SetFloat("_Emission", .85f);
                gameObject.AddComponent<MeshRenderer>().sharedMaterial = material;
                gameObject.AddComponent<SphereCollider>().radius = radius;
                body = gameObject.AddComponent<Rigidbody>();
                body.mass = Mathf.Max(.0001f, VoxelRubble.Mass(cells));
                // Keep the flight grid axis-aligned; released clusters remain free to rotate.
                body.constraints = RigidbodyConstraints.FreezeRotation;
                body.collisionDetectionMode = CollisionDetectionMode.ContinuousDynamic;
                body.linearVelocity = velocity; incoming = velocity; born = Time.time;
            }

            private void FixedUpdate()
            {
                if (!impacted) incoming = body.linearVelocity;
                if (!impacted && Time.time - born > 15f) Destroy(gameObject);
            }

            private void OnCollisionEnter(Collision collision)
            {
                if (impacted || collision.contactCount == 0) return;
                impacted = true;
                var contact = collision.GetContact(0);
                float energy = ImpactEnergy(body.mass, incoming);
                var world = new List<VoxelFragment>(cells.Count);
                foreach (var cell in cells) world.Add(new VoxelFragment(transform.TransformPoint(cell.Position), cell.Size, cell.Color));
                // No synthetic bounce or radial blast: retain incoming velocity and let
                // fragment colliders bounce off the still-intact wall through PhysX.
                GetComponent<Collider>().enabled = false;
                GetComponent<MeshRenderer>().enabled = false;
                body.isKinematic = true;
                var emission = rubble.EmitImpact(world, incoming, contact.point, contact.normal);
                StartCoroutine(FinishImpact(emission, contact.point, contact.normal, energy));
            }

            private IEnumerator FinishImpact(VoxelRubble.Emission emission, Vector3 point, Vector3 normal, float energy)
            {
                // If the body budget is full, keep the wall until queued fragments exist.
                while (emission != null && !emission.Spawned && !emission.Cancelled) yield return new WaitForFixedUpdate();
                if (emission != null && emission.Cancelled) { Destroy(gameObject); yield break; }
                // Preserve colliders for several physics passes before destructive edits.
                for (int i = 0; i < 3; i++)
                {
                    yield return new WaitForFixedUpdate();
                    if (emission != null && emission.Cancelled) { Destroy(gameObject); yield break; }
                }
                callback?.Invoke(point, normal, energy);
                Destroy(gameObject);
            }

            private void OnDestroy()
            {
                if (owner != null) owner.active--;
                if (mesh != null) Destroy(mesh);
                if (material != null) Destroy(material);
            }
        }
    }
}
