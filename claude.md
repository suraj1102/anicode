# CLAUDE.md

## Code

- Write the simplest code that works. No extra layers, patterns, or abstractions unless the code needs them now.
- Use clear names so the code explains itself.
- Add a comment only when the reason behind the code is not obvious. Say why, not what.
- Keep functions short and focused on one job.
- Do not add features, options, or error handling that nobody asked for.
- Do not change code, comments, or formatting outside the task.
- Match the style already in the project.
- If you see unrelated dead code, mention it. Do not delete it.

## Config

- Put all settings in TOML files under `config/`.
- Do not hardcode values that belong in config.
- Never put secrets in code or config files. Read them from .env file which puts them as environment variables.

## Language

- Use plain, simple technical English in code, comments, docs, and messages.
- No jargon or buzzwords. Avoid words like leverage, utilize, robust, seamless, delve, facilitate.
- No emojis anywhere.
- No em dashes or en dashes. Use commas, periods, or parentheses.
- ASCII only. Straight quotes, plain hyphens.
- No filler or praise. Get to the point.

## Commits

- One short line in the imperative mood, under 60 characters.
- Example: `add user login`, `fix date parsing`, `move db settings to toml`
- No body, no emojis, no "Generated with" or "Co-Authored-By" lines.
- One change per commit.

## Before you commit

- Run the tests and the linter. Both must pass unless specified.
- Remove debug prints and unused imports.
