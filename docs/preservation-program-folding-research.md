# Program folding research for campaign generalization

> **Retained technical research, superseded work order as of 2026-08-21.** Use
> its quotienting, ownership and migration ideas only after the neutral
> package/API and custom-package proof. Track-P/Level-2-first ordering and the
> Level-3-as-first-generality-proof premise are retired.

Version: 0.1
Status: research extension at revision
`b05873b0f5610cfaf777e94df1ac86595da7b0ed`; not an implementation contract
Track: `P` only
Consumer: the post-Level-2 canonical-state/scheduler refactor and the Level 3
generalization proof
Retirement: superseded when its observation contract, ownership laws,
capability inventory, and migration invariants live beside implemented code and
tests

## Answer

Yes, the seam model can carry materially more information and produce more
useful decisions. The strongest formulation is not “find the minimum-energy
module tree.” It is:

> Define the exact observation relation first; quotient only representations or
> behavior keys proven interchangeable; partition the remaining typed temporal
> hypergraph; and separately find a feasible path of verified revisions to a
> nondominated fold.

That divides the problem into three computations:

1. **Semantic quotienting** — what can safely become one thing?
2. **Ownership partitioning** — which surviving things should live and change
   together?
3. **Migration routing** — how can the current implementation reach that state
   without losing a real positive route or crossing an unverified revision?

The current campaign seam memo had the right empirical target. Independent
Fable and Oracle research passes agreed that its weak point was the weighted
`lambda` sum and its casual use of “Markov blanket.” The corrected memo now
uses a feasible set plus an objective vector, and calls handler access a
semantic footprint/dependency frontier unless a probabilistic
conditional-independence model is actually supplied.

The general reusable method lives in the installed
`optimize-program-seam-topology` skill. This document keeps the private remc2
application inside Track P.

## What is established, inferred, and conjectured

| Status | Claim |
|---|---|
| established method | program/system dependence graphs and slicing expose control/data influence beyond call graphs |
| established method | directed hypergraphs retain one-to-many transition relations lost by pairwise projection |
| established method | MDL selects between an over-simple model with many exceptions and an over-complex model with too many parts |
| established method | partition refinement, simulation/bisimulation, and translation validation support semantic quotienting |
| established method | Pareto, epsilon-constrained, lexicographic, exact, and multilevel search expose constrained tradeoffs |
| established biology | folding funnels, domains/contact maps, minimal/local frustration, allostery, chaperones, cotranslational folding, disorder, and neutral networks are real biological concepts |
| engineering synthesis | combine static source, dynamic conformance, state ownership, contract, history, and build evidence in one multilayer temporal program hypergraph |
| MagicCarpet policy | one canonical mutable carrier for each simulation fact; Unity remains non-authoritative |
| analogy | program revisions form a migration landscape with traps, scaffolds, and many neutral routes |
| conjecture | the right campaign fold makes marginal new shared behavior decline across Levels 3 through 24 |
| not claimed | programs possess thermodynamic free energy, one unique native fold, or spontaneous self-organization |

## The MagicCarpet semantic contract

Before any post-Level-2 fold, define an observation alphabet `Omega` at the
current accepted candidate. It should include at least:

```text
authoritative campaign state projection and hash
scheduler phase, dispatch site, slot visit, scan direction, rescan/runagain
RNG old state, consuming site/draw index, result, new state
allocation, reclamation, generation, and observed topology changes
ordered production effect/audio emissions and cursor binding
reset operation, reset result, and intentionally persistent state
package/profile/source-fact identities and preflight result
C ABI return, layouts, handle/generation state, errors, and mutation receipt
Unity-visible frame/journal results without Unity-side gameplay authority
```

The old and new implementations need an explicit relation:

```text
R(old_state, new_state)
    iff canonical_old(old_state) == canonical_new(new_state)
```

For every reached transition, corresponding old and new steps must emit equal
`Omega` observations and leave related successor states. Where the extracted
state space is bounded, exhaustiveness may be possible. Elsewhere this relation
defines differential traces and discriminating microcases; those remain bounded
evidence, not a theorem about every execution.

## The temporal hypergraph

Use typed vertices rather than treating files as the primary architecture:

```text
V_state       canonical or candidate mutable facts
V_fact        immutable package/source/artifact facts
V_transition  source-shaped handlers and pre/postpasses
V_phase       scheduler sites, iteration domains, continuations
V_identity    slots, generations, allocators, list/queue topology
V_observation hashes, effects, ABI and Unity projections
V_change      physical files, build units, commits, writers
```

