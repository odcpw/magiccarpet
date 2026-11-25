using System;
using System.Collections.Generic;
using System.Linq;

namespace MagicCarpet.Native
{
    // NewGameDraw_7EAE0, Create/MoveAnimObject and the two source map
    // animation tables. This owns presentation motion, never mission progress.
    internal sealed class SourceMapMotion
    {
        [Serializable] internal sealed class BackgroundBinding
        {
            public int x, y, first, last, selector, delay_seconds, phase, restart,
                sound_delay, sound, play_type, loops;
        }
        [Serializable] internal sealed class DecorationBinding
        {
            public int x, y, target_x, target_y, first, last, finish_first, finish_last,
                delay_seconds, kind, frontier, start_sound, start_play_type, finish_sound, finish_play_type;
        }
        internal readonly struct Sprite
        {
            internal readonly int Selector, X, Y;
            internal Sprite(int selector, int x, int y) { Selector = selector; X = x; Y = y; }
        }
        internal sealed class Path
        {
            internal short X, Y;
            private readonly short targetX, targetY, absX, absY, stepX, stepY;
            private short error;
            private readonly int stateX, stateY;

            internal Path(int targetX, int targetY, int x, int y, int step)
            {
                this.targetX = checked((short)targetX); this.targetY = checked((short)targetY);
                X = checked((short)x); Y = checked((short)y);
                var dx = unchecked((short)(targetX - x));
                var dy = unchecked((short)(targetY - y));
                absX = unchecked((short)(2 * Math.Abs((int)dx)));
                absY = unchecked((short)(2 * Math.Abs((int)dy)));
                stepX = (short)(Math.Sign(dx) * step); stepY = (short)(Math.Sign(dy) * step);
                error = unchecked((short)(absX <= absY ? absX - (absY >> 1) : (absY - absX) >> 1));
                stateX = Math.Sign(targetX - x); stateY = Math.Sign(targetY - y);
            }
            internal bool Move()
            {
                if (absX <= absY)
                {
                    if (Y == targetY) return true;
                    if (error >= 0) { X = unchecked((short)(X + stepX)); error = unchecked((short)(error - absY)); }
                    Y = unchecked((short)(Y + stepY)); error = unchecked((short)(error + absX));
                }
                else
                {
                    if (X == targetX) return true;
                    if (error >= 0) { Y = unchecked((short)(Y + stepY)); error = unchecked((short)(error - absX)); }
                    X = unchecked((short)(X + stepX)); error = unchecked((short)(error + absY));
                }
                if (stateX < 0 && X < targetX || stateX > 0 && X > targetX) X = targetX;
                if (stateY < 0 && Y < targetY || stateY > 0 && Y > targetY) Y = targetY;
                return false; // Arrival is reported on the following source move.
            }
        }
        private sealed class Background
        {
            internal BackgroundBinding Binding;
            internal int Phase, Selector;
            internal uint Start, Last;
        }
        private sealed class Decoration
        {
            internal DecorationBinding Binding;
            internal int Phase, Selector, X, Y;
            internal uint Last;
            internal Path Path;
        }
        private readonly Background[] backgrounds;
        private readonly Decoration[] decorations;
        private readonly List<Sprite> backgroundSprites = new(), decorationSprites = new();
        private Path viewportPath, travelPath;
        private string previous, pendingMission;
        private int actorX, actorY, actorFirst = 13, actorSelector = 13, stampX, stampY, shift;
        private uint actorTime;
        private bool travelNoActor;
        internal int X { get; private set; }
        internal int Y { get; private set; }
        internal bool Ready { get; private set; }
        internal int DescriptionViewportY { get; private set; }
        internal bool Travelling => pendingMission != null;
        internal IReadOnlyList<Sprite> BackgroundSprites => backgroundSprites;
        internal IReadOnlyList<Sprite> DecorationSprites => decorationSprites;
        internal Sprite? Actor { get; private set; }
        internal bool ActorIsTravelling { get; private set; }

