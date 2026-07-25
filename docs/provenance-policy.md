# Provenance-controlled reimplementation policy

Version: 0.5  
Effective: 2026-07-23 for preproduction and private research  
Status: adopted engineering policy; not a legal or public-release approval  
Applies to: private research, reverse engineering, implementation, assets,
tests, builds, documentation, agents, contractors, and releases  
Policy owner: Oliver  
Tracking bead: `mc-w58.2`

Program basis: [`docs/program-charter.md`](program-charter.md). Track P is an
owner-local preservation and research program that may use remc2-family source,
the owned MC2 build, Ghidra, Binary Ninja, DOSBox instrumentation and original
data inside the private boundary. Track S is a commercially releasable,
independently authored successor: it receives reviewed functional lessons and
methods, never reconstructed code, original assets, raw traces, address-derived
structures or MC2 compatibility obligations. Any public preservation release,
branding, importer, compatibility service or assertion of legality needs a
later explicit decision.

## Purpose

The project may study the exact lawfully acquired Magic Carpet 2 build and its
owned documentation deeply enough to understand behavior, including controlled
play, manuals, recorded observations, Ghidra, debugging, and measurement. MC1
is not a research, parity, or implementation input; its retained hashes and
paths exist only for private inventory and contamination denial. Access must
not become copied code, copied audiovisual expression, or an unreviewed
redistribution channel.

The program contains two different processes. Track P is a private
preservation/reconstruction laboratory. Track S is a **provenance-controlled
independent successor**. Track S is not described as a conventional legal
clean room unless separate analysts and implementers, access controls,
documented handoffs, and legal review are actually established.

This policy is an engineering control, not a legal opinion. It deliberately
uses a stricter publication boundary than “probably lawful.” Before any public
release, original-data importer, compatibility server, use of original names,
or claim of interoperability, a qualified reviewer must assess the exact
build, jurisdiction, contracts, marks, and distribution plan.

## Current legal-risk posture

The relevant principles do not create a blanket permission to clone a game:

- Copyright can protect program and audiovisual expression while excluding
  ideas, procedures, systems, methods of operation, and program logic from
  copyright protection as such.
- Swiss Copyright Act Article 21 permits a lawful user to decode program code
  for necessary interface information used for independently developed
  interoperable programs, subject to limits. That is narrower than a general
  gameplay-reconstruction permission.
- EU Directive 2009/24/EC Article 6 similarly limits decompilation without
  authorisation to information indispensable for interoperability, confines
  it to necessary portions, and restricts later use and disclosure.
- The bundled MC2 license text and current GOG User Agreement impose
  contractual restrictions beyond the general copyright idea/expression rule.
  The bundled terms prohibit reverse engineering except to the extent
  applicable law expressly permits it. Their hashes and summaries are in the
  private source register; how those clauses and local law apply to this
  project still requires qualified review. Retained MC1 license/hash records
  are historical contamination-denial entries only, not project inputs.
- Noncommercial intent, age, unavailability, ownership of a copy, and calling
  a project an “homage” do not themselves grant redistribution rights.

Operational consequence: Oliver has authorised private executable analysis
for this project, but the project records contract/statutory scope as
**unresolved**, keeps analyzer artifacts private, transfers only independently
written functional findings, and requires a separate release review. No
document may claim that Ghidra use or a public replica has already been
determined lawful.

Primary legal references:

- Swiss Federal Act on Copyright and Related Rights, Article 21:
  <https://www.fedlex.admin.ch/eli/cc/1993/1798_1798_1798/en>
- EU Directive 2009/24/EC, Articles 5 and 6:
  <https://eur-lex.europa.eu/legal-content/en/TXT/?uri=CELEX:32009L0024>
- United States Copyright Act, 17 USC 102(b):
  <https://www.copyright.gov/title17/92chap1.html>
- US Copyright Office, computer-program guidance:
  <https://www.copyright.gov/register/tx-programs.html>
- Current GOG User Agreement:
  <https://support.gog.com/hc/en-us/articles/212632089-GOG-User-Agreement>

## Physical boundaries

### Private reference lab

The current workspace,
`/home/oliver/Projects/odcpw/magiccarpet`, is the private reference lab. It is
never the public source repository and must not be pushed to a public remote.
It may contain:

