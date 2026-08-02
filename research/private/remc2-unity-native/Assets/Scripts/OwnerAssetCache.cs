using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using UnityEngine;

namespace MagicCarpet.Native
{
    public sealed class OwnerAssetCache
    {
        public const string Schema = "mc2-owner-unity-asset-cache/v1";
        private const string ExactManifestFileHash =
            "1eeb90e39927cb28e214e017c2603f3200ee2d3899f3f81ce53063f6d64a63d6";
        private const string ExactContentHash =
            "cb58b2190d3f6ed7c676e0180d0dfc909c7fe692e42946a927f887568be2eb8a";
        private const string ExactContractFileHash =
            "170bf5a1b9ee1190f89cf4d34ac9f139ca7ceb28761cd45785026575b112f110";
        private const string ExactContractSelfHash =
            "8007568bcfb4180137a7e0432fe2f094d44817fd771f90d4a98017036d9efcd4";
        private const string ExactSealHash =
            "becd3151922ccde6508ef4a77bf7987981c7fdedad04a3c040a9cdd72c88a96a";
        private const string PinnedRemc2Commit =
            "6806c7204d17220ecc0430759f784aea8ee2ddb2";

        [Serializable]
        private sealed class Manifest
        {
            public string schema;
            public Bindings bindings;
            public Output[] outputs;
            public string cache_content_sha256;
        }

        [Serializable]
        private sealed class Bindings
        {
            public string level_export_package_sha256;
            public string level_asset_closure_sha256;
            public string plan29_contract_file_sha256;
            public string plan29_contract_self_sha256;
        }

        [Serializable]
        private sealed class ConformanceSeal
        {
            public string schema;
            public string contract_file_sha256;
            public string contract_self_sha256;
            public string remc2_commit;
            public string seal_sha256;
        }

        [Serializable]
        private sealed class Output
        {
            public string path;
            public long bytes;
            public string sha256;
        }

        public string RootPath { get; }
        public string ContentHash { get; }
        public int OutputCount { get; }
        private readonly HashSet<string> approvedPaths;

        private OwnerAssetCache(
            string rootPath,
            string contentHash,
            int outputCount,
            HashSet<string> approvedPaths)
        {
            RootPath = rootPath;
            ContentHash = contentHash;
            OutputCount = outputCount;
            this.approvedPaths = approvedPaths;
        }

        public byte[] ReadOutput(string relative)
        {
            var canonical = relative.Replace(Path.DirectorySeparatorChar, '/');
            if (!approvedPaths.Contains(canonical))
                throw new InvalidDataException($"path is not a validated cache output: {relative}");
            return File.ReadAllBytes(ConfinedPath(RootPath, canonical));
        }

        public string ReadOutputText(string relative)
        {
            return Encoding.UTF8.GetString(ReadOutput(relative));
        }

        public static string ResolveOptionalPath(string[] arguments)
        {
            for (var index = 0; index + 1 < arguments.Length; index++)
            {
                if (arguments[index] == "-mc-asset-cache")
                    return Path.GetFullPath(arguments[index + 1]);
            }
            var environment = Environment.GetEnvironmentVariable("MC_ASSET_CACHE_PATH");
            return string.IsNullOrWhiteSpace(environment)
                ? null
                : Path.GetFullPath(environment);
        }

