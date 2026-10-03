## [0.12.0] — YYYY-MM-DD

### Highlights

- 🐍 **Powered by Monty 1.0.0** — Fimod adopts the first 1.0 release of its embedded Python runtime. More Python in your molds, still no Python installation.
- ✨ **More Python tools** — copy nested data with `copy.deepcopy`, use explicitly seeded `random` generators, merge dicts with `a | b`, and format values with `format()` or `%`.
- 🧰 **Python-first molds** — shipped molds now use `import re` and Python deduplication recipes. Legacy regex, deduplication, and flattening helpers require explicit compatibility activation.
- 🛡️ **Host-controlled capabilities** — new clock, entropy, and sleep requests stay under Fimod's sandbox policy in both molds and the REPL.

### Breaking Changes & Migration

- **built-ins:** `re_*` (including every `_fancy` variant), `it_unique`, `it_unique_by`, and `it_flatten` are deprecated and disabled by default. Calling one raises an error naming its replacement and the compatibility setting. Existing molds can retain their legacy behavior with:

    ```bash
    FIMOD_LEGACY_BUILTINS=1 fimod s -i data.json -m old_mold.py
    ```

    Only the exact value `1` enables compatibility. Activation is silent and does not require `--env`; no removal version is scheduled yet.

    | Legacy helper                       | Recommended migration                                                                |
    | ----------------------------------- | ------------------------------------------------------------------------------------ |
    | `re_*`, including `_fancy` variants | `import re`, using Match methods and Python replacement syntax                       |
    | `it_unique`, `it_unique_by`         | Python recipes that preserve the required ordering and equality rules                |
    | `it_flatten`                        | A recursive Python function; `itertools.chain.from_iterable` only flattens one level |

    Regex results and replacement syntax differ: use `match.group()` instead of `match["match"]`, and `r"\g<1>"` instead of legacy fancy-regex `$1` references. Native match offsets count characters rather than UTF-8 bytes. Monty 1.0.0's `re.split` currently omits captured separators, and `FIMOD_REGEX_BACKTRACK_LIMIT` applies only to legacy helpers. See the [migration guide](https://pytgaen.github.io/fimod/reference/built-ins/#legacy-built-ins) for recipes, equality differences, and native regex limitations.

    For example, an existing `re_search` call returning a named capture becomes:

    ```python
    import re

    def transform(data, **_):
        match = re.search(r"(?P<user>\w+)@(?P<domain>\w+)", data["email"])
        return match.group("user") if match else None
    ```

- **source builds:** the minimum supported Rust version moves from 1.95 to 1.96. Prebuilt binaries still require no Rust or Python installation.

### Features

- **monty:** upgrade `monty` and `monty-types` from 0.0.23 to exactly 1.0.0, bringing the embedded runtime's first 1.0 release to every mold and interactive session.
- **python:** support `format(value, spec)`, string/bytes `%` formatting, dict union, and `eval`, `exec`, and `locals` within Monty's runtime and sandbox.
- **stdlib:** expose `copy` / `deepcopy`, reproducible random generators with an explicit seed, and `time` clock reads when `allow_clock = true`.
- **diagnostics:** support `print(..., file=sys.stderr)` in molds. Debug mode routes both Python print streams to stderr so data output remains on stdout.

### Bug Fixes

- **gatekeeper:** use Monty's native Python truthiness for `gk_assert` and `gk_warn`, including empty dictionaries and other empty containers.
- **datetime:** respect explicit fixed-offset timezones passed to `datetime.now(tz)` when clock access is permitted.

### Refactoring

- **engine:** migrate mold execution, host callbacks, helpers, and serializers to Monty's native object graph and borrowed views, retaining Pipeline/Step host identities and direct serialization for compatible output values.
- **sandbox:** explicitly route Monty's clock, initial entropy, and sleep requests through Fimod's host policy. Clock permission does not enable entropy or sleeping; unseeded randomness, `os.urandom`, `time.sleep`, and `asyncio.sleep` remain denied. Time, memory, and host-call limits continue to apply.
- **molds:** migrate `@dedup_by`, `@log_parse`, `@markdown_toc`, and `@split_tags` off legacy helpers. Preserve JSON-shaped deduplication keys, absent log capture groups, and captured tag separators.

### Documentation

- **mold scripting:** make native Python regex and deduplication the default in guides, examples, and the cookbook; document explicit legacy activation and the behavioral differences to check when migrating existing molds.
- **monty engine:** describe Monty 1.0.0's language and stdlib capabilities, seeded randomness, clock permissions, stderr handling, and resource budgets.

### Housekeeping

- **build:** align the MSRV CI job and local MSRV check with Rust 1.96.
- **deps:** refresh Monty's dependency graph, including Ruff 0.0.14 and rustls 0.23.45; allow the BlueOak-1.0.0 license used by the new CBOR dependencies.
