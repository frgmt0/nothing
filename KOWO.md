# Kowo × nothing — the explainer

*(the "explain it like I'm in high school" version)*

---

## First: what even is `nothing`?

Normal code is an **essay**. You type characters into a file, and if you
misplace one semicolon, the whole essay is "broken" until you find it. The
computer spends half its life just trying to figure out what your essay
*meant*.

`nothing` throws out the essay. A program is **Lego**. Every edit snaps a
brick on or off, and the bricks physically only fit in ways that make sense.
Three wild things fall out of that:

1. **You cannot write a syntax error.** Not "we catch them fast" — the
   category does not exist. There's no text to typo.
2. **Unfinished is fine.** A gap in your program is a *hole* — a legit brick
   that means "haven't decided yet." The program still runs, gets to the
   hole, and politely says "I'm stuck right here, and here's everything I
   knew when I got stuck."
3. **Names are stickers, not glue.** Every variable is really an ID (like a
   phone number); the name is just the contact label. Rename anything,
   anywhere, in one move — it is *impossible* to rename the wrong thing.

Bonus: because the program is structure, the editor can show it however it
wants — as code, as a table, as friendly full sentences — and they're all the
same program. These views are called **projections**.

---

## The Jane Street move

Jane Street is a trading firm famous for using a nerdy language (OCaml)
nobody else uses. Here's the trick: **they don't sell the language. They
sell trading, and the language is why their trading is better.**

Kowo does the same. We don't stand on a corner yelling "adopt our weird
Lego language!" We build products that are *unreasonably good* because
they're made of Lego inside — products where competitors, stuck with essays,
literally cannot copy the best features.

---

## So what does Kowo actually sell?

### 1. The Rules HQ ⭐ (the main event)

Every company runs on hidden rules: "if the customer is under 25, the
insurance costs more," "free shipping over $50 unless it's Alaska."
Today those rules live in crusty spreadsheets or code that only two
engineers can read — and the people *responsible* for the rules (the
business folks, the auditors) can't read them at all.

Kowo sells the **home base for company rules**:

- The business person sees a clean **table** ("if this → then that").
- The engineer sees **code**.
- The auditor sees **history**: who changed *this exact rule*, when, and
  the diff says "threshold changed from 3 to 5" — not "line 847 changed."
- Same rules. Three views. One source of truth. That's projections.

Banks and insurance companies pay absurd money for "prove who changed what."
We can prove it *per brick*.

### 2. Spreadsheet 2.0

A spreadsheet is secretly a baby version of our idea (live values! visual
grid!) except it's held together with tape: any cell can silently be garbage,
and merging two people's edits to `budget_FINAL_v3_REAL.xlsx` is a crime
scene. Ours has typed cells (a price can't quietly become a date), holes
("we haven't picked the interest rate — here's what the model looks like
without it"), and real merging. Two analysts branch a model, both edit,
it merges like magic.

### 3. Automations that can't be broken

Think Zapier: "when an order comes in → check the amount → email the
customer." Today those tools are either safe-but-weak or powerful-but-scary.
Lego automations are both: a half-built flow still runs in practice mode and
tells you exactly what's missing, and the flowchart view *is* the code.

### 4. The merge fixer (the foot in the door)

This one we sell to programmers directly, for their *existing* languages:
a smarter merge for git. Our benchmark yesterday: on 16 realistic
conflict scenarios, git resolved **2** cleanly. We resolved **13**, and
the 3 we refused were genuine disagreements a human should settle. That
demo sells itself, and it gets Kowo into companies so we can upsell #1.

---

## And yes — the robots

We promised not to lead with AI, so here it is at the end. In every product
above, there's a feature no essay-based competitor can honestly offer:

> **Let an AI maintain your rules/models/automations — and it is
> physically incapable of breaking them.**

The AI doesn't write text and hope. It snaps bricks, and only legal bricks
exist. Every AI edit is labeled, auditable, and undoable — brick by brick.
You're not selling "AI writes code." You're selling **"you can finally
delegate the boring logic, safely."**

---

## The plan, in one sentence

Get in the door with the merge fixer (#4), wow them with the rename that
can't conflict, then sell them the Rules HQ (#1) — and let the Lego stay
our secret sauce, Jane Street style.