        public static OwnerAssetCache OpenAndValidate(
            string rootPath,
            string expectedPackageHash,
            string expectedClosureHash)
        {
            var root = Path.GetFullPath(rootPath);
            var manifestPath = ConfinedPath(root, "manifest.json");
            if ((File.GetAttributes(root) & FileAttributes.ReparsePoint) != 0)
                throw new InvalidDataException("owner asset cache root is a symlink");
            if (!File.Exists(manifestPath))
                throw new InvalidDataException("owner asset cache manifest is missing");
            if ((File.GetAttributes(manifestPath) & FileAttributes.ReparsePoint) != 0)
                throw new InvalidDataException("owner asset cache manifest is a symlink");
            if (Sha256File(manifestPath) != ExactManifestFileHash)
                throw new InvalidDataException("owner asset cache manifest metadata mismatch");
            var manifest = JsonUtility.FromJson<Manifest>(File.ReadAllText(manifestPath));
            if (manifest == null || manifest.schema != Schema)
                throw new InvalidDataException("unsupported owner asset cache schema");
            if (manifest.bindings == null ||
                manifest.bindings.level_export_package_sha256 != expectedPackageHash)
                throw new InvalidDataException("owner asset cache package binding mismatch");
            if (manifest.bindings.level_asset_closure_sha256 != expectedClosureHash)
                throw new InvalidDataException("owner asset cache closure binding mismatch");
            if (manifest.bindings.plan29_contract_file_sha256 != ExactContractFileHash ||
                manifest.bindings.plan29_contract_self_sha256 != ExactContractSelfHash)
                throw new InvalidDataException("owner asset cache PLAN29 contract binding mismatch");
            if (manifest.outputs == null || manifest.outputs.Length == 0)
                throw new InvalidDataException("owner asset cache has no outputs");

            var paths = new HashSet<string>(StringComparer.Ordinal);
            foreach (var output in manifest.outputs)
            {
                if (output == null || string.IsNullOrWhiteSpace(output.path) ||
                    !paths.Add(output.path))
                    throw new InvalidDataException("invalid or duplicate cache output path");
                ConfinedPath(root, output.path);
            }
            var expectedTree = new HashSet<string>(paths, StringComparer.Ordinal)
                { "manifest.json" };
            var actualTree = new HashSet<string>(StringComparer.Ordinal);
            foreach (var path in Directory.GetFileSystemEntries(
                         root, "*", SearchOption.AllDirectories))
            {
                var attributes = File.GetAttributes(path);
                if ((attributes & FileAttributes.ReparsePoint) != 0)
                    throw new InvalidDataException("owner asset cache contains a symlink");
                if ((attributes & FileAttributes.Directory) != 0) continue;
                actualTree.Add(path.Substring(root.Length)
                    .TrimStart(Path.DirectorySeparatorChar)
                    .Replace(Path.DirectorySeparatorChar, '/'));
            }
            if (!actualTree.SetEquals(expectedTree))
                throw new InvalidDataException("owner asset cache tree mismatch");

            var identity = new StringBuilder();
            foreach (var output in manifest.outputs.OrderBy(value => value.path, StringComparer.Ordinal))
            {
                var extension = Path.GetExtension(output.path);
                if (extension.Equals(".DAT", StringComparison.OrdinalIgnoreCase) ||
                    extension.Equals(".TAB", StringComparison.OrdinalIgnoreCase))
                    throw new InvalidDataException("raw DAT/TAB is forbidden in Unity cache");
                var path = ConfinedPath(root, output.path);
                var info = new FileInfo(path);
                if (!info.Exists)
                    throw new InvalidDataException($"cache output missing: {output.path}");
                if (info.Length != output.bytes)
                    throw new InvalidDataException($"cache output truncated: {output.path}");
                var hash = Sha256File(path);
                if (!hash.Equals(output.sha256, StringComparison.Ordinal))
                    throw new InvalidDataException($"cache output hash mismatch: {output.path}");
                identity.Append(output.path).Append('\0')
                    .Append(hash).Append('\0')
                    .Append(output.bytes).Append('\n');
            }
            var identityHash = Sha256Bytes(Encoding.ASCII.GetBytes(identity.ToString()));
            if (!identityHash.Equals(manifest.cache_content_sha256, StringComparison.Ordinal))
                throw new InvalidDataException("owner asset cache identity mismatch");
            if (identityHash != ExactContentHash)
                throw new InvalidDataException("owner asset cache is not the qualified Level 1 cache");
            var seal = JsonUtility.FromJson<ConformanceSeal>(
                File.ReadAllText(ConfinedPath(root, "conformance-identity.json")));
            if (seal == null || seal.schema != "mc2-plan29-cache-conformance-identity/v1" ||
                seal.contract_file_sha256 != ExactContractFileHash ||
                seal.contract_self_sha256 != ExactContractSelfHash ||
                seal.remc2_commit != PinnedRemc2Commit ||
                seal.seal_sha256 != ExactSealHash)
                throw new InvalidDataException("owner asset cache PLAN29 conformance seal mismatch");
            return new OwnerAssetCache(root, identityHash, manifest.outputs.Length, paths);
        }

        private static string ConfinedPath(string root, string relative)
        {
            if (Path.IsPathRooted(relative))
                throw new InvalidDataException("absolute cache output path is forbidden");
            var path = Path.GetFullPath(Path.Combine(root, relative));
            var prefix = root.EndsWith(Path.DirectorySeparatorChar.ToString(), StringComparison.Ordinal)
                ? root
                : root + Path.DirectorySeparatorChar;
            if (!path.StartsWith(prefix, StringComparison.Ordinal))
                throw new InvalidDataException("cache output path escapes cache root");
            return path;
        }

        private static string Sha256File(string path)
        {
            using var stream = File.OpenRead(path);
            using var sha = SHA256.Create();
            return Hex(sha.ComputeHash(stream));
        }

        private static string Sha256Bytes(byte[] bytes)
        {
            using var sha = SHA256.Create();
            return Hex(sha.ComputeHash(bytes));
        }

        private static string Hex(byte[] bytes)
        {
            var builder = new StringBuilder(bytes.Length * 2);
            foreach (var value in bytes) builder.Append(value.ToString("x2"));
            return builder.ToString();
        }
    }
}
