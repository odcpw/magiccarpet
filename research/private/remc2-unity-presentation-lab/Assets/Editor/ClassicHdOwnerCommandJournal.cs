#if UNITY_EDITOR
using System;
using System.IO;
using UnityEngine;

namespace MagicCarpet.PresentationLab.Editor
{
    /// <summary>
    /// Reads the owner-local PlaythroughRecorder command stream without
    /// importing its controller, timeline, or owner payload into the
    /// presentation lab.  The stream is deliberately fail-closed: only the
    /// sealed 10-byte native command encoding is accepted.  This is the
    /// PlaythroughRecorder file contract: bytes 6-7 carry its additive
    /// requested-spell field and bytes 8-9 are required to be zero.  It is
    /// not a raw in-memory dump of the generic Rust PlayerCommand, whose
    /// action payload has a different interpretation for those words.
    /// </summary>
    public sealed class ClassicHdOwnerCommandJournal : IDisposable
    {
        private const string Schema = "mc2-unity-playthrough/v1";
        private const string CommandEncoding = "mc-native-command-10-byte-le/v1";
        private const int CommandBytes = 10;
        private const int MaxCommandCount = 2_000_000;

        private readonly FileStream stream;
        private readonly BinaryReader reader;
        private readonly int commandCount;
        private int nextCommandIndex;
        private bool disposed;

        public string RootPath { get; private set; }
        public string ManifestPath { get; private set; }
        public string CommandsPath { get; private set; }
        public string ManifestSchema { get; private set; }
        public string ManifestStatus { get; private set; }
        public string ManifestCommandEncoding { get; private set; }
        public string RecordedPackagePath { get; private set; }
        public string RecordedClosurePath { get; private set; }
        public int CommandCount { get { return commandCount; } }
        public int ReadCount { get { return nextCommandIndex; } }

        public bool MatchesExternalInputs(
            string expectedPackagePath,
            string expectedClosurePath,
            out string reason)
        {
            reason = string.Empty;
            try
            {
                string recordedPackage = Path.GetFullPath(RecordedPackagePath);
                string recordedClosure = Path.GetFullPath(RecordedClosurePath);
                string expectedPackage = Path.GetFullPath(expectedPackagePath);
                string expectedClosure = Path.GetFullPath(expectedClosurePath);
                if (!string.Equals(recordedPackage, expectedPackage, StringComparison.Ordinal))
                {
                    reason = "owner command journal package binding mismatch: recorded=" +
                        recordedPackage + " requested=" + expectedPackage;
                    return false;
                }
                if (!string.Equals(recordedClosure, expectedClosure, StringComparison.Ordinal))
                {
                    reason = "owner command journal closure binding mismatch: recorded=" +
                        recordedClosure + " requested=" + expectedClosure;
                    return false;
                }
                return true;
            }
            catch (Exception exception)
            {
                reason = "owner command journal external input binding failed: " + exception.Message;
                return false;
            }
        }

        private ClassicHdOwnerCommandJournal(
            string rootPath,
            string manifestPath,
            string commandsPath,
            PlaythroughManifest manifest,
            FileStream commandStream)
        {
            RootPath = rootPath;
            ManifestPath = manifestPath;
            CommandsPath = commandsPath;
            ManifestSchema = manifest.schema;
            ManifestStatus = manifest.status;
            ManifestCommandEncoding = manifest.command_encoding;
            RecordedPackagePath = manifest.package_path;
            RecordedClosurePath = manifest.closure_path;
            commandCount = manifest.command_count;
            stream = commandStream;
            reader = new BinaryReader(stream);
        }

        public static bool TryOpen(string requestedPath, out ClassicHdOwnerCommandJournal journal, out string reason)
        {
            journal = null;
            reason = string.Empty;
            if (string.IsNullOrWhiteSpace(requestedPath))
            {
                reason = "owner command journal path is empty";
                return false;
            }

            string rootPath;
            string manifestPath;
            string commandsPath;
            FileStream commandStream = null;
            try
            {
                rootPath = Path.GetFullPath(requestedPath);
                if (!Directory.Exists(rootPath))
                {
                    reason = "owner command journal directory does not exist: " + rootPath;
                    return false;
                }

                RequireNotReparsePoint(rootPath, "owner command journal directory");
                manifestPath = ConfinedPath(rootPath, "manifest.json");
                commandsPath = ConfinedPath(rootPath, "commands.bin");
                RequireNotReparsePoint(manifestPath, "owner command journal manifest");
                RequireNotReparsePoint(commandsPath, "owner command journal commands");
                if (!File.Exists(manifestPath) || !File.Exists(commandsPath))
                {
                    reason = "owner command journal requires manifest.json and commands.bin";
                    return false;
                }

                PlaythroughManifest manifest = JsonUtility.FromJson<PlaythroughManifest>(
                    File.ReadAllText(manifestPath));
                ValidateManifest(manifest);
                long expectedBytes = checked((long)manifest.command_count * CommandBytes);
                long actualBytes = new FileInfo(commandsPath).Length;
                if (actualBytes != expectedBytes)
                {
                    reason = "owner command journal byte count mismatch: expected=" +
                        expectedBytes + " actual=" + actualBytes;
                    return false;
                }

                commandStream = new FileStream(
                    commandsPath,
                    FileMode.Open,
                    FileAccess.Read,
                    FileShare.Read,
                    CommandBytes,
                    FileOptions.SequentialScan);
                journal = new ClassicHdOwnerCommandJournal(
                    rootPath,
                    manifestPath,
                    commandsPath,
                    manifest,
                    commandStream);
                commandStream = null;
                return true;
            }
            catch (Exception exception)
            {
                reason = "owner command journal rejected: " + exception.Message;
                return false;
            }
            finally
            {
                if (commandStream != null)
                {
                    commandStream.Dispose();
                }
            }
        }

