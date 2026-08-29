# Reference programs and the Neovim keystroke baseline

Five reference programs, chosen per Phase 0, and a sixth added with
strings in Phase B2 (§6). They are written here as ordinary pseudocode —
not `nothing`'s own syntax, since `nothing` has no text syntax at all (that
is the point of the project). This file exists to answer one question
honestly: *how many keystrokes does a competent text editor need for these
programs?* That number is the yardstick the projectional editor is measured
against from Phase 3 onward. A reference, once written, is never
recomputed; a new language feature may add a reference, and does so with
its own permanent denominator.

## Counting method

For each program:

1. Enter insert mode once (`i`) at the top of an empty buffer.
2. Type every character of the program exactly as shown, in order,
   including indentation (two spaces per nesting level) and the newline
   between each line (`Enter`), but with **no trailing newline** after the
   final line.
3. Leave insert mode once (`Esc`).

No autoindent, no bracket-matching, no snippet expansion, and no motions
(`o`, `O`, `>>`, etc.) are credited — this is the naive "just type it"
baseline, which is the fairest apples-to-apples comparison against an
editor that also has no macros yet. Count = 1 (`i`) + (number of characters
typed, counting each newline as one character) + 1 (`Esc`).

Character counts below were produced mechanically (`wc -c` on a file
containing exactly the program text with no trailing newline), not counted
by eye.

---

## 1. Factorial

```
fn factorial(n: Num): Num =
  if n == 0 then
    1
  else
    n * factorial(n - 1)
```

Content characters: 82. Keystrokes: 1 (`i`) + 82 + 1 (`Esc`) = **84**.

## 2. List map

```
fn map(f: Num -> Num, xs: List<Num>): List<Num> =
  match xs with
  | [] -> []
  | (h :: t) -> f(h) :: map(f, t)
```

Content characters: 112. Keystrokes: 1 + 112 + 1 = **114**.

## 3. Two-field record constructor plus accessor

```
type Point = { x: Num, y: Num }

fn getX(p: Point): Num =
  p.x
```

Content characters: 63. Keystrokes: 1 + 63 + 1 = **65**.

## 4. Three-case state machine

```
type State = Idle | Running | Stopped

fn transition(s: State): State =
  match s with
  | Idle -> Running
  | Running -> Stopped
  | Stopped -> Idle
```

Content characters: 149. Keystrokes: 1 + 149 + 1 = **151**.

## 5. Function with a conditional nested three levels deep

```
fn classify(x: Num): Num =
  if x > 0 then
    if x > 10 then
      if x > 100 then
        3
      else
        2
    else
      1
  else
    0
```

Content characters: 144. Keystrokes: 1 + 144 + 1 = **146**.

---

## 6. Greeting formatter (added 2026-08-29, Phase B2)

```
fn greet(name: Str, formal: Bool): Str =
  if formal then
    "Good evening, " ++ name ++ "."
  else
    "hi " ++ name ++ "!"
```

Content characters: 125. Keystrokes: 1 + 125 + 1 = **127**.

This is the sixth reference, added with strings in Phase B2 because the
first five contain no text at all and so could not measure the thing B2
adds. It is deliberately unremarkable: a function that builds a piece of
text out of two literals, a variable and a conditional. The Neovim
denominator is computed by exactly the method above and joins the table
below on the same permanent terms as the other five — computed once,
never recomputed.

---

## The baseline

These numbers are the permanent Neovim baseline. They do not get
recomputed or "improved" later — every `RESULTS.md` entry computes the
`nothing` action/keystroke count for the same programs and divides by these
numbers to get a ratio. This table is that denominator, fixed forever:

| # | Program | Neovim keystrokes |
|---|---------|-------------------:|
| 1 | Factorial | 84 |
| 2 | List map | 114 |
| 3 | Record constructor + accessor | 65 |
| 4 | Three-case state machine | 151 |
| 5 | Three-deep nested conditional | 146 |
| 6 | Greeting formatter | 127 |

Row 6 was added on 2026-08-29 with Phase B2. Adding a *new* reference is
not the same thing as recomputing an old one: rows 1–5 are untouched and
untouchable, and row 6 is fixed forever from the moment it was written.

---

## 2026-08-26 — Mapping the five references onto the Phase-1 surface

Phase 3 records an action sequence for each reference program
(`bench/fixtures/<name>.actions`, replayed by `nothing-bench` and by the
REPL harness `cargo run -p nothing-action --bin repl`). Four of the five
references use language features that Phase 1 deliberately does not have —
recursion, lists, pattern matching, records, sum types — and the spec is
explicit that they are not to be added early. So each fixture builds the
**nearest well-typed Phase-1 program**, and this section records exactly
what was substituted, so nobody later mistakes the fixture for the
reference.

The Neovim baseline above is **not** recalculated to match the
approximations. It stays fixed forever, as declared. That makes the Phase 3
ratios flattering in a way that must not be read as progress; see
`bench/RESULTS.md` for the full caveat.

| # | Program | Fixture builds | Exact? |
|---|---------|----------------|--------|
| 1 | Factorial | `λn:Num. if n == 0 then 1 else n * ⦇⦈` | no |
| 2 | List map | `λf:Num -> Num. λxs:Num * Num. (f (fst xs), f (snd xs))` | no |
| 3 | Record ctor + accessor | `let mkPoint = λx:Num. λy:Num. (x, y) in λp:Num * Num. fst p` | no |
| 4 | State machine | `λs:Num. if s == 0 then 1 else if s == 1 then 2 else 0` | no |
| 5 | Nested conditional | `λx:Num. if 0 < x then (if 10 < x then (if 100 < x then 3 else 2) else 1) else 0` | yes* |
| 6 | Greeting formatter | `λx0:Str. λx1:Bool. if x1 then "Good evening, " ++ x0 ++ "." else "hi " ++ x0 ++ "!"` | yes* |

