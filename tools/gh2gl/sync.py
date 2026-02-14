#!/usr/bin/env python3
"""
Sync a fimod release from GitHub → GitLab.

Usage:
    uv run sync.py <version>             # e.g. v0.1.0
    uv run sync.py <version> --dry-run   # preview only
    uv run sync.py <version> --force     # overwrite existing GL release
"""

import sys
import os
import argparse
import tempfile
from pathlib import Path

import httpx
from dotenv import load_dotenv

load_dotenv()


# ── Helpers ───────────────────────────────────────────────────────────

def _die(msg: str) -> None:
    print(f"Error: {msg}", file=sys.stderr)
    sys.exit(1)


# ── Config ────────────────────────────────────────────────────────────

GITHUB_TOKEN   = os.environ.get("GITHUB_TOKEN")   or _die("GITHUB_TOKEN not set")
GITLAB_TOKEN   = os.environ.get("GITLAB_TOKEN")   or _die("GITLAB_TOKEN not set")
GITHUB_REPO    = os.getenv("GITHUB_REPO",    "pytgaen/fimod")
GITLAB_PROJECT = os.getenv("GITLAB_PROJECT", "pytgaen-group/fimod")
GITLAB_BASE    = os.getenv("GITLAB_BASE",    "https://gitlab.com").rstrip("/")
PACKAGE_NAME   = "fimod"


# ── HTTP clients ──────────────────────────────────────────────────────

def github_client() -> httpx.Client:
    return httpx.Client(
        base_url="https://api.github.com",
        headers={
            "Authorization": f"Bearer {GITHUB_TOKEN}",
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
        },
        timeout=60,
        follow_redirects=True,
    )


def gitlab_client() -> httpx.Client:
    return httpx.Client(
        base_url=f"{GITLAB_BASE}/api/v4",
        headers={"PRIVATE-TOKEN": GITLAB_TOKEN},
        timeout=120,
        follow_redirects=True,
    )


# ── GitHub helpers ────────────────────────────────────────────────────

def get_github_release(gh: httpx.Client, repo: str, tag: str) -> dict:
    r = gh.get(f"/repos/{repo}/releases/tags/{tag}")
    if r.status_code == 404:
        _die(f"GitHub release {tag} not found in {repo}")
    r.raise_for_status()
    return r.json()


def get_github_tag_sha(gh: httpx.Client, repo: str, tag: str) -> str:
    """Return the commit SHA that a GitHub tag points to."""
    r = gh.get(f"/repos/{repo}/git/ref/tags/{tag}")
    r.raise_for_status()
    obj = r.json()["object"]
    # Lightweight tag → commit SHA directly; annotated tag → need one more hop
    if obj["type"] == "commit":
        return obj["sha"]
    # Annotated tag: dereference the tag object to get the commit SHA
    r2 = gh.get(f"/repos/{repo}/git/tags/{obj['sha']}")
    r2.raise_for_status()
    return r2.json()["object"]["sha"]


def download_asset(gh: httpx.Client, asset: dict, dest: Path) -> Path:
    """Download a GitHub release asset (follows redirect to S3)."""
    r = gh.get(
        asset["url"],
        headers={"Accept": "application/octet-stream"},
    )
    r.raise_for_status()
    path = dest / asset["name"]
    path.write_bytes(r.content)
    return path


# ── GitLab helpers ────────────────────────────────────────────────────

def resolve_project_id(gl: httpx.Client, project_path: str) -> int:
    encoded = project_path.replace("/", "%2F")
    r = gl.get(f"/projects/{encoded}")
    if r.status_code == 404:
        _die(f"GitLab project not found: {project_path}")
    r.raise_for_status()
    return r.json()["id"]


def upload_package_file(
    gl: httpx.Client,
    project_id: int,
    package_version: str,
    filepath: Path,
    *,
    dry_run: bool = False,
) -> str:
    """Upload file to GitLab Generic Package Registry. Returns public download URL."""
    public_url = (
        f"{GITLAB_BASE}/api/v4/projects/{project_id}"
        f"/packages/generic/{PACKAGE_NAME}/{package_version}/{filepath.name}"
    )
    if dry_run:
        print(f"  [dry-run] would upload {filepath.name} → {public_url}")
        return public_url

    print(f"  Uploading {filepath.name} ({filepath.stat().st_size // 1024} KB)...")
    with filepath.open("rb") as f:
        r = gl.put(
            f"/projects/{project_id}/packages/generic"
            f"/{PACKAGE_NAME}/{package_version}/{filepath.name}",
            content=f.read(),
            headers={"Content-Type": "application/octet-stream"},
        )
    r.raise_for_status()
    return public_url


def ensure_gitlab_tag(
    gl: httpx.Client,
    project_id: int,
    tag: str,
    sha: str,
    *,
    dry_run: bool = False,
) -> None:
    """Create the git tag on GitLab if it doesn't exist yet."""
    r = gl.get(f"/projects/{project_id}/repository/tags/{tag}")
    if r.status_code == 200:
        print(f"  Tag {tag} already exists on GitLab.")
        return
    if dry_run:
        print(f"  [dry-run] would create GitLab tag {tag} → {sha[:12]}")
        return
    r = gl.post(
        f"/projects/{project_id}/repository/tags",
        json={"tag_name": tag, "ref": sha},
    )
    r.raise_for_status()
    print(f"  Tag {tag} created on GitLab ({sha[:12]}).")


