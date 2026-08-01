using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Text;

namespace MagicCarpet.PresentationLab.VoxelMode
{
    // The importer is deliberately editor-only and dependency-free. Unity's
    // JsonUtility cannot preserve arbitrary manifest fields, object keys, or
    // number spelling, so this small parser keeps the semantic contract
    // language-neutral without adding a runtime JSON package.
    internal enum VoxelJsonKind
    {
        Null,
        Boolean,
        Number,
        String,
        Array,
        Object,
    }

    internal sealed class VoxelJsonValue
    {
        private VoxelJsonValue(VoxelJsonKind kind)
        {
            Kind = kind;
        }

        internal VoxelJsonKind Kind { get; }
        internal bool BooleanValue { get; private set; }
        internal string StringValue { get; private set; }
        internal string NumberLexeme { get; private set; }
        internal List<VoxelJsonValue> ArrayValue { get; private set; }
        internal Dictionary<string, VoxelJsonValue> ObjectValue { get; private set; }

        internal static VoxelJsonValue Null() => new VoxelJsonValue(VoxelJsonKind.Null);

        internal static VoxelJsonValue Boolean(bool value)
        {
            return new VoxelJsonValue(VoxelJsonKind.Boolean) { BooleanValue = value };
        }

        internal static VoxelJsonValue Number(string lexeme)
        {
            return new VoxelJsonValue(VoxelJsonKind.Number) { NumberLexeme = lexeme };
        }

        internal static VoxelJsonValue String(string value)
        {
            return new VoxelJsonValue(VoxelJsonKind.String) { StringValue = value };
        }

        internal static VoxelJsonValue Array(List<VoxelJsonValue> values)
        {
            return new VoxelJsonValue(VoxelJsonKind.Array) { ArrayValue = values };
        }

        internal static VoxelJsonValue Object(Dictionary<string, VoxelJsonValue> values)
        {
            return new VoxelJsonValue(VoxelJsonKind.Object) { ObjectValue = values };
        }

        internal VoxelJsonValue Require(string key, string path)
        {
            if (Kind != VoxelJsonKind.Object || !ObjectValue.TryGetValue(key, out var value))
            {
                throw new VoxelManifestException(path, "missing required field '" + key + "'");
            }

            return value;
        }

        internal bool TryGet(string key, out VoxelJsonValue value)
        {
            if (Kind == VoxelJsonKind.Object)
            {
                return ObjectValue.TryGetValue(key, out value);
            }

            value = null;
            return false;
        }

        internal VoxelJsonValue RequireKind(VoxelJsonKind kind, string path)
        {
            if (Kind != kind)
            {
                throw new VoxelManifestException(path, "expected " + kind + ", got " + Kind);
            }

            return this;
        }

        internal string RequireString(string path)
        {
            RequireKind(VoxelJsonKind.String, path);
            return StringValue;
        }

        internal bool RequireBoolean(string path)
        {
            RequireKind(VoxelJsonKind.Boolean, path);
            return BooleanValue;
        }

        internal double RequireNumber(string path)
        {
            RequireKind(VoxelJsonKind.Number, path);
            if (!double.TryParse(NumberLexeme, NumberStyles.Float, CultureInfo.InvariantCulture, out var value) ||
                double.IsNaN(value) || double.IsInfinity(value))
            {
                throw new VoxelManifestException(path, "must be a finite number");
            }

            return value;
        }

        internal long RequireInt64(string path)
        {
            RequireKind(VoxelJsonKind.Number, path);
            if (NumberLexeme.IndexOfAny(new[] { '.', 'e', 'E' }) >= 0 ||
                !long.TryParse(NumberLexeme, NumberStyles.Integer, CultureInfo.InvariantCulture, out var value))
            {
                throw new VoxelManifestException(path, "must be an integer");
            }

            return value;
        }
    }

    internal sealed class VoxelManifestException : Exception
    {
        internal VoxelManifestException(string path, string message)
            : base(path + ": " + message)
        {
            Path = path;
        }

        internal string Path { get; }
    }