        internal SourceMapMotion(BackgroundBinding[] backgrounds, DecorationBinding[] decorations)
        {
            if (backgrounds?.Length != 15 || decorations?.Length != 11)
                throw new ArgumentException("source map animation tables are incomplete");
            this.backgrounds = backgrounds.Select(row => new Background {
                Binding = row, Phase = row.phase, Selector = row.selector }).ToArray();
            this.decorations = decorations.Select(row => new Decoration { Binding = row }).ToArray();
        }

        internal void Enter(GameRuntimeTransport.CampaignMenuSnapshot menu,
            Func<string, OwnerCampaignMenuCache.Mission> lookup, string lastLaunched,
            Action<int, int> stamp, int centerX, int centerY)
        {
            pendingMission = null; Actor = null; viewportPath = null; shift = 0;
            actorFirst = actorSelector = 13;
            var completed = menu.available_levels.LastOrDefault(level => menu.completed_levels.Contains(level));
            var frontier = menu.available_levels.FirstOrDefault(level => !menu.completed_levels.Contains(level));
            previous = lastLaunched;
            if (menu.phase != "results" && (completed == null || completed == previous)) previous = null;
            var focus = lookup(menu.phase == "results" ? menu.completed_level_id : previous ?? completed ?? frontier ?? menu.selected_level_id);
            X = focus.viewport_x; Y = focus.viewport_y;
            Ready = menu.phase != "results";
            if (Ready && previous == null && frontier != null)
            {
                var target = lookup(frontier);
                if (completed != null) { viewportPath = new Path(target.viewport_x, target.viewport_y, X, Y, 4); Ready = false; }
                else { X = Math.Min(640, target.viewport_x); Y = Math.Min(480, target.viewport_y); }
            }
            DescriptionViewportY = Y;
            OwnerCampaignMenuCache.Mission prior = null;
            foreach (var level in menu.available_levels)
            {
                if (!menu.completed_levels.Contains(level)) continue;
                if (menu.phase == "results" && level == completed) continue;
                var next = lookup(level);
                if (prior != null) StampPath(prior, next, centerX, centerY, stamp, false);
                prior = next;
            }
            var pathFrontier = menu.phase == "results" ? completed : frontier;
            if (previous != null && prior != null && pathFrontier != null)
                StampPath(prior, lookup(pathFrontier), centerX, centerY, stamp, true);
        }

        private static void StampPath(OwnerCampaignMenuCache.Mission from, OwnerCampaignMenuCache.Mission to,
            int cx, int cy, Action<int, int> stamp, bool omitArrival)
        {
            var path = new Path(to.portal_x + cx, to.portal_y + cy, from.portal_x + cx, from.portal_y + cy, 2);
            var lastX = (int)path.X; var lastY = (int)path.Y;
            var arrived = false;
            while (!arrived)
            {
                for (var pass = 0; pass < 3; pass++) arrived = path.Move();
                if (arrived && omitArrival) return;
                if (Math.Abs(path.X - lastX) > 8 || Math.Abs(path.Y - lastY) > 8)
                { stamp(path.X, path.Y); lastX = path.X; lastY = path.Y; }
            }
        }

        internal void BeginTravel(string target, GameRuntimeTransport.CampaignMenuSnapshot menu,
            Func<string, OwnerCampaignMenuCache.Mission> lookup, int cx, int cy,
            Action<int, int, int> sound)
        {
            if (!Ready || Travelling) throw new InvalidOperationException("map is already moving");
            var completed = menu.available_levels.LastOrDefault(level => menu.completed_levels.Contains(level));
            pendingMission = target;
            travelNoActor = previous == null && completed == null;
            if (travelNoActor) return;
            var start = lookup(previous ?? completed);
            actorX = start.portal_x + cx; actorY = start.portal_y + cy;
            var finish = lookup(target);
            travelPath = new Path(finish.portal_x + cx, finish.portal_y + cy, actorX, actorY, 2);
            stampX = actorX; stampY = actorY;
            if (Math.Abs(travelPath.X - finish.portal_x - cx) > 5 && Math.Abs(travelPath.Y - finish.portal_y - cy) > 6)
                sound(19, 2, 0);
        }

