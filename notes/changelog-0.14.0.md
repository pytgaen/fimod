## [0.14.0] — YYYY-MM-DD

### Highlights

- 🐍 **Powered by Monty 1.6.0** — the embedded Python runtime moves to Monty 1.6.0, with its CPython parity fixes and faster internals; molds keep working unchanged.
- ⚡ **Faster on large inputs** — values reach the mold through a single pre-sized arena, indented JSON is written straight from the Monty value, and the standard build is compiled for speed rather than minimal size; each change saves 10–24 % on 100,000 records, and peak memory drops by about a quarter.
- 💾 **Safe output files** — `-o` and `--in-place` write a temporary file next to the target and rename it, so an interrupted run never leaves a truncated file behind.
- 🔌 **Pipes that close early** — `fimod … | head -1` ends quietly with exit status `0` in every command.

### Features

- **monty:** upgrade `monty` and `monty-types` from 1.5.0 to exactly 1.6.0. The release is mostly internal (performance fast paths and CPython parity fixes); the mold language and the standard library modules available in 0.13.0 are unchanged.
- **templates:** minijinja 3.1.0 brings template rendering closer to Jinja2: booleans compare and sort like `0` / `1`, `max` and `groupby` return the first of equal items, `first`, `last`, `length`, `join` and related filters treat `none` and undefined values like Jinja2, `reverse` and `last` work on mappings, case filters handle special cases such as the final sigma, and `replace` accepts a `count` argument.

### Bug Fixes

- **output:** `-o` and `--in-place` write to a temporary sibling file, then rename it over the target, so a crash, a timeout or a full disk never leaves a truncated file. Writing through a symlink updates the file it points to and keeps the link; the target keeps its permissions; special files such as `/dev/null` are still written in place.
- **cli:** a stdout closed by its reader (`fimod s -i data.json -e 'data' | head -1`) is a normal end in every command, including `mold list`, `mold test`, `registry list`, `setup …` and `monty repl`. The exit status stays `0`, or the code set by `set_exit()`; `fimod mold test` still exits `1` when a case fails.
- **cli:** other stdout write failures, such as a full disk, report `Failed to write output to stdout` on stderr and exit with an error instead of panicking.
- **yaml:** parser size limits now scale with the input, so YAML documents above 250,000 nodes parse, while alias expansion bombs are still rejected.
- **toml:** TOML dates, times and datetimes reach the mold as RFC 3339 strings (`2024-01-02T03:04:05Z`, `2024-01-02`, `03:04:05`) instead of leaking the `toml` crate's internal wrapper object.
- **errors:** Python tracebacks name the mold as you typed it (or `<expr>` for `-e`) and no longer show the generated `transform()` call; syntax errors point at the line you wrote.

### Performance

- **convert:** input values are built into one Monty arena sized in advance instead of one arena per value merged level by level. On a 100,000-record filter, the whole command goes from about 270 ms to 225–245 ms, and peak memory drops by about 24 %.
- **output:** indented JSON, the default output format, is serialized directly from the Monty value, as compact JSON already was, with byte-identical output. An identity mold on 100,000 records runs 14–22 % faster.
- **build:** the standard binary is now compiled with `opt-level = "s"` instead of `"z"`, about 21–24 % faster on a 100,000-record filter. The UPX-compressed Linux download grows from about 3.8 MB to about 4.2 MB; `fimod-fast` keeps its 15–25 % lead.

### Refactoring

- **sandbox:** route Monty 1.6.0's new exclusive file creation request through the sandbox policy. Like `open()`, it raises `PermissionError`; the request has no Python API yet and cannot be reached from molds.

### Documentation

- **exit codes:** document how fimod ends when stdout is closed early and how other stdout write failures are reported.
- **formats:** document TOML date and time handling, and the range of integers kept exact on input (64 bits); larger integers become floats, and computed integers beyond that range are written as decimal strings.
- **install:** explain the 0.2–0.4 s startup cost of the UPX-compressed `standard` and `slim` builds on Linux and Windows, with two ways around it: one invocation for many files, or `fimod-fast`.
- **built-ins:** on large lists, recommend native Python (`sorted(..., key=...)`, comprehensions, direct indexing) over helpers and `dp_*` calls in a loop, with measured examples.
- **monty engine:** update the Monty version and recheck the documented `re` limitations on 1.6.0.

### Housekeeping

- **deps:** raise the minimum supported Rust version from 1.96 to 1.98, as required by Monty 1.6.0.
- **deps:** refresh compatible dependencies, including minijinja 3.1.0, fancy-regex 0.20.2 (soundness fixes in its optimizer, used only by the `re_*_fancy` helpers) and notify 8.2.2.
- **release:** compress the `.tar.gz` archives with `pigz -11`, about 4 % smaller for binaries not packed by UPX, still readable by any `tar`.
