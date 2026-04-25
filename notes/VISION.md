# Fimod — Vision

The editorial compass. What fimod is, what it refuses to become, and the non-negotiables that should outlast any single feature decision.

**When in doubt** on roadmap or design, this file wins over any individual note. Conversely, when this file is wrong, it should be edited explicitly — not eroded by incremental drift.

## Thesis

**Python-powered molding without Python installed.**

fimod is a single Rust binary that reads any common structured format (JSON, YAML, TOML, CSV, NDJSON, TXT, HTTP), runs a user-authored Python `transform(data, ...)` on it via an embedded VM (Monty), and writes the result back in any supported format.

One sentence: *CLI ergonomics, Python semantics, Rust trust boundary, zero install.*

## Who it's for

- **DevOps / CI engineers** patching configs, rewriting manifests, extracting fields from API responses inside pipelines that can't afford a Python install.
- **Data plumbers** converting between JSON/YAML/TOML/CSV without learning jq/yq DSLs.
- **Tool authors** shipping reusable transforms (molds) via git-hosted registries their teams can `-m @name`.

## Who it's explicitly NOT for

- People writing full applications. Molds are pure transforms — no classes, no state across invocations, no persistent side effects.
- People who need the full Python ecosystem (pandas, requests, sqlalchemy, …). Monty is a subset runtime by design.
- People running untrusted third-party molds as a service. fimod is a local human-driven CLI, not a multi-tenant sandbox.

## Non-negotiables

These are load-bearing. Changing any of them is a major version decision, not a feature tweak.

1. **Single static binary, no runtime install.** If a feature requires a user to `pip install` or `npm install` anything, it doesn't ship. Monty is the boundary — we don't re-expose CPython.
2. **Rust parses, Rust controls the sandbox.** All format parsing/serialization (serde) stays in Rust — Monty never sees bytes, only `MontyObject`. Every capability the mold reaches for outside the VM (filesystem, env vars, clock, network) goes through `engine.rs` and is gated by an explicit sandbox policy. The **mechanism** is non-negotiable; the **policy** is user-configurable via `sandbox.toml` (deny-all by default pre-0.5.0, selectable capabilities from 0.5.0 onward, `[[mount]]`-based FS access later). URL-hosted molds are safe to run because the host decides what they can touch — not because "Python can't do I/O."
3. **Python-shaped syntax, not a custom DSL.** Molds are written in the Python syntax users already know — not in a bespoke expression language layered on top. The honest caveat: Monty is a *subset* of Python (no classes yet, no `with`, no `match`, no PyPI imports). What fimod guarantees is "if it parses as Python and stays within Monty's subset, it runs" — not "anything you'd write in CPython works."
4. **The pipeline is one-shot and pure.** Read → Parse → Transform → Serialize → Write. No streaming by default, no daemon mode. A mold run is reproducible from its inputs.
5. **A registry is just a file repository.** A local directory or a git repo (GitHub / GitLab / plain HTTP) with an optional `catalog.toml`. Fimod reads; it does not host. No central index à la npm/PyPI, no auth server, no usage telemetry backend — the user owns the distribution surface. Auth for private repos goes through standard tokens (`GITHUB_TOKEN`, `GITLAB_TOKEN`, or `--token-env`), not a fimod account.

## Non-goals

Things that look tempting but we've decided against:

- **`fimod serve --port`** — an HTTP daemon exposing molds. `fimod stream` (stdin/stdout loop) covers the same need without a server crate or thread-safety headaches on the Monty side.
- **Generic `import` support in molds.** Monty is our ceiling; extending it to arbitrary PyPI packages would nullify the trust boundary and the single-binary story.
- **Threat-model-grade sandboxing of hostile code.** Resource limits (sandbox.toml in 0.5.0) are guardrails against runaway molds, not a defense against adversaries. fimod is not Pyodide.
- **Full stdlib parity with CPython.** We cover what Monty covers; gaps are filled by Rust-implemented built-ins (`re_*`, `dp_*`, `it_*`, `hs_*`, `gk_*`, `msg_*`, `tpl_*`, `env_subst`) — not by embedding a CPython interop layer.

## Tradeoffs we accept

- **Monty is young and its API breaks.** We pin a version and document breakage per release. Users get an unstable runtime; in exchange they get a Python subset in a compact binary with near-instant startup.
- **No classes, no `with`, no `match` (yet).** Monty's Python subset is evolving. We don't work around it — we wait.
- **Deny-by-default capabilities, limits-by-default resources.** Pre-0.5.0, every `OsCall` is denied (`None`) — keeps the security story simple at the cost of ergonomic paper cuts (`datetime.now() → None` silently). From 0.5.0, the sandbox splits the two concerns: capabilities (clock, env, FS) stay deny-by-default and require explicit opt-in via `sandbox.toml`; resource limits (CPU, memory) get reasonable hard-coded defaults (`2m` / `1GB`) that protect the host even without a policy file. See `notes/monty-default-sandboxing.md`.
- **Binary size > runtime perf for some dependencies.** We pick UPX-compressed builds, mimalloc, `opt-level = "z"`. fimod is a CLI, not a hot-loop service.
- **`serde_json::Value` as IR is a convenience, not a commitment.** Today every format round-trips through `serde_json::Value` because it's cheap and serde-compatible. The cost is real — comments in TOML/YAML are lost, CSV loses columnar locality, position information is flattened. A richer IR (per-format or enriched JSON with trivia) is on the table if the pain justifies the rewrite. The invariant is "there is *an* IR that decouples parsers from molds" — not "it must be `serde_json::Value` forever."

## The editorial compass

When evaluating a new feature or PR, ask in order:

1. **Does it preserve the thesis?** Python semantics, Rust trust boundary, single binary, zero install.
2. **Does it align with a non-negotiable?** If it weakens one, it needs an explicit decision logged here.
3. **Does it fit an existing user archetype?** Or is it a "wouldn't it be cool" without a named beneficiary?
4. **Is the UX shape clearer than its implementation?** A feature whose terminal example is awkward is a feature that doesn't belong — implementation difficulty is negotiable, UX isn't.
5. **Is it testable end-to-end via `tests/cli/*` or `tests-molds/`?** If the feature can't be fixtured, it probably has hidden coupling.