        internal string Step(GameRuntimeTransport.CampaignMenuSnapshot menu,
            Func<string, OwnerCampaignMenuCache.Mission> lookup, uint now, int mouseX, int mouseY,
            bool leftHeld, bool allowPan, int centerX, int centerY,
            Func<int, (int width, int height)> extent, Action<int, int> stamp,
            Action<int, int, int> sound, bool terminalEffectEnabled)
        {
            if (viewportPath != null)
            {
                var arrived = viewportPath.Move();
                X = viewportPath.X; Y = viewportPath.Y;
                if (arrived) { viewportPath = null; Ready = true; DescriptionViewportY = Y; }
            }
            else if (Ready && !Travelling && allowPan) Pan(mouseX, mouseY);
            StepBackground(now, leftHeld, sound);
            StepDecorations(menu, now, terminalEffectEnabled, sound);
            Actor = null;
            ActorIsTravelling = Travelling;
            if (Travelling)
            {
                var arrived = travelNoActor;
                if (!travelNoActor)
                {
                    for (var pass = 0; pass < 3; pass++) arrived = travelPath.Move();
                    actorX = travelPath.X; actorY = travelPath.Y;
                    var target = lookup(pendingMission);
                    SetActorDirection(target.portal_x + centerX - actorX, target.portal_y + centerY - actorY);
                    if (previous == null && !menu.completed_levels.Contains(pendingMission) && !arrived &&
                        (Math.Abs(actorX - stampX) > 8 || Math.Abs(actorY - stampY) > 8))
                    { stamp(actorX, actorY); stampX = actorX; stampY = actorY; }
                    SetActorSprite(now, extent);
                }
                if (arrived) { var result = pendingMission; previous = result; pendingMission = null; return result; }
            }
            else
            {
                var completed = menu.available_levels.LastOrDefault(level => menu.completed_levels.Contains(level));
                var frontier = menu.available_levels.FirstOrDefault(level => !menu.completed_levels.Contains(level));
                if (completed != null)
                {
                    var start = lookup(previous ?? completed);
                    var target = lookup(previous != null ? completed : frontier ?? completed);
                    actorX = start.portal_x + centerX; actorY = start.portal_y + centerY;
                    SetActorDirection(target.portal_x + centerX - actorX, target.portal_y + centerY - actorY);
                    SetActorSprite(now, extent);
                }
            }
            return null;
        }

        internal void Pan(int mouseX, int mouseY)
        {
            var edge = false;
            if (mouseX == 0) { X -= shift; edge = true; }
            else if (mouseX >= 638) { X += shift; edge = true; }
            if (mouseY == 0) { Y -= shift; edge = true; }
            else if (mouseY >= 478) { Y += shift; edge = true; }
            X = Math.Clamp(X, 0, 638); Y = Math.Clamp(Y, 0, 478);
            shift = edge ? Math.Min(24, shift + 4) : 0;
        }

        private void SetActorDirection(int dx, int dy)
        {
            // Only quadrants/cardinals are consumed from the source atan table.
            // Integer division quantizes a very small minor axis to table[0].
            if ((Math.Abs(dx) << 8) / Math.Max(1, Math.Abs(dy)) == 0) dx = 0;
            if ((Math.Abs(dy) << 8) / Math.Max(1, Math.Abs(dx)) == 0) dy = 0;
            var first = dx == 0 ? dy >= 0 ? 17 : 1 : dy == 0 ? dx > 0 ? 25 : 9 :
                dx > 0 ? dy > 0 ? 29 : 21 : dy > 0 ? 5 : 13;
            if (first != actorFirst) { actorFirst = first; actorSelector = first; }
        }
        private void SetActorSprite(uint now, Func<int, (int width, int height)> extent)
        {
            var before = extent(actorSelector);
            if (unchecked(now - actorTime) >= 16)
            { actorSelector = actorSelector < actorFirst + 3 ? actorSelector + 1 : actorFirst; actorTime = now; }
            Actor = new Sprite(actorSelector, actorX - before.width / 2, actorY - before.height / 2);
        }

