## [0.12.1] — YYYY-MM-DD

### Bug Fixes

- **version:** strip Cargo's exact-version marker from the embedded Monty version so `--version` and the REPL banner display `1.0.0` instead of `=1.0.0`. All build variants now use the shorter `fimod X.Y.Z standard|slim|fast (Monty 1.0.0)` version line.

### Housekeeping

- **deps:** refresh compatible transitive dependencies in the lockfile, including aws-lc-rs 1.18.1, hyper 1.11.1, h2 0.4.19, quinn 0.11.12, and encoding_rs 0.8.42. Direct dependencies are already current; the embedded Monty runtime remains pinned to 1.0.0.
