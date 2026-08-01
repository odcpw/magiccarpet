using System;
using System.IO;

namespace MagicCarpet.PresentationLab
{
    [Serializable]
    public sealed class OwnerInputValidation
    {
        private OwnerInputValidation(bool isValid, string reason)
        {
            IsValid = isValid;
            Reason = reason ?? string.Empty;
        }

        public bool IsValid { get; private set; }
        public string Reason { get; private set; }

        public static OwnerInputValidation Valid()
        {
            return new OwnerInputValidation(true, "owner package, closure, and cache paths are external to the presentation lab");
        }

        public static OwnerInputValidation Invalid(string reason)
        {
            return new OwnerInputValidation(false, reason);
        }
    }

    public static class OwnerInputValidator
    {
        public static OwnerInputValidation Validate(
            PresentationModeId mode,
            string packagePath,
            string closurePath,
            string cachePath,
            string projectRoot)
        {
            if (mode != PresentationModeId.ClassicHd && mode != PresentationModeId.UnityEnhanced)
            {
                return OwnerInputValidation.Invalid("unsupported presentation mode");
            }

            string[] paths = { packagePath, closurePath, cachePath };
            for (int index = 0; index < paths.Length; index++)
            {
                if (string.IsNullOrWhiteSpace(paths[index]))
                {
                    return OwnerInputValidation.Invalid("missing required owner input path " + (index + 1).ToString());
                }

                if (!Path.IsPathRooted(paths[index]))
                {
                    return OwnerInputValidation.Invalid("owner input paths must be absolute");
                }

                if (IsInside(projectRoot, paths[index]))
                {
                    return OwnerInputValidation.Invalid("owner inputs must remain outside the presentation-lab project");
                }
            }

            if (!Directory.Exists(packagePath))
            {
                return OwnerInputValidation.Invalid("owner package directory does not exist");
            }

            if (!File.Exists(closurePath))
            {
                return OwnerInputValidation.Invalid("owner closure file does not exist");
            }

            if (!Directory.Exists(cachePath))
            {
                return OwnerInputValidation.Invalid("owner asset-cache directory does not exist");
            }

            return OwnerInputValidation.Valid();
        }

        private static bool IsInside(string rootPath, string candidatePath)
        {
            if (string.IsNullOrWhiteSpace(rootPath))
            {
                return false;
            }

            try
            {
                string root = Path.GetFullPath(rootPath).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar) + Path.DirectorySeparatorChar;
                string candidate = Path.GetFullPath(candidatePath).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
                return string.Equals(candidate, root.TrimEnd(Path.DirectorySeparatorChar), StringComparison.OrdinalIgnoreCase) ||
                       candidate.StartsWith(root, StringComparison.OrdinalIgnoreCase);
            }
            catch (Exception)
            {
                return true;
            }
        }
    }
}