- owned installers, manuals, reference cards, and installed builds;
- hashes, version records, DOSBox configuration, saves, and controlled traces;
- Ghidra projects, disassembly, addresses, symbols, debugger output, memory
  captures, screenshots, frame captures, and raw analyst notes;
- temporary downloaded research media, provided its source and purpose are
  logged and the raw download is deleted when no longer needed;
- private research inventories and source registers;
- publication-safe specifications being prepared for transfer.

The entire current root is classified `PRIVATE_LAB`, including the top-level
owned-game directories. A `private` path component denotes especially
sensitive evidence but is not the boundary itself. Backups must preserve the
root’s classification and access boundary. OS permissions, backup targets,
retention, and a public allowlist are implemented and tested by `mc-t8x`.

### Successor implementation repository

The publication-safe successor will be a separate sibling repository with its own
Git history, remotes, build artifacts, dependency lockfiles, and CI. Its exact
name/path is frozen by `mc-t8x` and initialized by `mc-ads`; it is not created
implicitly by this policy.

The public repository may receive files only through the transfer process
below. It must never share a worktree, Git object database, artifact cache, or
bulk copy operation with the private lab. `mc-t8x` must choose a separate
user/container or another testable access-control boundary; a sibling path
alone is organization, not isolation. `mc-ads` proves the clean public root and
CI controls.

Until those beads pass, no implementation task may claim analyst/implementer
isolation. The current shared-agent filesystem is a private research surface.

### Reviewed lesson-transfer staging

`research/transfer/` inside the private lab is the only staging location for
candidate Track P-to-Track S lesson packages. A package is not considered
successor-safe merely
because it is placed there. It must pass the transfer checklist and then be
copied as named files into the public repository.

No recursive copy, repository mirroring, “copy everything then delete,” or
shared asset-library mount is permitted between the two roots.

## Source classification

Every source receives a source ID and one of these classes:

| Class | Examples | Default handling |
|---|---|---|
| `OWNED_BINARY` | GOG installer, installed EXE/data | Private only; hash and agreement recorded |
| `OWNED_DOCUMENT` | Manual, reference card | Private only; functional facts may be restated |
| `DIRECT_OBSERVATION` | Controlled play, input/output trace | Findings may transfer after review |
| `PRIVATE_ANALYSIS` | Ghidra/debugger/memory trace | Raw evidence private; behavior spec only |
| `PUBLIC_PRIMARY` | First-party interview, official law, official documentation | Cite and apply its license/terms |
| `PUBLIC_SECONDARY` | Guide, review, FAQ, longplay | Question/observation source; do not copy expression |
| `PUBLIC_CODE` | Source port, editor, tool, library | License and lineage review before opening or using |
| `ORIGINAL_PROJECT` | New code, art, audio, prose, tests | Publication eligible with authorship/license record |
| `THIRD_PARTY_LICENSED` | Unity package, font, SFX library | Publication eligible only within recorded terms |

The source register uses
[`docs/provenance/source-register-template.md`](provenance/source-register-template.md).
Unknown license, unknown lineage, or unclear redistribution terms mean
**research-only** until resolved.

The current owned and public evidence is indexed in the private
[`research/private/source-register.md`](../research/private/source-register.md).
That private register may contain local identifiers and hashes and is never
copied wholesale into the public repository.

## Transfer traffic light

Classification applies to each individual finding and each individual test
vector, not only to the package as a whole.

### Green: normally transferable

- externally observable rules stated in fresh project language;
- measurements with method, uncertainty, and source IDs;
- independently derived equations and state machines describing behavior;
- abstract data schemas and event semantics;
- public-domain facts and properly cited primary-source facts;
- synthetic test vectors created from the specification rather than copied
  saves, levels, tables, or memory;
- original design decisions and documented intentional deviations.

Green does not mean unattributed. Every transferred claim retains source IDs,
confidence, author, reviewer, and date.

### Amber: individual review required

- exact numeric constants or large parameter sets;
- distinctive selection, ordering, progression, or balance tables;
- detailed behavior learned only from decompilation;
- file formats, network formats, and interfaces;
- terminology, short names, spell names, creature names, icon concepts, and
  command labels;
