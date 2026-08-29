# `nothing` inside git

A `nothing` document is a binary file. `NTHG` magic, a node table, a name
table, a doc table, an action log — see `FORMAT.md`. There is no text file
to diff, because there is no parser: the tree *is* the program. Git has no
idea what to do with that, so out of the box a `.n` file behaves like a
JPEG:

```
$ git log -p -- program.n
Binary files a/program.n and b/program.n differ

$ git merge theirs
warning: Cannot merge binary files: program.n (HEAD vs. theirs)
CONFLICT (content): Merge conflict in program.n
```

Every concurrent change conflicts, and no change is ever legible in review.

The three commands in this document fix that, without asking anyone to
change how they use git. `git diff`, `git log -p`, `git show`, `git merge`,
`git rebase` and every tool built on them keep working; they just get taught
what a `nothing` document is.

## The recipe

Put `nothing` on your `PATH`, then, in a repository that holds `.n` files:

```sh
git config merge.nothing.name "nothing structural merge"
git config merge.nothing.driver "nothing merge-driver %O %A %B %L %P"
git config diff.nothing.textconv "nothing textconv"
```

and commit a `.gitattributes` at the repository root:

```gitattributes
*.n -text merge=nothing diff=nothing
```

`.gitattributes` is versioned and shared; the `git config` lines are not,
because git will not let a repository configure a program to run on
checkout. Every clone has to run those three lines once. Put them in your
project's setup script.

`-text` tells git never to apply end-of-line conversion to a `.n` file.
Without it a CRLF-normalising checkout would corrupt the binary. You do not
need `git config diff.nothing.binary true`: with a `textconv` configured git
diffs the *converted* text, which is text, so the binary heuristic never
fires. Set `binary` only if some other tool in your pipeline insists on
treating the raw blob as binary.

Optionally, for the typed-operation diff described below:

```sh
git config diff.nothing.command "nothing diff-driver"   # replaces the diff
```

## `nothing merge-driver` — merging

Git calls the driver with `%O %A %B %L %P`: the base file, *our* file, *their*
file, the conflict marker size, and the path being merged. The driver:

1. decodes all three files as `nothing` documents;
2. runs the Phase 9 structural three-way merge (`nothing_merge::merge_documents`)
   over them, name table and doc table included;
3. on a clean merge, checks the result is well-typed with the standard
   library in scope, writes it back over `%A` — the file git reads — and
   exits 0;
4. on conflicts, prints every conflict on standard error, leaves `%A`
   exactly as git wrote it, and exits 1, which git records as a conflicted
   path.

A file that will not decode, and a merge whose result is somehow not
well-typed, are both an exit of 1 with nothing written. The driver never
writes an ill-typed document: a merge that would break the language's
central invariant is a conflict, not a result. It never panics on bad
input either; a garbled blob is a conflict, not a crash.

The merged document is written with a fresh, empty action log. A merge is
not a sequence of edits by one author, so there is no honest log to carry
forward; the history lives in git from that point.

### What this buys you

Two branches merge cleanly whenever their edits touch disjoint *structure*,
regardless of what the bytes did:

- one branch edits definition `helper`, the other edits definition `main`;
- both branches edit the same definition, in different subtrees;
- one branch renames a definition while the other edits its body;
- one branch adds a definition, the other edits an existing one.

Every one of those is a byte-level conflict, every one of them merges, and
each has a test in `cli/tests/git_integration.rs` that drives real git.
Renames, reorderings, annotations and doc lines are merged the same way,
each on its own axis: a rename is a name-table write with no structural
footprint, so it commutes with anything that is not a competing rename.

### What a conflict looks like

A conflict is a real disagreement about one node, and it reads like one:

```
$ git merge ours
nothing merge-driver: program.n: 1 structural conflict(s)
conflict (same node, different values) in definition helper: the right operand (node 0.1)
  why:    one branch replaces `1` with `20` at the right operand (node 0.1); the
          other replaces `1` with `10` at the right operand (node 0.1). Those two
          edits touch the same nodes and do not commute, so neither can be
          replayed on top of the other.
  base:   1
  ours:   20
  theirs: 10

resolve them with `nothing edit program.n`, then `git add program.n`
Auto-merging program.n
CONFLICT (content): Merge conflict in program.n
Automatic merge failed; fix conflicts and then commit the result.
```

There are **no conflict markers in the file**, and there cannot be: a
`nothing` document has no text to interleave, and a half-merged tree with
`<<<<<<<` in it would not be a well-typed program. What you get in the
working tree is the file the driver declined to overwrite — the *ours* side,
which is what git puts in `%A` before calling it — plus all three stages in
the index. Resolve it the way you resolve any structural edit:

```sh
nothing edit program.n     # make the tree say what the merge should have said
git add program.n
git merge --continue
```

If you want the other side wholesale, `git checkout --theirs -- program.n`.
If you want to inspect the three stages, `git show :1:program.n`,
`:2:program.n` and `:3:program.n` write the base, ours and theirs blobs to
standard output; save them and hand them to `nothing merge <base> <a> <b>`
to see the same report outside the merge.

## `nothing textconv` — reading diffs

`textconv` is git's hook for "run this program over the blob, diff the
output instead". `nothing textconv <file>` prints a stable structural
rendering: one definition at a time, headed by its name and type, its doc
line if it has one, then its body one syntactic group per line.

