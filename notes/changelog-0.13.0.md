## [0.13.0] — YYYY-MM-DD

### Highlights

- 🐍 **Powered by Monty 1.5.0** — `match` / `case` statements, richer dataclasses, slice assignment and complex numbers land in molds, still without a Python installation.
- 🧰 **More standard library** — `hashlib`, `csv`, `tomllib`, `decimal`, `bisect`, `fnmatch`, `os.path` and `annotationlib` are now importable from molds and the REPL.
- 📝 **Jinja2-faithful templates** — minijinja 3 renders templates the way Jinja2 does; check the template notes below if your molds print lists or dicts directly.
- 🛡️ **Security refresh** — rustls and webpki-root-certs advisories fixed through a compatible dependency update.

### Features

- **monty:** upgrade `monty` and `monty-types` from 1.0.0 to exactly 1.5.0. Molds gain `match` / `case` with literal, sequence, mapping, class and capture patterns; dataclass `field()`, `default_factory` and `__post_init__`; slice assignment; subset/superset ordering on sets; and the `complex` type with `1j` literals.
- **stdlib:** expose `hashlib` digests, `csv.reader` / `csv.writer`, `tomllib.loads`, `Decimal` arithmetic, `bisect`, `fnmatch`, pure `os.path` manipulation and `annotationlib`. `tomllib.load` and `os.path` filesystem queries stay under the sandbox policy. `Decimal` results must be converted with `str()` or `float()` before returning, because they are not serialized to JSON yet.
- **regex:** native `re.split` now includes captured separators, matching the legacy `re_split` helper; the `re.finditer` workaround is no longer needed.
- **templates:** upgrade minijinja from 2 to 3, aligning rendering with Jinja2. A list or dict printed directly renders like Python (`{{ ["x", 1] }}` → `['x', 1]`), `tojson` uses spaced separators (`{"a": 1}`), `round` rounds half to even (`{{ 2.5 | round }}` → `2.0`), and division by zero is a render error that fails the mold. Use `{{ value | tojson }}` wherever the output must be JSON.

### Refactoring

- **sandbox:** route the new import, type-hint and directory-scan host requests through Fimod's policy. Local `.py` imports, which Monty 1.5.0 now forwards to the host, are explicitly left unserved: `import helpers` keeps raising `ModuleNotFoundError` in molds and the REPL, even when `helpers.py` sits next to the mold. A regression test guards this boundary.

### Documentation

- **monty engine:** document Monty 1.5.0's language features, the new stdlib modules with their sandbox limits, and the local-import refusal.
- **mold scripting:** describe Jinja2 rendering semantics for template molds and when to reach for `tojson`.
- **built-ins:** update the legacy regex migration guide now that native `re.split` keeps captured separators.

### Housekeeping

- **deps:** refresh compatible dependencies, including rustls fixes for RUSTSEC-2026-0346, RUSTSEC-2026-0347 and RUSTSEC-2026-0348, the webpki-root-certs advisory, and a replacement for the yanked clipboard-win release.
- **deps:** upgrade fancy-regex to 0.20, rustyline to 19, serde-saphyr to 2.1, and httpmock to 0.9 for tests.
