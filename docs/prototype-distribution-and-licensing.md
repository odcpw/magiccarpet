# Reversible prototype distribution and licensing guardrails

> **Later release appendix as of 2026-08-21.** Separate-repository and Track S
> instructions are retired. Use `provenance-policy.md` and
> `private-public-boundary.md` for the exact future release candidate; this file
> does not block private engine work.

Version: 1.0  
Status: historical release-risk input
Owner bead: `mc-9xb`

## Decision

The S0 successor repository is private, unpublished and all-rights-reserved by
default. It has no public remote, no open-source license, no public binary, no
service promise and no third-party contribution intake.

This is deliberately reversible:

- private development can later become source-available, open source or
  proprietary commercial development after a measured decision;
- no dependency or asset may require choosing that later business model now;
- no prototype hosting creates an expectation of availability, persistence,
  support, compatibility or data retention;
- user-owned Magic Carpet compatibility is deferred to its own legal and
  technical gate.

The private preservation lab and original successor remain physically separate
under [`private-public-boundary.md`](private-public-boundary.md).

## Repository and publication posture

Initial successor path:

```text
/home/oliver/Projects/odcpw/carpet-flight
```

Rules:

- create it as a separate private Git repository with the `odcpw` project
  identity;
- do not create or connect a public remote during S0;
- do not copy the preservation lab, its Git metadata or any parent directory;
- seed only independently authored source, synthetic fixtures and approved
  third-party dependencies;
- include a private-development notice stating that absence of a license does
  not grant reuse rights;
- do not label the repository `open source`, `free software`, `public domain`
  or `community project`;
- do not upload a build until a destination-specific distribution review
  passes;
- keep `.beads` and private planning material in the lab/control plane unless a
  reviewed export explicitly admits selected successor tasks.

If repository visibility ever changes, publication is a separate destructive
boundary check, not a toggle performed as part of ordinary Git setup.

## Product identity boundary

The successor:

- uses a new working and release name;
- is not named or marketed as Magic Carpet, Magic Carpet 2, MC2, Bullfrog,
  Electronic Arts or an official/remastered/recreated edition;
- does not use original logos, box art, screenshots, fonts, UI layouts,
  character names, spell names, realm names, map names, story text or audio;
- does not imply endorsement, continuity, compatibility or official status;
- may truthfully describe high-level genre inspiration only after a branding
  review;
- uses original mechanics specifications, code, worlds, art, audio and writing.

The private preservation deliverable may retain accurate historical naming
inside the private lab. That exception does not transfer.

## Prototype license classes

Every dependency and content item receives a manifest record with source,
exact version/hash, license, copyright/attribution, permitted use, distribution
conditions, modification status, replacement plan and owning milestone.

### Normally admissible

Subject to file-level review:

- independently authored project code and data;
- MIT, BSD-2-Clause, BSD-3-Clause, ISC and zlib-style code;
- Apache-2.0 code with required notices and patent terms retained;
- CC0 or clearly documented public-domain content;
- commercial or permissive assets whose license explicitly permits the
  intended game use, modification and redistribution;
- Unity editor/runtime/packages used under the then-current Unity terms and
  project eligibility;
- Blender and `bpy` as tools, with generated/exported artifacts reviewed
  separately from Blender's own license.

Admissible does not mean automatically accepted. Security, maintenance,
privacy, browser size and platform compatibility still apply.

### Isolated-tool only

GPL, LGPL, AGPL, source-available, research-only, non-commercial or
field-of-use-restricted software is not linked into or shipped with the S0
client/server without a specific legal review.

It may be evaluated as an isolated offline tool only when:

- input/output boundaries are documented;
- no copied code or incompatible runtime component enters the repository;
- network/service obligations are understood;
- generated output rights are clear;
- the tool can be replaced without changing canonical source data.

AGPL services and Blender add-ons receive explicit process/network review
before use.

### Forbidden in S0

- original MC1/MC2 executables, data, media, manuals or extracted payload;
- remc2/MC2HD/MagicBalls reconstructed or copyleft source in successor code;
- decompiler output, address-derived structures, memory snapshots or original
  recordings;
- ripped, scraped, leaked or provenance-unknown game assets;
- `personal use only`, `non-commercial`, `no derivatives`, `editorial use
  only`, or AI-training-only content;
- marketplace content whose redistribution/extraction terms are unknown;
- font, shader, sound, music, texture, map or model files without a manifest;
- arbitrary remote asset URLs or code downloaded at runtime.

## Unity and package guardrails

- Unity Personal eligibility and terms are rechecked at S0 start, any funding
  event, public demo, commercialization decision and release.
- The exact Editor, modules and package lock are recorded.
- Unity CLI, Pipeline and MCP are development tools only and are excluded from
  player/server artifacts.
