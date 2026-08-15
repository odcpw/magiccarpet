namespace MagicCarpet.Native
{
    public sealed partial class MagicCarpetRuntime
    {
        private sealed class Level2HudPresenter
        {
            private readonly MagicCarpetRuntime owner;

            internal Level2HudPresenter(MagicCarpetRuntime owner)
            {
                this.owner = owner;
            }

            internal void Present(NativeSim.CampaignDayHudSnapshot hud)
            {
                owner.RecordLevel2Presentation("hud");
                MagicCarpetRuntime.Require(
                    OwnerAssetCache.IsQualifiedDayHudSpriteSelector(
                        hud.minimapPanelSelector) &&
                    OwnerAssetCache.IsQualifiedDayHudSpriteSelector(
                        hud.castlePanelSelector) &&
                    OwnerAssetCache.IsQualifiedDayHudSpriteSelector(
                        hud.balloonPanelSelector) &&
                    OwnerAssetCache.IsQualifiedDayHudSpriteSelector(
                        hud.playerPanelSelector) &&
                    OwnerAssetCache.IsQualifiedDayHudSpriteSelector(
                        hud.playerIconSelector) &&
                    OwnerAssetCache.IsQualifiedDayHudSpriteSelector(
                        hud.lifeManaIconSelector),
                    "qualified Day HUD status roots");
                MagicCarpetRuntime.ValidateLevel2DayHudPresenterSlot(
                    hud.left,
                    hud.localPlayerEntityIndex);
                MagicCarpetRuntime.ValidateLevel2DayHudPresenterSlot(
                    hud.right,
                    hud.localPlayerEntityIndex);

                owner.level2PresentedDayHud = hud;
                owner.level2DayHudPresenterReady = false;
                if (owner.HasMaterializedLevel2DayHudRoots(hud))
                {
                    owner.ComputeLevel2DayHudTopBarLayout(hud);
                    owner.level2DayHudPresenterReady = true;
                }
            }
        }

        private void PresentLevel2DayHud(NativeSim.CampaignDayHudSnapshot hud)
        {
            level2DayHud = hud;
            Level2Hud.Present(hud);
        }
    }
}
