# ❤️ Acknowledgements

`fimod` is built on the shoulders of giants. We want to express our deep gratitude to the incredible open-source projects that make this tool possible.

## [Monty](https://github.com/pydantic/monty)
The core magic of `fimod` — executing Python without a Python installation — is driven by Monty (from the Pydantic team). It provides the embedded Python runtime in Rust that allows us to run your mold scripts safely and efficiently.

## [reqwest](https://crates.io/crates/reqwest)
Our robust HTTP client capabilities are powered by `reqwest`. This exceptional crate allows `fimod` to seamlessly fetch data from URLs, support HTTPS out-of-the-box, automatically handle system proxies, and manage connection pooling.

## [MiniJinja](https://github.com/mitsuhiko/minijinja)
Our Jinja2 templating engine (`tpl_render_str`, `tpl_render_from_mold`) is powered by MiniJinja, created by [Armin Ronacher](https://github.com/mitsuhiko) (the author of Jinja2 and Flask). A pure-Rust implementation that works natively with `serde_json::Value`, bringing full Jinja2 syntax — filters, loops, macros, template inheritance — without any Python dependency.

## [fancy-regex](https://crates.io/crates/fancy-regex)
The advanced regular expression features in our `re_*` built-in functions are made possible by `fancy-regex`. It brings powerful PCRE-like functionality to Rust, such as lookarounds and backreferences, unlocking complex text processing.

## Other Open Source Bricks
We also rely on several other phenomenal crates from the Rust ecosystem:

- **[Serde](https://serde.rs/) & ecosystem** (`serde_json`, `serde-saphyr`, `toml`, `csv`): For flawless and incredibly fast data parsing and serialization.
- **[Clap](https://docs.rs/clap/)**: For crafting our powerful, documented, and predictable command-line interface.
- **[Anyhow](https://docs.rs/anyhow/)**: For flexible and context-rich error handling.

A huge thank you to all the maintainers and contributors of these projects. Your dedication and hard work empower the community to build tools like `fimod`.
