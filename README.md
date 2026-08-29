# nothing

A projectional programming language. The AST is the source of truth. There
is no parser, no text file, no formatter, no syntax error. Editing is a
sequence of typed tree transformations that provably preserve
well-typedness. The editor and the language are one artifact.

## Holes, and why an unfinished program is still a program

In a text language you write a program by typing characters and a parser
tries to read a tree back out of them. Most of what you type on the way to
a finished program is not a program yet. `if x >` is not a program. That
state, the one your file spends most of its life in, is what a syntax error
is a report about.

`nothing` has no such state. You do not type a program, you build one.
Every keystroke is a typed transformation of the tree, and the available
transformations are defined so that a well-typed program plus any
transformation is either refused outright, changing nothing, or another
well-typed program. There is no third outcome and no moment in between.

That works only because the language can write down *nothing here yet*.
It is called an **empty hole**, written `⦇⦈`, and it is an ordinary
expression with an ordinary type, exactly like `1` or `true`.

A new document is one definition whose body is a hole:

```
main : ? = ⦇⦈
```

That program is finished, in the sense that matters: it is well typed. It
does not compute anything yet. Press `\` and the hole becomes a function
whose body is another hole:

```
main : ? = λx0:?. ⦇⦈
```

Still well typed. The parameter has no name yet and no type yet. `?` is the
unknown type, consistent with every type, so having not decided breaks
nothing downstream.

The second kind of hole is what happens when you write something that does
not fit. Say `greet : Str -> Str` exists, and you are filling the argument
of `print`, which wants a `Str`. You write `greet`. A function is not a
string. A text editor would accept it and let a compiler reject the file
later; a naive structural editor would refuse the keystroke and lose what
you meant. `nothing` does neither. It wraps what you wrote in a **non-empty
hole**, a quarantine:

```
main : Cmd ? = print ⦇greet⦈
```

Your `greet` is still there, unchanged. The brackets around it are what
keep the whole program well typed: a non-empty hole synthesises the unknown
type whatever is inside it, so the mismatch is contained at the node where
it happened instead of spreading. The editor says one thing is quarantined
and where it is. You finish the thought, and the quarantine closes itself:

```
main : Cmd ? = print (greet "world")
```

So: an empty hole is an expression meaning "nothing written here yet", a
quarantine is an expression meaning "this does not fit yet, and it is being
held rather than thrown away", and between them there is no point in
building a program at which the program is not a program. Nothing is ever
unparseable, because nothing is ever parsed.

## Ninety seconds in the editor

`nothing tutorial` opens a nine-step guided session inside the real editor.
The frames below are captured pty output from the test that types the whole
tutorial through the real key handler,
`cli/tests/tutorial.rs::the_whole_tutorial_typed_into_the_real_editor_runs_when_it_is_finished`,
at 120 columns by 40 rows. They are three separate frames from that one
session, not a continuous recording, and they are byte-exact.

```
$ nothing tutorial
```

Step 1. The document is one empty hole, the cursor is on it, and the pane
on the right says what to press.

```
┌ nothing ───────────────────────────────────────────────────────────────────────────┐┌ tutorial 1/9 ──────────────────┐
│ »⦇⦈«                                                                               ││ Step 1 of 9                    │
│                                                                                    ││ Write a function               │
│                                                                                    ││                                │
│                                                                                    ││ Nothing here is typed as text  │
│                                                                                    ││ and parsed: every key builds a │
│                                                                                    ││ piece of program.              │
│                                                                                    ││ Press \ to write a function.   │
│                                                                                    ││ You get λx0:?. ⦇⦈: a           │
│                                                                                    ││ parameter, a type nobody has   │
│                                                                                    ││ said yet, and ⦇⦈, a hole where │
│                                                                                    ││ the body will go.              │
│                                                                                    ││                                │
│                                                                                    ││ ▸ Write a function             │
│                                                                                    ││ · Name the parameter           │
│                                                                                    ││ · Give it a type               │
│                                                                                    ││ · Fill the hole                │
│                                                                                    ││ · Rename the definition        │
│                                                                                    ││ · Start a second definition    │
│                                                                                    ││ · Say main is a command        │
│                                                                                    ││ · Cause a quarantine           │
│                                                                                    ││ · Repair it and finish         │
└────────────────────────────────────────────────────────────────────────────────────┘└────────────────────────────────┘
program ⇒ ⦇⦈ · blocked on ⦇⦈#161922c6
node · expects ? · empty hole
```

Step 9. Two definitions now, a list pane on the left, and the quarantine
from the section above is on screen: `greet` has been written where a `Str`
was expected. The status line counts it and names its type. The program is
still well typed.

```
┌ defs 2/2 ──────────┐┌ main : Cmd ? ────────────────────────────────────────────────┐┌ tutorial 9/9 ──────────────────┐
│   greet : Str -> … ││ print ⦇»greet«⦈                                              ││ Step 9 of 9                    │
│ > main : Cmd ?     ││                                                              ││ Repair it and finish           │
│                    ││                                                              ││                                │
│                    ││                                                              ││ greet only needs its argument. │
│                    ││                                                              ││ Press space to apply it, then  │
│                    ││                                                              ││ " w o r l d " for the text.    │
│                    ││                                                              ││ The quarantine now holds text  │
│                    ││                                                              ││ after all. Press Enter and it  │
│                    ││                                                              ││ closes.                        │
│                    ││                                                              ││                                │
│                    ││                                                              ││ ✓ Write a function             │
│                    ││                                                              ││ ✓ Name the parameter           │
│                    ││                                                              ││ ✓ Give it a type               │
│                    ││                                                              ││ ✓ Fill the hole                │
│                    ││                                                              ││ ✓ Rename the definition        │
│                    ││                                                              ││ ✓ Start a second definition    │
│                    ││                                                              ││ ✓ Say main is a command        │
│                    ││                                                              ││ ✓ Cause a quarantine           │
│                    ││                                                              ││ ▸ Repair it and finish         │
└────────────────────┘└──────────────────────────────────────────────────────────────┘└────────────────────────────────┘
program ⇒ print ⦇greet⦈ · blocked on ⦇e⦈#ea624061
node · expects ? · variable · inside ⦇⦈ · does not fit yet · 1 quarantined · typing `greet` · ‹greet:Str -> Str›
```

Applying `greet` to a string makes it fit, and the quarantine goes away.
This frame is trimmed after the tick list begins; everything shown is
byte-exact.

```
┌ defs 2/2 ──────────┐┌ main : Cmd ? ────────────────────────────────────────────────┐┌ tutorial · done ───────────────┐
│   greet : Str -> … ││ print »(greet "world")«                                      ││ All 9 steps are done.          │
│ > main : Cmd ?     ││                                                              ││                                │
│                    ││                                                              ││ Press C-q to quit. The file is │
│                    ││                                                              ││ saved, and then performed.     │
│                    ││                                                              ││                                │
│                    ││                                                              ││ Run it again with:             │
│                    ││                                                              ││ nothing run tutorial.n         │
```

Quitting saves the document and performs it:

```
tutorial: saved tutorial.n
tutorial: running tutorial.n
hello, world
```

The program that got built is a real document you can reopen, edit and run:

```
greet : Str -> Str = λwho:Str. "hello, " ++ who
main : Cmd ? = print (greet "world")
```

## Install

There is no published package. None of this workspace's crates are on
crates.io: `cargo install nothing-lang` fails because no such crate exists,
and `cargo install nothing` installs an unrelated crate by someone else.
Build from source.

```sh
git clone https://github.com/frgmt0/nothing
cd nothing
cargo install --path cli
```

That installs one binary, `nothing`. To build without installing, which is
the command `.github/workflows/release.yml` runs:

```sh
cargo build --release -p nothing-cli --bin nothing
```

and the binary lands at `target/release/nothing`.

The workspace is Rust edition 2024 with resolver 3, so the floor is Rust
1.85, the first release that supports edition 2024. No `rust-version` is
declared and nothing older than current stable has been tested; CI builds
on `dtolnay/rust-toolchain@stable`.

**Prebuilt binaries.** `.github/workflows/release.yml` builds `nothing` for
macOS arm64 and Linux x86_64 on any pushed tag matching `v*`, and attaches
`nothing-macos-arm64.tar.gz` and `nothing-linux-x86_64.tar.gz` to a GitHub
release at [github.com/frgmt0/nothing](https://github.com/frgmt0/nothing).
Each tarball holds a single executable named after the asset, so unpacking
gives you `nothing-macos-arm64` or `nothing-linux-x86_64` and you rename it
to `nothing` yourself. No tag has been pushed yet, so there is nothing to
download today. Source is the only way in until v0.1.0 is cut.

## Start here

```sh
nothing tutorial
```

Nine steps inside the real editor: write a function, name and type its
parameter, fill a hole, rename a definition, start a second one, give
`main` a command type, cause a quarantine, repair it, run the result. It
takes under twenty minutes at a beginner's pace and it touches every core
concept once.

Progress is checked against the program you have actually built. Each step
is a structural query on the document, not a comparison against text you
typed or output that was printed, so there is no way to satisfy a step by
typing the right-looking thing. Quitting with `C-q` and reopening the same
file resumes where you stopped; there is no progress file, because the
position in the tutorial is read back off the document itself. The file it
writes is an ordinary document: `nothing edit tutorial.n` and `nothing run
tutorial.n` work on it afterwards.

After that:

- [`examples/`](examples/) has five complete programs that run, with
  [`examples/EXAMPLES.md`](examples/EXAMPLES.md) explaining what each one
  is for and what it prints. Run one with
  `nothing run examples/decision_table.n`.
- `nothing doc` renders the standard library, 37 definitions with types and
  doc lines. The committed copy is [`stdlib/REFERENCE.md`](stdlib/REFERENCE.md).
- `nothing --help` lists the subcommands: `tutorial`, `edit`, `run`,
  `check`, `doc`, `repl`, `protocol`, `mcp`, `merge`, `merge-driver`,
  `textconv`, `diff-driver`.

## Three numbers

Every number here comes from an executed run of a committed harness. The
files linked below carry the full tables, the methodology and the caveats.

### Keystrokes: 0.21x to 0.77x of the Neovim baseline

[`bench/RESULTS.md`](bench/RESULTS.md). The seven reference programs cost
between 0.21x and 0.77x the keystrokes Neovim needs for the same program.
The worst case is `record`, at 0.77x.

The denominator is the permanent Neovim baseline fixed in
[`bench/references.md`](bench/references.md): one keypress per character of
the reference text, plus one `i` and one `Esc`, with no motions, no
autoindent and no snippet expansion credited. It is hand-counted once per
reference program and never recomputed. The numerator is counted by
replaying committed keystroke fixtures through the real key handler.

The guard this project set itself is 3x. The current worst case is a
quarter of that budget. Both tripwires are asserted in code, so a
regression fails `cargo test --workspace` rather than waiting for someone
to reread the table.

### Merge: 18 of 21 clean, against git's 2

[`bench/MERGE.md`](bench/MERGE.md). Twenty-one scenarios, each built as a
common ancestor and two branches, merged twice.

| | scenarios | clean | clean and correct | conflicts |
| --- | ---: | ---: | ---: | ---: |
| `git merge-file` on the rendered text | 21 | 2 | 2 | 19 |
| structural merge on typed operations | 21 | 18 | 18 | 3 |

All 21 structural results are well typed.

The three structural conflicts are the control scenarios, the ones that
*should* conflict: two branches renaming the same function differently, two
branches moving the same function to two different places, and two branches
changing the same literal two different ways. Those are real disagreements
and an engine that called them clean would be broken, not clever.

### Agent: the text baseline won both runs

[`bench/AGENT.md`](bench/AGENT.md). Say this first, because it is the
result: **handing a model the action protocol did not beat handing it
program text.** It lost the first run and it lost the second.

Second run, 2026-08-29, 32 post-B2 programs using strings, lists, records
and `match`, model `claude-haiku-4-5-20251001`:

| | invalid edits | rate | reached target |
| --- | --- | ---: | --- |
| action protocol, interactive | 9 / 315 | 2.9 % | 23 / 32 |
| text baseline, one shot | 0 / 32 | 0.0 % | 30 / 32 |
| text baseline, interactive | 0 / 32 | 0.0 % | 31 / 32 |

The protocol's invalid-edit rate fell from 11.4 % in the first run to
2.9 % in the second, on larger programs, which is the run's real finding.
It is not a win. 0 % is still the number to beat, and the baseline hit the
target more often while spending a tenth of the model calls.

The one thing the protocol delivered as promised: **0 ill-typed
intermediate states out of 320 recorded steps.** Every refusal cost one
action and left the program exactly where it was. That guarantee is
structural and it held. It has not yet been shown to be worth what it
costs.

## The failure-mode guard

Read this section before adding any feature. It exists specifically so that
"just one more feature" cannot quietly replace fixing the thing that
actually matters.

**If, after four weeks of using the keyboard grammar (Phase 4), the
keystroke ratio versus Neovim exceeds 3× on the seven reference programs in
`bench/references.md`, the action grammar is wrong. Not incomplete, not in
need of more keybindings: *wrong*. The next sprint is spent fixing it, not
adding features.**

The keystroke benchmark (`cargo run -p nothing-bench -- keytable`, recorded
in `bench/RESULTS.md`) is not a vanity metric. It is the only honest signal
this project has for whether structural editing is actually faster to use
than typing text, which is the entire bet the project is making. A grammar
that loses to Neovim by more than 3× is not "a promising direction that
needs polish". It is evidence the verb/object mapping, the literal-entry
path, or the movement model is fundamentally off, and no amount of
additional constructs fixes that. Stop, re-read `KEYS.md`, and fix the
grammar first.

## Where everything is

- [`spec-DONE.md`](spec-DONE.md) is the research-phase plan, completed. Its
  own heading still reads `spec.md`, which is the name the rest of the repo
  refers to it by; there is no separate `spec.md` file.
- [`spec-build.md`](spec-build.md) is the v0.1.0 plan, phases B0 to B7.
- [`DECISIONS.md`](DECISIONS.md) records design commitments, the
  alternatives rejected, and every deviation.
- [`CONTRIBUTING.md`](CONTRIBUTING.md) has the three invariants and the
  full-thread checklist a language feature has to pass before it exists.
- [`FORMAT.md`](FORMAT.md) is the binary file format and its migration
  guarantee.
- [`KEYS.md`](KEYS.md) is the keyboard grammar, which still fits on one
  screen.
- [`GIT.md`](GIT.md) is the git merge driver and the `.gitattributes`
  recipe that makes `.n` files diff and merge structurally inside ordinary
  git workflows.
- [`bench/`](bench/) holds every number, each with the command that
  reproduces it. [`bench/BEGINNER.md`](bench/BEGINNER.md) is the exception:
  it is the protocol for a beginner-projection test that requires three
  people who do not program, and **it has not been run yet**.
