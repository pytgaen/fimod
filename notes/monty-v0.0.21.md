# Monty v0.0.20 + v0.0.21 — Impact on fimod

Date: 2026-08-10
Previous: v0.0.19 → v0.0.20 → v0.0.21

Upstream:

- Releases: <https://github.com/pydantic/monty/releases/tag/v0.0.20>, <https://github.com/pydantic/monty/releases/tag/v0.0.21>
- Compare: <https://github.com/pydantic/monty/compare/v0.0.19...v0.0.21>

Two version bumps are covered here because fimod is on v0.0.19 and the head of
the registry is v0.0.21. They are wildly asymmetric, so they are analysed
separately: v0.0.20 carries 68 PRs and every breaking change, v0.0.21 carries
two PRs and touches nothing fimod links against.

## Summary

v0.0.20 is a breaking host-API release with a large feature payload. Three
mechanical compile breaks are all it takes to build again — but the release
also silently relocates memory-limit enforcement from the interpreter's own
tracker to a global allocator, which fimod does not install. Left unaddressed,
`max_memory` becomes decorative: a mold that accumulates memory gradually was
measured reaching **1.7 GB RSS under a 50 MB limit**. v0.0.21 is a no-op for
fimod: `monty/src` and `monty-types/src` are byte-identical to v0.0.20.

## API surface consumed by fimod

Regenerated from the current `src/` tree. 21 distinct public symbols across
`monty` and `monty-types`.

| Monty type/function | Fimod consumers | Role in fimod |
|---|---|---|
| `MontyObject` | `convert.rs`, `engine.rs`, `format.rs`, `pipeline.rs`, `dotpath.rs`, `hash.rs`, `msg.rs`, `gatekeeper.rs`, `exit_control.rs`, `format_control.rs`, `env_helpers.rs`, `iter_helpers.rs`, `monty_args.rs`, `regex.rs`, `template.rs`, `cmd/monty.rs` | Python-side IR for pipeline data, arguments, return values, external functions, REPL output |
| `DictPairs` | `convert.rs`, `engine.rs`, `format.rs`, `regex.rs`, `env_helpers.rs`, `iter_helpers.rs`, `template.rs` | Ordered Python dict construction and iteration |
| `MontyDate` | `convert.rs`, `engine.rs` | JSON conversion, sandboxed `date.today()` |
| `MontyDateTime` | `convert.rs`, `engine.rs` | JSON conversion, sandboxed `datetime.now()` |
| `MontyTimeDelta` | `convert.rs` | Serialization of Monty timedeltas |
| `MontyTimeZone` | `convert.rs` | Serialization of Monty time zones |
| `ExcType` | `engine.rs`, `cmd/monty.rs` | Permission/timeout/memory exception classification |
| `ExtFunctionResult` | `engine.rs` | Return/error channel for host OS-call handling |
| `LimitedTracker` | `engine.rs:930`, `cmd/monty.rs:10,31` | Runtime memory/duration limits for molds and the REPL |
| `ResourceLimits` | `engine.rs:1033-1042` | Construction of memory and duration limits from `SandboxPolicy` |
| `MontyException` | `engine.rs`, `cmd/monty.rs` | Compile/runtime/callback error transport and translation |
| `MontyRun` | `engine.rs` | Mold compilation and iterative execution |
| `NameLookupResult` | `engine.rs`, `cmd/monty.rs` | Dynamic resolution of fimod external functions |
| `OsFunctionCall` | `engine.rs:1045-1095` | Typed clock, environment, filesystem and `open()` suspensions |
| `PrintWriter` | `engine.rs`, `cmd/monty.rs` | stdout or debug-stderr routing for Python `print()` |
| `PrintWriterCallback` | `engine.rs` | `StderrPrint` callback contract |
| `CompileOptions` | `engine.rs`, `cmd/monty.rs` | Compilation configuration |
| `RunProgress` | `engine.rs:945-1030` | Main execution loop over complete/function/OS/name/future states |
| `MontyRepl` | `cmd/monty.rs` | Persistent interactive Monty session |
| `ReplProgress` | `cmd/monty.rs` | REPL suspension/resumption loop |
| `detect_repl_continuation_mode` | `cmd/monty.rs:14` | Complete versus multiline REPL input detection |
| `ReplContinuationMode` | `cmd/monty.rs:14` | REPL prompt and pending-snippet state |

Exhaustive matches re-checked for this bump: every `match` on `MontyObject` in
fimod carries a catch-all arm (`convert.rs:145` `other => bail!`,
`convert.rs:198` and the `gatekeeper.rs`/`env_helpers.rs` sites are partial
matches), so the new `NotImplemented` variant does not break compilation.

