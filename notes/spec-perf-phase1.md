# Spec — Single-arena converter, closed stdout, direct indented output and `opt-level = "s"`

- **Date:** 12 November 2026
- **Status:** WP1 to WP5 implemented and review-approved on 12 November 2026, all uncommitted. WP4's last correction (medians in `notes/perf-baseline.md`) was checked by the orchestrator against the review's requests, not re-reviewed.
- **Source:** `notes/eval_112026.md` (measurements and rationale). Read sections 4.2, 4.4 and 5 before starting.
- **Base:** branch `feat/monty-1.6.0` at `3bd778d`, working tree clean apart from untracked notes.

This document is the complete brief for the agents doing the work. It is written for a reader who has not seen the discussion that led to it.

## 1. Goal and scope

Five work packages, done one after the other in this order:

- **WP1 — Single-arena converter.** Make the `serde_json::Value` to `MontyObject` conversion build one Monty arena directly, instead of one small arena per value merged level by level. Same signatures, same results. Ship it with tests that protect future Monty upgrades.
- **WP2 — A closed stdout is a normal end.** Today `fimod … | head -1` either panics or reports an error, depending on the output format. Make it end quietly for every output of `fimod shape`. It comes before WP3 so that WP3 changes no visible behaviour.
- **WP3 — Direct indented JSON output.** Serialize indented JSON, the default output format, straight from the Monty value, as compact JSON already is. Stop copying dictionary keys in the serializer. Same output bytes.
- **WP4 — Standard profile at `opt-level = "s"`.** Change the release profile from `"z"` to `"s"` and bring the documentation in line. It comes last so that the sizes and timings it records reflect the final code.

- **WP5 — A closed stdout is a normal end for the other commands.** WP2 covered `fimod shape`. The listing, registry, setup, mold-test and REPL commands still panic on `fimod mold list | head -1`. Apply the same rule to them.

Nothing else. Section 11 lists what is explicitly out of scope.

## 2. Roles and workflow

| Role | Model | Responsibility |
| --- | --- | --- |
| Orchestrator | main session | Launches agents, relays reports, escalates to the maintainer |
| Dev agent | Sonnet | Implements one work package, runs the checks, writes the dev report |
| Review agent | Opus | Reviews one finished work package against this spec, read-only |

For each work package in order:

1. The dev agent implements it and reports.
2. The review agent reviews it. If it requests changes, the same dev agent fixes them and the review is repeated.
3. Only when the review approves does the next work package start.

At the end, the orchestrator hands all reports to the maintainer, who decides on commits.

At most two fix rounds per work package. After that, or whenever an acceptance criterion cannot be met, stop and report to the maintainer instead of working around it.

## 3. Rules for every agent

- Read `AGENTS.md` and `AGENTS.base.md` first. For WP1, WP2 and WP3 also read "Key invariants" in `notes/ARCHITECTURE.md`, and "Monty 1.0.0 boundary" and "Native processing of JSON-compatible results" in `notes/DESIGN_NOTES.md`.
- **Git is read-only.** Allowed: `git status`, `git diff`, `git log`, `git show`. Forbidden: `add`, `commit`, `stash`, `checkout`, `switch`, `restore`, `reset`, `clean`, `push`, tags, pull requests. The maintainer commits.
- Do not edit `CHANGELOG.md`, `notes/eval_112026.md`, this spec, or anything under `tmp/` that you did not create.
- No new dependencies, no `unsafe`, no edits outside the files listed for the work package.
- Existing tests must pass unmodified. If an existing test fails, the change is wrong; do not adapt the test.
- No AI attribution anywhere (code, comments, docs).
- Code, comments and documentation in English. `notes/perf-baseline.md` is written in French; keep that file's language.
- `AGENTS.md` asks for the `rtk` prefix on verbose commands. If `rtk` is not on `PATH`, run the same commands through `mise exec --` and say so in the report.
- Scratch files (datasets, saved binaries) go under `tmp/perf-phase1/`, which is ignored by git.
- Timings on the development machine are noisy. Compare runs back to back, report medians with the observed spread, and never add complexity to reach a number. Report instead.

## 4. WP1 — Single-arena converter

### 4.1 Background

`json_to_monty(&Value)` and `json_into_monty(Value)` in `src/convert.rs` are how every non-CSV input, every helper result and the `args`, `env` and `headers` values reach Monty.

Today they build values with the public constructors (`MontyObject::string`, `::list`, `::dict`). Each constructor returns a `MontyObject` owning its own arena, a `Vec<MontyNode>`. A parent container then merges every child arena into its own. The effect is one heap allocation per leaf and one copy of every node per nesting level.

