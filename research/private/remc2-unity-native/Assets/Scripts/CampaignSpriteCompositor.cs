using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    /// <summary>
    /// Source-cell and linked-map-chain ordered indexed sprites. Destination
    /// blend operations read a GPU copy made immediately before their draw;
    /// a frame-wide grab would incorrectly ignore preceding sprites.
    /// </summary>
    internal sealed class CampaignSpriteCompositor : IDisposable
    {
        private readonly Camera camera;
        private readonly RenderTexture target;
        private readonly RenderTexture destination;
        private readonly CommandBuffer commands;
        private readonly List<CampaignEntitySurfaceMarker> ordered = new();
        private ushort[] cellOrders;
        private byte previousPixelMode;
        private bool disposed;

        internal int DrawCount { get; private set; }
        internal int DestinationCopyCount { get; private set; }

        internal CampaignSpriteCompositor(Camera camera, RenderTexture target)
        {
            this.camera = camera ?? throw new ArgumentNullException(nameof(camera));
            this.target = target ?? throw new ArgumentNullException(nameof(target));
            destination = new RenderTexture(target.descriptor) {
                name = "MC2 preceding indexed sprite destination", filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Clamp
            };
            destination.depth = 0;
            if (!destination.Create())
                throw new InvalidOperationException("source sprite destination target could not be created");
            commands = new CommandBuffer { name = "MC2 source ordered indexed sprites" };
            camera.AddCommandBuffer(CameraEvent.AfterEverything, commands);
        }

        internal static byte ResolvePixelMode(byte baseMode, uint controlFlags, byte previousMode)
        {
            // Literal pinned remc2 unsigned dw_w_b.byte[2] behavior. The
            // unmatched special-flag branch retains its prior pixel operation.
            if ((controlFlags & 0x00800000u) == 0) return baseMode;
            var byte3 = (byte)(controlFlags >> 24);
            if ((byte3 & 2) != 0) return 4;
            if ((byte3 & 4) != 0) return 5;
            return (byte3 & 1) != 0 ? (byte)3 : previousMode;
        }

        internal void SetSprites(IEnumerable<CampaignEntitySurfaceMarker> sprites, ushort[] sourceCellOrders)
        {
            if (disposed) throw new ObjectDisposedException(nameof(CampaignSpriteCompositor));
            if (sourceCellOrders == null || sourceCellOrders.Length != 65536)
                throw new InvalidOperationException("source sprite cell orders are incomplete");
            cellOrders = sourceCellOrders;
            ordered.Clear();
            foreach (var sprite in sprites)
                if (sprite.SpriteReady) ordered.Add(sprite);
            ordered.Sort(CompareSourceOrder);
        }

        private int CompareSourceOrder(CampaignEntitySurfaceMarker left, CampaignEntitySurfaceMarker right)
        {
            var leftCell = cellOrders[(left.SourceY >> 8) * 256 + (left.SourceX >> 8)];
            var rightCell = cellOrders[(right.SourceY >> 8) * 256 + (right.SourceX >> 8)];
            var comparison = leftCell.CompareTo(rightCell);
            if (comparison != 0) return comparison;
            return left.SourceDrawOrder.CompareTo(right.SourceDrawOrder);
        }

        /// Rebuild only after PresentView has installed this frame's copied
        /// source positions, camera and projection; DrawMesh captures matrices.
        internal void PrepareView()
        {
            if (disposed) throw new ObjectDisposedException(nameof(CampaignSpriteCompositor));
            commands.Clear();
            DrawCount = DestinationCopyCount = 0;
            commands.SetRenderTarget(target);
            commands.SetViewProjectionMatrices(camera.worldToCameraMatrix, camera.projectionMatrix);
            foreach (var sprite in ordered)
            {
                var cell = cellOrders[(sprite.SourceY >> 8) * 256 + (sprite.SourceX >> 8)];
                // A zero cell was never visited by the source terrain walk.
                if (cell == 0) continue;
                var mode = ResolvePixelMode(sprite.SourcePixelMode, sprite.SourceControlFlags, previousPixelMode);
                if (mode > 8) throw new InvalidOperationException("source sprite pixel operation is invalid");
                previousPixelMode = mode;
                if (mode is 2 or 3 or 6 or 7 or 8)
                {
                    commands.CopyTexture(target, destination);
                    commands.SetGlobalTexture("_SpriteDestination", destination);
                    DestinationCopyCount++;
                }
                sprite.DrawSourceSprite(commands, mode);
                DrawCount++;
            }
        }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            if (camera != null) camera.RemoveCommandBuffer(CameraEvent.AfterEverything, commands);
            commands.Release();
            destination.Release();
            if (Application.isPlaying) UnityEngine.Object.Destroy(destination);
            else UnityEngine.Object.DestroyImmediate(destination);
            ordered.Clear();
        }
    }
}
