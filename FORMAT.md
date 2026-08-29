# FORMAT.md

The on-disk format for `nothing` documents. This document is the source of
truth for the byte layout; the `store` crate is an implementation of it, not
the other way around. A hand-rolled binary codec, not JSON — see
`DECISIONS.md`, 2026-08-28, on why `nothing` is the name that appears in the
magic bytes below.

A "document" is the four things a saved program needs: the **definitions**,
the name table, the doc table, and the action log that produced it.

**Format version 8** (this revision) adds documentation: a **doc table**
(§7.1) between the name table and the action log, and one action tag
(`SetDoc`). It is the first version since 2 to change the *shape* of the
body, which is why it is a major bump rather than a tag addition. Versions
1 through 7 still open — see §11, which is normative: a migrating reader
for every previous version ships with every format change from here on.

**Format version 7** added effects: four node tags (`Print`, `Readline`,
`CmdPure`, `CmdBind`), one `Ty` tag (`Cmd`), and four action tags, and
nothing else — the layout of §3.1 was unchanged from version 2.

**Format version 6** added variants: two node tags (`Inj`, `Match`), one
`Ty` tag (`Variant`), and six action tags.

**Format version 5** added records: two node tags (`Record`, `Field`), one
`Ty` tag (`Record`), and eight action tags.

**Format version 4** added lists: three node tags (`Nil`, `Cons`, `Fold`),
one `Ty` tag (`List`), and three action tags.

**Format version 3** added string literals: a node tag, a `Ty` tag, an `Op`
tag and an action tag.

A `Match` is **the first node that carries two kinds of identity at once**,
and they are hashed differently. Its payload is the arm list: a varint arm
count, then that many constructor ids, then that many payload-binder ids;
its children are the scrutinee followed by the arm bodies in the same order.
A constructor id is document-global — the same identity in an injection here
and an arm there — so it hashes raw, exactly like a field id (§6). An arm's
payload binder binds only that arm's body, so it is pushed on the de-Bruijn
stack around that body and never reaches the hash, exactly like a lambda's
binder. That is why the *canonical* payload a `Match` hashes is only its
first half: the count and the constructor ids, with the binder ids appended
after it on disk and omitted from the hash. `match e { c x -> x }` and
`match e { c y -> y }` are therefore the same node, and renaming a
constructor changes no hash at all.

An `Inj` carries its constructor id as a raw 16-byte payload for the same
reason, and its payload expression as its one child. There is no separate
"nullary constructor" encoding: a case that carries nothing carries `{}`,
which is a `Record` with zero fields (`DECISIONS.md`, 2026-08-29).

A record is **the first node of variable arity**, and it needs no change to
the framing to be one. Every node table entry has always written a varint
child count (§4), so a node with three children and a node with nine are
the same shape of record on disk; `Record` is the first `Exp` variant to
use that. What it does add is a payload that must line up with the child
list: the field **identities**, in order, one 16-byte uuid each, preceded
by their count. The count is redundant with the child count and is written
anyway, so that a truncated or crossed file is a decode error rather than a
silently misaligned record (`store::nodes` rejects a `Record` whose payload
count and child count disagree).

A field identity is **an identity, not a name**, exactly like a binder's:
the display name lives in the name table (§7) and is renamed there, while
the uuid in the payload is what the program means. Unlike a binder's id,
though, a field id is **hashed raw** — see §6.

A list is **structure, not a payload**. `List` is the first type
constructor in this format that is neither a base type nor a pair of
types, and the three nodes that build and consume one carry no payload at
all: a list's contents are entirely in its children, so `Cons` frames like
`Pair` does and `Fold` frames like `If` does. That is why the addition
costs three tags and no layout change — the node framing of §4 already
describes an n-ary tree, and lists are just more of it.

A string literal is **data, not a name**. Every other payload in this
format that contains user-visible text — a definition's display name, a
binder's name — lives in the name table (§7), keyed by a uuid, because
those are *identities* the program refers to and the editor renames. The
bytes inside `Str` are none of that: they are the value the program
computes with, exactly as `Num`'s `i64` is. So they are stored inline in
the node (§5), they are hashed as meaning (§6), and renaming has nothing
to say about them.

---

## 1. Conventions

- All multi-byte fixed-width integers are **little-endian**.
- Variable-length integers ("varint") use unsigned LEB128: each byte
  contributes its low 7 bits to the value, least-significant group first;
  the high bit of a byte is a continuation flag (1 = more bytes follow, 0 =
  this is the last byte). A varint that would require more than 10 bytes
  (64 bits at 7 bits/byte) is invalid.