`V_change` is deliberately separate. A worker owning a file is not that file
owning simulation state.

For a source transition, record:

```text
e = <phase/site,
     conservative dispatch key,
     guard,
     reads and writes,
     allocations and reclamations,
     ordered RNG/input receipts,
     ordered emissions,
     observed/mutated slot/list/queue topology,
     immutable facts,
     continuation: next/rescan/retry/fault,
     cache/fact validity interval,
     static provenance and dynamic workload evidence>
```

This matters immediately. A simple read/write edge cannot distinguish the
source's full `runagain` slot rescan from a remembered-row follow-up. Equal
final entity membership also cannot distinguish a changed allocation/reuse
order. Phase, continuation, identity, and topology are semantic dimensions.

Static edges must conservatively include possible paths. Dynamic traces add
frequency, volume, realized context, and witnesses; failure to observe an edge
does not prove that it is unreachable.

## The candidate fold

A candidate is not just `vertex -> module`:

```text
Phi = (P, O, M, K, B)

P  physical/logical module assignment
O  canonical mutable ownership
M  materialization: canonical, borrowed, derived, projection, adapter
K  conservative behavior-key quotient
B  boundary realization: borrow, call, immutable reference, ABI, journal
```

Two layouts can have identical files and opposite architecture: one borrows one
canonical terrain carrier; another copies terrain into three writable domains
and reconciles them. `O`, `M`, and `B` distinguish them.

### Hard constraint classes

Keep three classes explicit:

1. semantic: arithmetic, source schedule, RNG, identity/topology, effects,
   reset, and failure/mutation behavior;
2. contract: package identity, source facts, ABI layout/lifecycle/errors,
   cursor/generation binding, and fail-closed selection; and
3. project policy: Rust simulation authority, Unity presentation-only role,
   Track P boundary, and one canonical mutable carrier.

All three are non-negotiable here, but only the first two describe source or
external behavior. The one-owner law is a sound MagicCarpet policy; it is not a
universal theorem for replicated databases or event-sourced systems.

## Measurable objective vector

For feasible candidates only, compare:

```text
C(Phi) = [
  duplicate writable representatives,
  copy/reconciliation operations and bytes,
  mutable and ordered hyperedge cuts,
  fixed-codec boundary information bits per source step,
  handler exposed mutable surface versus actually touched surface,
  per-level exception description,
  contract churn,
  cross-seam change and active-writer contention,
  runtime and memory,
  expected incremental build cost,
  migration cost,
  unresolved proof obligations
]
```

No coefficient converts one ABI consumer into milliseconds or one proof gap
into compile seconds. Use:

1. hard feasibility;
2. explicit budgets, such as zero ABI regression and bounded runtime/build
   change;
3. Pareto dominance inside a priority level; and
4. lexicographic priorities for policy, beginning with duplicate authority and
   source correctness.

### Boundary information rate

For a fixed trace schema and codec:

```text
BIR(Phi, workload) = encoded bits crossing candidate seams / source steps
```

This is a concrete measure of protocol volume and variety. Report it by
campaign/operation class. Do not call ordinary cut uncertainty “graph entropy”;
that term has specialized definitions with different assumptions.

### MDL

The campaign MDL surrogate becomes reproducible only under a fixed encoding:

```text
L_arch = L(shared engine AST/tokens)
       + L(boundary and dispatch schemas)
       + sum L(canonical level descriptors)
       + sum L(unavoidable level-specific exceptions)
       + L(temporary adapters)
```

The prediction is not merely fewer lines. Once capabilities saturate, the
fourth term should tend toward zero. Formatter, macro, or compressor changes
must not masquerade as architecture improvement.

## Safe semantic quotienting

### Dispatch keys

Start conservatively with:

```text
(phase, dispatch table/site, class, action, source-required subtype/model facts)
```

Site remains part of the key until the surrounding prologue, continuation,
RNG, topology, and authoritative transition are equivalent. If two sites select
the same callback but differ in rescan or RNG behavior, they are not equivalent.
If every protected observation matches across their reachable domains, the site
may be quotientable later. The current source-shaped key is the safe initial
partition, not necessarily the smallest final key.

### State lifetimes

Classify each fact relative to an operation and guard:

```text
immutable
resettable(reset_kind, guard)
persistent(reset_kind, guard)
scratch(unobservable after phase)
derived(function, validity interval, invalidation)
```