A `MontyObject` is one arena (`MontyGraph`) plus a root id. Nodes are stored in post-order: every child id is lower than the id of the container that holds it. `monty_types::unstable` exposes what is needed to fill an arena directly: `MontyGraph::with_capacity`, `MontyGraph::push`, `MontyNode`, `NodeId` and `object_from_graph`. fimod already reads arenas through that module; this work package also writes through it.

Local reference (standard profile, 100,000 records, 7.87 MB of JSON): the conversion takes about 62 ms today and about 27 to 35 ms with a single arena.

### 4.2 Required change

In `src/convert.rs`:

1. Add a private `count_nodes(&Value) -> usize`: one node per JSON value, plus one node per object key.
2. Rewrite `json_into_monty(Value)` to allocate `MontyGraph::with_capacity(count_nodes(&value))`, push children before their container, push object keys as `MontyNode::String`, and return `unstable::object_from_graph(graph, root)`. Strings are moved, not cloned.
3. Rewrite `json_to_monty(&Value)` the same way, cloning strings.
4. Keep the number mapping exactly: `as_i64` gives `Int`; otherwise `as_u64` gives `BigInt`; otherwise `Float(as_f64().unwrap_or(0.0))`.
5. Keep both public signatures and doc comments accurate. Remove helpers that become unused.

Constraints:

- **Exact capacity is mandatory.** Growing the arena by doubling can leave up to twice the needed capacity allocated. The sandbox counts allocated bytes against `max_memory`, so unused capacity would be charged to the mold.
- **No shared nodes.** Every object key gets its own node, as today. Sharing key nodes between objects is out of scope.
- Dictionary entry order must be preserved.
- Recursion is acceptable; the current code is recursive too.

### 4.3 Tests to add

These tests are the point of the work package as much as the speed. They must fail if a future Monty release changes how arenas are built, compared or loaded.

Unit tests in the existing `tests` module of `src/convert.rs`:

- **Test 1 — Reference built on public constructors.** Keep the previous implementation as a test-only function (`#[cfg(test)]`), built only on `MontyObject::none/bool/int/bigint/float/string/list/dict`. It is the oracle: the stable public API against the `unstable` one.
- **Test 2 — Fixed corpus.** For each document, assert that `json_into_monty` and `json_to_monty` both equal the reference with `==`, and that `py_repr()` strings are equal (structural equality alone could hide an `int` turned into a `float`). The corpus must include: `null`, booleans, `0`, `-1`, `i64::MIN`, `i64::MAX`, `u64::MAX`, `0.5`, `-0.0`, `1e308`, empty string, non-ASCII text, a string with escapes, empty array, empty object, nested arrays, nested objects, an object with at least 20 keys in non-sorted order, an array of records, and a document nested 100 levels deep built in code.
- **Test 3 — Generated documents.** A deterministic generator written in the test (fixed seed, no new dependency) producing at least 2,000 documents of bounded depth and width. Same assertions as the corpus.
- **Test 4 — Arena shape.** For the corpus, assert that the arena holds exactly `count_nodes` nodes and that the root is the last node (`unstable::graph_parts`).
- **Test 5 — Round trip.** `monty_to_json(json_into_monty(v.clone())) == v`, and `serde_json::to_vec(&MontySerialize(..))` equals `serde_json::to_vec(&v)`.

CLI tests in `tests/cli/monty.rs`, in the style of `test_monty_graph_shared_values_and_cycles_at_json_boundary`. Use a mold file: a lone `-e 'data'` is a native conversion that never starts Monty.

- **Test 6 — `test_monty_graph_host_built_input_round_trips_through_vm`.** A JSON input covering every scalar kind above plus nesting, passed through `def transform(data, **_): return data`. The output must equal the input, for `json` and `json-compact`.
- **Test 7 — `test_monty_graph_host_built_input_has_python_types`.** A mold returning the type names of the values and keys it receives (`int`, `float`, `str`, `bool`, `NoneType`, `list`, `dict`), and `data["n"] == 18446744073709551615` for a `u64::MAX` input.
- **Test 8 — `test_monty_graph_host_built_records_are_independent`.** Three identical input records. The mold reassigns a field in the first, deletes a key from the first and appends to a nested list in the second. Assert that only those records changed.

Performance smoke test in `tests/performance.rs`, following the existing opt-in pattern (`#[ignore]`, `median_elapsed`, `assert_under_budget`):

- **Test 9 — `value_to_monty_conversion_large_array_under_budget`.** Times `json_into_monty` alone on 100,000 records from `large_json_array`. Budget: 200 ms in release, 4,000 ms in debug.

### 4.4 Order of work and measurement

