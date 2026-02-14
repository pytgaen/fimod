# Mold Gallery

A curated selection of molds from the official registry, showcasing what fimod can do out of the box. All of these are available immediately after running `fimod registry setup`.

---

## 🌐 HTTP & APIs

### `@gh_latest` — GitHub latest release

Fetches the latest release tag from any GitHub repository. With `--arg asset`, resolves the full download URL for a specific release asset, substituting `{version}` and `{tag}` placeholders automatically.

```bash
# Get the latest release tag
fimod s -i https://github.com/cli/cli/releases/latest -m @gh_latest
# → v2.67.0

# Resolve a release asset URL
fimod s -i https://github.com/cli/cli/releases/latest -m @gh_latest \
  --arg repo=cli/cli \
  --arg asset=gh_{version}_linux_amd64.tar.gz
# → https://github.com/cli/cli/releases/download/v2.67.0/gh_2.67.0_linux_amd64.tar.gz
```

Pipe the resolved URL directly into a second `fimod` invocation to download the asset — no `xargs`, no shell intermediary:

```bash
# Download, filename inferred from the URL
fimod s -i https://github.com/sinelaw/fresh/releases/latest \
  -m @gh_latest \
  --arg repo="sinelaw/fresh" \
  --arg asset='fresh-editor_{version}-1_amd64.deb' \
  | fimod s -I - --output-format raw -O

# Download to an explicit filename
fimod s -i https://github.com/sinelaw/fresh/releases/latest \
  -m @gh_latest \
  --arg repo="sinelaw/fresh" \
  --arg asset='fresh-editor_{version}-1_amd64.deb' \
  | fimod s -I - --output-format raw -o fresh-editor.deb
```

---

### `@download` — File download (wget-like)

Downloads a URL to a local file. The output filename defaults to the last path segment of the URL; override it with `--arg out`.

```bash
# Download to the inferred filename
fimod s -i https://example.com/data/archive.tar.gz -m @download
# → writes archive.tar.gz

# Override the output filename
fimod s -i https://example.com/data/archive.tar.gz -m @download --arg out=backup.tar.gz
# → writes backup.tar.gz
```

Handles binary content correctly (images, archives, binaries) via the raw binary pass-through.

---

## 🛠️ Developer Tooling

### `@poetry_migrate` — Migrate `pyproject.toml` to Poetry 2 / uv

Converts a legacy [Poetry](https://python-poetry.org/) `pyproject.toml` (1.x) to the modern PEP 621 format — either for Poetry 2 or [uv](https://docs.astral.sh/uv/).

```bash
# Migrate to uv (default)
fimod s -i pyproject.toml -m @poetry_migrate -o pyproject.toml

# Migrate to Poetry 2
fimod s -i pyproject.toml -m @poetry_migrate -o pyproject.toml --arg target=poetry2
```

What gets migrated:

- `[tool.poetry]` metadata → `[project]` (name, version, description, authors, …)
- Dependency constraints (`^1.2.3`, `~1.2.3`, `*`) → PEP 440 equivalents
- `[tool.poetry.dev-dependencies]` / groups → `[dependency-groups]` (PEP 735)
- `[[tool.poetry.source]]` → `[[tool.uv.index]]`
- `build-system` updated to `hatchling` (uv) or `poetry-core>=2.0.0` (Poetry 2)

---

## ⚙️ DevOps & Config

### `@yaml_merge` — Patch a YAML file

Applies a set of `path=value` assignments to an existing YAML document using dot-path notation. Useful for patching Kubernetes manifests or any structured config without a full template engine.

```bash
# Scale a deployment and tag it for production
fimod s -i deployment.yaml -m @yaml_merge \
  --arg set="spec.replicas=3,metadata.labels.env=prod" \
  -o deployment.yaml
```

Values are auto-cast: `true`/`false` become booleans, numeric strings become integers.

```yaml
# Before
spec:
  replicas: 1
metadata:
  labels: {}

# After
spec:
  replicas: 3
metadata:
  labels:
    env: prod
```

---

### `@skylos_to_gitlab` — Dead code report → GitLab Code Quality

Converts [Skylos](https://github.com/duriantaco/skylos) dead code analysis output to the [GitLab Code Quality](https://docs.gitlab.com/ee/ci/testing/code_quality.html) (Code Climate) JSON format, ready to upload as a CI artifact.

```bash
# In a GitLab CI job
skylos --json > skylos_report.json
fimod s -i skylos_report.json -m @skylos_to_gitlab -o gl-code-quality-report.json
```

Each finding becomes a Code Climate issue with a stable `fingerprint` (MD5 of check name + file + line + symbol), so GitLab can track it across pipeline runs.

---

## 🔍 Data Exploration

### `@jq_compat` — Common jq operations

A lightweight jq-compatible interface for the three most-used operations: `get` (dot-path extraction), `map` (field projection), and `select` (filtering). Useful when you want the ergonomics of jq without the dependency.

```bash
# Extract a nested field
fimod s -i data.json -m @jq_compat --arg get=user.address.city

# Project a single field from an array
fimod s -i users.json -m @jq_compat --arg map=name

# Filter by field value
fimod s -i users.json -m @jq_compat --arg select=active=true
```

---

## 🛡️ Data Quality

### `@anonymize_pii` — Hash PII fields

Replaces the values of specified fields with their SHA-256 hash. Works on both arrays of objects and single objects.

```bash
fimod s -i users.json -m @anonymize_pii --arg fields=email,phone -o users_anon.json
```

```json
// Before
[{"id": 1, "email": "alice@example.com", "phone": "555-0100"}]

// After
[{"id": 1, "email": "a4c2e...", "phone": "f91b3..."}]
```

The original `id` and any non-listed fields are preserved. Deterministic hashing means the same input always produces the same fingerprint — useful for consistent pseudonymisation across datasets.
