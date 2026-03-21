# 🚀 Quick Start

## 📦 Install

=== ":material-linux: Linux / macOS"

    ```bash
    curl -fsSL https://raw.githubusercontent.com/pytgaen/fimod/main/install.sh | sh
    ```

    Downloads the latest pre-built binary for your platform (Linux x86_64/aarch64, macOS ARM).
    The script installs the binary then prompts you to run `fimod registry setup` to configure the official mold catalog.

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

=== ":material-microsoft-windows: Windows — PowerShell script"

    The pipe-to-execute pattern triggers antivirus false positives. Download first, then run:

    ```powershell
    Invoke-RestMethod https://raw.githubusercontent.com/pytgaen/fimod/main/install.ps1 -OutFile "$env:TEMP\fimod-install.ps1"
    & "$env:TEMP\fimod-install.ps1"
    ```

    Same env var options: `$env:FIMOD_VARIANT` · `$env:FIMOD_INSTALL` · `$env:FIMOD_VERSION`

    !!! tip "PATH configuration"
        The script checks whether the install directory is in your PATH. If not, it displays the commands to add it — copy and run them to make `fimod` available in new terminals.

=== ":material-microsoft-windows: Windows — via ubi (antivirus-friendly)"

    [ubi](https://github.com/houseabsolute/ubi) is a universal binary installer available on winget (pre-installed on Windows 10/11):

    ```powershell
    # 1. Install ubi (one-time, uses winget which is built into Windows)
    winget install houseabsolute.ubi

    # 2. Install fimod
    ubi --project pytgaen/fimod --in "$env:USERPROFILE\.local\bin"

    # 3. Add to PATH (if not already present)
    $BinDir = "$env:USERPROFILE\.local\bin"
    $UserPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
    if ($UserPath -notlike "*$BinDir*") {
        [Environment]::SetEnvironmentVariable('PATH', "$BinDir;$UserPath", 'User')
        $env:PATH = "$BinDir;$env:PATH"
    }

    # 4. Set up the official mold catalog
    fimod registry setup
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