- Package updates require license, source, lockfile, behavior and size review.
- Unity Asset Store content is not assumed reusable outside Unity or
  redistributable as source; each item receives its own record.
- No Unity cloud service becomes a mandatory architecture dependency during
  S0 without pricing, quota, data-flow, termination and export review.

## Original and commissioned content

Every original asset records:

- author or generating procedure;
- creation date and source files;
- references and their licenses;
- transformations and tools;
- model/provider and terms when generative assistance is used;
- human review and material edits;
- export/runtime derivatives;
- allowed publication/replacement class.

No prompt may request close imitation of protected Magic Carpet expression.
Reference boards use licensed/public-domain/original sources and record every
source.

External artists, composers, writers and contractors require written terms
covering ownership or license, modification, worldwide game use, promotional
use, source delivery, warranties, third-party inputs and credit before work is
integrated.

## Contributions

S0 accepts no unsolicited external contribution.

Before contribution intake:

- choose the project/source license;
- choose assignment, contributor-license-agreement or developer-certificate
  policy;
- publish contribution and provenance rules;
- define security reporting and code of conduct;
- require contributors to identify third-party and generated material;
- define review authority and rejection/removal procedure.

Pull requests, patches, assets or ideas arriving before that gate are not
merged merely because the repository is visible.

Agent-authored work remains subject to the same provenance and review rules.
The task/bead, model/tool, human reviewer, source inputs and accepted output
are retained where practical without storing secrets or proprietary prompts.

## Prototype hosting

Private S0/S1 hosting is disposable:

- no SLA, uptime promise or durable world guarantee;
- no paid entitlement, store, subscription or donation consideration;
- room identity is ephemeral;
- minimum personal data and bounded logs;
- explicit deletion/expiry;
- infrastructure can be destroyed and recreated from manifests;
- hosting limits and shutdown state are visible to testers;
- a public hostname, analytics provider or external account requires its own
  privacy/security review.

Friends receive a test notice, supported-browser statement, data/telemetry
summary and feedback route. They do not receive a commercial-service promise.

## Compatibility and owner-data boundary

During S0/S1 the successor:

- does not import, locate, validate or upload original Magic Carpet data;
- does not expose a compatibility promise;
- does not execute the original EXE;
- does not accept preservation fixtures or reconstructed state;
- does not share code with the private preservation runtime.

A future owner-data adapter requires a separate architecture, rights, privacy,
distribution and contamination gate. Its absence cannot block the original
game.

## Reversible dependency test

Before accepting a dependency, service or asset, answer:

1. Can it be removed without rewriting authoritative simulation?
2. Can it be replaced without losing canonical original source data?
3. Can the project build/test without a private account or remote service?
4. Are all shipped obligations satisfiable under proprietary and open-source
   future options?
5. Can notices/source offers/attributions be generated automatically?
6. Does browser delivery permit the planned compression, caching and CDN use?
7. Does its license permit commercial evaluation even though no business model
   is chosen?

A `no` blocks S0 unless an explicit expiring exception names containment,
replacement owner and decision gate.

## Automated controls

The successor repository must eventually enforce:

- allowlisted file extensions and maximum sizes;
- known forbidden hashes and private-lab path/name patterns;
- archive, executable, decompiler, Ghidra and original-media rejection;
- dependency lock and license/SBOM generation;
- asset/provenance manifest completeness;
- secret and credential scanning;
- no symlink/path escape into the lab;
- no unapproved runtime URL/provider;
- notice/attribution generation;
- clean-checkout build without the preservation tree mounted.

S0 repository creation seeds the rules and synthetic negative fixtures; later
beads harden them.

## Decision gates

| Gate | Decision |
|---|---|
| S0 approval | Private repository and synthetic technical proof |
| S0 retain/cut | Keep Unity stack or run one measured challenger |
| S1 content authorization | Admit original social-demo content only after technical and DAG gates |
| Friend-link review | Approve one disposable private hosted build and tester notice |
| Post-feasibility business gate | Premium/free/donation/open-source/source-available/proprietary decision |
| Public repository gate | Source license, contribution, branding, notices and contamination audit |
| Commercial release gate | Entity, contracts, privacy, ratings, support, trademark and final license review |

No earlier gate decides a later business model accidentally.

## Acceptance evidence

- The initial repo remains private and has no public remote.
- Its notice and manifests state the temporary all-rights-reserved posture.
- Synthetic clean-room fixtures build without the lab mounted.
- Dependency/content manifest schema represents every allowed class.
- Forbidden private/proprietary fixture tests fail.
- No contribution intake or public-service promise exists.
- A post-feasibility decision remains open and evidence-driven.

This is an engineering guardrail, not a substitute for counsel at public or
commercial release gates.
