namespace MagicCarpet.Native
{
    public sealed partial class MagicCarpetRuntime
    {
        private sealed class Level2EntityPresenter
        {
            private readonly MagicCarpetRuntime owner;

            internal Level2EntityPresenter(MagicCarpetRuntime owner)
            {
                this.owner = owner;
            }

            internal void Update()
            {
                owner.RecordLevel2Presentation("entity");
                var suppressionFlags = owner.CurrentNativeFinalPresentationOwns()
                    ? owner.terrainFrameInfo.flags
                    : 0u;
                if (owner.CurrentNativeFinalPresentationOwns())
                    owner.managedEntitySubmissionCount++;
                if (owner.entityRegistry.Count == 0) return;
                foreach (var marker in owner.entityRegistry)
                {
                    var entity = owner.entities[marker.Index];
                    var goatOrdinal =
                        owner.goatOrdinalByEntity != null &&
                        marker.Index < owner.goatOrdinalByEntity.Length
                            ? owner.goatOrdinalByEntity[marker.Index]
                            : -1;
                    var liveGoat = goatOrdinal >= 0;
                    var sourceX = entity.x;
                    var sourceY = entity.y;
                    var sourceZ = entity.z;
                    var sourceYaw = entity.yaw;
                    var animationFrame = marker.Group?.key.animation_frame ?? 0;
                    var active = entity.active != 0;
                    var drawingEnabled = true;
                    if (liveGoat)
                    {
                        var goat = owner.goats[goatOrdinal];
                        MagicCarpetRuntime.Require(
                            goat.entityIndex == marker.Index &&
                            goat.authority == NativeSim.GoatLive,
                            $"stable live Goat mapping {marker.Index}");
                        sourceX = goat.x;
                        sourceY = goat.y;
                        sourceZ = goat.z;
                        sourceYaw = goat.yaw;
                        animationFrame = goat.animationFrame;
                        active = goat.active != 0;
                        drawingEnabled = (goat.controlFlags & 0x00000400) == 0;
                    }
                    var nearestX = owner.unwrappedSourceX +
                        MagicCarpetRuntime.ModularDelta(
                            sourceX,
                            owner.presentationState.cameraX);
                    var nearestY = owner.unwrappedSourceY +
                        MagicCarpetRuntime.ModularDelta(
                            sourceY,
                            owner.presentationState.cameraY);
                    marker.Node.position = MagicCarpetRuntime.SourceToUnity(
                        nearestX,
                        nearestY,
                        sourceZ);
                    if (marker.Renderer != null)
                    {
                        var retiredOrReplacedImportedActor =
                            MagicCarpetRuntime.IsRetiredOrReplacedImportedActor(
                                entity,
                                marker.QualifiedFrozenActor);
                        var nativeFrozenActor =
                            !liveGoat && marker.QualifiedFrozenActor;
                        var distanceSquared = owner.presentationRig == null
                            ? float.PositiveInfinity
                            : UnityEngine.Vector3.SqrMagnitude(
                                marker.Node.position - owner.targetPosition);
                        marker.Renderer.enabled =
                            !retiredOrReplacedImportedActor &&
                            active && drawingEnabled &&
                            !(liveGoat &&
                              (suppressionFlags &
                               NativeSim.TerrainFrameGoatsComposited) != 0) &&
                            !(nativeFrozenActor &&
                              !MagicCarpetRuntime.ManagedFrozenActorVisible(
                                  1,
                                  marker.QualifiedFrozenActor,
                                  suppressionFlags)) &&
                            ((owner.debugEntities && !owner.qualifiedMode) ||
                             distanceSquared >= 1f);
                        if (marker.OwnerSprite && marker.Group != null &&
                            marker.SpriteVisual != null)
                        {
                            if (retiredOrReplacedImportedActor) continue;
                            MagicCarpetRuntime.Require(
                                liveGoat
                                    ? entity.authority == NativeSim.GoatLive &&
                                      marker.Group.group_id == "reset-050"
                                    : entity.authority == NativeSim.FrozenObserved ||
                                      entity.authority == NativeSim.VillagerLive,
                                $"owner sprite authority {marker.Index}");
                            if (owner.flightCamera != null)
                            {
                                marker.Node.rotation =
                                    owner.flightCamera.transform.rotation;
                                owner.UpdateOwnerEntitySprite(
                                    marker.Index,
                                    sourceYaw,
                                    animationFrame,
                                    marker.Group,
                                    marker.SpriteVisual,
                                    marker.Renderer);
                            }
                        }
                    }
                }
            }
        }

        private void UpdateEntityPresentation()
        {
            Level2Entities.Update();
        }
    }
}
