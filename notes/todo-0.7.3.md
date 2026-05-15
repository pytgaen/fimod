# TODO 0.7.3 Follow-up

Post-release cleanup for the 0.7.3 cycle.

Most items from the original 0.7.3 simplification backlog shipped in the
0.7.3 release and are documented in `CHANGELOG.md`. This file keeps only the
remaining actionable follow-ups so it stays useful as an implementation note.

## Open

### CLI plumbing via Clap

`cmd/shape.rs::build_script_refs` still reparses `std::env::args()` manually
to preserve the ordering of `-m` / `-e` steps.

Risk:

- it duplicates parsing behavior already owned by Clap;
- it has edge cases around rare flag syntaxes and `--`;
- it makes the central mold-chain contract harder to reason about.

Preferred direction:

- use Clap-provided argument indices (`ArgMatches::indices_of`) or another
  first-class ordered step representation;
- keep the existing chain-order tests and add coverage for the edge cases that
  motivated the change.

## Explicitly Deferred

- **MontyRun compilation cache**: one-shot CLI invocations do not benefit much;
  batch/chain gains are not worth upstream API coupling yet.
- **Batch parallelism via `rayon --jobs N`**: needs a dedicated concurrency
  design pass covering output ordering, exit-code semantics, sandbox behavior,
  and registry/cache contention.

## Cross-references

- `notes/CODE_LAYOUT.md` — current file/module map.
- `notes/ARCHITECTURE.md` — current architecture and extension points.
- `notes/DESIGN_NOTES.md` — design invariants.
- `CHANGELOG.md` — shipped 0.7.3 items.
