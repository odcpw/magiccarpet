using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.IO;
using System.Reflection;
using System.Runtime.Serialization;
using System.Runtime.Serialization.Json;
using System.Text;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using Newtonsoft.Json.Serialization;

namespace MagicCarpet.Native
{
    // Accelerate only canonical owner-produced identities. Other syntax,
    // layouts and coercions retain the existing DataContract behavior.
    internal sealed class RuntimeIdentityJsonDecoder
    {
        private sealed class Layout
        {
            internal readonly Dictionary<string, FieldInfo> Fields = new Dictionary<string, FieldInfo>(StringComparer.Ordinal);
            internal readonly List<string> Required = new List<string>();
            internal readonly HashSet<string> NamesIgnoringCase = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
            internal Layout(Type type)
            {
                foreach (var field in type.GetFields(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic))
                    if (!field.IsNotSerialized)
                    {
                        Fields.Add(field.Name, field);
                        NamesIgnoringCase.Add(field.Name);
                        if (!field.IsDefined(typeof(OptionalFieldAttribute), false)) Required.Add(field.Name);
                    }
            }
        }

        private static readonly UTF8Encoding StrictUtf8 = new UTF8Encoding(false, true);
        private static readonly ConcurrentDictionary<Type, Layout> Layouts = new ConcurrentDictionary<Type, Layout>();
        private static readonly JsonLoadSettings LoadSettings = new JsonLoadSettings
        {
            DuplicatePropertyNameHandling = DuplicatePropertyNameHandling.Error
        };
        private static readonly JsonSerializerSettings Settings = new JsonSerializerSettings
        {
            ContractResolver = new DefaultContractResolver { IgnoreSerializableAttribute = false },
            TypeNameHandling = TypeNameHandling.None,
            MetadataPropertyHandling = MetadataPropertyHandling.Ignore,
            DateParseHandling = DateParseHandling.None,
            NullValueHandling = NullValueHandling.Include,
            CheckAdditionalContent = true
        };
        private readonly Type identityType;

        internal RuntimeIdentityJsonDecoder(Type identityType) => this.identityType = identityType;

        internal object Decode(byte[] bytes)
        {
            try
            {
                var text = StrictUtf8.GetString(bytes);
                var document = JToken.Parse(text, LoadSettings);
                // The legacy JSON reader rejects these raw XML noncharacters.
                if (text.IndexOf('\ufffe') < 0 && text.IndexOf('\uffff') < 0 && document is JObject &&
                    text == document.ToString(Formatting.None) && CanonicalLayout(document, identityType))
                    return document.ToObject(identityType, JsonSerializer.Create(Settings));
            }
            catch (JsonException) { }
            catch (DecoderFallbackException) { }
            // Preserve both accepted legacy coercions and its rejection of
            // missing required fields, malformed input and null value types.
            using (var stream = new MemoryStream(bytes, false))
                return new DataContractJsonSerializer(identityType).ReadObject(stream);
        }

        private static bool CanonicalLayout(JToken token, Type type)
        {
            if (token.Type == JTokenType.Null) return !type.IsValueType;
            if (type == typeof(string)) return token.Type == JTokenType.String;
            if (type == typeof(bool)) return token.Type == JTokenType.Boolean;
            if (type.IsPrimitive && type != typeof(float) && type != typeof(double) && type != typeof(char))
                return token.Type == JTokenType.Integer;
            if (type.IsArray)
            {
                if (!(token is JArray array) || type.GetArrayRank() != 1) return false;
                var element = type.GetElementType();
                foreach (var item in array) if (!CanonicalLayout(item, element)) return false;
                return true;
            }
            if (!type.IsSerializable || (type.BaseType != typeof(object) && type.BaseType != typeof(ValueType)) ||
                type.IsDefined(typeof(DataContractAttribute), false) || !(token is JObject obj)) return false;
            var layout = Layouts.GetOrAdd(type, value => new Layout(value));
            foreach (var property in obj.Properties())
            {
                if (!layout.Fields.TryGetValue(property.Name, out var field))
                {
                    // Both serializers ignore unrelated extension members.
                    // Case aliases and DCS's metadata token need legacy rules.
                    if (property.Name == "__type" || layout.NamesIgnoringCase.Contains(property.Name)) return false;
                    continue;
                }
                if (!CanonicalLayout(property.Value, field.FieldType)) return false;
            }
            foreach (var name in layout.Required) if (!obj.ContainsKey(name)) return false;
            return true;
        }
    }
}