```
$ git log -p -- program.n
diff --git a/program.n b/program.n
index 1d6943b..a11279b 100644
--- a/program.n
+++ b/program.n
@@ -1,6 +1,6 @@
 def helper : Num -> Num
   λn:Num.
-    n + 1
+    n + 10
 end

 def main : Num
```

That is `git log -p` on a binary file, in an ordinary review, with an
ordinary hunk. The rendering is deterministic — definition order is document
order, no ids, no hashes, no timestamps, no hash-map iteration — so the same
document always produces the same bytes, and a small edit produces a small
hunk.

**The rendering is a projection, not the file.** It is lossy: hole
identities, the action log and the raw ids are not in it. You cannot
`git apply` a textconv diff, `git add -p` cannot stage it, and no tool can
reconstruct a document from it. It exists to be read.

## `nothing diff-driver` — the typed-operation diff

`textconv` gives you a text diff of a rendering. The structural diff knows
more than that: it recovers the *typed operations* that turn one version
into the other. `diff.nothing.command` is git's external-diff hook, and
`nothing diff-driver` implements it.

```
$ git -c diff.nothing.command="nothing diff-driver" diff trunk HEAD -- program.n
--- a/program.n
+++ b/program.n
definitions
  definition `helper` edited
    [Replace] replaces `1` with `10` at the right operand (node 0.1) -- now `10`
  definition `main` edited
    [Replace] replaces `2` with `5` at the argument of an application (node 1) -- now `5`
```

The output has up to three sections, always in this order: `names`, every id
whose name changed, in id order; `documentation`, every changed doc line, in
id order; and `definitions`, in document order with removals last. Each
definition line says what happened to it — added, removed, renamed,
re-annotated, moved or edited — and an edited one is followed by a line per
`Operation`: its kind, what it did, and what the node became. Nothing in the
output depends on hash iteration order or on when it ran. A comparison with
no structural change says `no structural change`.

### textconv or the external diff?

You cannot have both on the same path at the same time — `diff.<name>.command`
replaces the whole diff machinery, so it wins over `diff.<name>.textconv`
whenever both are set.

|                               | `textconv`               | `diff-driver`               |
|-------------------------------|--------------------------|-----------------------------|
| `git diff`                    | yes                      | yes                         |
| `git log -p`, `git show`      | yes                      | only with `--ext-diff`      |
| `-U`, `--stat`, `--word-diff` | respected                | ignored                     |
| `git add -p`, `git apply`     | no                       | no                          |
| shows                         | line diff of a rendering | the typed operations        |

The `--ext-diff` row is git's rule, not ours: `git diff` runs an external
diff driver by default, but `git log`, `git show` and friends do not unless
you pass `--ext-diff`. With `diff.nothing.command` set and the flag left
off, `git log -p` falls back to `Binary files ... differ`.

Keep `textconv` configured as the default — it is the one that behaves like
git everywhere — and reach for the external driver when you want to see what
the merge engine sees:

```sh
git -c diff.nothing.command="nothing diff-driver" diff trunk HEAD -- program.n
git -c diff.nothing.command="nothing diff-driver" log -p --ext-diff -- program.n
```

or, if you want it permanently:

```sh
git config diff.nothing.command "nothing diff-driver"
git config alias.ops "log -p --ext-diff"
```

`GIT_EXTERNAL_DIFF` semantics apply: git calls the driver with seven
arguments (`path old-file old-hex old-mode new-file new-hex new-mode`), with
two more appended for a rename or copy, and with `path` alone for an
unmerged file. All three shapes are handled. A side git hands over as
`/dev/null` — a file being added or deleted — is reported as such and the
definitions are listed instead; a side that will not decode is named, not
fatal. The driver always exits 0, because a non-zero exit from an external
diff makes git print `fatal: external diff died` and abandon the whole diff.

## Caveats

- The `git config` lines are per-clone. `.gitattributes` alone does nothing
  without them; git will silently fall back to binary behaviour, which is
  exactly the "it conflicts again" symptom.
- The drivers shell out to `nothing`. If it is not on the `PATH` git sees
  (which is not always your interactive `PATH` — GUI clients differ), use an
  absolute path in the config value.
- `git diff --stat` on a `.n` file still reports `Bin 433 -> 433 bytes`.
  Stat lines are computed from the blob, before textconv.
- Merging is only as good as the structural merge. Two edits to the *same*
  node conflict, and should.

## Proving it works

`cli/tests/git_integration.rs` drives real `git` in a scratch repository on
every `cargo test`. It builds documents programmatically, makes two
branches, and checks that:

- the merge conflicts **without** the driver configured, so the test cannot
  pass vacuously;
- the same merge exits 0 **with** it, leaves no conflict markers, leaves a
  clean `git status`, and produces a file `nothing check` accepts;
- disjoint edits inside one definition, a rename against a body edit, and an
  addition against an edit all merge;
- two edits to the same node still conflict, and the report reaches the user;
- `git log -p` shows the structural rendering rather than
  `Binary files ... differ` once `textconv` is configured;
- `git log -p` still shows `Binary files ... differ` with only
  `diff.nothing.command` set, and shows the typed operations with
  `--ext-diff`.

It skips, rather than fails, when `git` is not on the `PATH`, and removes
its scratch repository afterwards.
