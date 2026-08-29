# The beginner-projection test

This file is the protocol and the results for Phase B6's third checkbox in
`spec-build.md`:

> **The beginner-projection test, actually run.** Show a program in beginner
> projection to three real people who do not program. Ask each to say what it
> does. Record verbatim answers in a file. This closes the item spec.md could
> not. **Done when** three transcripts exist and at least two of three
> correctly described the program, and if they did not, the projection is
> revised and re-tested rather than the bar lowered.

`spec-DONE.md` closed the implementation half of this item already
("verified by showing it to an actual person") without ever running that
verification. This file is where it actually gets run, and until it is run
the checkbox stays unchecked. Nothing below the heading **The three
transcripts** has happened yet.

## The program

The program is the `factorial` reference fixture, the one embedded in
`nothing-tui-demo` and used throughout `bench/RESULTS.md` as reference
program 1. In `nothing`'s own syntax (rendered, not typed, since the language
has no text syntax) it is:

```
main : Num -> Num = λx0:Num. if x0 == 0 then 1 else x0 * main (x0 - 1)
```

the ordinary recursive factorial: multiply the input by the factorial of one
less, until the input is 0, where the answer is 1.

This program was chosen over the other six references for three reasons.
It is short enough to read in one sitting. It is a program almost everyone
already has an intuition for, once described, which makes a wrong answer
informative rather than just confused. And its beginner-projection rendering
is already pinned by a passing test, so the exact words the participant is
handed are not something this file has to assert without evidence.

### The actual rendering

`tui/src/beginner.rs` has a test, `snapshot_factorial_fixture`, that asserts
the beginner projection of exactly this program is the following string
(quoted verbatim from the test, not retyped from memory):

```
a function taking x0 (a number) and returning if whether x0 equals 0 then 1
otherwise the product of x0 and main applied to the difference between x0
and 1
```

(the line breaks above are only for this file's width; the actual string has
none). That is the plain sentence with no cursor markup. The real on-screen
view additionally wraps whatever node the cursor is focused on in `»…«`
(`KEYS.md`, Phase 11), and `AppState::factorial()` opens with the cursor at
the root, so the whole sentence would be bracketed once at the start and end
the first time the fixture is opened. That combination is not itself the
subject of a snapshot test (`the_root_is_delimited_once` proves the "wrapped
once" property on a different example, `square_and_compare`, not on
factorial), so it is not asserted here as a fact and it is left for whoever
runs this test to read directly off the screen rather than take on faith.
Whatever the screen actually shows, cursor brackets included, is what goes
in front of the participant, and a screenshot or a transcript of it belongs
next to the answer it produced.

### How to reach it

```
cargo run -p nothing-tui --bin nothing-tui-demo
```

opens the factorial fixture with no file argument (see the comment on the
`[[bin]]` entry in `tui/Cargo.toml`: this binary exists to exercise the TUI
standalone with the same fixture `nothing edit` would need a file for).
Once it is open, press `Ctrl-P` three times. `KEYS.md` Phase 11 fixes the
cycle as `auto → text → state machine → beginner → auto`; a fresh session
has no override, so the first press forces text, the second forces state
machine, and the third forces beginner. Do not press it a fourth time, which
would return to auto (and, for this program, back to text, since factorial's
shape does not match the one auto-recognized shape, the chained-`if`
state-machine pattern).

## The setup

**Medium:** the real running program on a real terminal, not a printout and
not a description of it. Record the terminal's column width in the
transcript (the beginner sentence is one long run-on clause and will wrap
across several lines at typical widths; the wrapping is cosmetic and is not
part of what is being tested, but it should still be written down so the
session is reproducible).

**What the participant is told beforehand:** that they are going to look at
a computer program on a screen, and that they will be asked what it does.
Nothing else. Specifically not told: the name of the language, the word
"projection," the word "cursor" or what the bracket marks around a span
mean, that this is an editor rather than a static screen, or anything about
how the sentence was generated. If the participant asks what the `»` and
`«` marks are, the only permitted answer is that they mark a piece of the
program the person running the test happened to have selected and that they
can be ignored; this is telling them about on-screen highlighting, not about
the language, and it should be logged as a follow-up in the transcript
either way.

**What the participant is not told:** anything about what the program is
supposed to do, that it is "about numbers" or "about multiplication," or any
term from the language (function, recursion, applied, otherwise). No part of
the setup should use a word that appears in the rendering itself as a hint.

## The question

Ask exactly:

> What does this program do?

Allowed follow-ups, used only if the first answer is very short or the
participant stops talking and looks unsure whether they are done:

- "Can you say more about that?"
- "What happens first, and what happens next?"
- "Take your time, there's no wrong answer."

