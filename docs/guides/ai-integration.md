# AI Integration

Fimod is an excellent tool for AI coding assistants (like Cursor, GitHub Copilot, Claude Code, Windsurf, or Antigravity) because it provides a reliable, dependency-free way to read and modify structured data (`.yaml`, `.json`, `.toml`, `.csv`) directly from the shell.

If you teach your AI assistant how to use Fimod, it will stop trying to write fragile intermediate python/node scripts to edit config files, and instead use Fimod's robust CLI.

---

## The Fimod Skill

We provide a standard **Agent Skill** documentation to teach any AI about Fimod. This replaces writing custom prompts.

**👉 [Read the Fimod Agent Skill](../../.agents/skills/fimod/SKILL.md)**

### How to use it:

*   **Autonomous Agents (Antigravity, etc.)**: Copy the `.agents/skills/fimod` directory directly into your project's `.agents/skills/` directory (or use this repository as-is).
*   **Cursor / Windsurf**: Copy the contents of `SKILL.md` into your `.cursorrules` or `.windsurfrules` file.
*   **GitHub Copilot (VS Code)**: Copy the contents of `SKILL.md` into `.github/copilot-instructions.md`.
*   **Claude Code**: Append the contents of `SKILL.md` to your `CLAUDE.md` file.