A field can persist through soft reset but reset on campaign restart. A cache
can be valid only within one scheduler interval. Bare lifetime labels are
insufficient.

### Canonical topology

Exact list/free-stack/slot/queue topology is canonical only when a backward
slice from a protected future observation reaches its identity or order. A
counterfactual reorder that preserves logical contents is the decisive test.
If later behavior distinguishes it, preserve it; otherwise it can be derived.

### Semantic asset closure

Starting from authoritative state mutations and effects, slice backward through
loader, decoder, selector, closure, and package boundaries. Any terminal owner
byte that reaches those observations is a simulation fact regardless of a
directory or field named `presentation`.

This formalizes the current MSPRD discovery and can find the next such input
without trusting naming conventions.

### Capability closure

Create a conservative behavior reachability graph. Add an edge when a handler,
constructor, action transition, objective transition, scheduler gate, or package
activation can make another key selectable:

```text
Reach(initial package keys) subset_of implemented keys
```

Preflight must cover spawned and transitioned rows, not only authored starting
entities. Exact general reachability is undecidable, but MagicCarpet's bounded
arena and source-shaped tables allow useful conservative closure with
fail-closed overapproximation.

### Production effects versus parity traces

The production effect journal and diagnostic parity trace remain separate here.
Prove noninterference:

```text
authoritative(run(trace_on)) == authoritative(run(trace_off))
```

The trace must not affect schedule, RNG, allocation, hashes, or production
effects. This is a MagicCarpet design fact, not a universal ban on event-sourced
authority.

## Solver stack

Use the cheapest solver that can change the decision:

1. deterministic extraction and hard-constraint propagation;
2. contract proved must-link units and behavior quotients;
3. exact CP-SAT/MILP on a reduced graph when the decision variables are small
   enough;
4. multilevel hypergraph coarsen/partition/uncoarsen for larger proposals;
5. Pareto local search or seeded evolutionary search for nonlinear secondary
   objectives;
6. epsilon-constrained re-evaluation of promising candidates; and
7. incremental warm-start from the current architecture with an explicit
   movement budget.

The measured MagicCarpet top-level fold may be small enough for an exact
CP-SAT experiment after semantic coarsening, but that is a hypothesis until the
actual inventory and solver gap are recorded. Exact optimization of a deficient
graph would still be precisely wrong.

### Local frustratometer

For an admissible move, compute the reduction in one fixed boundary objective.
Under an additive cut objective this is the same local move-gain quantity used
by partition refiners. Standardize against other admissible local destinations:

```text
z_move = (gain - mean(admissible gains)) / stddev(admissible gains)
```

This gives “local frustration” a falsifiable software meaning. A stronger global
measure forces a seam and recomputes the feasible optimum:

```text
seam_regret(e) = optimum_cost(e forced split) - optimum_cost(unconstrained)
```

Neither may convert a hard semantic violation into a tolerable score.

## Protein folding: what the analogy contributes

| Protein concept | MagicCarpet use | Limit |
|---|---|---|
| Anfinsen | fixed semantics strongly constrain legal implementations | no unique module tree follows |
| Levinthal | do not enumerate all partitions; exploit constraints and local structure | not evidence for one search algorithm |
| funnel | many verified migration routes can approach one useful basin | no thermodynamic energy |
| domain/contact map | transition/state contacts matter more than file adjacency | dense calls alone do not define ownership |
| minimal frustration | remove mutually incompatible ownership/interface pulls | necessary ABI/scheduler tension may remain |
| local frustration | locate expensive forced seams and anomalous move gains | must be defined from a real objective/null model |
| allostery | RNG, order, allocation, and invalidation create long-range effects | analogy does not locate them; slicing does |
| coevolution | co-change may reveal contacts | correlation needs direct/indirect disambiguation |
| chaperone | facade/dual observer/adapter can reduce migration risk | it must not become a second writer or permanent layer |
| cotranslational folding | generalize ownership while new campaign behavior is discovered | partial evidence must not freeze a universal abstraction |
| intrinsic disorder | presentation/policy/tooling may remain deliberately flexible | deterministic state cannot become amorphous |
| neutral network | behavior-preserving refactors can improve access to future features | neutrality requires evidence at every revision |

The deepest transfer is that endpoint structure and folding kinetics are
different questions. A beautiful final diagram does not supply a safe patch
sequence.

## Novel falsifiable predictions

### Capability saturation law

