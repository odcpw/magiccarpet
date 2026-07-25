# Approved services for private Magic Carpet evidence

Version: 1.0  
Effective: 2026-07-23  
Classification: `PRIVATE_ANALYSIS`

## Default

No remote service is approved to receive owned binaries, installed game data,
manuals, saves, screenshots, video/audio, Ghidra projects, disassembly,
decompiler output, addresses, debugger/memory traces, raw analyst notes,
private source registers, or unreviewed transfer staging.

## Approved local processing

| Tool class | Execution | Network | Allowed input | Required record |
|---|---|---|---|---|
| DOSBox/GOG runtime | Local | No required egress | Owned working installs | BuildManifest plus experiment/run record |
| Hash/file metadata tools | Local | No egress | Private files | Inventory/campaign record |
| Ghidra and local extensions | Local | Deny/verify egress | Exact owned BuildManifest copies | Tool/version/extension hashes and campaign |
| Local debugger/capture tools | Local | Deny/verify egress | Exact owned run | Tool/version, capture minimisation, campaign |
| `bwrap` public-worker sandbox | Local | Unshared network | Public candidate only | Isolation probe and worker receipt |

“Local” does not approve telemetry, automatic sample submission, cloud symbol
lookups, remote extensions, or model/API integrations. Each tool is pinned and
egress-tested by its provisioning bead before private input.

## Approved public research

Normal web access may retrieve public pages, manuals already lawfully viewable
at their public URLs, official documentation, law, guides, and public video
metadata/streams for private observation. Private local files, excerpts,
captures, hashes beyond deliberate source citation, or analyzer context are
never uploaded as part of a search or prompt.

Temporary public downloads follow the provenance policy: record source,
access date, checksum/metadata when useful, purpose, retained notes, and
deletion disposition.

## Approved private source control

| Service | Repository | Approved data | Prohibited data | Transport and credentials | Retention and deletion | Owner and review |
|---|---|---|---|---|---|---|
| GitHub private Git repository | `odcpw/magiccarpet` | Authored project policy, planning, source, tests, synthetic fixtures, qualification code/contracts, a pinned public MC2HD submodule revision, and the bounded owner-local source patch explicitly committed on 2026-07-25 | Owned installers, manuals, game data, saves, decoded owner assets, screenshots, video/audio, generated scene packages/caches, analyzer projects, dumps, raw traces, raw source registers, Beads databases, agent state, build products, and unreviewed transfer staging | SSH Git transport through the `odcpw` account; credentials remain in the system keyring and are never committed | Repository must remain private. Retained until Oliver deletes it through GitHub; deletion must be followed by remote-unavailability verification. GitHub backups and legal retention remain governed by GitHub's current terms. | Oliver explicitly approved the private commit-and-push workflow on 2026-07-25. Review on any visibility, provider, organization, or data-category change; no AI/Copilot ingestion or third-party app access is approved by this row. |

Before every push, review the staged tree and reachable Git objects for the
prohibited categories above. A private visibility setting is not permission to
commit owned game content or raw evidence.

## Current agent access

The current Codex task and explicitly assigned private read-only
research/review agents operate on the `PRIVATE_LAB` side. This authorizes local
file access within their bounded task; it does not approve external upload.

Future implementation agents are `PUBLIC_CANDIDATE` workers. They receive
accepted transfer packages only and must start inside
`tools/provenance/run-public-sandbox` or a stronger clean CI/container
boundary. Accessing this private task’s session history is prohibited.

## Exceptions and review

Any new remote service, plugin, telemetry path, model, cloud storage, crash
reporter, collaborative asset tool, or hosted analyzer requires a new row with
exact provider/service, data categories, purpose, terms, retention/training
state, transport, credentials, deletion procedure, approval owner, expiry,
and a synthetic no-private-data test. Unknown behavior means not approved.