- screenshots used only as temporary comparison evidence;
- small excerpts from public projects needed to understand an external format;
- compatibility behavior that may require exact quirks.

Amber material transfers only when the reviewer records why it is necessary,
why the representation is functional rather than copied expression, whether
it was externally measured or extracted, what was rewritten or independently
measured, the minimum subset needed, and the legal-review disposition.

A vector is not “synthetic” merely because it was retyped or reformatted. A
vector encoding a complete extracted constant table, level, save, packet,
memory layout, or distinctive selection/arrangement remains Amber or Red.

Private comparison builds may use original labels for research. A build shared
with friends or hosted in a browser is a distribution candidate: its title,
marks, names, text, and overall expressive similarity require release review.
Common descriptive words are not assumed safe merely because they are short.

### Red: never transferred into a distributable repository

- original or decompiled source code, analyzer-generated pseudocode, function
  names, symbol maps, comments, assembly listings, addresses, or lifted control
  flow;
- copied implementation structures whose form follows the executable rather
  than a behavior specification;
- original binaries, data archives, installers, manuals, saves, level files,
  tables dumped from memory, or cryptographic keys;
- original models, sprites, textures, palettes, animations, maps, geometry,
  icons, fonts, UI frames/layout artwork, screenshots, video, or cutscenes;
- original music, samples, voices, dialogue, objective prose, story text,
  names presented as a wholesale catalog, or marketing copy;
- original trademarks, logos, trade dress, or Bullfrog/EA/GOG branding used as
  the project’s public identity;
- code or assets from an external project whose license, provenance, or
  compatibility is unresolved;
- GPL/AGPL code copied into a differently licensed program without an explicit
  compatible architecture and release decision.

Hashing, renaming, converting, tracing, redrawing, resampling, or running an
asset through a generative tool does not change its Red classification.

## Reverse-engineering campaign protocol

Every executable-analysis campaign is created as a bounded bead and begins
with:

1. subsystem and exact behavioral question;
2. executable/source IDs, hashes, version, environment, and tool versions;
3. legal/contract risk state and approved private scope;
4. reproduction procedure and controlled inputs;
5. expected output artifact and explicit forbidden output;
6. stop conditions and data-minimisation/deletion plan.

The analyst evidence log records:

- addresses, symbols, screenshots, and raw traces in private fields only;
- observed behavior separately from inferred mechanism;
- contradictions and alternative explanations;
- dynamic reproduction against the owned running build;
- confidence and what evidence could falsify the conclusion.

The analyst then creates a transfer package with
[`docs/provenance/analysis-transfer-template.md`](provenance/analysis-transfer-template.md).
The package describes behavior without original addresses, code, screenshots,
copied data, or decompiler phrasing. Every accepted rule has independently
constructed positive, boundary/rejection and invariant synthetic vectors
covering its material branches. Every `BLACK_BOX_MEASURED` or `EXTRACTED`
rule additionally has a discriminating reproduction against the exact
unmodified owned build. `DOCUMENTED` and `ORIGINAL` rules may receive an
explicit independently reviewed exact-build exemption with a narrower stated
scope.

Campaign setup and private evidence use
[`docs/provenance/private-analysis-campaign-template.md`](provenance/private-analysis-campaign-template.md).
`mc-w58.3` turns that schema into the behavioral protocol and `mc-51m`
instantiates it in the Ghidra lab.

## Track S implementation rules

An implementation worker must:

- work from an accepted transfer package or original project design;
- record its own design choices, algorithms, and test construction;
- avoid opening the corresponding Red evidence while implementing;
- ask for clarification when a state transition is missing instead of
  searching private analyzer output;
- use neutral project identifiers until public terminology is approved;
- create only original or properly licensed audiovisual expression;
- include the source/transfer IDs in the change record without embedding
  private evidence;
- reject tasks that instruct it to “match this code,” “trace this asset,” or
  reproduce copied level/prose/media content.

For decompilation-derived Amber findings, the same agent must not implement
the corresponding subsystem unless an exception records why separation is
impracticable, identifies the exact findings, and requires heightened review.
Normal implementation agents are started without private conversation context,
use only accepted transfer packages, and run behind the access boundary chosen
by `mc-t8x`. A second review checks the result for analyzer-shaped names,
structure, comments, tables, and copied expression. These controls still do
not transform the process into a legal clean room.

