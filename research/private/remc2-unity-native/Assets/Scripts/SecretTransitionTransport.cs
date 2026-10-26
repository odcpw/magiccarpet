using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.Serialization;
using System.Runtime.Serialization.Json;
using System.Xml;
using UnityEngine;

namespace MagicCarpet.Native
{
    /// <summary>
    /// Typed consumer DTOs and validation for the Rust-owned accepted runtime
    /// boundary receipt. The discriminator, never the resulting mode or cue,
    /// selects the transition kind.
    /// </summary>
    internal static class SecretTransitionTransport
    {
        internal const string ReceiptContract =
            "carpet-flight-secret-transition-receipt/v1";
        internal const string OrdinaryCompletionBoundaryKind = "ordinary_completion";
        internal const string SecretTransitionBoundaryKind = "secret_transition";
        internal const string EntryKind = "entry";
        internal const string CompletedReturnKind = "completed_return";
        internal const string AbandonedReturnKind = "abandoned_return";

        [DataContract]
        internal sealed class AcceptedBoundaryReceipt : IExtensibleDataObject
        {
            private bool ordinaryCompletionFieldPresent;
            private bool secretTransitionFieldPresent;
            private GameRuntimeTransport.CompletionTransitionReceipt ordinaryCompletion;
            private Receipt secretTransition;

            [DataMember(Name = "kind", IsRequired = true, Order = 1)]
            public string kind;

            [DataMember(Name = "ordinary_completion", EmitDefaultValue = false, Order = 2)]
            public GameRuntimeTransport.CompletionTransitionReceipt ordinary_completion
            {
                get => ordinaryCompletion;
                set
                {
                    ordinaryCompletionFieldPresent = true;
                    ordinaryCompletion = value;
                }
            }

            [DataMember(Name = "secret_transition", EmitDefaultValue = false, Order = 3)]
            public Receipt secret_transition
            {
                get => secretTransition;
                set
                {
                    secretTransitionFieldPresent = true;
                    secretTransition = value;
                }
            }

            public ExtensionDataObject ExtensionData { get; set; }

            [OnDeserializing]
            private void BeginDeserialization(StreamingContext context)
            {
                ordinaryCompletionFieldPresent = false;
                secretTransitionFieldPresent = false;
            }

            internal bool HasOrdinaryCompletionField => ordinaryCompletionFieldPresent;
            internal bool HasSecretTransitionField => secretTransitionFieldPresent;
        }

        [DataContract]
        internal sealed class Receipt : IExtensibleDataObject
        {
            [DataMember(Name = "receipt_contract", IsRequired = true, Order = 1)]
            public string receipt_contract;

            [DataMember(Name = "kind", IsRequired = true, Order = 2)]
            public string kind;

            [DataMember(Name = "before", IsRequired = true, Order = 3)]
            public GameRuntimeTransport.Identity before;

            [DataMember(Name = "after", IsRequired = true, Order = 4)]
            public GameRuntimeTransport.Identity after;

            [DataMember(Name = "source_session", IsRequired = true, Order = 5)]
            public EngineSessionTransport.Identity source_session;

            [DataMember(Name = "origin_level_id", IsRequired = true, Order = 6)]
            public string origin_level_id;

            [DataMember(Name = "secret_level_id", IsRequired = true, Order = 7)]
            public string secret_level_id;

            [DataMember(Name = "receipt_sha256", IsRequired = true, Order = 8)]
            public string receipt_sha256;

            public ExtensionDataObject ExtensionData { get; set; }
        }

        /// <summary>Decode one bounded CAPI envelope and reject unknown union fields.</summary>
        internal static AcceptedBoundaryReceipt Decode(byte[] bytes, uint maximumLength)
        {
            if (bytes == null || bytes.Length == 0 || checked((uint)bytes.Length) > maximumLength)
                throw new InvalidDataException("accepted boundary receipt exceeds its JSON bounds");

            var wireShape = ValidateWireShape(bytes);
            using var stream = new MemoryStream(bytes, false);
            var serializer = new DataContractJsonSerializer(typeof(AcceptedBoundaryReceipt));
            var envelope = serializer.ReadObject(stream) as AcceptedBoundaryReceipt;
            if (stream.Position != stream.Length)
                throw new InvalidDataException("accepted boundary receipt has trailing JSON data");
            if (envelope == null || envelope.kind != wireShape.kind ||
                envelope.HasOrdinaryCompletionField != wireShape.hasOrdinaryCompletion ||
                envelope.HasSecretTransitionField != wireShape.hasSecretTransition)
            {
                throw new InvalidDataException("accepted boundary receipt does not match its validated JSON shape");
            }
            // The exact raw field scan above is authoritative for unknown and
            // duplicate members. Discard opaque extension metadata only after
            // that scan has proven both objects contain their complete schema.
            envelope.ExtensionData = null;
            if (envelope.secret_transition != null)
                envelope.secret_transition.ExtensionData = null;
            ValidateEnvelope(envelope);
            return envelope;
        }