Let `N_l` be new shared behavior implementation required by campaign level `l`.
After the inventory reaches most source behavior, expected `N_l` should decline,
apart from genuinely novel subsystems. A species-accumulation curve over
reachable keys can estimate the remaining surface.

If Levels 3 through 24 continue to require approximately constant new campaign
shell code, the fold is wrong: level number still keys behavior, descriptors
contain executable semantics, or the capability model missed a shared axis.

### Dispatch quotient prediction

Starting site-sensitive prevents false merges. Once schedule context is
explicit, some sites should become provably interchangeable. None quotienting
suggests stronger site dependence; widespread early quotienting suggests the
original key encoded incidental structure. Both outcomes teach us something.

### Reset compression prediction

Moving the exact baseline/live twins into one immutable baseline plus one live
aggregate should reduce transaction-clone bytes by approximately the removed
baseline payload while leaving `Omega` unchanged. Measure clone bytes, reset
projection, repeat-reset idempotence, and replay trace.

### Boundary-information precursor

Rising BIR or mutable-cut rate at an unchanged seam should precede more
reconciliation code, broad contexts, and edit collisions. Measure prospectively
during the campaign generalization rather than retrofitting the claim.

### Architecture hysteresis

Repeated reoptimization without movement cost will thrash seams as workloads
change. Adding a migration distance/budget should sharply reduce cumulative
movement with little steady-state boundary loss. Compare both strategies over
successive level inventories.

### Semantic-asset closure

Backward authoritative slicing should find simulation-relevant owner artifacts
more reliably than manually maintained presentation/simulation labels. Plant
influential and diagnostic-only bytes in exporter tests to measure false
negatives and false positives.

### Neutral-neighborhood/evolvability

After canonical-state and scheduler extraction, a larger fraction of new
behavior-family patches should avoid touching ownership, scheduler, ABI, and
Unity composition roots. Measure future change sets; do not claim evolvability
from appearance.

### Semantic contact order

Long-range contacts involving scheduler phase, RNG, allocation, topology, or
language/ABI boundaries should predict migration difficulty better than textual
distance or call count. Test against elapsed work, follow-up fixes, and changed
proof surfaces. Until calibrated, this is a conjecture.

## Migration implication for Levels 2 through 24

This research does not authorize a refactor before the current Level 2 positive
path is conformant and playable. The immediate course remains:

1. carry the smallest exact MSPRD-derived simulation fact into Rust;
2. land the blocked Level 2 behavior through Rust, C ABI, and Unity;
3. freeze the revision-bound `Omega` receipts at the green candidate; and
4. forbid new Level-N session shells, writable mirrors, and per-level ABI growth
   unless a positive-path blocker proves they are necessary.

After Level 2:

1. census reachable behavior, objectives, topology, and semantic facts for the
   remaining owner levels using source/exporter analysis only;
2. perform physical moves separately from semantic carrier changes;
3. classify state relative to reset operations and introduce one immutable
   baseline plus one canonical live state;
4. use dual observation from one authority, never dual writes;
5. collapse terrain, arena/entity, RNG, and journal duplication one carrier at
   a time;
6. extract the literal source scheduler and prove phase/continuation traces;
7. compute conservative capability closure and quotient dispatch keys only
   after equivalence;
8. generalize package/campaign selection through the existing ABI where
   possible; and
9. use Level 3 as the first full descriptor + missing-shared-capability proof,
   then repeat the census/diff/implement loop.

The work unit remains one independently landable capability or one canonical
carrier transition. The hypergraph chooses clean ownership lanes; it does not
justify a new planning bureaucracy.

## Mystery register

