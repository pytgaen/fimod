# Fimod — Roadmap

- **Updated:** 9 August 2026
- **Status:** canonical planning source
- **Scope:** version themes and decision criteria, not promised release dates

This roadmap supersedes the proposal embedded in `notes/eval_072026.md`. Items
remain conditional until they enter an active release cycle; shipped behavior
must move to the appropriate user documentation rather than remain here.

## Strategic direction

Fimod should now evolve from a feature-rich data transformation CLI into a
reliable, testable and reproducible mold runner for CI and configuration
workflows.

The product thesis remains:

> CLI ergonomics, Python semantics, Rust trust boundary, zero install.

Format conversion and inline expressions are the acquisition surface: they make
Fimod immediately useful. Reusable molds, fixtures and Git-hosted registries
provide the longer-term team value. Reproducibility and trust in CI should
become the next differentiator.

This direction serves the three audiences already identified in `VISION.md`:

- DevOps and CI engineers transforming manifests and API responses;
- data plumbers moving between common structured formats;
- tool authors sharing tested transforms through registries.

It does not change the project boundaries: no HTTP server, generic PyPI imports,
central registry, hosted account or hostile multi-tenant sandbox.

## Product progression

```text
write -> test -> publish -> pin -> verify -> run in CI
```

Fimod already covers most of `write -> test -> publish`:

- inline expressions and mold files;
- typed arguments and mold defaults;
- `fimod mold test` fixtures;
- local and remote registries;
- catalogs with content hashes;
- Git refs that can be pinned manually.

The main product gap is the project-level workflow from `pin` to `run in CI`.
The exact UX remains to be specified before implementation.

## Version themes

### 0.9.1 — Finish and publish the reliability cycle

**Goal:** ship the reliability work before adding new product surface.

Current work already covers:

- chain-wide runtime budgets and bounded dynamic step injection;
- exact JSON integers through the Rust/Monty boundary;
- bounded-memory NDJSON identity conversion;
- safer installers and bounded process-wide caches;
- HTTP, batch raw, duration parsing and library API fixes;
- Windows installer and transformation smoke coverage;
- dependency and public documentation corrections;
- migration of `monty` and `monty-types` 0.0.19 to crates.io, including the
  required host API adaptations.

Remaining release work:

1. Resolve the Monty 0.0.19 duration semantics around time spent in Rust host
   callbacks. Either preserve the documented wall-clock limit at resume
   boundaries or explicitly change and document the policy.
2. Add focused Monty 0.0.19 regression coverage for the adopted capabilities
   and host-boundary behavior.
3. Synchronize the public Monty documentation:
   - simple classes and their serialization boundary;
   - partial `unicodedata`;
   - supported text codecs;
   - assertion-message policy;
   - current host callback API.
4. Rerun the complete release gates after the Monty migration.
5. Complete the normal version, changelog, release and publication workflow.

**Exit criteria:**

- all lint, test, documentation, installer, performance and prerelease gates
  pass on the final release state;
- the documented duration policy matches the implemented Monty 0.0.19
  behavior;
- no known major correctness or data-loss issue remains;
- version `0.9.1` is published through the normal release workflow.

No unrelated feature should enter this cycle.

### 0.10 — Contract and consolidation

**Goal:** make existing behavior easier to maintain and safe to depend on.

#### Public contract

Define and test the compatibility surface for:

- CLI arguments, stdout/stderr separation and exit codes;
- `transform(data, **_)` and the `args`, `env`, `headers`, `pipeline` context;
- mold defaults and typed arguments;
- dynamic pipeline mutations;
- `catalog.toml` and registry resolution;
- sandbox capabilities and resource limits;
- the experimental Rust library API;
- Monty upgrade and deprecation policy.

#### Pipeline consolidation

- unify input reading, format detection and parsing between the normal,
  identity and library paths;
- extract one output destination planner for stdout, files, batch, raw,
  `--in-place`, URLs and mold overrides;
- preserve measured fast paths behind explicit parity tests;
- add an explicit configurable limit for HTTP response bodies;
- complete bounded-memory CSV to NDJSON identity conversion after consolidating
  CSV header handling.

#### Performance and maintainability

- run performance tests sequentially before establishing a baseline;
- record startup time, throughput and peak RSS on representative input sizes;
- verify parity between normal, identity and library execution;
- classify internal notes as active, decision record or archived;
- restore one canonical roadmap source instead of leaving future work spread
  across evaluation and design notes.

**Exit criteria:**

- equivalent inputs produce equivalent results through normal, identity and
  library paths;
- CPU, memory and HTTP-body limits are documented at the correct boundary and
  covered by tests;
- each distributed platform has a functional binary smoke test;
- the compatibility contract is published and has survived at least two
  development cycles without an unintended mold break.

### 0.11 — Reproducible team molds

**Goal:** turn molds into dependable project assets rather than scripts found
only in a user-level registry configuration.

This is a new product track and requires a short specification before code.

#### Authoring workflow

Consider:

- `fimod mold init <name>` to create a minimal mold, fixture directory and
  metadata skeleton;
