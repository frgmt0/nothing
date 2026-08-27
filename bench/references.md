# Reference programs and the Neovim keystroke baseline

Five reference programs, chosen per Phase 0. They are written here as
ordinary pseudocode — not `nothing`'s own syntax, since `nothing` has no
text syntax at all (that is the point of the project). This file exists to
answer one question honestly: *how many keystrokes does a competent text
editor need for these five programs?* That number is the yardstick the
projectional editor is measured against from Phase 3 onward.

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

## The baseline

These five numbers are the permanent Neovim baseline. They do not get
recomputed or "improved" later — Phase 3's and Phase 4's `RESULTS.md`
entries compute the `nothing` action/keystroke count for the same five
programs and divide by these numbers to get a ratio. This table is that
denominator, fixed forever:

| # | Program | Neovim keystrokes |
|---|---------|-------------------:|
| 1 | Factorial | 84 |
| 2 | List map | 114 |
| 3 | Record constructor + accessor | 65 |
| 4 | Three-case state machine | 151 |
| 5 | Three-deep nested conditional | 146 |