Not allowed, under any circumstance: confirming or denying whether part of
an answer is right before the session ends, supplying any word from the
rendering the participant has not already said, asking a question that
names an operation ("does it add something?" / "does it multiply?"), or
explaining what any word in the sentence means. If the participant asks
directly what a word means ("what's a function?"), the answer is "I can't
help with that, just tell me what you make of it," recorded verbatim in the
transcript alongside the question that prompted it.

## Participants

Three people, each of whom does not write code and is not currently learning
to. Prior exposure to spreadsheets, formulas, or "if this then that" style
consumer tools does not disqualify someone; writing or having recently
written source code in any language does. Three different people for the
first run. If a second run is needed, three different people again, not a
mix of old and new (see below).

## Recording

Write down what each participant says as they say it: their words, in the
order they said them, including false starts, "um," and self-corrections.
Do not clean it up, do not summarize it, and do not fix grammar. If a
participant gestures at the screen or points, note what they pointed at in
brackets, but do not translate the gesture into the words you think they
meant. The transcript is evidence; a paraphrase is not.

## The pass bar and what "correct" means

**Bar:** at least two of the three participants must be judged correct.
This number does not move, before or after the fact.

**Decided now, before any transcript exists, so it cannot be adjusted to
fit whatever answers come in:** an answer is correct if it states, in the
participant's own words, both of the following, however phrased:

1. that the program's core operation is repeated multiplication where each
   step uses a smaller version of the same number (equivalently: it keeps
   multiplying the number by one less than itself, and then one less than
   that, and so on); and
2. that this repetition has a stopping point at zero (or "when it gets down
   to nothing," "when there's nothing left," or any equivalent), at which
   point the answer is 1.

Neither the word "factorial," "recursion," nor "recursive" is required.
Getting the direction of counting wrong (counting up instead of down),
missing the stopping condition entirely, or describing only "it's a
function that takes a number and returns a number" without describing what
happens to the number does not meet the bar. A participant does not need to
use the phrase "multiplies x0 by main applied to x0 minus 1"; a description
like "it multiplies a number by all the smaller numbers down to one, and if
you start at zero you just get one" is correct even though it never repeats
the rendering's own words back verbatim, because restating the sentence is
not the same thing as understanding it, and a participant who paraphrases
correctly has demonstrated more than one who echoes it.

The judgment call is made by whoever runs the test, in writing, against the
two numbered criteria above, at the time each transcript is recorded, not in
a batch after seeing all three. Write the reasoning next to each verdict,
not just the verdict.

## If fewer than two of three pass

The projection is revised (most likely a change to `phrase`/`assemble` in
`tui/src/beginner.rs`, since that is the entire beginner-projection
vocabulary) and the test is re-run in full with three participants who have
not seen the program before, meaning three new people, not the same three
again. The bar stays two of three. It does not become one of three, and it
does not become "the answers were close." A failed run is not deleted or
edited to make room for the next one: it is left in this file exactly as
recorded, and the new run is appended below it with its own date, the same
discipline `bench/RESULTS.md` states at its own top ("entries are appended,
never edited: the point of this file is the trend line, and a trend line
you are allowed to retouch is not evidence of anything"). Whoever reads this
file later should be able to see every attempt, not just the one that
passed.

## The three transcripts

**This section is UNFILLED. No session has been run.** The beginner-
projection test is the human-required item this file exists to eventually
close, and until the three slots below are filled in with real, dated,
verbatim transcripts and a verdict against the criteria above, this Phase B6
checkbox is not done. Nothing in this section may be filled in by an
assistant standing in for a real, non-programming person; that would defeat
the entire point of the test.

### Participant 1

- Date:
- Terminal width (columns):
- Exact on-screen text shown (paste it, cursor marks included):
- Follow-ups asked, if any, and why:
- Verbatim answer:
- Verdict (correct / not correct) and the reasoning against the two
  numbered criteria above:

### Participant 2

- Date:
- Terminal width (columns):
- Exact on-screen text shown (paste it, cursor marks included):
- Follow-ups asked, if any, and why:
- Verbatim answer:
- Verdict (correct / not correct) and the reasoning against the two
  numbered criteria above:

### Participant 3

- Date:
- Terminal width (columns):
- Exact on-screen text shown (paste it, cursor marks included):
- Follow-ups asked, if any, and why:
- Verbatim answer:
- Verdict (correct / not correct) and the reasoning against the two
  numbered criteria above:

### Result

- Correct: \_\_ / 3
- Bar (2 / 3) met: yes / no
- If not met: what in the projection is being revised, and the date of the
  re-run with three new participants (append as a new dated run below this
  one, do not edit the slots above).

## Reproduce this

```
cargo run -p nothing-tui --bin nothing-tui-demo
```

then press `Ctrl-P` three times to reach the beginner projection of the
factorial fixture. Read the exact question from **The question** above,
word for word. Record the answer verbatim per **Recording** above. Judge it
against **The pass bar and what "correct" means** above, in writing, before
moving to the next participant. Repeat for three participants total, then
fill in **The three transcripts** and **Result**.