- machine-readable and/or JUnit output from `fimod mold test` for CI systems;
- a documented golden path from a local mold to a versioned team registry;
- validation that catalog metadata, companion files and test fixtures are
  coherent before publication.

#### Project-level reproducibility

Specify the smallest useful mechanism for declaring and freezing mold inputs.
Possible shapes include a project-local `fimod.toml`, a lock file, or a
`registry freeze/verify` workflow. Do not implement several overlapping
mechanisms.

The selected design should answer:

- which registry and exact Git ref or content hash supplied a mold;
- whether CI can fail closed when the remote content differs;
- how private registry authentication remains environment-based;
- how local development updates a pin intentionally;
- how the mechanism works without a Fimod account or central index;
- whether the existing `catalog.toml` hash is sufficient or needs a stronger
  project-level contract.

#### Adoption proof

Validate the workflow with at least three external CI/config use cases:

1. the initial problem and previous shell/Python solution;
2. the Fimod command or mold;
3. the measurable gain;
4. the observed limitation;
5. a reusable fixture or example committed to the project.

At least one use case should consume a shared registry from CI.

**Exit criteria:**

- a new user can create, test, publish, pin and verify a mold from documented
  commands;
- at least one real team registry is consumed reproducibly in CI;
- external use cases, rather than a feature matrix, identify the next product
  investment.

### 0.12 — Record-scale processing, only if usage confirms it

**Goal:** support large NDJSON and CSV transforms with bounded memory without
changing the one-shot mold contract silently.

The main candidate is an explicit record-processing mode:

- compile a mold once;
- apply `transform(record, ...)` independently to each input record;
- preserve input order by default;
- define fail-fast and partial-output behavior;
- keep dynamic pipeline behavior and resource budgets explicit;
- avoid reinterpreting the existing `data` array contract.

Mechanical optimizations remain measurement-driven:

1. Operate directly on `MontyObject` in selected `it_*` helpers, starting with
   a profiled helper and preserving the current ordering and missing-value
   semantics.
2. Resolve the final format earlier and write supported outputs through
   buffered writers, without weakening atomic file replacement or output
   parity.
3. Stream exact-identity CSV conversions after CSV header handling has been
   consolidated; arbitrary molds retain the existing one-shot data contract.
4. Measure `MontyRun` compilation against cloning before deciding whether a
   bounded compiled-mold cache is justified.

Benchmark commands and measurements belong in `notes/perf-baseline.md`, not in
this roadmap. Each optimization must be accepted independently from a profile
showing that it addresses a dominant cost.

Parallel batch processing, a generalized streaming engine or a new dependency
should only follow a profile showing that it solves the dominant cost.

**Exit criteria:**

- bounded peak RSS on representative large NDJSON and CSV inputs;
- stable, documented record semantics and error behavior;
- unchanged output for existing one-shot molds;
- a demonstrated user workload that benefits from the mode.

If those conditions are not met, skip this release theme and continue toward
1.0 with the existing one-shot model.

### 1.0 — Freeze the supported contract

**Goal:** promote a validated product contract rather than declare stability
from test count alone.

Requirements for a `1.0-rc`:

- no known major correctness or data-loss issue;
- stable and versioned CLI, mold, registry and sandbox contracts;
- a published deprecation policy;
- mold compatibility demonstrated across multiple Monty upgrades;
- green release, installer and binary smoke tests for each distributed
  platform;
- reproducible performance and RSS baselines;
- several external, reproducible workflows;
- at least one shared registry used outside the maintainer's own environment;
- continued compliance with the non-goals in `VISION.md`.

Release signing or attestations should only be added with an explicit trust
model. Checksums remain necessary but should not be described as signatures.

## Features not to prioritize

Do not prioritize these without evidence from real workflows:

- more formats or built-ins because they appeared in an old backlog;
- Rust implementations of `df_diff`, `df_patch` or `df_merge` before validating
  them as molds;
- `--jobs N` before measuring batch compilation and I/O costs;
- WASM or additional distribution channels without an installation problem;
- a richer intermediate representation without demonstrated data-loss pain;
- classes as a headline feature rather than a Monty compatibility detail.

The following remain outside the product vision:

- an HTTP daemon;
- arbitrary PyPI imports or a complete CPython runtime;
- a central mold registry or hosted account system;
- a promise to execute hostile molds safely in a multi-tenant service.

## Decision rules for new work

Before adding an item to the committed roadmap:

1. Name the target user and concrete workflow.
2. Show the intended terminal UX before choosing the implementation.
3. State which existing contract or invariant it touches.
4. Define an end-to-end fixture or observable acceptance criterion.
5. Measure first when the proposal is primarily about performance.
6. Prefer implementing domain-specific behavior as a mold before promoting it
   to a Rust built-in.
7. Record whether the item is committed, experimental or only an idea.

## Recommended sequence

```text
publish 0.9.1
    -> consolidate and document the contract in 0.10
    -> validate reproducible team molds in 0.11
    -> add record-scale processing only if usage proves the need
    -> freeze the validated contract for 1.0
```
