# Skylos to GitLab Code Quality

This mold converts JSON reports from `duriantaco/skylos` (a Python static analysis tool) into the GitLab Code Quality (Code Climate) format. This allows Skylos results to be visualized directly in GitLab Merge Requests.

## Usage

```bash
fimod s -i skylos_report.json -m @skylos_to_gitlab -o code_quality.json
```

### In a GitLab CI pipeline

```yaml
code_quality:
  script:
    - skylos --json > skylos_report.json
    - fimod s -i skylos_report.json -m @skylos_to_gitlab -o gl-code-quality-report.json
  artifacts:
    reports:
      codequality: gl-code-quality-report.json
```

## Transformation Logic

The converter maps Skylos report sections to Code Climate issue categories:

| Skylos Section | Code Climate `check_name` |
| :--- | :--- |
| `unused_functions` | `unused-function` |
| `unused_imports` | `unused-import` |
| `unused_variables` | `unused-variable` |
| `unused_classes` | `unused-class` |
| `unused_parameters` | `unused-parameter` |
| `unused_files` | `dead-file` |

### Fingerprinting
A deterministic fingerprint is generated for each issue using a hash of the check name, file path, line number, and item name. This allows GitLab to track issues across commits even if line numbers shift slightly (though strict line matching is used here).

## Output Format

The output is a JSON array of issue objects:

```json
[
  {
    "description": "Unused function: my_func",
    "check_name": "unused-function",
    "fingerprint": "a1b2c3d4",
    "severity": "info",
    "location": {
      "path": "src/main.py",
      "lines": {
        "begin": 42
      }
    }
  }
]
```
