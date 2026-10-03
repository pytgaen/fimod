# Shared agent guidelines

## Execution and decisions

- Complete requested implementation and verification. A clear local change request
  authorizes its edits and checks; reuse approvals already given. Reviews and
  proposals stay read-only unless edits are requested.
- Read what the task needs. Give a brief scope and validation plan for substantial
  work; resolve routine choices from repository evidence.
- State material assumptions. Ask only when missing information changes scope,
  public behavior, compatibility, security, or existing user work.
- If blocked, finish independent work and report the exact blocker. Distinguish
  explicit requirements from your interpretation of guidelines.

## Scope and verification

- Make the smallest correct change using existing patterns and helpers. Add
  dependencies or abstractions only when the requested outcome requires them.
- Match local style, avoid unrelated cleanup, and retain safety guarantees.
  Remove dead code introduced by your change.
- Verify the requested behavior and complete required project checks. Add useful
  regression coverage for bugs; avoid tests that merely mirror implementation.
  Broaden or repeat passing checks only for new changes or unresolved concerns.
- Report changes, checks, and limitations concisely in the user's language.
  Separate observed evidence from assumptions and local checks from live proof.

## Git and user work

- Check `git status --short --branch` before edits. Preserve staged, unstaged, and
  untracked work. If the dirty state has not been authorized for this task, ask
  one scope question before editing; reuse that answer throughout the task.
- Commit only on an explicit request. Establish the full scope before staging;
  do not guess a subset. `commit tout` authorizes `git add -A`.
- Follow repository branch, commit, and release conventions. Never bypass hooks
  or signing, or force-push shared branches.
- Destructive or history-rewriting operations need explicit authorization and a
  recovery plan. A backup tag protects commits, not dirty files.
- Pushes, PR creation/merge, published tags, releases, and messages to others need
  explicit authorization for the action; local edit approval alone does not grant it.
