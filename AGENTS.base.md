# AGENTS.base.md

Behavioral guidelines to reduce common LLM coding mistakes. Bias toward caution over speed; use judgment on trivial tasks.

## 1. Think Before Coding
- State assumptions explicitly; if uncertain, ask.
- Present multiple interpretations — don't pick silently.
- If a simpler approach exists, say so.

## 2. Simplicity First
- Minimum code that solves the problem. Nothing speculative.
- No features, abstractions, or error handling beyond what was asked.
- If 200 lines could be 50, rewrite.

## 3. Surgical Changes
- Touch only what the request requires; match existing style.
- Don't refactor adjacent code or delete pre-existing dead code.
- Remove only orphans YOUR changes created.

## 4. Goal-Driven Execution
- Turn tasks into verifiable goals ("fix bug" → "write failing test, then pass it").
- For multi-step work, state a brief plan with verification per step.

## 5. Secure & Professional Git Workflow
- Feature/fix work on dedicated branches via PR; never commit directly on `main`.
- Atomic commits, semantic-release format (`feat(scope):`, `fix(scope):`); one logical change per commit.
- Before any destructive op (`reset --hard`, `rebase`, squash, force-push), create a `backup/<name>` tag and confirm with user.
- Never `--no-verify`, never bypass signing, never force-push shared branches.
- Confirm before any remote-visible action (push, PR create/merge, tag push, gh comment).

---
**Working if:** fewer unnecessary diffs, fewer rewrites from overcomplication, clarifying questions before implementation.