1. Before touching `src/convert.rs`: build the release binary and save it as `tmp/perf-phase1/fimod-before-wp1`. Generate the 100,000-record dataset with the generator in the appendix of `notes/eval_112026.md`, as `tmp/perf-phase1/users100k.json`.
2. Add test 9 and run it against the current implementation. Record the median.
3. Add tests 1 to 8. Tests 2 to 8 must pass against the current implementation too; they describe behaviour that must not change.
4. Implement section 4.2. Run test 9 again and record the median.
5. Build the release binary again and compare the two binaries on the dataset:

   ```bash
   hyperfine -N --warmup 2 --runs 9 \
     "tmp/perf-phase1/fimod-before-wp1 s --sandbox-file= -i tmp/perf-phase1/users100k.json -e '[r for r in data if r[\"active\"]]' --output-format json-compact" \
     "target/release/fimod s --sandbox-file= -i tmp/perf-phase1/users100k.json -e '[r for r in data if r[\"active\"]]' --output-format json-compact"
   /usr/bin/time -f '%e s %M KiB' <each command> > /dev/null
   ```

6. Check that the tests can fail. Apply each of these mutations in turn, confirm that at least one new test fails, then revert it: push an integer as a float; swap two entries of an object; drop the last element of an array. List the failing tests in the report.

### 4.5 Documentation

- `notes/DESIGN_NOTES.md`, "Monty 1.0.0 boundary": state that the two converters fill one pre-sized arena through `unstable::MontyGraph` and `object_from_graph`, and that a test-only reference built on the public constructors guards equivalence across Monty upgrades.
- `notes/perf-baseline.md`: add a dated section with the before and after medians of test 9, the CLI timings and the peak memory of step 5, the machine and the commands. Leave existing sections untouched.

### 4.6 Acceptance criteria

- [ ] `json_to_monty` and `json_into_monty` keep their signatures and no call site changes.
- [ ] Tests 1 to 9 exist and pass. The mutation checks of step 6 are reported.
- [ ] `task lint` and `task test` pass, with no existing test modified.
- [ ] `cargo test --release --test performance -- --ignored --nocapture --test-threads=1` passes.
- [ ] The conversion is at least 35 % faster after the change than before under the allocator of the shipped binary (`CountingMiMalloc` over mimalloc), same machine, before and after alternated. Test 9 does not measure this: `tests/performance.rs` links the library without the binary's global allocator, so it runs under the system allocator and only guards its time budget. Amended on 12 November 2026: the original criterion applied the threshold to test 9, which showed 33 % under the system allocator for about 55 % under mimalloc (`notes/perf-baseline.md`).
- [ ] The CLI comparison is not slower, and peak memory (`%M`) does not grow by more than 5 %.
- [ ] The two documentation updates are present.

If the speed or memory criterion is not met, stop and report the numbers.

### 4.7 Files

`src/convert.rs`, `tests/cli/monty.rs`, `tests/performance.rs`, `notes/DESIGN_NOTES.md`, `notes/perf-baseline.md`.

## 5. WP2 — A closed stdout is a normal end

### 5.1 Background

When the reader of fimod's stdout closes the pipe before fimod has finished writing (`fimod … | head -1`), nothing went wrong: the reader got what it wanted. fimod currently handles this in two different ways, neither of them right. Observed on 100,000 records piped to `head -1`:

| Output | Mechanism | Today |
| --- | --- | --- |
| Indented JSON, YAML, TOML, CSV | `print!` in `output_result` | panic, exit status 134 |
| `print()` inside a mold | `print!` in Monty's `PrintWriter::Stdout` | panic, exit status 134 |
| Compact JSON, NDJSON, Lines, TXT, raw | `write_bytes_to` | `Error: Failed to write binary output to stdout`, exit status 1 |
| Streaming identity conversions (`-e data`) | `BufWriter` over stdout | `Error: Failed to stream …`, exit status 1 |

Target behaviour: a closed stdout is a normal end. fimod stops writing to stdout, prints nothing about it, and exits with the status the command would have had if writing had succeeded: 0, or the code requested through `set_exit()`.

### 5.2 Required change

1. `src/pipeline.rs`: add one private helper that writes bytes to stdout, flushes, and maps a failure of kind `io::ErrorKind::BrokenPipe` to success. Use it in `output_result`, replacing `print!`, and in the stdout arm of `write_bytes_to`.
2. Streaming identity conversions to stdout (`write_json_array_reader_as_format`, `write_ndjson_reader_as_json`): a `BrokenPipe` raised while streaming ends the stream and counts as normal completion. Stop reading the input; do not parse the rest of it.
3. `src/engine.rs`: outside debug mode, give Monty a `PrintWriterCallback` that writes mold `print()` output to stdout, modelled on the existing `StderrPrint`, instead of `PrintWriter::Stdout`. After a `BrokenPipe` it stops writing and keeps returning success, so the mold runs to completion. Write through `std::io::stdout()` so that ordering with the final output is unchanged. Check how the callback trait routes `print(..., file=sys.stderr)` and keep that output on stderr.