        /// <summary>Rejects unknown tags and envelopes carrying zero or two receipts.</summary>
        internal static void ValidateEnvelope(AcceptedBoundaryReceipt envelope)
        {
            if (envelope == null || envelope.ExtensionData != null)
                throw new InvalidDataException("accepted boundary receipt has unknown or missing fields");

            var hasOrdinary = envelope.ordinary_completion != null;
            var hasSecret = envelope.secret_transition != null;
            if (envelope.kind == OrdinaryCompletionBoundaryKind)
            {
                if (!envelope.HasOrdinaryCompletionField || !hasOrdinary ||
                    envelope.HasSecretTransitionField)
                    throw new InvalidDataException("ordinary boundary envelope must carry only its ordinary receipt");
                return;
            }
            if (envelope.kind == SecretTransitionBoundaryKind)
            {
                if (envelope.HasOrdinaryCompletionField ||
                    !envelope.HasSecretTransitionField || !hasSecret)
                    throw new InvalidDataException("secret boundary envelope must carry only its secret receipt");
                if (envelope.secret_transition.ExtensionData != null)
                    throw new InvalidDataException("secret transition receipt contains unknown fields");
                return;
            }
            throw new InvalidDataException("accepted boundary receipt has an unknown kind");
        }

        private readonly struct BoundaryWireShape
        {
            internal readonly string kind;
            internal readonly bool hasOrdinaryCompletion;
            internal readonly bool hasSecretTransition;

            internal BoundaryWireShape(
                string kind,
                bool hasOrdinaryCompletion,
                bool hasSecretTransition)
            {
                this.kind = kind;
                this.hasOrdinaryCompletion = hasOrdinaryCompletion;
                this.hasSecretTransition = hasSecretTransition;
            }
        }

        /// <summary>
        /// Inspect the JSON-to-Infoset structure before DataContract decoding.
        /// This preserves strict field rejection without treating an opaque
        /// ExtensionDataObject as proof that a valid member was unknown.
        /// </summary>
        private static BoundaryWireShape ValidateWireShape(byte[] bytes)
        {
            if (bytes == null)
                throw new InvalidDataException("accepted boundary receipt has no JSON bytes");
            if (bytes.Length == 0)
                throw new InvalidDataException("accepted boundary receipt has empty JSON bytes");

            var quotas = new XmlDictionaryReaderQuotas
            {
                MaxDepth = 64,
                MaxStringContentLength = bytes.Length,
                MaxArrayLength = bytes.Length,
                MaxBytesPerRead = Math.Min(bytes.Length, 4096),
                MaxNameTableCharCount = bytes.Length
            };

            using var reader = JsonReaderWriterFactory.CreateJsonReader(bytes, quotas);
            if (reader.MoveToContent() != XmlNodeType.Element)
                throw new InvalidDataException("accepted boundary receipt must be a JSON object");

            BeginJsonObject(reader, "root", "accepted boundary receipt");
            var rootFields = new HashSet<string>(StringComparer.Ordinal);
            string kind = null;
            var hasOrdinaryCompletion = false;
            var hasSecretTransition = false;

            while (reader.MoveToContent() == XmlNodeType.Element)
            {
                var field = reader.LocalName;
                AddUniqueField(rootFields, field, new[] {
                    "kind", "ordinary_completion", "secret_transition"
                }, "accepted boundary receipt");

                if (field == "kind")
                {
                    kind = ReadJsonString(reader, "kind", "accepted boundary kind");
                }
                else if (field == "ordinary_completion")
                {
                    RequireJsonElement(reader, "ordinary_completion", "object",
                        "ordinary completion receipt");
                    hasOrdinaryCompletion = true;
                    reader.Skip();
                }
                else
                {
                    hasSecretTransition = true;
                    ValidateSecretReceiptWireShape(reader);
                }
            }

            EndJsonObject(reader, "root", "accepted boundary receipt");
            if (reader.MoveToContent() != XmlNodeType.None)
                throw new InvalidDataException("accepted boundary receipt has trailing JSON data");

            if (kind == OrdinaryCompletionBoundaryKind)
            {
                if (!hasOrdinaryCompletion || hasSecretTransition)
                    throw new InvalidDataException(
                        "ordinary boundary envelope must carry only its ordinary receipt");
            }
            else if (kind == SecretTransitionBoundaryKind)
            {
                if (hasOrdinaryCompletion || !hasSecretTransition)
                    throw new InvalidDataException(
                        "secret boundary envelope must carry only its secret receipt");
            }
            else
            {
                throw new InvalidDataException("accepted boundary receipt has an unknown kind");
            }

            return new BoundaryWireShape(kind, hasOrdinaryCompletion, hasSecretTransition);
        }