- A UUID (`Id` or `HoleId`, both wrap `uuid::Uuid`) is written as its raw 16
  bytes, exactly `Uuid::as_bytes()` / `Uuid::from_bytes()`. These bytes are
  an opaque blob, not a number — no endianness applies to them.
- A string is a varint byte length followed by that many UTF-8 bytes. No
  terminator.
- A bool is one byte, `0x00` or `0x01`. Any other value is invalid.
- Every variable-shaped record in this format (a node table entry, a log
  entry) is prefixed with its own byte length as a varint, in addition to
  being tag-driven. This is redundant with the tag-driven shape but is
  there deliberately: a reader can skip a record it doesn't understand (a
  future tag) without knowing that tag's internal shape.

## 2. Hash algorithm

**blake3**, 256-bit output, used exactly as `blake3::hash` /
`blake3::Hasher`. Chosen over sha2 for speed with no format-relevant
downside — the hash is an internal content-address and integrity check, not
a cryptographic commitment exposed to adversaries. A `Digest` is always 32
raw bytes.

## 3. File header

```
offset  size  field
0       4     magic:   4E 54 48 47   ("NTHG", ASCII)
4       1     version_major:  0x08
5       1     version_minor:  0x00
6       1     kind:    0x01 (Document — the only kind this version defines)
```

