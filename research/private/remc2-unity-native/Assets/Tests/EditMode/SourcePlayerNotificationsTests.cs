using System;
using System.IO;
using NUnit.Framework;

namespace MagicCarpet.Native.Tests
{
    public sealed class SourcePlayerNotificationsTests
    {
        private static string Language(int selector) => selector switch {
            425 => "Paused!", 374 => "died", 283 => "banished",
            378 => "Tasks completed", 379 => "Fly to the exit point",
            _ => throw new ArgumentOutOfRangeException(nameof(selector))
        };

        [Test]
        public void PausedParticipantRowsRetainSourceOrderZeroTimerAndOriginalOffsets()
        {
            var notices = new[] {
                new EngineSessionTransport.ParticipantNotification { participant = "111",
                    display_name = "PLAYER", kind = "participant.died", remaining_frames = 0 },
                new EngineSessionTransport.ParticipantNotification { participant = "425",
                    display_name = "Vodor", kind = "participant.banished", remaining_frames = 199 }
            };
            var rows = SourcePlayerNotificationsPresenter.Rows(notices, true, false, false, 14, Language);
            Assert.That(rows.Length, Is.EqualTo(3));
            Assert.That((rows[0].Text, rows[0].X, rows[0].Y, rows[0].PausedLabel),
                Is.EqualTo(("Paused!", 132, 50, true)));
            Assert.That((rows[1].Text, rows[1].X, rows[1].Y), Is.EqualTo(("PLAYER died", 204, 50)));
            Assert.That((rows[2].Text, rows[2].X, rows[2].Y), Is.EqualTo(("Vodor banished", 132, 64)));
            Assert.That(notices[0].remaining_frames, Is.Zero, "render observations never age timers");

            rows = SourcePlayerNotificationsPresenter.Rows(notices, true, true, true, 14, Language);
            Assert.That(rows.Length, Is.EqualTo(4));
            Assert.That((rows[0].X, rows[0].Y), Is.EqualTo((6, 6)));
            Assert.That((rows[1].Text, rows[1].X, rows[1].Y),
                Is.EqualTo(("Tasks completed. Fly to the exit point.", 78, 6)));
            Assert.That((rows[2].Text, rows[2].X, rows[2].Y), Is.EqualTo(("PLAYER died", 6, 20)));
            Assert.That((rows[3].X, rows[3].Y), Is.EqualTo((6, 34)));

            notices[0].kind = "participant.unbound";
            Assert.Throws<InvalidDataException>(() => SourcePlayerNotificationsPresenter.Rows(
                notices, false, false, false, 14, Language));
        }
    }
}