        private static void ValidateSecretReceiptWireShape(XmlDictionaryReader reader)
        {
            BeginJsonObject(reader, "secret_transition", "secret transition receipt");
            var fields = new HashSet<string>(StringComparer.Ordinal);
            var requiredFields = new[] {
                "receipt_contract", "kind", "before", "after", "source_session",
                "origin_level_id", "secret_level_id", "receipt_sha256"
            };

            while (reader.MoveToContent() == XmlNodeType.Element)
            {
                var field = reader.LocalName;
                AddUniqueField(fields, field, requiredFields, "secret transition receipt");
                if (field == "before" || field == "after" || field == "source_session")
                {
                    RequireJsonElement(reader, field, "object", "secret transition " + field);
                    reader.Skip();
                }
                else
                {
                    ReadJsonString(reader, field, "secret transition " + field);
                }
            }

            EndJsonObject(reader, "secret_transition", "secret transition receipt");
            if (fields.Count != requiredFields.Length)
                throw new InvalidDataException("secret transition receipt is missing required fields");
        }

        private static void BeginJsonObject(
            XmlDictionaryReader reader,
            string elementName,
            string contract)
        {
            RequireJsonElement(reader, elementName, "object", contract);
            if (reader.IsEmptyElement)
                throw new InvalidDataException(contract + " is missing required fields");
            reader.ReadStartElement();
        }

        private static void EndJsonObject(
            XmlDictionaryReader reader,
            string elementName,
            string contract)
        {
            if (reader.NodeType != XmlNodeType.EndElement ||
                reader.LocalName != elementName ||
                reader.NamespaceURI.Length != 0)
            {
                throw new InvalidDataException(contract + " has an invalid object shape");
            }
            reader.ReadEndElement();
        }

        private static void RequireJsonElement(
            XmlDictionaryReader reader,
            string elementName,
            string type,
            string contract)
        {
            if (reader.NodeType != XmlNodeType.Element ||
                reader.LocalName != elementName ||
                reader.NamespaceURI.Length != 0 ||
                reader.GetAttribute("type") != type)
            {
                throw new InvalidDataException(contract + " has an invalid JSON type");
            }

            for (var index = 0; index < reader.AttributeCount; index++)
            {
                reader.MoveToAttribute(index);
                var isJsonType = reader.NamespaceURI.Length == 0 && reader.LocalName == "type";
                reader.MoveToElement();
                if (!isJsonType)
                    throw new InvalidDataException(contract + " has an unknown JSON attribute");
            }
        }

        private static string ReadJsonString(
            XmlDictionaryReader reader,
            string elementName,
            string contract)
        {
            RequireJsonElement(reader, elementName, "string", contract);
            return reader.ReadElementContentAsString();
        }

        private static void AddUniqueField(
            HashSet<string> fields,
            string field,
            string[] allowedFields,
            string contract)
        {
            if (Array.IndexOf(allowedFields, field) < 0 || !fields.Add(field))
                throw new InvalidDataException(contract + " has an unknown or duplicate field");
        }