`NTHG` is a deliberate four-letter shorthand for `nothing`, the project's
real name (see `DECISIONS.md`, "`nothing` is the real name, not a
placeholder"). `version_major` changes on a breaking layout change;
`version_minor` on an additive, backward-compatible one. A reader must
reject a file whose `version_major` it does not recognise, and may ignore
`version_minor`. `kind` is reserved for future file kinds (e.g. a
node-table-only export); this version of the format only ever writes
`0x01`.

Everything after the 7-byte header is, in order: the **definition list**
(§3.1), the **name table** (§7), the **doc table** (§7.1), the **action
log** (§8). A `version_major` of `0x01` selects the version-1 layout
instead (**node table**, **root index**, name table, action log) and is
decoded through the migrating reader in §11; a `version_major` of `0x02`
through `0x07` selects the same layout as this one *without* the doc
table, and is read by the same body decoder minus that section.

## 3.1 The definition list

A document is an *ordered, non-empty* set of top-level definitions. A
definition is four things:

- an **id**, drawn from the same `Id` (uuid) space as every binder in the
  language — a definition is a binder, and its identity is a uuid nothing
  ever types;
- a **display name**, which is *not* stored here: it lives in the name
  table (§7) keyed by the definition's id, exactly like a lambda's binder
  name. Renaming a definition is a name-table write and nothing else;
- a **type annotation**, a `Ty` (§5.1), which may be `Hole` (`?`);
- a **body**, an `Exp`, encoded as a node table exactly as in version 1.

A body refers to another definition with `Var(id)` — the ordinary variable
form — because the top level *is* a mutually recursive binding group and
its members are variables like any other (see `DECISIONS.md`, 2026-08-28,
"Definition references are `Var`"). The consequence for this format is
that no new node tag is needed: a cross-definition reference is a `Var`
node whose `Id` happens to name a definition, and a self-reference — the
way recursion is written now — is the same thing.

```
definition_list:
  def_count: varint                 (>= 1; a document with no definitions is invalid)
  def_count × definition

definition:
  def_len:     varint               (byte length of everything below, for skippability)
  id:          16 bytes
  ann:         Ty (§5.1)
  node_table:  as §4
  root_index:  varint               (always node_count - 1)
```

Order is significant and preserved: it is the order the editor lists
definitions in, and the merge engine treats a change of order as a move
(`merge::document`). Order has no effect on typing or evaluation — every
definition is in scope in every body, including its own, which is what
makes mutual recursion expressible without a keyword.

**Typing a document.** The definition context is `{id ↦ ann}` for every
definition in the list; each body is checked in that context extended by
its own local binders. So a definition's annotation is what its *callers*
see — an unannotated (`?`) definition is usable everywhere, gradually, and
an annotated one is checked precisely. A body that mentions an id which is
not a definition and not a local binder does not synthesise, so a
**dangling reference makes the document ill-typed**: "well-typed" implies
"every reference resolves", and the editor never writes such a file (see
`DECISIONS.md`, 2026-08-28, "Deleting a definition rewrites its
references to empty holes").

**`main`.** `nothing run` evaluates the definition displayed as `main`.
That is a name-table lookup, not a structural property: any definition can
be `main` and renaming is all it takes. A document with no `main` is a
perfectly good document; `nothing run` refuses it and lists what is
there.

## 4. Content-addressed node table

An expression is a tree (not a DAG — `Exp`'s children are `Box`, never
shared), so the table is a flat list of nodes in the order a bottom-up
(post-order) walk visits them: every node's children appear in the table
*before* the node itself, at lower indices, so a child is always referenced
by an already-emitted table index. The root of the expression is therefore
always the **last** entry in the table. This version of the format never
deduplicates two structurally-identical subtrees that occur at different
positions — it always appends a fresh entry per occurrence. (A future
revision could reference an earlier entry by matching hash instead of
appending a duplicate; that's out of scope for Phase 7 and is left for
whichever of Phase 8/9 needs it.)

```
node_table:
  node_count: varint
  node_count × node_entry

node_entry:
  hash:          32 bytes            (blake3 digest, see §6)
  tag:           1 byte              (see §5)
  payload_len:   varint
  payload:       payload_len bytes   (tag-dependent, see §5)
  children_count: varint
  children:      children_count × varint   (indices into this table, each < this entry's own index)

root_index: varint                  (always node_count - 1; written explicitly for a reader that doesn't want to assume it)
```

`hash` is the node's **content hash** as defined in §6 — it is a function
of tree *shape*, not of the literal bytes in `payload`, so two nodes with
different `payload` can have the same `hash` (e.g. two `Var` nodes bound at
the same de Bruijn depth but referencing different concrete `Id`s). A
reader that cares about content-addressing (matching subtrees across
documents, future incremental evaluation) reads `hash`; a reader that just
wants the expression back reads `tag`, `payload`, and `children`, and can
ignore `hash` entirely except as an integrity check (§6, "verification on
decode").

### 5. Node tags and payloads

| tag | `Exp` variant | payload | children |
|-----|---------------|---------|----------|
| 0 | `Var(id)` | 16 bytes: `id` | none |
| 1 | `Lam(id, ty, body)` | 16 bytes: `id`, then a `Ty` (§5.1) | `[body]` |
| 2 | `Ap(f, a)` | empty | `[f, a]` |
| 3 | `Num(n)` | 8 bytes: `n` as `i64` LE | none |
| 4 | `Bool(b)` | 1 byte: `b` | none |
| 5 | `BinOp(op, l, r)` | 1 byte: `op` (§5.2) | `[l, r]` |
| 6 | `If(c, t, e)` | empty | `[c, t, e]` |
| 7 | `Let(id, bound, body)` | 16 bytes: `id` | `[bound, body]` |
| 8 | `Pair(l, r)` | empty | `[l, r]` |
| 9 | `Proj(side, e)` | 1 byte: `side` (§5.3) | `[e]` |
| 10 | `EmptyHole(h)` | 16 bytes: `h` | none |
| 11 | `NonEmptyHole(h, e)` | 16 bytes: `h` | `[e]` |
| 12 | `Str(s)` | a string (§1): varint byte length, then that many UTF-8 bytes | none |
| 13 | `Nil` | empty | none |
| 14 | `Cons(head, tail)` | empty | `[head, tail]` |
| 15 | `Fold(list, init, step)` | empty | `[list, init, step]` |
| 16 | `Record(fields)` | a varint field count `n`, then `n` × 16 bytes: the field `id`s in order | the `n` field values, in the same order |
| 17 | `Field(subject, id)` | 16 bytes: `id` | `[subject]` |
| 18 | `Inj(ctor, payload)` | 16 bytes: `ctor` | `[payload]` |
| 19 | `Match(scrutinee, arms)` | a varint arm count `n`, then `n` × 16 bytes: the constructor `id`s in order, then `n` × 16 bytes: the arm payload-binder `id`s in the same order | the scrutinee, then the `n` arm bodies in the same order |
| 20 | `Print(text)` | empty | `[text]` |
| 21 | `Readline` | empty | none |
| 22 | `CmdPure(value)` | empty | `[value]` |
| 23 | `CmdBind(command, id, body)` | 16 bytes: `id` | `[command, body]` |

This table is exhaustive over the twenty-four `Exp` variants as of Phase
B3's effect feature. Tags 0–11 are unchanged from version 2, tag `12` is the
version-3 addition, tags `13`–`15` are the version-4 additions, tags
`16`–`17` are the version-5 additions, tags `18`–`19` are the version-6
additions, and tags `20`–`23` are the version-7 additions.
A `CmdBind`'s binder binds only its *body*, so it is pushed on the de-Bruijn
stack around the second child and never reaches the hash — exactly like a
`Let`'s binder (§6), which is the node it is shaped after.
If a twenty-fifth variant is added to `core::exp::Exp`, it gets the next tag
(`24`) and a row here; a reader must treat an unrecognised tag as a hard
decode error, not skip it silently (the `payload_len`/`children_count`
framing lets it skip the *bytes*, but it cannot reconstruct an `Exp` it has
no variant for).

Tags `16` and `19` are the rows whose payload length depends on their child
count. A decoder must read the varint first and check it against the entry's
`children_count` — `n` against `children_count` for `Record`, `n + 1` for
`Match`, whose children are one scrutinee plus one body per arm; a mismatch
is a decode error (`MissingChild`), never a truncation to the shorter of the
two.

#### 5.1 `Ty` encoding

Recursive, self-delimiting (no length prefix needed — the shape is
determined entirely by the tag byte):

| tag | `Ty` variant | further bytes |
|-----|--------------|----------------|
| 0 | `Num` | none |
| 1 | `Bool` | none |
| 2 | `Arrow(a, b)` | `Ty(a)` then `Ty(b)` |
| 3 | `Prod(a, b)` | `Ty(a)` then `Ty(b)` |
| 4 | `Hole` | none |
| 5 | `Str` | none |
| 6 | `List(a)` | `Ty(a)` |
| 7 | `Record(fields)` | a varint field count `n`, then `n` × (16 bytes: the field `id`, then `Ty` of that field) |
| 8 | `Variant(ctors)` | a varint constructor count `n`, then `n` × (16 bytes: the constructor `id`, then `Ty` of that case's payload) |
| 9 | `Cmd(a)` | `Ty(a)` |

#### 5.2 `Op` encoding (1 byte)

`Add = 0`, `Sub = 1`, `Mul = 2`, `Lt = 3`, `Eq = 4`, `Concat = 5`.

#### 5.3 `Side` encoding (1 byte)

`L = 0`, `R = 1`.

## 6. Content hash (alpha-equivalence, excludes names)

The content hash of a node is `blake3(tag_byte || canonical_payload ||
child_hash_1 || child_hash_2 || ...)`, computed bottom-up. This is the
value stored as `hash` in every node table entry (§4), and
`content_hash(exp)` (the hash of a whole expression) is simply the hash of
its root node — the `hash` field of the table's last entry.

The whole point of this hash is the property Phase 7 asks for: **two
alpha-equivalent expressions hash identically**, where alpha-equivalent
means "same shape, binders used in the same pattern, but different `Id`s
and different display names." Concretely, this affects two things that
carry identity but no meaning:

- **Binder/variable `Id`s.** A `Var` reference is hashed as a **de Bruijn
  index** relative to an in-progress stack of enclosing binders (`Lam`,
  `Let`), not as its literal UUID. Walking into a `Lam` or `Let` pushes
  that binder's `Id` onto the stack (a `Let`'s *bound* expression is hashed
  in the *outer* scope — this language's `let` is non-recursive, matching
  `action::act::ctx_and_expected_ty_at`, which computes the bound
  expression's context before extending it); walking back out pops it. A
  `Var(id)` whose `id` is on the stack is canonicalised to `0x00` followed
  by a varint: the distance from the top of the stack (`0` = the innermost
  enclosing binder). A `Var(id)` whose `id` is *not* on the stack (a free
  variable — reachable when hashing a subterm in isolation, not the whole
  document) is canonicalised to `0x01` followed by its literal 16 bytes,
  because there is no shape-only description of "which outer thing this
  refers to" — for a free variable, the `Id` *is* the meaning. `Lam` and
  `Let` nodes themselves never put their binder's `Id` into the canonical
  payload — the position on the stack is the only thing that matters, which
  is exactly de Bruijn canonicalisation.
- **`HoleId`s.** Excluded from the canonical payload entirely, for both
  `EmptyHole` and `NonEmptyHole` — a hole's identity is a session-local
  bookkeeping label (which fresh-ID stream produced it), never something
  two structurally-identical expressions are expected to share, and unlike
  a `Var` there is no cross-reference elsewhere in the tree that a hole's
  `Id` needs to stay consistent with. So `EmptyHole` canonicalises to just
  its tag byte with an empty payload and no children, and `NonEmptyHole`
  canonicalises to its tag byte, an empty payload, and its wrapped
  expression's hash as its one child.

A `Ty` annotation (on `Lam`) *is* included in the canonical payload, in
full, via the §5.1 encoding — `λx:Num.x` and `λx:Bool.x` are genuinely
different programs, and nothing about that distinction is identity rather
than meaning. That now includes a `Ty::Record`'s field ids, for the same
reason the next paragraph gives.

**Field ids are hashed raw, and that is deliberate.** A field id looks like
a binder id and is not one. A binder's id is meaningless outside the term
that binds it, which is why de Bruijn canonicalisation can erase it; a
field's id has no binder at all — it is a free, document-wide identity that
a `Record` in one definition and a `Field` in another use to mean *the same
field*. There is no stack to measure it against and no shape-only
description of "which field this is", so `Record` writes the ids into its
canonical payload verbatim and `Field` writes the one it projects. The
consequence is exactly the one the design wants: `{x = 1}` and `{y = 1}`
have **different** content hashes even though they are the same shape,
because they are different records; and renaming the field `x` to `y`
changes **no** hash at all, because a display name is not in any payload.
Move detection in `merge` inherits both halves of that: a record that was
renamed is still recognisably the same subtree, and a record whose field
set changed is not.

Everything else (`Num`'s `i64`, `Bool`'s bit, `Str`'s UTF-8 bytes,
`BinOp`/`Proj`'s `Op`/`Side` tag) is meaning, not identity, and is hashed
as its literal bytes. A `Str` node in particular hashes its **text**, with
its varint length prefix, into the canonical payload: `"a" ++ "b"` and
`"a" ++ "c"` are different programs and must have different content
hashes, or the incremental evaluator would reuse a cached result across an
edit that changed the answer — see the
"canonical payload" column implied by §5's payload column, minus the `Id`
and `HoleId` fields called out above.

**Verification on decode:** after decoding the full node table back into an
`Exp`, a reader recomputes `content_hash` on the reconstructed expression
(the same bottom-up procedure, run fresh) and compares it to the `hash`
stored on the table's last (root) entry. A mismatch means the bytes were
corrupted somewhere in the tree and decoding must fail rather than return a
silently-wrong program. This is a whole-tree check, not a per-node one —
because the hash is a Merkle hash (each node's hash folds in its children's
hashes), any corruption in `payload` or `children` anywhere in the tree
changes the reconstructed shape at that point, which propagates up to
change the root hash too.

## 7. Name table

The in-memory `NameTable` (`core::names`) is a layered stack of overlays —
a base table plus per-user overlays that shadow it, used so two
simultaneous editors can each see their own preferred name for the same
`Id` without touching each other's layer. **The persisted format flattens
this stack to a single layer before writing** (`NameTable::flatten`, the
already-existing method): the file stores the *resolved* view — the name
each `Id` actually displays as, with the layering already applied — not
the overlay structure itself.

This is a deliberate choice, not an oversight: overlays exist to let
several live sessions disagree about display names while editing the same
document *concurrently*; a saved document is a single snapshot, and by the
time it's saved, that disagreement has already been resolved into whatever
the saving session was actually looking at. Persisting the full overlay
stack would mean persisting "whose name table is this," which the format
has no other concept of (there is no per-user identity anywhere else in
the file). A session that opens a saved document and wants to layer its
own overlay on top does so in memory, the same way it would build any
other overlay, via `NameTable::overlay` on the flattened table the file
handed it.

```
name_table:
  entry_count: varint
  entry_count × name_entry

name_entry:
  id:   16 bytes
  name: string (§1)
```

Entries are written in ascending order of the `Id`'s raw `u128` value
(`Id::as_u128`). This is not meaningful to a reader — order doesn't affect
the resulting table — but it makes two independent encodings of the same
`NameTable` byte-identical regardless of the source `im::HashMap`'s
internal iteration order, which is what the round-trip test in §9 needs.

## 7.1 Doc table

A definition may carry one line of documentation. Docs are **metadata, not
AST**: a doc line is not a node, it has no type, it does not participate in
content addressing (§6), and changing one cannot make a program ill-typed.
It is stored exactly the way a display name is — a table keyed by the
definition's `Id`, beside the tree rather than inside it — for exactly the
same reason: it is a fact *about* a binder, not a part of the term it binds
(see `DECISIONS.md`, 2026-08-29, "Doc lines are metadata beside the tree").

`DocTable` (`core::docs`) is layered like `NameTable`, and is flattened the
same way before writing, for the same reasons (§7). An id with an empty doc
line has no entry: setting a doc line to the empty string removes it, so
there is exactly one encoding of "undocumented".

```
doc_table:
  entry_count: varint
  entry_count × doc_entry

doc_entry:
  id:   16 bytes
  line: string (§1)
```

Entries are written in ascending order of the `Id`'s raw `u128` value, for
the byte-identical round-trip reason given in §7.

A document written by a session that has the standard library in scope
stores **only its own layer** (`DocTable::own`, `NameTable::own`): the
prelude's names and doc lines belong to the prelude shipped in the binary,
not to the file. A file that borrows `min` from the standard library
contains a `Var` node with `min`'s id and nothing else about `min` — no
name entry, no doc entry, no copy of its body.

## 8. Action log

Each entry in `action::log::ActionLog` is a `(Action, timestamp: u64,
author: AuthorId(u64))` triple, in append order (index order in the log is
significant and preserved — it *is* the log).

```
action_log:
  entry_count: varint
  entry_count × log_entry

log_entry:
  entry_len: varint                 (byte length of everything below, for skippability)
  timestamp: 8 bytes, u64 LE (milliseconds, see action::log::now_millis)
  author:    8 bytes, u64 LE (AuthorId's inner value)
  action:    tag (1 byte) + tag-dependent payload, see below
```

| tag | `Action` variant | payload |
|-----|-------------------|---------|
| 0 | `MoveChild(n)` | varint: `n` |
| 1 | `MoveParent` | empty |
| 2 | `MoveNextSibling` | empty |
| 3 | `MovePrevSibling` | empty |
| 4 | `Delete` | empty |
| 5 | `ConstructNum(n)` | 8 bytes: `n` as `i64` LE |
| 6 | `ConstructBool(b)` | 1 byte |
| 7 | `ConstructVar(id)` | 16 bytes |
| 8 | `ConstructLam` | empty |
| 9 | `ConstructAp` | empty |
| 10 | `ConstructBinOp(op)` | 1 byte (§5.2) |
| 11 | `ConstructIf` | empty |
| 12 | `ConstructLet` | empty |
| 13 | `ConstructPair` | empty |
| 14 | `ConstructProj(side)` | 1 byte (§5.3) |
| 15 | `ConstructNonEmptyHole` | empty |
| 16 | `SetAnn(ty)` | `Ty` (§5.1) |
| 17 | `SetBinderId(id)` | 16 bytes |
| 18 | `Rename(id, name)` | 16 bytes, then a string (§1) |
| 19 | `Finish` | empty |
| 20 | `CreateDefinition` | empty |
| 21 | `DeleteDefinition` | empty |
| 22 | `SetDefAnn(ty)` | `Ty` (§5.1) |
| 23 | `MoveNextDef` | empty |
| 24 | `MovePrevDef` | empty |
| 25 | `MoveToDef(id)` | 16 bytes |
| 26 | `ConstructStr(s)` | a string (§1) |
| 27 | `ConstructNil` | empty |
| 28 | `ConstructCons` | empty |
| 29 | `ConstructFold` | empty |
| 30 | `ConstructRecord` | empty |
| 31 | `ConstructField(id)` | 16 bytes |
| 32 | `AddField` | empty |
| 33 | `RemoveField` | empty |
| 34 | `MoveFieldPrev` | empty |
| 35 | `MoveFieldNext` | empty |
| 36 | `SetField(id)` | 16 bytes |
| 37 | `SetFieldId(id)` | 16 bytes |
| 38 | `ConstructInj` | empty |
| 39 | `ConstructMatch` | empty |
| 40 | `AddArm` | empty |
| 41 | `RemoveArm` | empty |
| 42 | `SetConstructor(id)` | 16 bytes |
| 43 | `SetArmBinderId(id)` | 16 bytes |
| 44 | `ConstructPrint` | empty |
| 45 | `ConstructReadline` | empty |
| 46 | `ConstructPure` | empty |
| 47 | `ConstructBind` | empty |
| 48 | `SetDoc(id, string)` | 16 bytes + string (§1) |

This table is exhaustive over the forty-nine `Action` variants as of
format version 8. Tags 0–19 are unchanged from version 1, tags 0–25 from
version 2, tags 0–26 from version 3, tags 0–29 from version 4, tags
0–37 from version 5, tags 0–43 from version 6 and tags 0–47 from version
7, so an older log decodes under the current reader without translation.
The same rule as §5 applies to a future fiftieth variant: next tag, new
row, hard decode error on an unrecognised tag rather than a silent skip.

`SetDoc` carries the id it documents rather than documenting whatever the
cursor is in, for the same reason `Rename` carries one: a doc line is
metadata keyed by an `Id`, and the action that writes one says which. Like
`Rename` it is total — it cannot fail, it cannot change a type, and it
produces exactly one log entry. Setting an empty line is how a doc is
cleared.

`AddArm` and `RemoveArm` carry no payload for the reason `AddField` and
`RemoveField` carry none: the identities they mint come from the
document's fresh-id stream, which the log replays deterministically, and
the identities they remove are the ones the cursor is already on.
`SetConstructor` and `SetArmBinderId` do carry an id, because they *point
at* an identity rather than minting one — exactly the split between
`AddField` and `SetFieldId`.

Renaming a *field* also uses tag 18, `Rename`, for the same reason
renaming a definition does: a field's display name is a name-table entry
keyed by an `Id`, and there is exactly one action in this format that
writes one. `SetField` and `SetFieldId` are not renames — they re-aim a
projection at a different field and re-identify a record's field
respectively, and both change the program, not its vocabulary.

Note that renaming a *definition* uses tag 18, `Rename` — the same action
that renames a lambda binder, because a definition's display name lives in
the same name table keyed by the same kind of `Id`. There is no separate
rename-definition action, and there is deliberately no
`DeleteDefinition(id)` payload: the action deletes the definition the
cursor is in, and its effect on other definitions (rewriting references to
the deleted id into fresh empty holes) is derived at replay time from the
document the log has built so far, which keeps replay deterministic
without storing the rewrite.

`Id`s inside actions (`ConstructVar`, `SetBinderId`, `Rename`,
`ConstructField`, `SetField`, `SetFieldId`) are written
literally, not de-Bruijn-canonicalised — the action log is a replayable
history of concrete edits against concrete `Id`s (some freshly generated at
apply time by `action::act::Fresh`), not a content-addressed structure, and
replaying it must reproduce the exact same `Id`s the original session used.

## 8.1 The `Fresh` stream and replay

Actions that need new `Id`s (`ConstructLam`, `ConstructLet`,
`CreateDefinition`, and the empty holes minted by `Delete`,
`DeleteDefinition` and the auto-wrapping constructions) draw them from
`action::act::Fresh`, a deterministic uuid stream seeded per session.
Replaying a log from the same starting document with a fresh `Fresh`
reproduces the same ids, which is what makes the log a faithful history
rather than an approximation.

## 9. What "round-trips byte-identically" means

`serialise(document) == encode(decode(serialise(document)))`. Concretely,
the test in `store` is: take each of the ten programs in
`core::examples`, wrap each as a `Document` (with a name table and a short
action log), encode it, decode it, encode the result again, and assert the
two encoded byte vectors are `==`. This is the whole reason §7 fixes the
name table's entry order and §4 fixes the node table's post-order walk —
without a canonical order, two semantically-identical documents could
legally encode to different bytes, which would fail this test even though
nothing was actually wrong. §7.1 fixes the doc table's entry order for the
same reason, and the standard library is the largest document this holds
for: `stdlib/std.n` is asserted to re-encode byte-identically from the
replay of its own action log.

## 10. Debug JSON export

`store::json::to_debug_json` renders a `Document` as human-readable JSON —
for eyeballing a file's contents in a terminal or a bug report, nothing
else. It is export-only: there is no JSON importer, and the binary format
in §§3–8 is the only format this crate reads. Its shape is not specified
byte-for-byte here the way the binary format is; it is not a wire format,
and Phase 7 only asks that it exist.


## 11. Migration from versions 1, 2, 3, 4, 5, 6 and 7

Format stability starts at version 1, so every earlier version's files
open. The reader dispatches on `version_major` in the header (§3):

- `0x01` → `store::v1::decode_document_v1`, the version-1 layout: node
  table, root index, name table, action log. It is a complete, separate
  reader kept for exactly this purpose; it is never used to *write*
  except by the migration test's fixture builder
  (`store::v1::encode_document_v1`, which exists so the migration path is
  exercised against real v1 bytes rather than a hypothesis).
- `0x02` → the version-2 body layout, which is the §3.1 layout: the same
  `decode_defs` the current version uses reads it, because version 3 added
  only tags, never a shape. A version-2 file simply cannot contain any of
  them. `store::v2::encode_document_v2` exists for the same reason
  `encode_document_v1` does — the committed v2 fixtures under
  `store/fixtures/v2/` are generated by it, so the version-3 reader is
  exercised against real version-2 bytes rather than a hypothesis.
- `0x03` → the same version-2/§3.1 body layout, minus the tags added in
  version 4. `store::v3::encode_document_v3` exists for the same reason
  `encode_document_v2` does — the committed v3 fixtures under
  `store/fixtures/v3/` were generated by the *unmodified* version-3 encoder
  before the list tags were written, so the version-4 reader is exercised
  against bytes an older build really produced. One of them,
  `bench_greeting.v3.nothing`, carries string literals, which are the thing
  version 3 could express and version 2 could not.
- `0x04` → the same version-2/§3.1 body layout, minus the tags added in
  version 5. `store::v4::encode_document_v4` exists for the same reason
  `encode_document_v3` does — the committed v4 fixtures under
  `store/fixtures/v4/` contain nothing a version-4 build could not have
  written — asserted, not assumed, by
  `store/tests/migration.rs::no_version_four_artifact_contains_a_version_five_form`
  — so the version-5 reader is exercised against real older bytes. Two of
  them, `bench_list_map.v4.nothing` and `list_sum.v4.nothing`, carry list
  nodes, which are the thing version 4 could express and version 3 could
  not.
- `0x05` → the same version-2/§3.1 body layout, minus the tags added in
  version 6. `store::v5::encode_document_v5` exists for the same reason
  `encode_document_v4` does — the committed v5 fixtures under
  `store/fixtures/v5/` contain nothing a version-5 build could not have
  written — asserted, not assumed, by
  `store/tests/migration.rs::no_version_five_artifact_contains_a_version_six_form`
  — so the version-6 reader is exercised against real older bytes. Several
  of them carry record nodes, which are the thing version 5 could express
  and version 4 could not, asserted by
  `a_version_five_artifact_still_carries_the_records_that_made_it_version_five`.
- `0x06` → the same version-2/§3.1 body layout, minus the tags added in
  version 7. `store::v6::encode_document_v6` exists for the same reason
  `encode_document_v5` does — the committed v6 fixtures under
  `store/fixtures/v6/` contain nothing a version-6 build could not have
  written — asserted, not assumed, by
  `store/tests/migration.rs::no_version_six_artifact_contains_a_version_seven_form`
  — so the version-7 reader is exercised against real older bytes. Several
  of them carry variant nodes, which are the thing version 6 could express
  and version 5 could not, asserted by
  `a_version_six_artifact_still_carries_the_variants_that_made_it_version_six`.
- `0x07` → the same version-2/§3.1 body layout, without the doc table
  version 8 added. `store::v7::encode_document_v7` exists for the same
  reason `encode_document_v6` does — the committed v7 fixtures under
  `store/fixtures/v7/` were generated by the *unmodified* version-7 encoder
  before the doc table was written, asserted rather than assumed by
  `store/tests/migration.rs::no_version_seven_artifact_carries_a_doc_line`
   — so the version-8 reader is exercised against bytes an older build
  really produced. Several of them carry command nodes, which are the thing
  version 7 could express and version 6 could not, asserted by
  `a_version_seven_artifact_still_carries_the_commands_that_made_it_version_seven`.
- `0x08` → the current layout: §3.1, name table, doc table, action log.
- anything else → `DecodeError::UnsupportedVersion`.

A version-1 file is a single expression. It becomes a document with
exactly one definition:

| field | value |
|-------|-------|
| id | `6d61696e-0000-0000-0000-000000000000` (`"main"` in ASCII, then zeros) |
| annotation | `Ty::Hole` |
| body | the version-1 root expression, unchanged |
| display name | `main`, written into the migrated document's name table |

The id is a **fixed constant**, `core::doc::MAIN_ID`, not a freshly
generated uuid. That matters for two reasons. Re-migrating the same file
twice produces the same document, so migration is idempotent and content
hashes are stable. And two people who migrate the *same* v1 file
independently get the same definition id, so the merge engine matches
their definitions instead of seeing an add-and-delete pair.

Version 2 → 3, version 3 → 4, version 4 → 5, version 5 → 6 and version
6 → 7 need no rewriting at all: every earlier node, type, operator and action tag means
the same thing under the current version, and each new version only added
tags the older one never wrote. The migration is the header bump, and it
happens on save.

Version 7 → 8 is the first migration since 1 → 2 that changes a shape
rather than adding a tag, and it is still not a rewrite: a version-7 file
has no doc table, and a document decoded from one gets an **empty** one.
Undocumented is the honest reading of a file written before doc lines
existed, and it is representable — the doc table's empty-string rule (§7.1)
means "no entry" and "no doc" are the same state. Saving writes version 8
with whatever doc lines the session has since set.

Migration is read-only and lossless in the direction that matters: the
name table and the action log carry across untouched (§8's tags 0–19 are
version-stable across all eight versions, 20–25 across the last seven,
26–29 across the last five, 30–37 across the last four, 38–43 across the
last three, 44–47 across the last two, and 48 is new in version 8),
and the expression is byte-identical after re-encoding as the single
definition's node table. There is no downgrading writer; saving a migrated
file writes the current version, and the older bytes on disk are only
replaced when the user saves.
