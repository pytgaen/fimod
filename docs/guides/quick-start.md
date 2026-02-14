# 🚀 Quick Start

## 📦 Install

=== ":material-download: curl | sh (recommended)"

    ```bash
    curl -fsSL https://raw.githubusercontent.com/pytgaen/fimod/main/install.sh | sh
    ```

    Downloads the latest pre-built binary for your platform (Linux x86_64/aarch64, macOS ARM).

    **Options** (environment variables):

    | Variable | Default | Description |
    |---|---|---|
    | `FIMOD_VARIANT` | *(full)* | `slim` to exclude HTTP input and remote mold loading |
    | `FIMOD_INSTALL` | `/usr/local/bin` | Install directory (falls back to `~/.local/bin` if not writable) |
    | `FIMOD_VERSION` | latest | Pin a specific version (e.g. `v0.2.1`) |

    ```bash
    # Install the slim variant to a custom directory
    FIMOD_VARIANT=slim FIMOD_INSTALL=~/.local/bin curl -fsSL https://raw.githubusercontent.com/pytgaen/fimod/main/install.sh | sh
    ```

=== ":material-package-down: cargo install"

    ```bash
    cargo install --git https://github.com/pytgaen/fimod
    ```

=== ":material-source-branch: Build from source"

    ```bash
    git clone https://github.com/pytgaen/fimod && cd fimod
    cargo build --release
    # binary at target/release/fimod
    ```

!!! tip "Check your install"
    ```bash
    fimod --version
    ```

---

## 🎯 First Try (Hello World)

Let's test fimod with a simple inline expression. Create a sample JSON file or pipe it in:

```bash
echo '[{"name": "Alice"}, {"name": "Bob"}]' | fimod s -e 'len(data)'
# Output: 2
```

---

## �️ Next steps

<div class="grid cards" markdown>

-   :material-compass-outline: [**Quick Tour**](quick-tour.md) — 5-minute showcase of features

-   :material-lightbulb-on-outline: [**Concepts**](concepts.md) — pipeline, Monty, security model

-   :material-language-python: [**Mold Scripting**](mold-scripting.md) — built-in helpers

-   :material-console-line: [**CLI Reference**](cli-reference.md) — all options and flags

-   :material-chef-hat: [**Cookbook**](../cookbook.md) — recipes for common tasks

</div>