## Public fixtures and CI

Public tests and CI use only:

- generated terrains, maps, command streams, saves, protocol packets, and
  replays;
- original tiny benchmark assets with recorded authorship;
- third-party fixtures whose license expressly permits redistribution;
- synthetic constants and vectors included in an accepted transfer package.

Before public initialization, `mc-ads` must implement:

- an allowlisted `tests/fixtures` manifest with provenance fields;
- a denylist for private extensions, paths, installer/archive signatures, known
  owned-file hashes, and analyzer project formats;
- a secret/binary/large-file scan and generated license/SBOM report;
- a check that every asset/font/audio dependency has author, source, license,
  modification, attribution, and redistribution fields;
- a check that public docs do not link to private paths or contain addresses,
  screenshots, original prose, or analyzer excerpts;
- a clean-clone build proving no dependency on the private lab.

CI passing is necessary but not sufficient: novel expressive similarity,
branding, and compiled artifacts still receive human release review.

These are specified controls, not current CI capabilities. No checklist or
bead may mark them operational before `mc-t8x` and `mc-ads` produce evidence.

## External code and tools

Before cloning, opening, indexing, benchmarking, linking, generating from, or
copying an external project, record:

- upstream URL and immutable revision;
- license file and notices at that revision;
- whether the code may derive from Bullfrog binaries or original data;
- permitted research, modification, linking, distribution, and network use;
- copyleft boundary and source-offer obligations;
- tool output ownership and whether generated output embeds input material;
- removal plan and substitute if the dependency becomes unusable.

Public reimplementations such as source ports are question generators unless a
separate bead explicitly approves their license and lineage for another use.
No implementation worker reads them by default.

FrankenSim remains unavailable to project agents until `mc-p1f` records Jeff’s
written permission or an applicable license change for the exact revision.
That gate is optional and cannot block the baseline implementation.

No original binary, asset, manual, screenshot, or private trace is uploaded to
an unapproved third-party analysis, model-training, conversion, or hosting
service. Local tools and private storage are the default.

Every private campaign records tools, versions, local/remote execution,
services/models that could receive content or context, data-egress behavior,
approval basis, and deletion/retention. The approved-services register is
created by `mc-t8x`; until then, no new remote service receives private
evidence.

## Reviewer roles

An **engineering provenance reviewer**:

- did not author the transfer package being reviewed;
- may inspect private evidence when assigned to the private campaign;
- checks classification, source traceability, independent wording,
  minimisation, synthetic verification, and implementation contamination;
- cannot decide contested contract, copyright, trademark, or
  anti-circumvention questions.

A **legal reviewer** is qualified counsel for the relevant jurisdiction and
release model. Legal review is mandatory when a finding remains Amber due to
bulk extracted data or interoperability necessity, when original terminology
or compatibility behavior is proposed publicly, after a provenance incident
with distribution, and before a public importer, compatibility server, source
release, or hosted game.

Oliver owns product and risk decisions but a product approval is not recorded
as legal advice. Reviewers disclose prior exposure and conflicts in the
package. No author self-approves a transfer.

## Track P original-data importers and compatibility services

An importer that asks each user to supply owned data may be architecturally
possible, but it is not approved by this policy. Likewise, an original-client
compatibility server may involve protocol, trademark, contract, anti-
circumvention, and interoperability questions.

Either requires its own:

- user story and necessity;
- jurisdiction and contract review;
- no-bundled-data proof;
- format/protocol evidence provenance;
- output/extraction limits;
- abuse and takedown process;
- counsel or rights-holder decision;
- separately approved bead and release gate.

Track S must not depend on either. A separately gated owner-supplied-data
adapter belongs only to Track P and may use only independently selected local
input in private stores; it must never upload, host, peer transfer or become a
dependency of the successor.

## Temporary evidence and deletion

Temporary downloads are placed under the private lab or a dedicated temporary
directory, never the public repository. Before deletion, record source URL,
access date, checksum when useful, technical metadata, questions answered, and
retained derived notes. Retain the minimum necessary evidence and label every
retained frame or excerpt private.

Deleting a raw file does not make an extracted asset publication-safe.

