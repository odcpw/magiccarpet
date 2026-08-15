using UnityEngine;

namespace MagicCarpet.Native
{
    public sealed partial class MagicCarpetRuntime
    {
        private sealed class Level2AudioPresenter
        {
            private readonly MagicCarpetRuntime owner;

            internal Level2AudioPresenter(MagicCarpetRuntime owner)
            {
                this.owner = owner;
            }

            internal void Initialize()
            {
                MagicCarpetRuntime.Require(
                    owner.campaignSession != null,
                    "Level 2 Day music presenter has a campaign owner");
                owner.level2DayMusic = owner.campaignSession.DayMusicState();
                owner.level2DayMusicFadeAccumulator = 0f;
            }

            internal void Update()
            {
                owner.RecordLevel2Presentation("audio");
                MagicCarpetRuntime.Require(
                    owner.campaignSession != null,
                    "Level 2 Day music update has a campaign owner");
                owner.level2DayMusic = owner.campaignSession.DayMusicState();
                if (owner.level2DayMusic.timerActive != 0)
                {
                    owner.level2DayMusicFadeAccumulator +=
                        Time.fixedDeltaTime * owner.level2DayMusic.timerHz;
                    var callbacks = Mathf.FloorToInt(
                        owner.level2DayMusicFadeAccumulator);
                    if (callbacks > 0)
                    {
                        owner.level2DayMusicFadeAccumulator -= callbacks;
                        owner.level2DayMusic =
                            owner.campaignSession.AdvanceDayMusic(
                                checked((uint)callbacks));
                    }
                }
                else
                {
                    owner.level2DayMusicFadeAccumulator = 0f;
                }
            }

            internal void Reset()
            {
                Initialize();
            }

            internal void Dispose()
            {
                owner.level2DayMusic = default;
                owner.level2DayMusicFadeAccumulator = 0f;
            }
        }

        private void UpdateLevel2DayMusicPresentation()
        {
            Level2Audio.Update();
        }

        private void InitializeLevel2DayMusicPresentation()
        {
            Level2Audio.Initialize();
        }

        private void ResetLevel2DayMusicPresentation()
        {
            Level2Audio.Reset();
        }
    }
}