    internal static class VoxelJsonParser
    {
        internal static VoxelJsonValue Parse(byte[] utf8, string path)
        {
            if (utf8 == null)
            {
                throw new VoxelManifestException(path, "JSON bytes are null");
            }

            try
            {
                return Parse(Encoding.UTF8.GetString(utf8), path);
            }
            catch (DecoderFallbackException error)
            {
                throw new VoxelManifestException(path, "JSON is not valid UTF-8: " + error.Message);
            }
        }

        internal static VoxelJsonValue Parse(string text, string path)
        {
            var parser = new Parser(text, path);
            var value = parser.ParseValue();
            parser.SkipWhitespace();
            if (!parser.AtEnd)
            {
                throw parser.Error("trailing data after JSON value");
            }

            return value;
        }

        private sealed class Parser
        {
            private readonly string text;
            private readonly string path;
            private int index;

            internal Parser(string text, string path)
            {
                this.text = text ?? throw new ArgumentNullException(nameof(text));
                this.path = path;
            }

            internal bool AtEnd => index >= text.Length;

            internal void SkipWhitespace()
            {
                while (!AtEnd && char.IsWhiteSpace(text[index]))
                {
                    index++;
                }
            }

            internal VoxelManifestException Error(string message)
            {
                return new VoxelManifestException(path + "@" + index, message);
            }

            internal VoxelJsonValue ParseValue()
            {
                SkipWhitespace();
                if (AtEnd)
                {
                    throw Error("unexpected end of JSON");
                }

                switch (text[index])
                {
                    case '{':
                        return ParseObject();
                    case '[':
                        return ParseArray();
                    case '"':
                        return VoxelJsonValue.String(ParseString());
                    case 't':
                        ConsumeLiteral("true");
                        return VoxelJsonValue.Boolean(true);
                    case 'f':
                        ConsumeLiteral("false");
                        return VoxelJsonValue.Boolean(false);
                    case 'n':
                        ConsumeLiteral("null");
                        return VoxelJsonValue.Null();
                    default:
                        if (text[index] == '-' || (text[index] >= '0' && text[index] <= '9'))
                        {
                            return VoxelJsonValue.Number(ParseNumber());
                        }

                        throw Error("unexpected JSON token");
                }
            }

            private VoxelJsonValue ParseObject()
            {
                index++;
                var values = new Dictionary<string, VoxelJsonValue>(StringComparer.Ordinal);
                SkipWhitespace();
                if (!AtEnd && text[index] == '}')
                {
                    index++;
                    return VoxelJsonValue.Object(values);
                }

                while (true)
                {
                    SkipWhitespace();
                    if (AtEnd || text[index] != '"')
                    {
                        throw Error("object key must be a string");
                    }

                    var key = ParseString();
                    if (!values.TryAdd(key, null))
                    {
                        throw Error("duplicate object key '" + key + "'");
                    }

                    SkipWhitespace();
                    if (AtEnd || text[index] != ':')
                    {
                        throw Error("expected ':' after object key");
                    }

                    index++;
                    var value = ParseValue();
                    values[key] = value;
                    SkipWhitespace();
                    if (AtEnd)
                    {
                        throw Error("unterminated object");
                    }

                    if (text[index] == '}')
                    {
                        index++;
                        return VoxelJsonValue.Object(values);
                    }

                    if (text[index] != ',')
                    {
                        throw Error("expected ',' or '}' in object");
                    }

                    index++;
                }
            }

            private VoxelJsonValue ParseArray()
            {
                index++;
                var values = new List<VoxelJsonValue>();
                SkipWhitespace();
                if (!AtEnd && text[index] == ']')
                {
                    index++;
                    return VoxelJsonValue.Array(values);
                }

                while (true)
                {
                    values.Add(ParseValue());
                    SkipWhitespace();
                    if (AtEnd)
                    {
                        throw Error("unterminated array");
                    }

                    if (text[index] == ']')
                    {
                        index++;
                        return VoxelJsonValue.Array(values);
                    }

                    if (text[index] != ',')
                    {
                        throw Error("expected ',' or ']' in array");
                    }

                    index++;
                }
            }

