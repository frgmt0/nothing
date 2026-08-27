# nothing

A projectional programming language. The AST is the source of truth. There
is no parser, no text file, no formatter, no syntax error. Editing is a
sequence of typed tree transformations that provably preserve
well-typedness. The editor and the language are one artifact.

See `spec.md` for the full plan, `DECISIONS.md` for design commitments and
deviations from them, and `bench/references.md` for the keystroke baseline
this project is measured against.

## The failure-mode guard

Read this section before adding any feature. It exists specifically so that
"just one more feature" cannot quietly replace fixing the thing that
actually matters.

**If, after four weeks of using the keyboard grammar (Phase 4), the
keystroke ratio versus Neovim exceeds 3× on the five reference programs in
`bench/references.md`, the action grammar is wrong — not incomplete, not
in need of more keybindings, *wrong* — and the next sprint is spent fixing
it, not adding features.**

The keystroke benchmark (`cargo run -p nothing-bench`, `bench/RESULTS.md`
once it exists) is not a vanity metric. It is the only honest signal this
project has for whether structural editing is actually faster to use than
typing text, which is the entire bet the project is making. A grammar that
loses to Neovim by more than 3× is not "a promising direction that needs
polish" — it is evidence the verb/object mapping, the literal-entry path,
or the movement model is fundamentally off, and no amount of additional
constructs fixes that. Stop, re-read `KEYS.md`, and fix the grammar first.