| Mystery | Why it matters | Smallest decisive evidence |
|---|---|---|
| exact `Omega` sufficient for Level 2 state equivalence | an incomplete alphabet can bless a false fold | enumerate current authoritative consumers and build one trace containing phase, RNG, identity, effects, reset, ABI, and hash |
| semantic atom granularity | too fine explodes search; too coarse hides seams | compare field, coherent-state-group, and handler-family graphs; retain the smallest granularity whose candidate seams are stable |
| complete guarded lifetime classification | reset semantics can depend on operation/map/mode | inventory writers/readers under every reached reset kind and plant a post-reset distinguishing read |
| source-observable topology closure | preserving every link may overconstrain; dropping one may break order | backward slice from `Omega` plus counterfactual reorders of derived candidates |
| conservative dispatch capability closure | authored rows miss spawned/action transitions | constructor/action reachability inventory and a planted missing transitive handler |
| minimum safe behavior key | site may be essential or incidental | compare same class/action across sites including prologue, RNG, topology, continuation, and successor relation |
| boundary information codec and workload strata | BIR is meaningless without stable encoding/sampling | version one canonical trace codec and report per-level/per-phase distributions |
| capability saturation curve for Levels 3–24 | sizes the remaining shared implementation and tests the fold | deterministic full-level inventory with cumulative/new reachable keys, source facts, objective forms, and behavior families |
| direct versus indirect co-change | history can misidentify broad edits as semantic contacts | normalize large commits; condition on shared dependencies; compare with source/runtime edges |
| exact solver feasibility after coarsening | determines whether CP-SAT can prove a top-level optimum | emit the reduced variable/constraint count, run a bounded exact solve, report incumbent, bound, gap, and time |
| robust Pareto seam | a result may be an artifact of one workload or coefficient | omit/perturb nonsemantic layers, bootstrap workloads, and compare exact/local/multilevel candidate fronts |
| reset clone savings | validates the highest-confidence performance prediction | instrument old/new bytes and distributions on identical release candidates |
| diagnostic trace noninterference | evidence machinery must not change the simulation | trace-on/trace-off differential `Omega` test |
| semantic asset closure beyond MSPRD | prevents later presentation authority leaks | slice every Level 2/3 reached authoritative mutation/effect into owner artifact reads |
| chaperone debt | temporary adapters can become kinetic traps | record single authority, callers, removal condition, and remaining cutover steps beside each adapter |
| neutral-neighborhood gain | tests whether the fold actually helps later levels | compare future capability patches' touched ownership/scheduler/ABI/Unity surfaces before and after refactor |

## Falsification conditions

Reject or revise this model if fresh evidence shows:

- product/level number is genuinely the primary source behavior dispatch key;
- exact source semantics require several independently writable physical
  carriers with a consistency protocol not represented here;
- later levels do not show capability convergence and their unique behavior is
  not explainable as real new subsystems;
- the proposed canonical state cannot preserve reset/hash/order without broader
  duplication;
- a lower-cut fold consistently worsens delivery, runtime, or correctness under
  representative workloads; or
- candidate seams are unstable under reasonable evidence and granularity
  perturbations.

A negative result is valuable. It prevents an elegant but false architecture
from becoming 22 more levels of debt.

## Research anchors

- [Parnas, information-hiding decomposition](https://doi.org/10.1145/361598.361623)
- [Ferrante, Ottenstein, and Warren, program dependence graph](https://doi.org/10.1145/24039.24041)
- [Horwitz, Reps, and Binkley, system dependence graph and slicing](https://research.cs.wisc.edu/wpis/papers/toplas90.pdf)
- [Karypis et al., multilevel hypergraph partitioning](https://doi.org/10.1109/92.748202)
- [Rissanen, minimum description length](https://doi.org/10.1016/0005-1098(78)90005-5)
- [Tishby, Pereira, and Bialek, information bottleneck](https://arxiv.org/abs/physics/0004057)
- [Rosvall and Bergstrom, map equation](https://pmc.ncbi.nlm.nih.gov/articles/PMC2234100/)
- [Abadi and Lamport, refinement mappings](https://lamport.azurewebsites.net/pubs/abadi-existence.pdf)
- [Pnueli, Siegel, and Singerman, translation validation](https://doi.org/10.1007/BFb0054170)
- [Anfinsen, protein-folding principles](https://pubmed.ncbi.nlm.nih.gov/4124164/)
- [Bryngelson and Wolynes, spin glasses/minimum frustration](https://pubmed.ncbi.nlm.nih.gov/3478708/)
- [Bryngelson et al., funnels and folding landscapes](https://arxiv.org/abs/chem-ph/9411008)
- [Ferreiro et al., localized frustration](https://pmc.ncbi.nlm.nih.gov/articles/PMC2148382/)
- [Holm and Sander, contact-matrix folding-unit parser](https://pubmed.ncbi.nlm.nih.gov/7937738/)
- [Plaxco, Simons, and Baker, contact order and folding rates](https://pubmed.ncbi.nlm.nih.gov/9545386/)
- [Morcos et al., direct coupling and contact inference](https://pmc.ncbi.nlm.nih.gov/articles/PMC3241805/)
- [Waudby, Dobson, and Christodoulou, cotranslational folding](https://doi.org/10.1016/j.tibs.2019.06.008)

The reusable skill contains the extended bibliography and application catalog;
this private document remains the only home for the remc2-specific mapping.
