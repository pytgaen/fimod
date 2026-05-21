# TODO 0.8.0

Scope note for the post-0.7.3 work that has outgrown a 0.7.4 patch branch.

Current state:

- branch name is still `feat/v0.7.4`, but the accumulated scope now looks like
  `0.8.0`;
- base release is `v0.7.3`;
- this file tracks what has already been done since `v0.7.3` and what still
  needs an explicit planning decision.

## Already Done Since v0.7.3

### Mold Contract Cleanup

Commit: `ca59e1f fix(mold): align inline wrapper with kwargs convention`

- Inline `-e` wrappers now follow the same kwargs convention as reusable molds:
  generated transforms keep `**_` compatibility with the context parameters.
- Documentation examples were normalized around `transform(data, ..., **_)`.
- Internal architecture/design notes and project synthesis were refreshed to
  match the current mold contract.
- `.gitignore` / local assistant guidance were adjusted around repo-local notes
  and workflow artifacts.

Validation status:

- committed before this planning note;
- covered by the normal CLI/mold contract tests in the branch history.

### Output Fast Path And `fimod-fast`

Commit: `f2be104 perf(pipeline): add fast output path and ffimod profile`

- Added `MontySerialize` so selected output formats can serialize
  `MontyObject` directly without allocating an intermediate `serde_json::Value`.
- Fast path applies to `json-compact`, `ndjson`, `lines`, and `txt` when the
  run is compatible with direct serialization.
- Added `fimod-fast`, a speed-optimized binary using the new `release-fast` profile
  (`opt-level = 3`).
- Renamed the fast binary to `fimod-fast` for consistency with `fimod-slim`.
- Added Taskfile entries for local fast builds and Linux fast distribution
  artifacts.
- Added opt-in performance smoke tests in `tests/performance.rs`.
- Added `notes/perf-baseline.md` with v0.7.3 baseline numbers and v0.7.4-dev /
  `fimod-fast` comparisons.

Validation status:

- `rtk task lint` passed;
- `rtk task test` passed;
- `rtk task test:performance` passed with 6 performance smoke tests.

### Watch Debounce Fixes

Commit: `f2be104 perf(pipeline): add fast output path and ffimod profile`

- Removed `notify-debouncer-mini` from watch mode and implemented debounce
  directly over `notify` events.
- Watch mode no longer reruns on its own input/mold reads.
- Watch mode now filters more strictly on write-like events before rerunning the
  pipeline.
- Added CLI regression coverage for watch mode not looping on its own reads.

Validation status:

- committed in the same performance/watch cleanup commit;
- `rtk task lint` passed;
- `rtk task test` passed.

### CLI Plumbing Via Clap

Current worktree, not committed yet.

- Removed the manual `std::env::args()` scan from `cmd/shape.rs`.
- The ordered `-m` / `-e` chain is now derived from Clap argument indices
  (`ArgMatches::indices_of`) and passed explicitly into the shape pipeline.
- Watch mode reuses the same ordered chain on every rerun.
- Added integration coverage for:
  - existing mixed mold/expression ordering;
  - non-step flags interleaved with `-m` / `-e`;
  - attached short values such as `-mfoo` and `-eexpr`.
- Updated `notes/SYNTHESE_PROJET.md` to mark the old CLI parsing risk as
  treated.
- Removed the closed `notes/todo-0.7.3.md` follow-up note.

Validation status:

- `rtk cargo test --test cli chain` passed;
- `rtk task lint` passed;
- `rtk task test` passed.

## Scope Candidates Still To Decide

### Documentation Honesty Around Monty / Python

The current product message is strong: Python-powered, no Python installed.
For 0.8.0, decide whether the public docs should make the Monty boundary even
more explicit near the top:

- Python syntax and common builtins are supported;
- this is not CPython;
- no PyPI ecosystem, no full stdlib parity;
- Fimod provides Rust built-ins to fill practical gaps.

Status:

- partially improved in the mold contract cleanup;
- still worth a dedicated docs pass if 0.8.0 is positioned as a broader
  semantics/architecture release.

### Fast Variant Distribution And Version Identity

`fimod-fast` is now coherent with `fimod-slim` as a named variant, but the
distribution UX still needs a dedicated pass before release.

Decision needed:

- review GitHub CI/release workflows so the fast variant is either published as
  an official artifact or intentionally kept out of public releases;
- review `install.sh` so users can either keep the default `fimod` install path
  or choose a variant (`normal`, `slim`, `fast`) explicitly;
- find the right UX for proposing that choice without making the default install
  noisy or blocking for scripts;
- make `fimod --version` expose the active variant (`normal`, `slim`, `fast`) so
  installed binaries are diagnosable.

Open design point:

- decide whether variants are separate binary names only (`fimod`, `fimod-slim`,
  `fimod-fast`) or whether `install.sh` can install one selected variant as the
  canonical `fimod` command.

### Branch / Release Shape

The current branch name `feat/v0.7.4` no longer matches the real scope.

Decision needed:

- rename/recreate branch as `feat/v0.8.0` or equivalent;
- decide whether the existing two commits stay as-is or are reorganized before
  PR;
- prepare PR title/body as the future squash commit source for changelog
  generation.

## Deferred Beyond Immediate 0.8.0 Planning

### MontyRun Compilation Cache

One-shot CLI invocations do not benefit much. Batch/chain gains may exist, but
the feature is not worth upstream Monty API coupling yet.

Planning questions:

- measure whether repeated mold compilation is a real bottleneck in batch or
  long chains;
- decide whether the cache belongs in fimod or should wait for a Monty-level
  API;
- define invalidation semantics for local molds, URL molds, registry molds,
  inline expressions, and sandbox settings.

### Batch Parallelism Via `rayon --jobs N`

Needs a dedicated concurrency design pass before implementation.

Planning questions:

- preserve output ordering for directory output and stdout;
- define exit-code semantics when one or more files fail;
- confirm sandbox behavior under concurrent Monty executions;
- avoid registry/cache contention across workers;
- decide whether `--jobs` should apply only to batch mode or also to
  `--input-list`.