## Changes impacting fimod

### v0.0.19 → v0.0.20

| Symbol | Changed? | Nature | Action required |
|---|---|---|---|
| `LimitedTracker` | **yes** | removed; `ResourceTracker` went from trait to concrete struct (PR #613) | **Breaking** — `engine.rs:8,930`, `cmd/monty.rs:4,31` |
| `MontyRepl<T>` | **yes** | generic parameter dropped | **Breaking** — `cmd/monty.rs:10` |
| `ReplProgress<T>`, `RunProgress<T>`, `MontyRun::start<T>` | **yes** | generic parameter dropped | No fimod site annotates them explicitly |
| `ResourceLimits::new()` | **yes** | removed; `Default` impl only | **Breaking** — `engine.rs:1034` |
| `ResourceLimits.max_memory` | **yes** | no longer tracker-counted; enforced from allocator globals (PRs #646, #667, #668) | **Breaking (silent)** — see below |
| `ResourceLimits.max_recursion_depth` | yes | `Option<usize>` → `usize` (default 1000) | None — fimod never sets it |
| `MontyObject` | yes | new `NotImplemented` variant (PR #567) | None — catch-all arms cover it |
| `OsFunctionCall` | yes | `is_filesystem()` removed, `embedded_null_message()` added; variants unchanged | None — fimod uses neither |
| `ExcType`, `MontyException` | no | internal `estimate_size()` helpers removed | RAS |
| `PrintWriter`, `PrintWriterCallback` | no | `io.rs` untouched | RAS |
| `NameLookupResult`, `ExtFunctionResult` | no | `results.rs` untouched | RAS |
| `CompileOptions` | no | `run_options.rs` untouched | RAS |
| `DictPairs`, `MontyDate`, `MontyDateTime`, `MontyTimeDelta`, `MontyTimeZone` | no | unchanged in `object.rs` | RAS |
| `MontyRun`, `RunProgress` variants | no | same five variants | RAS |
| `detect_repl_continuation_mode`, `ReplContinuationMode` | no | unchanged | RAS |

### v0.0.20 → v0.0.21

`diff -rq` over `monty/src` and `monty-types/src` reports **no differences**.
Only `Cargo.toml`, `Cargo.lock` and `.cargo_vcs_info.json` change (the version
bump itself). The two PRs — #696 (version bump) and #697 (protocol version
utils) — land in `monty-runtime`, `monty-pool` and `monty-proto`, none of which
fimod depends on. Zero impact.

## Breaking changes

### 1. `LimitedTracker` removed (compile error)

`ResourceTracker` was a trait with `LimitedTracker`/`NoLimitTracker` impls; it
is now a single concrete struct. Every generic parameter threaded through the
host API disappears with it.

- Before: `LimitedTracker::new(limits)` / `MontyRepl<LimitedTracker>`
- After: `ResourceTracker::new(limits)` / `MontyRepl`

Files:

- `src/engine.rs:8` — import
- `src/engine.rs:930` — `let tracker = LimitedTracker::new(limits);`
- `src/cmd/monty.rs:4` — import
- `src/cmd/monty.rs:10` — `type FimodRepl = MontyRepl<LimitedTracker>;`
- `src/cmd/monty.rs:31` — `MontyRepl::new(..., LimitedTracker::new(...), ...)`

### 2. `ResourceLimits::new()` removed (compile error)

Only the `Default` impl remains, and it now seeds `max_recursion_depth` to
`DEFAULT_MAX_RECURSION_DEPTH` (1000) rather than `None`.

- Before: `ResourceLimits::new()`
- After: `ResourceLimits::default()`

File: `src/engine.rs:1034`.

### 3. `max_memory` enforcement moved to the global allocator (no compile error)

This is the one the compiler cannot catch, and the reason this upgrade is not
a five-minute job.

In v0.0.19, `LimitedTracker` kept a `current_memory: Cell<usize>` counter fed
by the Monty heap's own object accounting. In v0.0.20 that counter is gone.
`ResourceTracker::check_time()` and `check_allocation()` now call:

```rust
fn probe_memory() -> usize {
    LIVE_MEMORY.load(Relaxed).saturating_sub(BASELINE_MEMORY.load(Relaxed))
}
```

`LIVE_MEMORY` and `BASELINE_MEMORY` are `pub static AtomicUsize` in
`monty-types`, written **only** by a global allocator that charges them — the
`monty-alloc` crate. `ResourceLimits::max_memory`'s own doc comment says it:
*"Requires the executable to install and arm `monty-alloc`; otherwise the limit
is silently not enforced."*

fimod installs `mimalloc` as its `#[global_allocator]` (`src/main.rs:1-2`), so
`LIVE_MEMORY` stays 0, `BASELINE_MEMORY` stays `usize::MAX`, and
`probe_memory()` returns 0 forever.

Measured, mold accumulating 3M dicts under `max_memory = "50MB"`:

| Build | Outcome | Peak RSS |
|---|---|---|
| v0.0.19 (current `main`) | `sandbox exploded: max_memory exceeded (50MB)` | 145 MB |
| v0.0.21, plain bump | **completes normally** | **1725 MB** |
| v0.0.21 + counting allocator | `sandbox exploded: max_memory exceeded (50MB)` | 70 MB |

A single massive allocation is still caught — `check_large_result` passes the
estimated size as `additional`, which alone exceeds the cap — so
`[0] * 20_000_000` still explodes at ~9 MB RSS on both versions. Only gradual
accumulation escapes, which is the shape most real molds have.

Note that fimod has **no test covering actual memory enforcement**: the
`max_memory` occurrences in `tests/` only assert config parsing and
`fimod setup` output. This regression would ship green.

Two ways out, and they trade against each other:

- **A — adopt `monty_alloc::LimitedAllocator`.** No `unsafe` in fimod, upstream
  owns the correctness, and it adds a hard ceiling that ends the process above
  the soft limit. But `LimitedAllocator` forwards to `System`, so fimod loses
  mimalloc.
- **B — keep mimalloc, wrap it with a counting allocator.** ~35 lines charging
  `LIVE_MEMORY`/`BASELINE_MEMORY` around mimalloc. Validated in the probe: the
  table above's third row is exactly this. It requires `unsafe` blocks, which
  collide with `unsafe_code = "deny"` (`Cargo.toml:72`) — an
  `#[expect(unsafe_code, reason = ...)]` on the allocator module, mirroring
  what monty-alloc itself does.

Both probes were benchmarked at constant Monty version, 30 runs, 2.6 MB JSON
through a filter-and-project mold (`hyperfine --warmup 3`):

| Build | Mean | σ |
|---|---|---|
| v0.0.19 mimalloc (current `main`) | 214.1 ms | 26.0 ms |
| **B** — v0.0.21 mimalloc + counting | 220.0 ms | 23.9 ms |
| **A** — v0.0.21 monty-alloc (`System`) | 263.2 ms | 15.9 ms |

B is 1.20 ± 0.15 times faster than A and lands within noise of today's build,
so keeping mimalloc is worth the ~35 unsafe lines: option A would make every
fimod invocation ~20% slower to buy enforcement fimod already has today.

Either way the baseline must be armed before the first mold runs, and the
allocator-backed accounting turns out to be *more* faithful than v0.0.19's
(70 MB peak versus 145 MB for the same 50 MB budget).

## New capabilities unlocked

For mold authors, all from v0.0.20. The first four were run through the probe
binary as real molds, not read off the changelog:

- **`import collections`** — `deque`, `namedtuple`, `defaultdict`, `Counter`
  (PRs #608, #653). The `User*` classes are not implemented. Verified:
  `Counter` over a list of strings, and `defaultdict(list)` group-by.
- **`import itertools`** — `count`, `repeat`, `pairwise`, `compress`, `islice`,
  `chain`, `cycle` (PRs #632, #635), bounded by the recursion limit (#685).
  Verified: `islice(cycle(data), 7)`.
- **`import dataclasses`** — native `@dataclass` defined inside the sandbox,
  `is_dataclass`, `__dataclass_fields__` (PR #626). `__post_init__` is
  explicitly unsupported and rejected rather than silently skipped. Verified:
  `@dataclass class User` built from input rows, attribute reads, and attribute
  assignment after construction.
- **Function decorators** (PR #590). Verified: a `@twice` wrapper.
- **Class `__annotations__`**, stringized (PR #593).
- **User-defined `__iter__` / `__next__` / `__contains__` dispatch** (PR #609).
- **`NotImplemented`** and its use in user-defined equality (PR #567).
- **`Path / Path` and `str / Path`** in the `/` operator (PR #621).
- **`in` on bytes** fixed and made linear/interruptible (PRs #625, #628, #643).

Note that `__import__("collections")` is *not* a path into these — fimod
resolves external functions by name and rejects `__import__`. Molds must use a
real `import` statement.

Not user-facing in fimod: `os.listdir` and the filesystem wrappers (PR #622),
mount confinement via cap-std (PRs #669, #680, #681) — fimod's sandbox denies
filesystem access before those paths are reached.

### Pre-existing doc drift found while checking this

`docs/reference/monty-engine.md:194` tells mold authors "You cannot define
classes — use dicts and functions instead". A plain `class Point` with
`__init__` and a method runs fine, and it runs fine on **v0.0.19 too** —
`Node::ClassDef` is in the v0.0.19 compiler. This line was already wrong before
this bump; it is not something v0.0.20 changed. Fix it alongside, but do not
attribute it to this release.

## Dependency effects

- **The `jiter` git patch becomes unnecessary.** `Cargo.toml:97-102` pins
  `jiter` to a pydantic git rev because Monty 0.0.19 required `jiter ^0.15.0`,
  whose crates.io release pulled pyo3 0.28.x and tripped RUSTSEC-2026-0176/0177.
  Monty 0.0.21 resolves `jiter 0.16.0` from crates.io. The patch's own comment
  anticipated this: *"Remove once a published Monty version accepts jiter 0.16+."*
  Dropping it returns fimod to an all-registry dependency graph.
- Two new transitive deps: `ruff_python_codegen`, `ruff_python_literal`.

## Upgrade steps — applied

Option **B** was chosen for the allocator. All of the following landed together:

1. `Cargo.toml`: `monty = "0.0.21"`, `monty-types = "0.0.21"`.
2. Dropped the `[patch.crates-io] jiter` block; `jiter 0.16.0` now resolves
   from the registry, and no git source remains in `Cargo.lock`.
3. `src/engine.rs:8,930` + `src/cmd/monty.rs:3,10,31`: `LimitedTracker` →
   `ResourceTracker`, `MontyRepl<..>` type parameter dropped.
4. `src/engine.rs:1034`: `ResourceLimits::new()` → `ResourceLimits::default()`.
5. New `src/mem_limit.rs`: `CountingMiMalloc`, mimalloc wrapped to charge
   `LIVE_MEMORY`/`BASELINE_MEMORY`, installed as `#[global_allocator]` in
   `src/main.rs` with `arm_baseline()` as the first statement of `main`. The
   `unsafe_code = "deny"` lint is lifted for that module only, via
   `#[expect(unsafe_code, reason = ...)]` carrying the justification. Unlike
   `monty-alloc` it never ends the process itself — the soft limit reaching the
   interpreter's next checkpoint is what raises, which is how the limit behaved
   before 0.0.20.
6. `tests/cli/sandbox.rs`:
   `test_sandbox_max_memory_stops_gradual_accumulation` — 3M dicts appended
   under a 50 MB cap, expecting exit 137. Confirmed to **fail** when the global
   allocator is reverted to bare mimalloc (the mold returns `3000000`), so it
   guards the invariant rather than merely passing.
7. `docs/reference/monty-engine.md`: `LimitTracker`/`LimitedTracker` renamed to
   `ResourceTracker`, allocator-backed `max_memory` documented, and the
   mold-author list extended with `collections`, `itertools`, `dataclasses`,
   classes/decorators, and the `__import__` caveat. The stale "You cannot
   define classes" line is gone.

Verification on the real tree, not a probe: `cargo clippy --all-targets -D
warnings` clean, `cargo test` green (661 tests, 0 failures), `--features slim`
and `--features fast` both check, release binary reports
`Monty engine: v0.0.21`, and the 50 MB gradual-accumulation case stops at
70 MB RSS.

## Risk

**Medium as analysed, low as shipped.** The compile surface was small and fully
mapped — three mechanical edits. The whole risk lived in step 5: a plain tag
bump compiles, passes fimod's then-current test suite, and ships a sandbox
whose advertised `max_memory = 2GB` default (documented in
`docs/guides/cli-reference.md:543,564`) no longer holds — a security guarantee
regressing silently rather than a bug that surfaces on its own. Step 5 closed
it and step 6 now guards it.

What remains worth watching:

- The counting allocator charges `layout.size()`, which is what the caller
  asked for, not what mimalloc rounded up to. The accounting is therefore a
  slight under-estimate of true RSS, the same way `monty-alloc`'s is against
  `System`.
- `arm_baseline()` runs once per process, so under `fimod shape --watch` the
  budget is measured against the baseline captured at startup rather than
  re-armed per run.

## Recommendation

**Upgraded, as v0.0.21 directly** — v0.0.20 had no reason to exist as an
intermediate stop. The feature payload is genuinely useful to mold authors
(`collections`, `itertools`, native `@dataclass`), the jiter patch removal
returns fimod to an all-registry dependency graph, and the allocator-backed
accounting is more faithful than what it replaces (70 MB peak versus 145 MB for
the same 50 MB budget) at no measurable cost, since mimalloc is retained.