            private string ParseString()
            {
                if (AtEnd || text[index] != '"')
                {
                    throw Error("expected string");
                }

                index++;
                var value = new StringBuilder();
                while (!AtEnd)
                {
                    var character = text[index++];
                    if (character == '"')
                    {
                        return value.ToString();
                    }

                    if (character < 0x20)
                    {
                        throw Error("control character in string");
                    }

                    if (character != '\\')
                    {
                        value.Append(character);
                        continue;
                    }

                    if (AtEnd)
                    {
                        throw Error("unterminated string escape");
                    }

                    var escape = text[index++];
                    switch (escape)
                    {
                        case '"': value.Append('"'); break;
                        case '\\': value.Append('\\'); break;
                        case '/': value.Append('/'); break;
                        case 'b': value.Append('\b'); break;
                        case 'f': value.Append('\f'); break;
                        case 'n': value.Append('\n'); break;
                        case 'r': value.Append('\r'); break;
                        case 't': value.Append('\t'); break;
                        case 'u': value.Append(ParseUnicodeEscape()); break;
                        default: throw Error("unknown string escape");
                    }
                }

                throw Error("unterminated string");
            }

            private char ParseUnicodeEscape()
            {
                if (index + 4 > text.Length)
                {
                    throw Error("short unicode escape");
                }

                var value = 0;
                for (var offset = 0; offset < 4; offset++)
                {
                    var digit = text[index++];
                    var parsed = digit >= '0' && digit <= '9'
                        ? digit - '0'
                        : digit >= 'a' && digit <= 'f'
                            ? digit - 'a' + 10
                            : digit >= 'A' && digit <= 'F'
                                ? digit - 'A' + 10
                                : -1;
                    if (parsed < 0)
                    {
                        throw Error("invalid unicode escape");
                    }

                    value = (value * 16) + parsed;
                }

                return (char)value;
            }

            private string ParseNumber()
            {
                var start = index;
                if (text[index] == '-')
                {
                    index++;
                }

                if (AtEnd)
                {
                    throw Error("short number");
                }

                if (text[index] == '0')
                {
                    index++;
                }
                else if (text[index] >= '1' && text[index] <= '9')
                {
                    while (!AtEnd && text[index] >= '0' && text[index] <= '9')
                    {
                        index++;
                    }
                }
                else
                {
                    throw Error("invalid number integer part");
                }

                if (!AtEnd && text[index] == '.')
                {
                    index++;
                    var fractionStart = index;
                    while (!AtEnd && text[index] >= '0' && text[index] <= '9')
                    {
                        index++;
                    }

                    if (fractionStart == index)
                    {
                        throw Error("fraction requires digits");
                    }
                }

                if (!AtEnd && (text[index] == 'e' || text[index] == 'E'))
                {
                    index++;
                    if (!AtEnd && (text[index] == '+' || text[index] == '-'))
                    {
                        index++;
                    }

                    var exponentStart = index;
                    while (!AtEnd && text[index] >= '0' && text[index] <= '9')
                    {
                        index++;
                    }

                    if (exponentStart == index)
                    {
                        throw Error("exponent requires digits");
                    }
                }

                return text.Substring(start, index - start);
            }

            private void ConsumeLiteral(string literal)
            {
                if (index + literal.Length > text.Length ||
                    !string.Equals(text.Substring(index, literal.Length), literal, StringComparison.Ordinal))
                {
                    throw Error("invalid literal");
                }

                index += literal.Length;
            }
        }
    }

    internal static class VoxelJsonCanonical
    {
        internal static byte[] Utf8(VoxelJsonValue value)
        {
            return Encoding.UTF8.GetBytes(Write(value));
        }

        internal static string Write(VoxelJsonValue value)
        {
            var output = new StringBuilder();
            WriteValue(value, output);
            return output.ToString();
        }