Constraints:

- Only `BrokenPipe` is a normal end. Any other failure to write to stdout is an error: exit status 1 and a message, for every output format. Today the `print!` paths panic on such failures; returning an error instead is intended.
- When stdout stays open, output bytes and their order are identical to today for every format and for mold `print()`.
- Writes to files (`-o`, `--in-place`, batch output) are untouched.
- No `process::exit` in library code, no signal handling, no `unsafe`.
- Do not reword existing error messages.

### 5.3 Tests to add

A new file `tests/cli/closed_stdout.rs`, registered in `tests/cli.rs`, with its tests under `#[cfg(unix)]`. Write one helper that spawns fimod with stdout and stderr piped, reads a few bytes of stdout, drops the handle, waits, and returns the exit status and stderr. The output must be larger than a pipe buffer: produce at least 2 MB.

- **Test 1 — `test_closed_stdout_is_silent_success_for_every_output_format`.** A `--no-input` mold returning a large list of records, for `json`, `json-compact`, `ndjson`, `yaml` and `csv`, and a large list of strings for `lines`. Each run: exit status 0, empty stderr.
- **Test 2 — `test_closed_stdout_is_silent_success_for_identity_conversions`.** `-e data` on a large JSON array file with the default format, with `--output-format ndjson` and with `--output-format csv`, and on a large NDJSON file with `--output-format json`. Each run: exit status 0, empty stderr.
- **Test 3 — `test_closed_stdout_is_silent_success_for_mold_print`.** A mold that prints 200,000 lines and then returns a value: exit status 0, empty stderr.
- **Test 4 — `test_closed_stdout_keeps_set_exit_code`.** A mold that calls `set_exit(3)` and returns a large result: exit status 3, empty stderr.
- **Test 5 — `test_mold_print_output_is_unchanged_when_stdout_is_open`.** A mold using `print("a", "b", sep="-", end="!")`, a bare `print()`, and `print("x", file=sys.stderr)`, then returning a small value. Assert the exact stdout and stderr texts.
- **Test 6 — `test_full_stdout_still_fails`.** Linux only: stdout redirected to `/dev/full`. The command fails with a non-zero exit status and a non-empty stderr, for `json` and `json-compact`.

### 5.4 Order of work

1. Write tests 1 to 6 first and run them against the current code. Tests 1 to 4 must fail before the change; report which ones fail and how. Test 5 must pass before and after.
2. Implement section 5.2.
3. Check that the tests can fail. Apply each mutation in turn, confirm that at least one new test fails, then revert it: the helper no longer maps `BrokenPipe` to success; a `BrokenPipe` while streaming is returned as an error; the print callback returns an error on a closed stdout. List the failing tests in the report.
4. Build the release binary and report the output of this command, run under `bash`, for the default format, `--output-format yaml` and `--output-format ndjson`:

   ```bash
   target/release/fimod s --sandbox-file= -i tmp/perf-phase1/users100k.json -m tmp/perf-phase1/identity.py | head -1; echo "exit=${PIPESTATUS[0]}"
   ```

### 5.5 Documentation

- `docs/reference/exit-codes.md`: add a short section stating that a closed stdout is not an error and does not change the exit status.
- `notes/DESIGN_NOTES.md`: add a short subsection stating the rule and where it is enforced (the stdout helper, the streaming conversions, the mold print callback).

### 5.6 Acceptance criteria

- [ ] Tests 1 to 6 exist and pass. Tests 1 to 4 were shown to fail before the change. The mutation checks of step 3 are reported.
- [ ] `task lint` and `task test` pass, with no existing test modified.
- [ ] `cargo test --release --test performance -- --ignored --nocapture --test-threads=1` passes.
- [ ] The three commands of step 4 print one line and `exit=0`, with nothing on stderr.
- [ ] The two documentation updates are present.

### 5.7 Files

`src/pipeline.rs`, `src/engine.rs`, `tests/cli.rs`, `tests/cli/closed_stdout.rs`, `docs/reference/exit-codes.md`, `notes/DESIGN_NOTES.md`.

## 6. WP3 — Direct indented JSON output

### 6.1 Background

When a mold chain finishes, `process_single_input` in `src/pipeline.rs` holds the result as a `MontyObject`. For compact JSON, NDJSON, Lines and TXT it serializes that value directly through `convert::MontySerialize`. Indented JSON, which is the default output format, is not on that list: the whole result is first copied into a `serde_json::Value` tree by `monty_to_json`, then serialized by `output_result`.