def release_exists(gl: httpx.Client, project_id: int, tag: str) -> bool:
    r = gl.get(f"/projects/{project_id}/releases/{tag}")
    return r.status_code == 200


def delete_release(gl: httpx.Client, project_id: int, tag: str) -> None:
    r = gl.delete(f"/projects/{project_id}/releases/{tag}")
    r.raise_for_status()


def create_release(
    gl: httpx.Client,
    project_id: int,
    tag: str,
    name: str,
    description: str,
    links: list[dict],
    *,
    dry_run: bool = False,
) -> None:
    if dry_run:
        print(f"  [dry-run] would create GitLab release {tag} with {len(links)} asset(s)")
        for lnk in links:
            print(f"    - {lnk['name']}")
        return

    payload = {
        "tag_name": tag,
        "name": name,
        "description": description,
        "assets": {"links": links},
    }
    r = gl.post(f"/projects/{project_id}/releases", json=payload)
    r.raise_for_status()
    print(f"  Release {tag} created on GitLab.")


# ── Main ──────────────────────────────────────────────────────────────

def main() -> None:
    parser = argparse.ArgumentParser(description="Sync GitHub release → GitLab")
    parser.add_argument("version", help="Release tag, e.g. v0.1.0")
    parser.add_argument("--dry-run", action="store_true", help="Preview without writing")
    parser.add_argument("--force",   action="store_true", help="Overwrite existing GL release")
    args = parser.parse_args()

    tag     = args.version
    dry_run = args.dry_run

    print(f"Syncing {GITHUB_REPO} {tag} → GitLab {GITLAB_PROJECT}")
    if dry_run:
        print("(dry-run mode — nothing will be written)")

    with github_client() as gh, gitlab_client() as gl:
        # 1. Resolve GitLab project ID
        print("\n[1/5] Resolving GitLab project ID...")
        project_id = resolve_project_id(gl, GITLAB_PROJECT)
        print(f"  Project ID: {project_id}")

        # 2. Fetch GitHub release
        print(f"\n[2/5] Fetching GitHub release {tag}...")
        gh_release = get_github_release(gh, GITHUB_REPO, tag)
        release_name  = gh_release["name"] or tag
        release_body  = gh_release["body"] or ""
        assets        = gh_release["assets"]
        print(f"  Found: {release_name} ({len(assets)} asset(s))")

        # 2b. Ensure the git tag exists on GitLab
        print(f"\n  Resolving commit SHA for {tag} on GitHub...")
        commit_sha = get_github_tag_sha(gh, GITHUB_REPO, tag)
        print(f"  SHA: {commit_sha[:12]}")
        ensure_gitlab_tag(gl, project_id, tag, commit_sha, dry_run=dry_run)

        # 3. Check / handle existing GL release
        print("\n[3/5] Checking GitLab...")
        if release_exists(gl, project_id, tag):
            if args.force:
                if not dry_run:
                    print(f"  Deleting existing GL release {tag}...")
                    delete_release(gl, project_id, tag)
                else:
                    print(f"  [dry-run] would delete existing GL release {tag}")
            else:
                print(f"  Release {tag} already exists on GitLab. Use --force to overwrite.")
                sys.exit(0)
        else:
            print("  No existing release, proceeding.")

        # 4. Download GitHub assets + upload to GL package registry
        print(f"\n[4/5] Downloading {len(assets)} asset(s) from GitHub...")
        links = []

        with tempfile.TemporaryDirectory() as tmpdir:
            tmp = Path(tmpdir)

            for asset in assets:
                name = asset["name"]
                size_kb = asset["size"] // 1024
                print(f"  Downloading {name} ({size_kb} KB)...")
                local_path = download_asset(gh, asset, tmp) if not dry_run else tmp / name

                # Upload versioned asset
                url = upload_package_file(
                    gl, project_id, tag, local_path, dry_run=dry_run
                )
                links.append({
                    "name":      name,
                    "url":       url,
                    "link_type": "package",
                })

            # Also upload a VERSION file to both versioned + latest slots
            version_file = tmp / "VERSION"
            version_file.write_text(tag)

            print(f"\n  Uploading VERSION file (tag={tag}, slot=latest)...")
            upload_package_file(gl, project_id, tag,     version_file, dry_run=dry_run)
            upload_package_file(gl, project_id, "latest", version_file, dry_run=dry_run)

        # 5. Create GitLab release
        print(f"\n[5/5] Creating GitLab release {tag}...")
        create_release(
            gl, project_id, tag, release_name, release_body, links,
            dry_run=dry_run,
        )

    print("\nDone ✓")
    if not dry_run:
        print(
            f"\nDownload URL pattern:\n"
            f"  {GITLAB_BASE}/api/v4/projects/{project_id}"
            f"/packages/generic/{PACKAGE_NAME}/{tag}/<asset>"
        )


if __name__ == "__main__":
    main()