        /// <summary>
        /// Binds a secret receipt to the exact observed before/after identities
        /// and validates its semantic transition fields. Kind is read from the
        /// receipt; mode is checked only as a consequence of that kind.
        /// </summary>
        internal static Receipt ValidateSecretTransition(
            AcceptedBoundaryReceipt envelope,
            GameRuntimeTransport.Identity before,
            GameRuntimeTransport.Identity after)
        {
            ValidateEnvelope(envelope);
            if (envelope.kind != SecretTransitionBoundaryKind)
                throw new InvalidDataException("boundary receipt is not a secret transition");

            var receipt = envelope.secret_transition;
            GameRuntimeTransport.ValidateIdentityPresentation(receipt.before);
            GameRuntimeTransport.ValidateIdentityPresentation(receipt.after);
            EngineSessionTransport.ValidateIdentity(receipt.source_session);
            if (receipt.after.session != null)
                EngineSessionTransport.ValidateIdentity(receipt.after.session);
            if (receipt.receipt_contract != ReceiptContract ||
                receipt.kind is not (EntryKind or CompletedReturnKind or AbandonedReturnKind) ||
                !SameRuntimeIdentity(receipt.before, before) ||
                !SameRuntimeIdentity(receipt.after, after) ||
                !SameSessionIdentity(receipt.source_session, receipt.before?.session) ||
                !IsSemanticLevelId(receipt.origin_level_id) ||
                !IsSemanticLevelId(receipt.secret_level_id) ||
                receipt.origin_level_id == receipt.secret_level_id ||
                !IsLowerHex(receipt.receipt_sha256, 64) ||
                (receipt.before?.mode != GameRuntimeTransport.PlayingMode &&
                 !(receipt.kind == AbandonedReturnKind && receipt.before?.mode == GameRuntimeTransport.PausedMode)) ||
                GameRuntimeTransport.ParseCounter(receipt.after.generation, "generation") !=
                    checked(GameRuntimeTransport.ParseCounter(receipt.before.generation, "generation") + 1))
            {
                throw new InvalidDataException("secret transition receipt is not bound to the accepted boundary");
            }

            if (receipt.kind == EntryKind)
            {
                if (receipt.after.mode != GameRuntimeTransport.PlayingMode ||
                    receipt.before.active_runtime_flow?.bound_level_id != receipt.origin_level_id ||
                    receipt.after.active_runtime_flow?.bound_level_id != receipt.secret_level_id ||
                    !HasSecretAssociation(
                        receipt.before.active_runtime_flow,
                        receipt.origin_level_id,
                        receipt.secret_level_id) ||
                    !HasSecretAssociation(
                        receipt.after.active_runtime_flow,
                        receipt.origin_level_id,
                        receipt.secret_level_id) ||
                    receipt.before.secret_campaign?.return_origin != null ||
                    receipt.after.secret_campaign?.return_origin != receipt.origin_level_id ||
                    !PreservesSecretCampaignProgress(
                        receipt.before.secret_campaign,
                        receipt.after.secret_campaign,
                        receipt.secret_level_id,
                        allowNewCompletion: false) ||
                    receipt.after.pending_definition != null ||
                    !SameSecretEntrySession(receipt.source_session, receipt.after.session))
                {
                    throw new InvalidDataException("secret entry receipt does not bind the installed destination session");
                }
            }
            else if (receipt.after.mode != GameRuntimeTransport.CampaignMode ||
                     receipt.before.active_runtime_flow?.bound_level_id != receipt.secret_level_id ||
                     !HasSecretAssociation(
                         receipt.before.active_runtime_flow,
                         receipt.origin_level_id,
                         receipt.secret_level_id) ||
                     !HasSecretAssociation(
                         receipt.after.active_runtime_flow,
                         receipt.origin_level_id,
                         receipt.secret_level_id) ||
                     receipt.after.campaign_menu?.phase != "mission_select" ||
                     receipt.after.campaign_menu?.selected_level_id != receipt.origin_level_id ||
                     receipt.after.pending_definition != null ||
                     receipt.after.session != null ||
                     receipt.before.secret_campaign?.return_origin != receipt.origin_level_id ||
                     receipt.after.secret_campaign == null ||
                     receipt.after.secret_campaign.return_origin != null ||
                     !PreservesSecretCampaignProgress(
                        receipt.before.secret_campaign,
                        receipt.after.secret_campaign,
                        receipt.secret_level_id,
                        allowNewCompletion: receipt.kind == CompletedReturnKind))
            {
                throw new InvalidDataException("secret return receipt does not restore its selected origin map");
            }

            return receipt;
        }

