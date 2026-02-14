# Sketchnote — fimod: The Flexible & Secure Data Mold

## Core concept: the casting mold metaphor

> You pour raw data into a Python mold → it comes out reshaped the way you want.

---

## Visual structure (4 nodes + 1 peripheral)

```text
╔══════════════════════════════════════════════════════════╗
║                                                          ║
║       🦀 fimod — The Flexible & Secure Data Mold        ║
║                                                          ║
╚══════════════════════════════════════════════════════════╝

         ┌─────────────────────────────────────┐
         │           🔥 THE MOLD (core)        │
         │                                     │
         │  def transform(data,                │
         │                args,                │
         │                env,                 │
         │                headers):            │
         │      ...                            │
         │      return data                    │
         │                                     │
         │  Rust 🦀 pours → Python 🐍 shapes   │
         └─────────────────────────────────────┘
                ▲                    │
                │                    ▼
  ┌─────────────────────┐  ┌──────────────────────┐
  │      📥 INPUT       │  │      📤 OUTPUT        │
  │                     │  │                       │
  │  JSON  YAML  TOML   │  │  JSON  YAML  TOML    │
  │  CSV   TXT   stdin  │  │  CSV   TXT   stdout  │
  │  🌐 HTTP URL        │  │  📁 file via -o       │
  └─────────────────────┘  └──────────────────────┘

         ┌─────────────────────────────────────┐
         │       ⛓️  4 FORMS OF MOLD           │
         │                                     │
         │  📄 File      normalize.py           │
         │  ─────────────────────────────────  │
         │  💻 Inline    -e "[x['name'].upper() for x in data]"    │
         │  ─────────────────────────────────  │
         │  🔗 Chained   -e … -e … -e …        │
         │                ↓  ↓  ↓              │
         │          transformation pipeline    │
         │  ─────────────────────────────────  │
         │  📦 Registry  @source/mold           │
         └─────────────────────────────────────┘
```

---

## Peripheral: Mold Registries

```text
              ┌──────────────────────────────┐
              │      📦 MOLD REGISTRIES      │
              │                              │
              │  🌍 Public    @community      │
              │  🔒 Private   @my-team (Github/Gitlab Token)       │
              │                              │
              │  Add:                        │
              │  fimod registry add \        │
              │    https://…/catalog.toml    │
              │                              │
              │  Use:                        │
              │  fimod shape -i data.json \  │
              │    @source/mold              │
              └──────────────────────────────┘
```

---

## Security banner (border element, style: "containment wall")

```text
┌─ 🛡️ RUST SANDBOX ──────────────────────────────────────────┐
│                                                             │
│   🚫 Filesystem     🚫 Direct network     🚫 Shell          │
│                                                             │
│   ✅ Env vars: opt-in only — --env PATTERN                  │
│      (glob: MY_VAR, PREFIX_*, * — default: none)           │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## "Why fimod?" block (to place near the header)

```text
┌─────────────────────────────────────────────────────────┐
│                    💡 WHY FIMOD?                        │
│                                                         │
│  ⚡ Single binary — drop & run, no runtime to install   │
│  🔁 Convert any format → any format (JSON↔CSV↔YAML…)   │
│  🎛️  Parameterize with --arg key=value                  │
│  📚 Process many files at once with --slurp             │
└─────────────────────────────────────────────────────────┘
```

---

## Recommended color palette

| Zone          | Color        | Rationale                           |
|---------------|--------------|-------------------------------------|
| fimod logo    | Steel blue   | Rust identity / robustness          |
| Mold core     | Orange       | Heat, forge, transformation         |
| Input         | Light green  | Live incoming data                  |
| Output        | Dark green   | Shaped outgoing data                |
| Security      | Brick red    | Boundary, restriction               |
| Registries    | Purple       | Community, sharing                  |

---

## Central message (sketchnote tagline)

> **One Rust binary. Your Python scripts. Zero Python to install.**
> Pour your data into the mold. It comes out in the shape you want.

---

## Notes for the illustrator

- The **casting mold** (central block) is the dominant element — larger than all others.
- The **`transform()` signature** must be readable — it is the only visible code in the sketchnote.
- The **Input → Mold → Output arrows** form the horizontal backbone.
- The **4 mold forms** (file / inline / chained / registry) are a vertical block placed right or below.
- The **security banner** frames the whole composition like a transparent containment wall.
- **Registries** are a secondary element, placed in a bottom corner.
- Priority icons: 🦀 (Rust) · 🐍 (Python) · 🔥 (forge/mold) · 🛡️ (sandbox) · ⛓️ (chaining)