Variables render as `x0`, `x1`, … because the fixtures rename each binder to
the default name Phase 5's name table hands a freshly constructed one (`x`
and the first unused index), and `construct-var` then refers to it by that
name. The names in this table are the binders' intended meanings; writing
them instead would change the rendered programs but not one action count, a
rename being one action whatever the name.

### 1. Factorial — the recursive call is a hole

`n * factorial(n - 1)` becomes `n * ⦇⦈`. Phase 1 has no recursion; it
arrives in Phase 6 ("Add recursion. Either a `letrec` form or a fixpoint
combinator"). An **empty hole** is precisely the right encoding: it is the
language's own way of writing "an expression belongs here and has not been
written", the program remains well-typed with it in place, and Phase 6's
first act can be to fill it. Nothing is faked. `n == 0` is kept from the
reference verbatim.

*What is lost:* the actual recursion, and therefore the ability to
evaluate. This is the only fixture that still contains a hole, and there is
a test asserting that no other one does.

### 2. List map — a pair is the list

**Superseded on 2026-08-29; kept because the numbers it produced are in
`RESULTS.md` and those entries are history.**

Phase 1 has no lists and no pattern matching, and the spec forbids adding
them ("You will want to add records, lists, strings, and polymorphism during
Phase 1. Do not."). A product type is the longest fixed-length sequence the
type grammar can express, so `map` becomes map over a two-element list
encoded as `Num * Num`: take a function, take the container, rebuild the
container with the function applied to each element. The *shape* of the
reference — the thing that makes `map` worth benchmarking — survives; the
recursion over a cons-list does not.

*What is lost:* arbitrary length, and with it the `match`/recursion that
makes the reference 114 keystrokes. The fixture is genuinely a smaller
program.

### 2. List map — a real list, eliminated by fold (2026-08-29)

Lists arrived with the second half of Phase B2, and the paragraph above
said exactly what was missing, so the fixture was rewritten rather than a
seventh reference being invented for a feature reference 2 was already
about. The fixture is now

```
λx0:Num -> Num. λx1:List Num. fold x1 nil (λx2:Num. λx3:List Num. x0 x2 :: x3)
```

which maps a function over a cons list of *any* length and rebuilds it
element by element. The denominator is untouched: 114 was computed once on
2026-08-26 from the reference text and is fixed forever, and the reference
text has not changed either. What changed is the numerator, and it went
**up** — 29 keystrokes to 44, 0.25× to 0.39× — because the fixture is now
a bigger program than it was. That direction is the point: the old ratio
was flattering `nothing` by measuring it on a smaller program than the one
Neovim was charged for.

*What is still lost:* two things, both honest.
- **Polymorphism.** `map` is `List Num -> List Num`, not `List a -> List b`;
  the language has no type variables (`spec-build.md` defers them past
  v0.1.0), so the reference's implicit generality is not expressible.
- **`match`.** The eliminator is `fold`, the only one the language has, so
  the recursion the reference writes out by hand is the recursion fold
  performs. The program means the same thing; it does not spell it the same
  way. This is also why the result type synthesises as `List ?` rather than
  `List Num` — `nil` synthesises `List ?`, and nothing in a `main : ?`
  definition pins it down. It is consistent with `List Num` everywhere it
  matters, which is what gradual typing is for.

Still marked `*` (approximate) in the tables, for those two reasons.

### 3. Record constructor + accessor — pairs, positionally

`type Point = { x: Num, y: Num }` becomes the structural product
`Num * Num`; the constructor becomes the curried `λx:Num. λy:Num. (x, y)`;
the accessor `p.x` becomes `fst p`. There is no nominal type declaration in
Phase 1 to encode, so the fixture defines the constructor with a `let` and
returns the accessor as the program's value — matching the reference, which
also defines both and calls neither.

*What is lost:* the field **names**. A product's components are positional,
so `getX` and `getY` are `fst` and `snd`, and nothing in the program records
that component 0 is called `x`. This is the substitution that gives up the
most.

### 4. State machine — numeric codes and a chain of `if`

No sum types, no `match`. `Idle`/`Running`/`Stopped` are encoded as the
codes `0`/`1`/`2` and the match becomes nested equality tests, with the
final `else` acting as the `Stopped` case.

*What is lost:* exhaustiveness. The reference's `match` can be checked for a
missing case; a chain of `if`s with a catch-all `else` cannot. Also the
distinction between a state and any other number — `transition(7)` is
well-typed here and was not in the reference.

### 5. Nested conditional — direct, modulo one operator

This one fits the surface. The single change is that `x > 0` is written
`0 < x`: Phase 1's operator set is `Add`/`Sub`/`Mul`/`Lt`/`Eq`, with no `>`,
so the operands are swapped. The three-level nesting — the entire point of
this reference — is reproduced exactly, which is why it is marked "yes*":
the program means what the reference means, with only the operator spelling
differing.

### 6. Greeting formatter — direct, modulo currying (2026-08-29)

Added with Phase B2, and the only approximation is that `nothing` has no
multi-argument functions: `greet(name, formal)` is two nested lambdas, and
`greet("Ada")("Bob")`-style application is how a caller would use it. Every
other part is exact — both string literals, both joins, the conditional,
the parameter reference in each branch. That is why it is "yes*" in the
same sense as reference 5: the program means what the reference means.

`++` is spelled `&` at the keyboard (see `KEYS.md`), which changes no
count here: `&` is one keystroke and `++` in the reference is two
characters, so if anything the fixture is charged less than the reference
is, in the fixture's favour, and by two keystrokes across the whole
program.