        private static bool HasSecretAssociation(
            GameRuntimeTransport.RuntimeFlowDefinition flow,
            string originLevelId,
            string secretLevelId)
        {
            return flow?.secret_entries?.Any(entry => entry != null &&
                entry.origin_level_id == originLevelId &&
                entry.secret_level_id == secretLevelId) == true;
        }

        private static bool PreservesSecretCampaignProgress(
            GameRuntimeTransport.SecretCampaignSnapshot before,
            GameRuntimeTransport.SecretCampaignSnapshot after,
            string secretLevelId,
            bool allowNewCompletion)
        {
            if (after?.discovered_levels == null || after.completed_levels == null ||
                !after.discovered_levels.Contains(secretLevelId))
            {
                return false;
            }

            var previousDiscoveries = before?.discovered_levels ?? Array.Empty<string>();
            var previousCompletions = before?.completed_levels ?? Array.Empty<string>();
            if (previousDiscoveries.Any(level => !after.discovered_levels.Contains(level)) ||
                previousCompletions.Any(level => !after.completed_levels.Contains(level)) ||
                after.discovered_levels.Any(level =>
                    !previousDiscoveries.Contains(level) && level != secretLevelId))
            {
                return false;
            }

            var newCompletions = after.completed_levels.Where(
                level => !previousCompletions.Contains(level));
            return newCompletions.All(level => allowNewCompletion && level == secretLevelId) &&
                (!allowNewCompletion || after.completed_levels.Contains(secretLevelId));
        }

        private static bool SameRuntimeIdentity(
            GameRuntimeTransport.Identity left,
            GameRuntimeTransport.Identity right)
        {
            return left != null && right != null &&
                JsonUtility.ToJson(left) == JsonUtility.ToJson(right);
        }

        private static bool SameSessionIdentity(
            EngineSessionTransport.Identity left,
            EngineSessionTransport.Identity right)
        {
            return left != null && right != null &&
                left.engine_contract == right.engine_contract &&
                left.package_graph_hash == right.package_graph_hash &&
                left.definition_hash == right.definition_hash &&
                left.definition_revision == right.definition_revision &&
                left.session_id == right.session_id &&
                left.generation == right.generation &&
                left.tick == right.tick;
        }

        private static bool SameSecretEntrySession(
            EngineSessionTransport.Identity source,
            EngineSessionTransport.Identity destination)
        {
            return source != null && destination != null &&
                source.session_id == destination.session_id &&
                GameRuntimeTransport.ParseCounter(destination.generation, "session generation") ==
                    checked(GameRuntimeTransport.ParseCounter(source.generation, "session generation") + 1);
        }

        private static bool IsSemanticLevelId(string value)
        {
            if (string.IsNullOrEmpty(value)) return false;
            var segments = value.Split('.');
            if (segments.Length < 2 || segments[0] != "level" ||
                value.Contains("level1") || value.Contains("level2") ||
                value.Contains("source_slot"))
            {
                return false;
            }
            return segments.All(segment => segment.Length > 0 &&
                segment[0] is >= 'a' and <= 'z' &&
                segment.All(character => character is >= 'a' and <= 'z' or >= '0' and <= '9' or '_') &&
                !IsPrivateSourceIdentitySegment(segment));
        }

        private static bool IsPrivateSourceIdentitySegment(string segment)
        {
            var components = segment.Split('_');
            for (var index = 0; index < components.Length; index++)
            {
                var component = components[index];
                if (component is "source" or "raw" ||
                    HasNumericSuffix(component, "source") ||
                    HasNumericSuffix(component, "raw") ||
                    HasNumericSuffix(component, "level") ||
                    new[] { "class", "model", "action", "selector", "family", "slot" }
                        .Any(prefix => component == prefix || HasNumericSuffix(component, prefix)) ||
                    (component == "level" && index + 1 < components.Length &&
                        components[index + 1].Length > 0 &&
                        components[index + 1].All(character => character is >= '0' and <= '9')))
                {
                    return true;
                }
            }
            return false;
        }

        private static bool HasNumericSuffix(string value, string prefix)
        {
            return value.StartsWith(prefix, StringComparison.Ordinal) &&
                value.Length > prefix.Length &&
                value.Substring(prefix.Length).All(character => character is >= '0' and <= '9');
        }

        private static bool IsLowerHex(string value, int width)
        {
            return value?.Length == width &&
                value.All(character => character is >= '0' and <= '9' or >= 'a' and <= 'f');
        }
    }
}
