namespace MagicCarpet.Native
{
    public sealed partial class MagicCarpetRuntime
    {
        private sealed class CampaignMinimapPresenter
        {
            private readonly MagicCarpetRuntime owner;

            internal CampaignMinimapPresenter(MagicCarpetRuntime owner)
            {
                this.owner = owner;
            }

            internal void Project(ulong expectedTick)
            {
                owner.RecordLevel2Presentation("minimap");
                MagicCarpetRuntime.Require(
                    owner.campaignSession != null && owner.level2Mode,
                    "Level 2 minimap projection has a campaign owner");
                var state = owner.campaignSession.MinimapState(expectedTick);
                var entities = owner.campaignSession.MinimapEntities(
                    expectedTick,
                    state);
                var marks = owner.campaignSession.MinimapMarks(
                    expectedTick,
                    state);
                MagicCarpetRuntime.Require(
                    state.generation == owner.campaignSession.Generation &&
                    state.tick == expectedTick,
                    "Level 2 minimap generation/tick projection binding");
                owner.level2MinimapState = state;
                owner.level2MinimapEntities = entities;
                owner.level2MinimapMarks = marks;
                owner.level2MinimapProjectionReady = true;
            }

            internal void Dispose()
            {
                owner.level2MinimapState = default;
                owner.level2MinimapEntities =
                    System.Array.Empty<NativeSim.CampaignMinimapEntity>();
                owner.level2MinimapMarks =
                    System.Array.Empty<NativeSim.CampaignMinimapMark>();
                owner.level2MinimapProjectionReady = false;
            }
        }

        private void ProjectLevel2Minimap(ulong expectedTick)
        {
            Level2Minimap.Project(expectedTick);
        }
    }
}