## Incident and exception procedure

If Red or unknown material enters a transfer package or public repository:

1. stop affected implementation and publication;
2. record the file, source, commits/builds, agents, and downstream derivatives;
3. quarantine rather than casually rewriting history;
4. determine every dependent artifact and whether independent recreation is
   possible;
5. remove and rebuild contaminated artifacts through reviewed specifications;
6. rotate any leaked secrets and preserve a private incident record;
7. obtain legal review when distribution occurred or provenance cannot be
   reconstructed.

Exceptions require a written rationale, exact files/uses, rights basis,
expiration/review date, and Oliver’s approval. “Noncommercial,” “old game,”
“everyone does it,” or “the user owns a copy” are not exception rationales.

## Release gates by track

The following claims require different evidence:

- **Private research build:** may use owned reference material locally; never
  distributed.
- **Track P owner-local browser test:** private preservation build using
  Oliver-supplied local data; no hosted assets and no implied publication
  approval.
- **Track S friends/browser test:** distribution candidate; original expression,
  provenance scan, license manifest, neutral branding, and explicit tester
  terms required.
- **Public source release:** clean repository and history, synthetic fixtures,
  dependency/license/SBOM audit, expressive-similarity review, and legal review.
- **Public hosted game:** all public-source gates plus service, privacy,
  moderation, trademark, ratings, and takedown readiness.
- **Feature-complete homage:** refers only to the frozen functional manifest,
  never copied content or implied EA/Bullfrog endorsement.

## Policy acceptance and delegated enforcement

Completed by `mc-w58.2`:

- [x] Source classes, traffic-light rules, reviewer authority, and register
      fields are defined.
- [x] Initial owned/public sources, installer hashes, bundled terms, and
      contamination decisions are registered.
- [x] The current root is declared private and the required future isolation
      architecture is unambiguous.
- [x] Raw executable-analysis evidence is private-only and the functional
      transfer schema is defined.
- [x] Public fixture and CI rules are specified.
- [x] Importers, compatibility services, branding, and public releases require
      separate decisions.
- [x] A harmless candidate transfer plus independent hash-bound index review
      demonstrates the pre-receipt transfer flow without original evidence;
      destination receipt enforcement remains an operational proof in
      `mc-51m` and the first real consuming task.

Operational proof:

- [x] `mc-t8x`: storage, permissions, agent/service controls, and
      public/private isolation are independently accepted. Backup remains
      explicitly `UNVERIFIED`, so no owned file may be moved or deleted.
- [x] `mc-w58.3`: the behavioral reverse-engineering protocol and campaign
      governance are frozen.
- [ ] `mc-51m`: instantiate the private Ghidra lab, manifests, logs, and
      reproducible tooling.
- [ ] `mc-ads`: initialize the clean public repository and enforce fixture,
      provenance, license, SBOM, and private-material CI checks.

Closing `mc-w58.2` means the rules and schemas are accepted prerequisites. It
does not claim that downstream controls, legal review, or public-release
approval already exist.

## Governance and revision

Re-review this policy when an agreement or applicable law changes; a public
repository or new remote analysis service is introduced; agent isolation
changes; an Amber exception is approved; the release model, branding,
jurisdiction, importer, or compatibility scope changes; or a provenance
incident occurs.

Revision history:

- 0.1, 2026-07-23: initial policy, source classes, and handoff templates.
- 0.2, 2026-07-23: separated policy definition from downstream enforcement;
  added current GOG terms, per-finding Amber controls, campaign/tool exposure,
  reviewer roles, agent isolation, governance, and smoke-transfer requirement.
- 0.3, 2026-07-23: aligned executable-derived transfer verification with the
  behavioral protocol: branch-covering synthetic vectors plus exact-build
  discriminating reproduction, with only documented/original exemptions.
- 0.4, 2026-07-23: narrowed active research to the exact owned MC2 build,
  recorded MC1 solely as contamination-denial inventory, integrated the
  replacement-expression game-module boundary and optional local-only
  owner-data adapter, and updated accepted downstream-control status.
- 0.5, 2026-07-23: split private remc2/MC2 preservation from the independently
  authored commercial successor; restricted reconstructed-code and
  original-data use to Track P and redefined transfer as reviewed lessons.
