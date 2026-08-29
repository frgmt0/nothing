# Five programs

Five small, complete `nothing` programs. Each one is under thirty definitions, is well typed against the standard library, and has no holes left in it. Each is shipped twice: as `<name>.actions`, the action script that builds it, and as `<name>.n`, the document that script produces.

The `.n` file is not written by hand. It is what you get by replaying the `.actions` script through the real action calculus, starting from the empty program with the standard library in scope, and encoding the result. `cli/tests/examples.rs` asserts that replay reproduces the committed bytes exactly, so the scripts and the documents cannot drift apart. To rebuild the documents after editing a script:

```
cargo test -p nothing-cli --test examples -- --ignored
```

The scripts carry `#` comments explaining what each program is and why it is written the way it is. The action verbs they use are documented by `script::HELP` in `action/src/script.rs`, and `nothing edit` is the same set of actions with keys on them.

Two limits of the language shape all five programs, and are worth stating once here rather than five times below. There is no division, only `+ - * < ==` and `++`, so anything that needs a ratio either picks a smaller output unit or writes its own division. And there is nothing anywhere in the language or the standard library that turns a number into a string, so a program that computes numbers either hands them back for `nothing run` to print, or maps them to text through comparisons.

Every output quoted below was produced by running the committed `.n` file. `cli/tests/examples.rs` asserts each one on every test run.

## `unit_converter`

Eleven definitions of pure arithmetic: Celsius to Fahrenheit, kilometres to metres, miles to kilometres, hours to minutes to seconds, kilograms to grams. Every conversion is a multiplication, because there is no division. Celsius to Fahrenheit is `c * 9 / 5 + 32`, which needs one, so `c_to_f_tenths` computes `c * 18 + 320` and answers in tenths of a degree instead; `miles_to_km_hundredths` does the same with the 1.609 kilometres in a mile. Picking the smaller output unit is how you write exact arithmetic when the only numbers are whole ones. `main` has type `List Num` and no command in it, so `nothing run` evaluates it and prints the value: the four sample temperatures run through `map c_to_f_tenths`, then `append`ed to four one-off conversions. It demonstrates definitions calling definitions, `map` and `append` from the standard library, and the fact that a program with a numeric answer does not need effects at all.

```
$ nothing run examples/unit_converter.n
320 :: 680 :: 986 :: 2120 :: 42000 :: 4186 :: 10800 :: 5000 :: nil
```

Reading the list: 0, 20, 37 and 100 degrees Celsius are 32.0, 68.0, 98.6 and 212.0 degrees Fahrenheit; then a 42 kilometre marathon in metres, a 26 mile marathon in hundredths of a kilometre, three hours in seconds, and five kilograms in grams.

## `grade_calculator`

Eleven definitions turning five marks into a report. It exists to show the two workarounds the language asks for. Because there is no division, `div` is defined here by repeated subtraction, recursing on itself the way `range` does in the standard library, with a `b < 1` guard so a zero divisor stops rather than running forever; `average` is `div (sum scores) (length scores)`. Because there is no number-to-string conversion, `letter` gets from a number to text by comparing it against 90, 80, 70 and 60 with `lte` and choosing one of five string literals. That threshold move is the general answer for any program that has to say something in words about a number. The rest is standard library: `sum`, `length`, `map`, `all`, `join`, `print_all`. `main` has a command type, so `nothing run` performs it and the three lines below are what it wrote.

```
$ nothing run examples/grade_calculator.n
class grade: B
grades: A, C, B, D, A
everyone passed
```

## `state_machine`

A turnstile with three states, driven by words read from standard input. The states are a variant: `Locked`, `Unlocked`, `Jammed`. A variant type is a list of constructor identities and identities are never spelled in an annotation, so there is no way to write the type down; the constructors come into being as match arms instead. `label` is the first definition in the file for that reason, and `add-arm` there is what mints all three identities. `step` opens a second match and aims its arms at the same three with `set-constructor`, and its arm bodies inject back into them. For the same reason the starting state is written inline in `main` rather than being a definition of its own: every definition in the file carries a real type annotation, and `` `Locked {} `` has no annotation anyone can write. `main` has a command type, so `nothing run` performs it: `readline` takes a word, `let` carries the state to the next turn, `print` says where the machine got to, and `bind` sequences the lot. It reads exactly three words and prints three lines.

```
$ printf 'coin\nkick\nfix\n' | nothing run examples/state_machine.n
the turnstile is unlocked
the turnstile is jammed
the turnstile is locked
```

## `text_game_turn`

One turn of a text adventure, in eleven definitions. The rooms are records, and the point of the example is where their fields come from: `room` is the only definition that writes a record literal, so `name`, `look` and `exits` are minted exactly once there, and `look_of` and `exits_of` read them back by name from other definitions. That is what field identities buy over a nest of pairs. Because a record type cannot be spelled in an annotation either, there is no `observatory` definition; the rooms live in `rooms`, whose type `List ?` can be written, and `find_room` picks one out with a `fold`, which is the only list eliminator the language has. `respond` is a chain of string comparisons, because `readline` hands back a `Str` and comparing it is the only thing a program can do with one. `main` prints a banner, then reads two words and answers each.

```
$ printf 'look\nnorth\n' | nothing run examples/text_game_turn.n
The observatory. Try look, exits, north or inventory.
A cold glass dome. The telescope is pointed at nothing in particular.
Shelves of star charts, most of them wrong.
```

## `decision_table`

A lending rule set held as data rather than as code, in eight definitions. Each rule is a record with three condition fields and one outcome field, and the four rules are a `List ?`; adding a rule means adding a line to `rules`, and neither `matches` nor `decide` changes. The rules are built by calling one constructor function, `rule`, rather than by writing four record literals, because a record type is a list of field identities and two literals with the same field names would still be two different sets of identities. `applicant` does the same job for the other record. `matches` reads the condition fields off a rule and the two fields off an applicant and combines them with `and`, `between` and `lte` from the standard library. `decide` folds the rules with the default outcome as the starting value, and because `fold` runs from the front of the list, the first matching rule is the one whose outcome survives: first match wins, which is how a decision table is normally read. `main` maps `decide` over five applicants and prints the results.

```
$ nothing run examples/decision_table.n
young saver
standard
referred
senior
declined
```

The five lines are the five applicants in `applicants`: 22 years old with a score of 640 meets the young saver rule; 40 with 720 meets the standard rule; 40 with 650 misses the 700 floor and falls through to referred; 70 with 300 meets the senior rule, which has no floor; and 15 with 800 matches no rule at all, so it gets the fold's starting value.

## What each one is for

| example | definitions | shows |
| --- | ---: | --- |
| `unit_converter` | 11 | arithmetic without division, `map`, `append`, a value-typed `main` |
| `grade_calculator` | 11 | recursion, self-written division, thresholds instead of number formatting |
| `state_machine` | 5 | variants, `add-arm`, `set-constructor`, two matches on one constructor set, effects |
| `text_game_turn` | 11 | records, field identities read across definitions, `fold`, effects |
| `decision_table` | 8 | records as data, a rule list, first-match-wins folding |

## Running them

```
nothing run examples/<name>.n          evaluate or perform main
nothing check examples/<name>.n        type check and count holes
nothing doc examples/<name>.n          render the doc lines
nothing edit examples/<name>.n         open in the editor
```

`state_machine` and `text_game_turn` read from standard input; the exact input each expects is in its section above. The other three read nothing. All five exit 0.
