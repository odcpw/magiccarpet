using System;
using System.Collections;
using System.Globalization;
using System.IO;
using UnityEngine;

namespace MagicCarpet.Native
{
    // Opt-in full-frame recording of live gameplay. No actor or projectile is staged.
    internal sealed class ModernCharacterGameplayCapture : MonoBehaviour
    {
        private Func<bool> ready;
        private Func<string> report;
        private string directory;
        private int previousRate;
        internal void Bind(Func<bool> ready, Func<string> report)
        { this.ready = ready; this.report = report; StartCoroutine(Record()); }
        private IEnumerator Record()
        {
            var args = Environment.GetCommandLineArgs();
            var option = Array.IndexOf(args, "-mc-character-gameplay-capture");
            if (option < 0 || option + 1 >= args.Length) throw new ArgumentException("Gameplay capture requires a new directory");
            directory = Path.GetFullPath(args[option + 1]);
            if (Directory.Exists(directory)) throw new IOException("Gameplay capture already exists");
            Directory.CreateDirectory(directory);
            var count = 360;
            var fps = 30;
            var frames = Array.IndexOf(args, "-mc-character-gameplay-frames");
            if (frames >= 0) count = int.Parse(args[frames + 1], CultureInfo.InvariantCulture);
            var rate = Array.IndexOf(args, "-mc-character-gameplay-fps");
            if (rate >= 0) fps = int.Parse(args[rate + 1], CultureInfo.InvariantCulture);
            if (count < 30 || count > 3600) throw new ArgumentOutOfRangeException("Gameplay frames must be 30..3600");
            if (fps != 30 && fps != 60 && fps != 120) throw new ArgumentOutOfRangeException("Gameplay capture fps must be 30, 60 or 120");
            var deadline = Time.realtimeSinceStartup + 120;
            while (!ready())
            {
                if (Time.realtimeSinceStartup > deadline) throw new TimeoutException("Character gameplay never became ready");
                yield return null;
            }
            previousRate = Time.captureFramerate;
            Time.captureFramerate = fps;
            Debug.Log("MC_CHARACTER_GAMEPLAY_RECORDING native_gameplay=True performance_measurement=False");
            using (var records = new StreamWriter(Path.Combine(directory, "native-frames.jsonl")))
                for (var i = 0; i < count; i++)
                {
                    yield return new WaitForEndOfFrame();
                    ScreenCapture.CaptureScreenshot(Path.Combine(directory, $"frame-{i:D5}.png"));
                    records.WriteLine(report());
                }
            yield return new WaitForSecondsRealtime(.5f);
            File.WriteAllText(Path.Combine(directory, "capture.json"),
                $"{{\"frames\":{count},\"fps\":{fps},\"width\":{Screen.width},\"height\":{Screen.height},\"engine\":\"Unity\",\"native_gameplay\":true,\"performance_measurement\":false}}");
            Debug.Log("MC_CHARACTER_GAMEPLAY_COMPLETE frames=" + count);
            Application.Quit();
        }
        private void OnDestroy() { if (directory != null) Time.captureFramerate = previousRate; }
    }
}