        private void StepBackground(uint now, bool leftHeld, Action<int, int, int> sound)
        {
            backgroundSprites.Clear();
            foreach (var item in backgrounds)
            {
                var row = item.Binding;
                if (item.Phase == 0) { item.Phase = 2; item.Start = now; item.Last = now; }
                else if (item.Phase == 1)
                {
                    if (unchecked(now - item.Last) >= 8)
                    {
                        if (item.Selector <= row.last - 2) item.Selector++;
                        else { if (row.restart == 1) item.Phase = 0; item.Selector = row.first; }
                        item.Last = now;
                    }
                    backgroundSprites.Add(new Sprite(item.Selector, row.x, row.y));
                    if (leftHeld) item.Selector++;
                }
                else if (unchecked(now - item.Start) / 100 > row.delay_seconds)
                {
                    item.Phase = 1;
                    if (row.sound_delay != -1) sound(row.sound, row.play_type, row.loops);
                }
            }
        }

        private void StepDecorations(GameRuntimeTransport.CampaignMenuSnapshot menu, uint now,
            bool terminalEffectEnabled, Action<int, int, int> sound)
        {
            decorationSprites.Clear();
            var frontier = menu.available_levels.Count(level => menu.completed_levels.Contains(level));
            foreach (var item in decorations)
            {
                var row = item.Binding;
                if (row.kind == 2 && menu.phase == "results") continue;
                if (row.kind == 2 && frontier != row.frontier || row.kind == 3 && !terminalEffectEnabled)
                { item.Phase = 0; continue; }
                var initial = row.kind == 1 ? 2 : 1;
                var moving = initial + 1;
                if (item.Phase == 0) { item.Last = now; item.Phase = 1; continue; }
                if (row.kind == 1 && item.Phase == 1)
                { if (unchecked(now - item.Last) / 100 > row.delay_seconds) item.Phase = 2; continue; }
                if (item.Phase == initial)
                {
                    item.X = row.x; item.Y = row.y;
                    item.Path = new Path(row.target_x, row.target_y, row.x, row.y, 2);
                    item.Selector = row.first; item.Phase++;
                    sound(row.start_sound, row.start_play_type, 0);
                    continue;
                }
                if (item.Phase == moving)
                {
                    var arrived = false;
                    for (var pass = 0; pass < 4; pass++) arrived = item.Path.Move();
                    item.X = item.Path.X; item.Y = item.Path.Y;
                    if (unchecked(now - item.Last) >= 8)
                    { item.Selector = item.Selector < row.last ? item.Selector + 1 : row.first; item.Last = now; }
                    decorationSprites.Add(new Sprite(item.Selector, item.X, item.Y));
                    if (arrived && row.kind != 3)
                    {
                        item.Selector = row.finish_first; item.Last = now; item.Phase++;
                        sound(row.finish_sound, row.finish_play_type, 0);
                    }
                }
                else if (item.Phase == moving + 1)
                {
                    var finished = false;
                    if (unchecked(now - item.Last) >= 8)
                    {
                        if (item.Selector < row.finish_last) item.Selector++;
                        else { finished = true; item.Selector = row.kind == 1 ? row.finish_last : row.finish_first; }
                        item.Last = now;
                    }
                    if (!finished) decorationSprites.Add(new Sprite(item.Selector, item.X, item.Y));
                    else item.Phase = row.kind == 1 ? 0 : item.Phase + 1;
                }
            }
        }
    }
}