Separately, the direct serializer (`ObjectSerialize` in `src/convert.rs`) copies every dictionary key into a temporary `String` before writing it.

Direct serialization of a format that was not explicitly requested as one of the four above is guarded by `convert::is_json_native`. The guard keeps values that need normalization (dates, tuples, keys that collide once stringified) and values that must fail (non-finite floats, unsupported types) on the `Value` path, so behaviour stays identical.

Local reference (standard profile, 100,000 records written): converting to `Value` and serializing indented JSON takes about 88 ms. Direct indented serialization with borrowed keys takes about 29 ms, plus about 14 ms for the guard pass.

### 6.2 Required change

1. `src/convert.rs`, `ObjectSerialize`, dictionary arm: when the key node is a string (`as_str()` returns `Some`), pass the borrowed `&str` to `serialize_entry`. Otherwise keep the current `to_string()` fallback.
2. `src/pipeline.rs`, `process_single_input`: make `DataFormat::Json` eligible for direct serialization. Serialize with `serde_json::to_vec_pretty(&convert::MontySerialize(monty))`, append one `\n`, and write through the existing `write_bytes_to`.
3. Indented JSON is direct **only** when `convert::is_json_native(monty)` is true, whether the format was explicit, inferred from the output extension, inherited from the input, or set by the mold. Do not add `"json"` to `previously_direct`.

Constraints:

- Output bytes are identical to today for every value, trailing newline included.
- When serialization fails, an existing target file stays untouched: produce all bytes before writing.
- `--check`, `--debug`, the library API (`run_pipeline`) and the slurp path keep the `Value` path. Change nothing there.
- Leave `is_json_native` as it is.

No visible behaviour changes in this work package. Closed-stdout behaviour was settled by WP2: `write_bytes_to` already ends quietly, and the tests in `tests/cli/closed_stdout.rs` must still pass.

### 6.3 Tests to add

Unit test in `src/convert.rs`:

- **Test 1 — `monty_serialize_borrowed_keys_match_value_path`.** For values containing dictionaries with string keys and with non-string keys (`int`, `bool`, `None`, `float`), assert that `serde_json::to_vec` and `serde_json::to_vec_pretty` give the same bytes for `MontySerialize(&obj)` and for `monty_to_json(obj.clone())`. Exclude keys that collide once stringified: the two paths differ there by design.

CLI tests in `tests/cli/output_file.rs`, following `test_final_output_format_keeps_normalization_and_newlines`: each case asserts an exact expected text and runs twice, without and with `--debug`. `--debug` keeps the `Value` path, so the pair proves both paths agree.

- **Test 2 — `test_default_json_output_is_identical_on_direct_and_value_paths`.** Results to cover: a list of records, nested containers, an empty list and an empty dict, non-ASCII text, a string with a quote and a newline, `0.5`, `1.0`, `-0.0`, `18446744073709551615`, `True`, `False`, `None`. Destinations to cover: stdout with no format flag, `--output-format json`, `-o out.json` (compare the file content), and `set_output_format('json')` called by the mold.
- **Test 3 — `test_default_json_output_keeps_value_path_for_non_native_results`.** A tuple, `date(2026, 9, 9)`, `{1: 'first', '1': 'last'}` and `18446744073709551616`, each with its exact expected indented output.
- **Test 4 — `test_default_json_output_errors_leave_target_untouched`.** `float('nan')` with `-o out.json` pointing at a file containing `keep me`: the command fails, stderr contains `Cannot represent float`, and the file is unchanged.

Performance smoke test in `tests/performance.rs`:

- **Test 5 — `cli_default_indented_json_output_under_budget`.** Modelled on `cli_final_ndjson_output_under_budget`: 20,000 records through an identity mold file, default format, once to stdout and once to `-o out.json`. Assert that the output parses and holds 20,000 items. Budget: 300 ms in release, 3,000 ms in debug.

### 6.4 Order of work and measurement

1. Save the release binary built from the tree as it stands after WP2 as `tmp/perf-phase1/fimod-before-wp3`. `tmp/perf-phase1/identity.py` (an identity mold) already exists.
2. Add tests 1 to 5. Tests 1 to 4 must pass against the current code too.
3. Implement section 6.2.
4. Compare the two binaries with `hyperfine` and `/usr/bin/time` as in section 4.4, on two commands with the default output format: the identity mold (`-m tmp/perf-phase1/identity.py`) and the filter expression.
5. Check that the tests can fail. Apply each mutation in turn, confirm that at least one new test fails, then revert it: remove the `is_json_native` condition for indented JSON; omit the trailing newline; write an empty key in the borrowed-key branch. List the failing tests in the report.

### 6.5 Documentation

