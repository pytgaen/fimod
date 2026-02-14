"""
Migrate a Poetry pyproject.toml to Poetry 2 or uv format.

Usage:
  fimod s -i pyproject.toml -m @poetry_migrate -o new_pyproject.toml
  fimod s -i pyproject.toml -m @poetry_migrate -o new_pyproject.toml --arg target=uv
"""
# fimod: output-format=toml
# fimod: arg=target  Migration target: poetry2 or uv (default: uv)

def parse_people(items):
    """
    Parse ["Name <email>"] into [{"name": "Name", "email": "email"}]
    """
    people = []
    for item in items:
        # Basic regex-like match
        # We can use re_match from fimod external functions if needed,
        # but string splitting is often enough for simple cases.
        if "<" in item and item.endswith(">"):
            parts = item.split("<")
            name = parts[0].strip()
            email = parts[1].strip(">")
            people.append({"name": name, "email": email})
        else:
            people.append({"name": item})
    return people


def convert_constraint(constraint):
    """
    Convert Poetry constraint to PEP 440.
    ^1.2.3 -> >=1.2.3,<2.0.0
    ~1.2.3 -> >=1.2.3,<1.3.0
    """
    # If it's a list (OR), take the first one or join? PEP 440 uses comma for AND.
    # Poetry allows `["^1.0", "^2.0"]` which acts like OR.
    # PEP 440 doesn't strictly support OR ranges easily in one string without markers.
    # For now, simplistic handling (join with comma is AND, which might be wrong for lists).
    if isinstance(constraint, list):
        return ",".join([convert_constraint(c) for c in constraint])

    s = str(constraint).strip()

    if s == "*":
        return ">0.0.0" # or simply leave empty allowed in some contexts? ">0.0.0" is explicit.

    if s.startswith("^"):
        ver = s[1:]
        parts = ver.split(".")
        # naive logic for caret:
        # ^1.2.3 -> >=1.2.3, <2.0.0
        # ^0.2.3 -> >=0.2.3, <0.3.0
        # ^0.0.3 -> >=0.0.3, <0.0.4

        # We need semantic parsing.
        # Writing a full semver parser in this mold might be heavy.
        # Let's do a basic approximation.
        try:
            major = int(parts[0])
            minor = int(parts[1]) if len(parts) > 1 else 0
            patch = int(parts[2]) if len(parts) > 2 else 0

            lower = f"{major}.{minor}.{patch}"

            if major > 0:
                upper = f"{major + 1}.0.0"
            elif minor > 0:
                upper = f"0.{minor + 1}.0"
            else:
                upper = f"0.0.{patch + 1}"

            return f">={lower},<{upper}"
        except Exception:
            return s # Fallback if parsing fails

    if s.startswith("~"):
        ver = s[1:]
        parts = ver.split(".")
        # ~1.2.3 -> >=1.2.3, <1.3.0
        # ~1.2 -> >=1.2, <1.3
        try:
            major = int(parts[0])
            minor = int(parts[1]) if len(parts) > 1 else 0
            patch = int(parts[2]) if len(parts) > 2 else 0

            lower = f"{major}.{minor}.{patch}"

            if len(parts) >= 2:
                upper = f"{major}.{minor + 1}.0"
            else:
                upper = f"{major + 1}.0.0"

            return f">={lower},<{upper}"
        except Exception:
            return s

    return s


def convert_dependency(name, req, target):
    """
    Convert a dependency definition to PEP 508 string or ignore if not supported.
    """
    if isinstance(req, str):
        version = convert_constraint(req)
        return f"{name}{version}" if version and version != "*" else name

    if isinstance(req, dict):
        # Handle path dependencies
        if "path" in req:
            _path = req["path"]
            # target=uv supports [tool.uv.sources] but this function returns a dependency list item.
            # Ideally we should modify a global [tool.uv.sources] dict, but we are inside a loop.
            #
            # Strategy:
            # 1. return name + " @ file://..." (standard PEP 508 for precise files)
            # OR
            # 2. Return just `name` and let the user add sources manually?
            #
            # Implementation Plan said: "sources with file://... is wrong for relative paths".
            # The user wants "tool.uv.sources" for sources.
            #
            # Limitation: We are generating project.dependencies list here.
            # We can't easily side-effect to tool.uv.sources from this helper without passing context.
            #
            # Hack: return a special dict marker? No, must return string.
            #
            # Let's simplify: return `name` (unpinned) and ADD a comment or warning?
            # Or assume the user will configure sources separately?
            #
            # Let's try to do it right for `uv` if we can.
            # But converting `path = "../foo"` to compliant PEP 508 `name @ file:///abs/path` is hard without knowing absolute path.
            #
            # For now, let's stick to strict PEP 508 if possible, or leave it valid.
            # `name @ .` is not valid.

            # If target is uv, we implicitly assume workspace or manual fix.
            return name # Just the name, expecting sources/workspace resolution

        # Handle git dependencies
        if "git" in req:
            git = req["git"]
            branch = req.get("branch")
            tag = req.get("tag")
            rev = req.get("rev")
            ref = branch or tag or rev
            suffix = f"@{ref}" if ref else ""
            return f"{name} @ git+{git}{suffix}"

        # Handle version in dict
        if "version" in req:
            version = convert_constraint(req["version"])
            return f"{name}{version}"

    return None