        public bool ReadNext(out ClassicHdOwnerCommand command, out string reason)
        {
            command = default(ClassicHdOwnerCommand);
            reason = string.Empty;
            if (disposed)
            {
                reason = "owner command journal is disposed";
                return false;
            }

            if (nextCommandIndex >= commandCount)
            {
                reason = "owner command journal has no remaining commands";
                return false;
            }

            try
            {
                byte[] bytes = reader.ReadBytes(CommandBytes);
                if (bytes.Length != CommandBytes)
                {
                    reason = "owner command journal ended before its manifest command count";
                    return false;
                }

                if (bytes[8] != 0 || bytes[9] != 0)
                {
                    reason = "owner command journal reserved bytes are non-zero at command " +
                        nextCommandIndex;
                    return false;
                }

                short requestedSpell = unchecked((short)(bytes[6] | (bytes[7] << 8)));
                command = new ClassicHdOwnerCommand(
                    unchecked((sbyte)bytes[0]),
                    unchecked((sbyte)bytes[1]),
                    unchecked((sbyte)bytes[2]),
                    unchecked((sbyte)bytes[3]),
                    unchecked((sbyte)bytes[4]),
                    unchecked((sbyte)bytes[5]),
                    requestedSpell);
                nextCommandIndex++;
                return true;
            }
            catch (Exception exception)
            {
                reason = "owner command journal read failed at command " +
                    nextCommandIndex + ": " + exception.Message;
                return false;
            }
        }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            reader.Dispose();
            stream.Dispose();
        }

        private static void ValidateManifest(PlaythroughManifest manifest)
        {
            if (manifest == null)
            {
                throw new InvalidDataException("owner command journal manifest is invalid JSON");
            }
            if (!string.Equals(manifest.schema, Schema, StringComparison.Ordinal))
            {
                throw new InvalidDataException("unsupported owner command journal schema");
            }
            if (!string.Equals(manifest.status, "complete", StringComparison.Ordinal))
            {
                throw new InvalidDataException("owner command journal is not complete");
            }
            if (!string.Equals(manifest.command_encoding, CommandEncoding, StringComparison.Ordinal) ||
                manifest.command_bytes != CommandBytes)
            {
                throw new InvalidDataException("owner command journal encoding is not the sealed 10-byte native format");
            }
            if (manifest.command_count <= 0 || manifest.command_count > MaxCommandCount)
            {
                throw new InvalidDataException("owner command journal command count is outside the supported bound");
            }
            if (string.IsNullOrWhiteSpace(manifest.package_path) ||
                string.IsNullOrWhiteSpace(manifest.closure_path))
            {
                throw new InvalidDataException("owner command journal does not identify its external owner package and closure");
            }
        }

        private static string ConfinedPath(string rootPath, string fileName)
        {
            string path = Path.GetFullPath(Path.Combine(rootPath, fileName));
            string prefix = rootPath.TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar;
            if (!path.StartsWith(prefix, StringComparison.Ordinal) ||
                !string.Equals(Path.GetFileName(path), fileName, StringComparison.Ordinal))
            {
                throw new InvalidDataException("owner command journal path escaped its root");
            }
            return path;
        }

        private static void RequireNotReparsePoint(string path, string description)
        {
            if ((File.GetAttributes(path) & FileAttributes.ReparsePoint) != 0)
            {
                throw new InvalidDataException(description + " is a symlink or reparse point");
            }
        }

        [Serializable]
        private sealed class PlaythroughManifest
        {
            public string schema;
            public string status;
            public string package_path;
            public string closure_path;
            public string command_encoding;
            public int command_bytes;
            public int command_count;
        }
    }
}
#endif