- `notes/ARCHITECTURE.md`, key invariant "CLI serialization follows the final format and destination": add indented JSON to the formats that can serialize `MontyObject` directly.
- `notes/DESIGN_NOTES.md`, "Native processing of JSON-compatible results": state that indented JSON joins the direct serializers and always requires a JSON-native value.
- `notes/perf-baseline.md`: add a dated section with the timings and peak memory of step 4, the machine and the commands.

### 6.6 Acceptance criteria

- [ ] Tests 1 to 5 exist and pass. The mutation checks of step 5 are reported.
- [ ] `task lint` and `task test` pass, with no existing test modified.
- [ ] `cargo test --release --test performance -- --ignored --nocapture --test-threads=1` passes.
- [ ] The identity-mold command with default output is at least 8 % faster than before, same machine, runs back to back.
- [ ] Peak memory (`%M`) of that command is not higher than before.
- [ ] The closed-stdout tests of WP2 still pass.
- [ ] The three documentation updates are present.

If the speed or memory criterion is not met, stop and report the numbers.

### 6.7 Files

`src/convert.rs`, `src/pipeline.rs`, `tests/cli/output_file.rs`, `tests/performance.rs`, `notes/ARCHITECTURE.md`, `notes/DESIGN_NOTES.md`, `notes/perf-baseline.md`.

## 7. WP4 — Standard profile at `opt-level = "s"`

### 7.1 Background

`[profile.release]` in `Cargo.toml` uses `opt-level = "z"`. The maintainer decided to move the standard build to `"s"`: a little larger, noticeably faster. Users who need maximum speed are pointed to the fast build (`release-fast`, `opt-level = 3`), which does not change.

Local reference (native build, one core, before the other work packages): `"s"` is 17 to 25 % faster than `"z"` on filter, CSV and sort workloads; the binary goes from 11.8 MB to 13.3 MB raw and from 3.8 MB to 4.2 MB after `upx --best --lzma`.

### 7.2 Required change

1. Save the release binary built from the tree as it stands after WP3 as `tmp/perf-phase1/fimod-z`.
2. `Cargo.toml`: set `opt-level = "s"` in `[profile.release]` and update the trailing comment. Leave `[profile.release-fast]` alone.
3. Measure the size of the distributed standard binary, which is a musl build packed with UPX:

   ```bash
   CARGO_ZIGBUILD_ZIG_PATH="$(mise which zig)" mise exec -- cargo zigbuild --release --target x86_64-unknown-linux-musl
   cp target/x86_64-unknown-linux-musl/release/fimod tmp/perf-phase1/fimod-musl-s
   "$(mise which upx)" --best --lzma tmp/perf-phase1/fimod-musl-s
   stat -c %s tmp/perf-phase1/fimod-musl-s
   ```

4. Replace the "~3.3 MB" size claim with the measured value, rounded to one decimal, in `README.md` (line 20), `docs/index.md` (lines 12 and 47) and `docs/guides/comparison.md` (line 59).
5. `notes/VISION.md` (line 51): the accepted trade-off cites `opt-level = "z"`; change it to `"s"`. Change nothing else in that file.
6. Compare `tmp/perf-phase1/fimod-z`, `target/release/fimod` and `target/release-fast/fimod-fast` on the filter command of section 4.4. `README.md` (line 62) and `docs/guides/comparison.md` (line 60) say the fast build is "15-25% faster" than the standard one. Keep the wording if the measured gap falls between 10 and 30 %; otherwise replace the range with the measured one.
7. `notes/perf-baseline.md`: add a dated section with raw and UPX sizes for `"z"` and `"s"`, the three-way timings of step 6, and the commands. Leave existing sections, which describe past states, untouched.

If the musl build cannot be produced locally, do not guess a size. Leave the size claims as they are and flag them as unresolved in the report.

### 7.3 Acceptance criteria

- [ ] `Cargo.toml` has `opt-level = "s"` for `release` and `3` for `release-fast`.
- [ ] `task lint` and `task test` pass.
- [ ] The performance smoke tests pass in both profiles: `task test:performance` and `task test:performance:fast`.
- [ ] Every size claim listed in step 4 matches the measured value, or is flagged as unresolved.
- [ ] `notes/VISION.md` and `notes/perf-baseline.md` are updated as specified.
- [ ] No other file changed.

### 7.4 Files

`Cargo.toml`, `README.md`, `docs/index.md`, `docs/guides/comparison.md`, `notes/VISION.md`, `notes/perf-baseline.md`.

## 8. WP5 — A closed stdout is a normal end for the other commands

### 8.1 Background

WP2 made `fimod shape` end quietly when the reader of stdout goes away. Every other command still writes with `println!` or `print!`, which panic on a closed stdout. Observed with the reader closed before the command writes:

| Command | Today |
| --- | --- |
| `fimod mold list`, `fimod registry list`, `fimod registry cache info`, `fimod setup sandbox show` | panic, exit status 134 |
| `fimod setup completions --shell bash` | `Error: failed to write completion script`, exit status 1 |
| `fimod monty repl` (`print()` goes through Monty's `PrintWriter::Stdout`) | panic |

Where the writes are:

- Library crate: `src/registry/molds.rs` (47 `println!`), `src/registry/config.rs` (18, plus an interactive prompt written through a locked stdout), `src/registry/catalog.rs` (10), `src/test_runner.rs` (7).
- Binary crate: `src/cmd/setup.rs` (24), `src/cmd/monty.rs` (one `println!` for REPL results and three `PrintWriter::Stdout`), `src/cmd/completions.rs` (`write_registration` to `io::stdout()`).

The rule is the one of WP2: a closed stdout is a normal end. The command stops writing to stdout, prints nothing about it, carries on, and exits with the status it would have had. Any other failure to write to stdout is an error.

### 8.2 Required change

1. Add a library module `src/stdout.rs`, declared in `src/lib.rs`, as the single home of the rule for command output:
   - a function that writes formatted text to stdout. On `BrokenPipe` it remembers that stdout is closed and skips later writes. On any other error it remembers the first one and returns normally.
   - two macros mirroring `print!` and `println!`, usable from both the library and the binary crate.
   - a function that hands the remembered write error to `main`.
2. Replace every `println!` and `print!` in the files listed in 8.1 with those macros. Change nothing else at the call sites: no signature changes, no reordering.
3. `src/main.rs`: when a command would exit with status 0 and a non-`BrokenPipe` stdout write error was remembered, fail with the message `Failed to write output to stdout` and its cause. A command that already fails keeps its own failure.
4. `src/cmd/completions.rs`: a `BrokenPipe` while writing the completion script is a normal end; other errors stay as they are.
5. `src/registry/config.rs`, interactive prompt: writing the prompt to a closed stdout must neither panic nor fail the command. Behaviour with an open stdout is unchanged.
6. `src/cmd/monty.rs`: print REPL results through the macro, and replace the three `PrintWriter::Stdout` with a small callback defined in that file, which sends stdout text through the new module. Keep `print(..., file=sys.stderr)` on stderr: the `PrintWriterCallback` trait routes stderr to stdout by default, so override `stderr_write` and `stderr_push`, as `MoldPrint` does in `src/engine.rs`.

Constraints:

- When stdout stays open, the bytes written to stdout and stderr and the exit status are identical to today for every command.
- A closed stdout never changes an exit status: `fimod mold test` still exits 1 when a case fails.
- `eprintln!` sites are untouched. `src/pipeline.rs` and `src/engine.rs` are untouched: WP2's `write_stdout`, `ClosedStdoutTracker` and `MoldPrint` stay as they are.
- The remembered state is process-wide. Keep it inside `src/stdout.rs`, as small as possible, and say why in a comment.
- No `process::exit` in library code, no signal handling, no `unsafe`, no new dependency.

### 8.3 Tests to add

In `tests/cli/closed_stdout.rs`, reusing its helpers. Close the reader before spawning (`std::io::pipe()`, as the existing streaming regression test does), because these commands write little. Isolate the configuration the way `tests/cli/registry.rs` does (`HOME` set to a temporary directory) and register a local registry holding at least one mold.

- **Test 1 — `test_closed_stdout_is_silent_success_for_listing_commands`.** `mold list`, `registry list`, `registry cache info`, `setup sandbox show`, `setup completions --shell bash`: exit status 0, empty stderr.
- **Test 2 — `test_closed_stdout_keeps_mold_test_exit_status`.** `fimod mold test` on a mold with one passing case: exit status 0. With one failing case: exit status 1. No panic text on stderr in either run.
- **Test 3 — `test_closed_stdout_is_silent_success_for_repl`.** stdin `print("a")` then `1+1`: with stdout open the output is exactly `a` and `2` on two lines; with the reader closed, exit status 0 and no panic text on stderr.
- **Test 4 — `test_full_stdout_fails_listing_commands`.** Linux only: `mold list` and `registry list` with stdout on `/dev/full` exit non-zero with `Failed to write output to stdout` on stderr.

### 8.4 Order of work

1. Build the release binary from the tree as it stands and save it as `tmp/perf-phase1/fimod-before-wp5`.
2. Write tests 1 to 4 and run them against the current code. Report which fail and how.
3. Implement section 8.2.
4. With stdout open and `HOME` isolated, run each command of test 1, `mold show` on a registered mold, `mold test`, and the REPL input of test 3, on both binaries. Stdout, stderr and exit status must be identical. Report the comparison.
5. Check that the tests can fail. Apply each mutation in turn, confirm that at least one new test fails, then revert it: the write function panics on `BrokenPipe`; `main` ignores the remembered error; the REPL callback returns an error on a closed stdout.
6. On a fresh release build, report the output of these two commands under `bash`:

   ```bash
   target/release/fimod mold list | head -1; echo "exit=${PIPESTATUS[0]}"
   target/release/fimod registry list | head -1; echo "exit=${PIPESTATUS[0]}"
   ```

### 8.5 Documentation

- `docs/reference/exit-codes.md`, "Closed stdout": say that the rule applies to every command.
- `notes/DESIGN_NOTES.md`, "Closed stdout is a normal end": add the command-output module and its remembered error.
- `notes/CODE_LAYOUT.md`: add `src/stdout.rs`.

### 8.6 Acceptance criteria

- [ ] Tests 1 to 4 exist and pass, and were shown to fail before the change where the behaviour changed. The mutation checks of step 5 are reported.
- [ ] `task lint` and `task test` pass, with no existing test modified.
- [ ] The open-stdout comparison of step 4 shows no difference.
- [ ] The two commands of step 6 print one line and `exit=0`, with nothing on stderr.
- [ ] No `println!` or `print!` remains in the files listed in 8.1.
- [ ] The three documentation updates are present.

### 8.7 Files

`src/stdout.rs` (new), `src/lib.rs`, `src/main.rs`, `src/registry/molds.rs`, `src/registry/config.rs`, `src/registry/catalog.rs`, `src/test_runner.rs`, `src/cmd/setup.rs`, `src/cmd/monty.rs`, `src/cmd/completions.rs`, `tests/cli/closed_stdout.rs`, `docs/reference/exit-codes.md`, `notes/DESIGN_NOTES.md`, `notes/CODE_LAYOUT.md`.

## 9. Review checklist

The review agent works read-only: it may build and run tests, but does not edit files.

- **Scope.** `git status` and `git diff` show changes only in the files listed for the work packages done so far. Nothing from section 11 slipped in.
- **Correctness (WP1).** Children are pushed before their container. Capacity is exact. Strings are moved in the owned variant. The number mapping is unchanged. No node is shared.
- **Correctness (WP2).** Only `BrokenPipe` on stdout is treated as a normal end; every other write failure still fails. The exit status requested by `set_exit()` survives. Streaming stops reading its input. Mold `print()` output is byte-identical when stdout stays open. No `process::exit` in library code.
- **Correctness (WP5).** Every `println!` and `print!` of the listed files goes through the new module. A closed stdout never changes the exit status (`fimod mold test` still exits 1 when a case fails). Any other stdout write failure makes a command that would have succeeded fail with a message. Output is byte-identical when stdout stays open. WP2's code in `src/pipeline.rs` and `src/engine.rs` is untouched.
- **Correctness (WP3).** Indented JSON is direct only behind `is_json_native`. `previously_direct` is unchanged. All bytes exist before anything is written. Non-string keys still go through `to_string()`.
- **Test strength.** For each new test, name the defect it would catch. Check the reported mutation results against the test code. A test that would still pass with the key invariant broken is a finding.
- **Independent verification.** Rerun `task lint`, `task test` and the performance tests. Rerun the timing comparison if the reported gain is within 10 points of the threshold.
- **Documentation.** Every changed number is backed by a measurement in the report. Historical sections of `notes/perf-baseline.md` are untouched.
- **Rules.** No commit was made, no dependency added, no AI attribution, no existing test modified.

## 10. Report formats

Dev report:

1. Files changed, one line each.
2. Commands run, with pass or fail and the relevant output lines.
3. Measurements: before, after, spread, machine conditions.
4. Mutation checks (WP1, WP2, WP3, WP5): mutation, tests that failed.
5. Deviations from this spec and open questions.

Review report:

1. Verdict: `APPROVE` or `CHANGES REQUESTED`.
2. Findings, most severe first: file and line, what is wrong, what input or situation shows it, required fix.
3. What was verified independently, and what was not.

## 11. Out of scope

Do not implement any of these, even if they look adjacent:

- Reading JSON, NDJSON or CSV text straight into the arena without `serde_json::Value`.
- Sharing key nodes between objects.
- Rewriting `csv_to_monty`.
- Direct serialization for YAML, TOML or CSV output, for the slurp path or for the library API.
- Making `is_json_native` cheaper, or folding it into the serializer.
- Stream mode, batches, parallelism, allocator accounting.
- The defects listed in `notes/eval_112026.md` section 3.
- The fast build profile, UPX settings, release workflows.