        private static void WriteValue(VoxelJsonValue value, StringBuilder output)
        {
            switch (value.Kind)
            {
                case VoxelJsonKind.Null:
                    output.Append("null");
                    break;
                case VoxelJsonKind.Boolean:
                    output.Append(value.BooleanValue ? "true" : "false");
                    break;
                case VoxelJsonKind.Number:
                    output.Append(NormalizeNumber(value.NumberLexeme));
                    break;
                case VoxelJsonKind.String:
                    WriteString(value.StringValue, output);
                    break;
                case VoxelJsonKind.Array:
                    output.Append('[');
                    for (var index = 0; index < value.ArrayValue.Count; index++)
                    {
                        if (index != 0) output.Append(',');
                        WriteValue(value.ArrayValue[index], output);
                    }
                    output.Append(']');
                    break;
                case VoxelJsonKind.Object:
                    output.Append('{');
                    var keys = new List<string>(value.ObjectValue.Keys);
                    keys.Sort(StringComparer.Ordinal);
                    for (var index = 0; index < keys.Count; index++)
                    {
                        if (index != 0) output.Append(',');
                        WriteString(keys[index], output);
                        output.Append(':');
                        WriteValue(value.ObjectValue[keys[index]], output);
                    }
                    output.Append('}');
                    break;
                default:
                    throw new ArgumentOutOfRangeException();
            }
        }

        private static string NormalizeNumber(string lexeme)
        {
            // Qualified BUILD-29 exports contain canonical decimal/integer
            // numbers. Normalize equivalent authored spellings so the editor
            // hash agrees with Python for ordinary fixed-point values, while
            // rejecting non-finite values before this point.
            var hasExponent = lexeme.IndexOfAny(new[] { 'e', 'E' }) >= 0;
            if (!hasExponent)
            {
                if (lexeme.IndexOf('.') < 0)
                {
                    if (lexeme == "-0") return "0";
                    return lexeme;
                }

                if (!double.TryParse(lexeme, NumberStyles.Float, CultureInfo.InvariantCulture, out var decimalValue) ||
                    double.IsNaN(decimalValue) || double.IsInfinity(decimalValue))
                {
                    throw new VoxelManifestException("manifest", "number is not finite");
                }

                if (decimalValue == 0.0 && lexeme.StartsWith("-", StringComparison.Ordinal))
                {
                    return "-0.0";
                }

                var normalizedDecimal = decimalValue.ToString("R", CultureInfo.InvariantCulture);
                if (normalizedDecimal.IndexOfAny(new[] { 'e', 'E' }) < 0 &&
                    normalizedDecimal.IndexOf('.') < 0)
                {
                    normalizedDecimal += ".0";
                }

                return normalizedDecimal.ToLowerInvariant();
            }

            if (!double.TryParse(lexeme, NumberStyles.Float, CultureInfo.InvariantCulture, out var value) ||
                double.IsNaN(value) || double.IsInfinity(value))
            {
                throw new VoxelManifestException("manifest", "number is not finite");
            }

            var normalized = value.ToString("R", CultureInfo.InvariantCulture).ToLowerInvariant();
            var exponentIndex = normalized.IndexOf('e');
            if (exponentIndex < 0)
            {
                return normalized.IndexOf('.') >= 0 ? normalized : normalized + ".0";
            }

            var exponent = normalized.Substring(exponentIndex + 1);
            var sign = string.Empty;
            if (exponent.StartsWith("+", StringComparison.Ordinal) || exponent.StartsWith("-", StringComparison.Ordinal))
            {
                sign = exponent.Substring(0, 1);
                exponent = exponent.Substring(1);
            }

            exponent = exponent.TrimStart('0');
            if (exponent.Length == 0) exponent = "0";
            if (exponent.Length == 1) exponent = "0" + exponent;
            return normalized.Substring(0, exponentIndex) + "e" + sign + exponent;
        }

        private static void WriteString(string value, StringBuilder output)
        {
            output.Append('"');
            foreach (var character in value ?? string.Empty)
            {
                switch (character)
                {
                    case '"': output.Append("\\\""); break;
                    case '\\': output.Append("\\\\"); break;
                    case '\b': output.Append("\\b"); break;
                    case '\f': output.Append("\\f"); break;
                    case '\n': output.Append("\\n"); break;
                    case '\r': output.Append("\\r"); break;
                    case '\t': output.Append("\\t"); break;
                    default:
                        if (character < 0x20 || character > 0x7e)
                        {
                            output.Append("\\u");
                            output.Append(((int)character).ToString("x4", CultureInfo.InvariantCulture));
                        }
                        else
                        {
                            output.Append(character);
                        }
                        break;
                }
            }

            output.Append('"');
        }
    }
}
