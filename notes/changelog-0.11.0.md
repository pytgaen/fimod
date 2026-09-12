## [0.11.0] — YYYY-MM-DD

### Highlights

- 🐍 **More Python tools in molds** — Monty v0.0.23 adds `functools`, binary/text encodings, more `itertools` adaptors, `str.format`, and `datetime.time`.
- 🛡️ **Optional host-call quota** — limit how often a mold or REPL snippet hands execution back to fimod with `sandbox.max_suspensions`, disabled by default.

### Features

- **monty:** upgrade `monty` and `monty-types` to v0.0.23 and migrate Pipeline/Step method routing to host-owned objects identified by UUID.
- **formats:** serialize `datetime.time` results as ISO time strings through both output conversion paths, preserving microseconds and UTC offsets.
- **sandbox:** add an integer `max_suspensions` quota per mold or REPL snippet, configurable through `setup sandbox get/set/show`. Missing or zero leaves the quota disabled; exceeding a positive quota stops execution before servicing the next host request, without a catchable Python exception.

### Documentation

- **monty engine:** synchronize the runtime version, Python capabilities and stdlib tables; document the host suspension quota and time serialization.

### Housekeeping

- **deps:** remove the obsolete get-size2 compatibility pin and its Dependabot/outdated exclusions now that the upgraded Ruff uses compact_str 0.10.
- **deps:** refresh fancy-regex to 0.19.1, indexmap to 2.14.2, reqwest to 0.13.5, serde-saphyr to 1.2.0, and toml to 1.1.5 in the lockfile.