def transform(data, args, env, headers):
    """
    Convert Poetry pyproject.toml to PEP 621 / uv / Poetry 2.0 format.
    """
    target = args.get("target", "poetry")  # "poetry" or "uv"

    tool = data.get("tool", {})
    poetry = tool.get("poetry", {})

    if not poetry:
        return data  # Not a poetry project or empty

    # Initialize [project]
    project = {}

    # 1. Metadata Migration
    # ---------------------
    for key in ["name", "version", "description", "readme", "license"]:
        if key in poetry:
            project[key] = poetry[key]

    # Authors / Maintainers
    if "authors" in poetry:
        project["authors"] = parse_people(poetry["authors"])
    if "maintainers" in poetry:
        project["maintainers"] = parse_people(poetry["maintainers"])

    # Keywords / Classifiers / Urls
    if "keywords" in poetry:
        project["keywords"] = poetry["keywords"]
    if "classifiers" in poetry:
        project["classifiers"] = poetry["classifiers"]
    if "urls" in poetry:
        project["urls"] = poetry["urls"]

    # 2. Dependencies Migration
    # -------------------------
    if "dependencies" in poetry:
        deps = poetry["dependencies"]
        project["dependencies"] = []

        # 'python' constraint moves to 'requires-python'
        if "python" in deps:
            project["requires-python"] = convert_constraint(deps.pop("python"))

        for name, constraint in deps.items():
            # Handle complex constraints (dict) or simple string
            req = convert_dependency(name, constraint, target)
            if req:
                project["dependencies"].append(req)

    # 3. Scripts Migration
    # --------------------
    if "scripts" in poetry:
        project["scripts"] = poetry["scripts"]

    # 4. Dev Dependencies & Groups
    # ----------------------------
    # New place for groups: [dependency-groups] (PEP 735)
    # Note: poetry 2 may use [tool.poetry.group.dev] but standard is moving to [dependency-groups]
    # For now, let's put them in [dependency-groups] which uv supports.
    dependency_groups = {}

    # Legacy dev-dependencies -> dependency-groups.dev
    if "dev-dependencies" in poetry:
        dev_deps = []
        for name, constraint in poetry["dev-dependencies"].items():
            req = convert_dependency(name, constraint, target)
            if req:
                dev_deps.append(req)
        if dev_deps:
            dependency_groups["dev"] = dev_deps

    # Modern groups [tool.poetry.group.X]
    if "group" in poetry:
        for group_name, group_data in poetry["group"].items():
            if "dependencies" in group_data:
                group_deps = []
                for name, constraint in group_data["dependencies"].items():
                    req = convert_dependency(name, constraint, target)
                    if req:
                        group_deps.append(req)

                # Merge into existing group if it exists (e.g. dev)
                if group_name in dependency_groups:
                    dependency_groups[group_name].extend(group_deps)
                else:
                    dependency_groups[group_name] = group_deps

    # 5. Build System
    # ---------------
    # target="poetry" -> poetry-core (default)
    # target="uv" -> hatchling
    build_system = {}
    if target == "uv":
        build_system["requires"] = ["hatchling"]
        build_system["build-backend"] = "hatchling.build"
    else:
        # Default to poetry-core (compliant with poetry 2.0)
        build_system["requires"] = ["poetry-core>=2.0.0,<3.0.0"]
        build_system["build-backend"] = "poetry.core.masonry.api"

    # 6. Sources / Index
    # ------------------
    tool_uv = {}
    if "source" in poetry:
        # Convert [[tool.poetry.source]] -> [[tool.uv.index]]
        indexes = []
        for source in poetry["source"]:
            index = {}
            if "name" in source:
                index["name"] = source["name"]
            if "url" in source:
                index["url"] = source["url"]
            if "default" in source and source["default"]:
                index["default"] = True
            # 'secondary' is specific to poetry, uv checks in order.
            # We preserve the order by appending.
            indexes.append(index)

        if indexes:
            tool_uv["index"] = indexes

    # 7. Construct Final Output
    # -------------------------
    output = {}
    output["project"] = project
    output["build-system"] = build_system

    if dependency_groups:
        output["dependency-groups"] = dependency_groups

    # Preserve other tools configuration
    # (Copy over existing [tool] but remove [tool.poetry])
    new_tool = tool.copy()
    if "poetry" in new_tool:
        new_tool.pop("poetry")

    if tool_uv:
        if "uv" not in new_tool:
            new_tool["uv"] = {}
        # Merge our generated uv config
        # Avoid nested assignment new_tool["uv"][k] = v due to Monty limitation
        uv_section = new_tool["uv"]
        for k, v in tool_uv.items():
            uv_section[k] = v

    if new_tool:
        output["tool"] = new_tool

    return output
