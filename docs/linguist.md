# Linguist — language statistics in Blue Code

Open a project folder in Blue Code and a **Languages** card appears in the
bottom-right corner of the editor — the same stacked bar and list GitHub shows.
When the biggest language is one of the HackerOS languages it reads
"**H# leads this project**" with the language's icon, as on bit.io.

* **Every GitHub language.** The database is GitHub Linguist's own
  `languages.yml` (836 languages: extensions, exact filenames, interpreters,
  colours) plus its `vendor.yml` / `documentation.yml`, compiled into the shell
  (`src-tauri/resources/linguist/languages.json`, refreshed by
  `scripts/update-linguist-db.py`).
* **HackerOS languages built in:** **H#** (`.h#`), **H# Interface** (`.h#i`),
  **HackerScript** (`.hcs`), **Hacker Lang** (`.hl`), **hk** (`.hk`, `Bit.hk`),
  with the bit.io colours. (`.hs` stays Haskell.)
* **GitHub's counting rules:** bytes per language; only *programming* and
  *markup* count (JSON/YAML/Markdown don't); vendored, generated, documentation
  and binary files are skipped; files come from `git ls-files` (exactly what
  GitHub would see) or a `.gitignore`-aware walk outside git.
* **Ambiguous extensions** (`.h`, `.m`, `.pl`, `.ts`, `.r`, `.v`) are decided by
  content; extension-less scripts by their shebang.
* **`.gitattributes`:** `linguist-language=`, `linguist-vendored`,
  `linguist-generated`, `linguist-documentation`, `linguist-detectable` — like GitHub.

## Your own languages

Command palette → **Custom Languages (.blue/languages.json)…** (or the ⚙ button
on the card) creates `<project>/.blue/languages.json` from a template:

```json
{ "languages": [
  { "name": "MyLang", "type": "programming", "color": "#ff5fa2",
    "extensions": [".ml1", ".mylang"], "filenames": ["Mylangfile"], "interpreters": ["mylang"] }
] }
```

The same file at `~/.config/Blue-Environment/blue-code/languages.json` applies
to every project; project entries win, and custom entries win over GitHub's
(and the built-ins) for the extensions they claim. Bad entries are dropped, never fatal.

Palette: **Toggle: Language Statistics**, **Rescan Project Languages**. The card
rescans after a save and when the window regains focus.
