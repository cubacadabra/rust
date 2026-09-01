# AGENTS.md

## Model Routing Policy

Optimize for:

1. Finishing the task correctly without wasting human time.
2. Avoiding retries caused by using a model that is too weak for the current problem.
3. Minimizing use of expensive models when the remaining work is straightforward.

Do not optimize purely for token cost.

A cheap model struggling for several turns is worse than using a stronger model immediately.

## Available Model Tiers

Treat the available models approximately as:

* **Luna High**: normal implementation, mechanical coding, well-understood changes, cleanup, tests, repetitive edits.
* **Terra**: use when its speed/capability profile is especially appropriate for the task.
* **Sol High**: difficult reasoning, architecture, debugging, unfamiliar systems, ambiguous requirements, cross-cutting changes.
* **Sol XHigh**: exceptional escalation for problems that remain difficult after serious reasoning with Sol High.

Do not use Sol XHigh merely because a task is large. Use it when the task is genuinely reasoning-hard.

## Core Rule

Choose the model for the **current intellectual phase**, not for the entire task.

A single task may legitimately follow this pattern:

```
Sol High
-> Luna High
-> Sol High
-> Luna High
```

Escalate when reasoning becomes difficult.

De-escalate as soon as the hard reasoning has been resolved and the remaining work becomes predictable.

## Start With Luna High When

Use Luna High when most of the following are true:

* The requested change is clearly specified.
* The relevant files or subsystem are already known.
* The implementation pattern already exists elsewhere in the repository.
* There is little architectural ambiguity.
* The work mostly consists of:

  * adding or modifying ordinary functions
  * updating call sites
  * renaming
  * straightforward refactoring
  * adding tests for understood behavior
  * fixing compiler errors resulting from a known change
  * formatting or cleanup
  * updating configuration
  * repetitive edits
* Failure is likely to produce an obvious compiler, test, or lint error.
* A competent engineer could describe the implementation steps before beginning.

Do not escalate merely because many files need editing if the edits are mechanical.

## Start With Sol High When

Prefer Sol High immediately when any of these are significant:

* The root cause of a bug is unknown.
* The task requires understanding an unfamiliar subsystem.
* Several systems interact in ways that must first be understood.
* There are multiple plausible architectures.
* A bad early decision would cause substantial rework.
* The change crosses important abstraction boundaries.
* Concurrency, ownership, synchronization, memory safety, ABI/FFI, rendering, networking, persistence, distributed state, or security semantics are central to the problem.
* Existing behavior is poorly documented or surprising.
* Tests fail for reasons that are not obvious.
* The task asks for a substantial architectural refactor.
* Requirements conflict or contain important ambiguity.
* Correctness depends on identifying hidden invariants.
* The agent must infer why the existing code was designed the way it was.
* A previous attempt using a weaker model already failed or wandered.

When genuinely uncertain whether a task is routine or reasoning-heavy, favor Sol High.

Avoid spending several weak-model iterations discovering that Sol was needed all along.

## Escalation From Luna

While using Luna High, reassess continuously.

Escalate to Sol High if ANY of the following occurs:

* Two materially different attempted fixes fail.
* The same test/build failure persists after one reasonable fix.
* A fix creates unexpected failures in another subsystem.
* The agent discovers that its original mental model of the code was wrong.
* The problem requires choosing between competing architectural approaches.
* The agent starts guessing instead of deriving behavior from the code.
* More than a small amount of code must be explored merely to understand what is happening.
* An apparent local bug turns out to involve multiple layers.
* The agent cannot confidently state the root cause.
* The agent finds an undocumented invariant that materially changes the solution.
* The implementation begins requiring compensating hacks.
* Progress has become exploratory rather than executable.

Do not keep trying increasingly speculative Luna fixes.

One early escalation is cheaper than several failed attempts.

## De-escalation From Sol

Sol High should actively look for the point at which the difficult reasoning is finished.

Switch back to Luna High when all of the following are approximately true:

* The root cause or architecture is understood.
* The important design decision has been made.
* Dangerous or subtle core changes are complete, or their exact implementation is clear.
* Remaining work can be written as an explicit checklist.
* Remaining failures are expected consequences of the change.
* No unresolved architectural question remains.

Typical Luna work after Sol includes:

* updating call sites
* implementing repeated versions of an established pattern
* fixing straightforward compile errors
* adding ordinary tests
* updating mocks
* changing configuration
* deleting obsolete code
* updating comments/docs
* running builds
* resolving simple lint warnings
* final cleanup

Do not keep using Sol simply because Sol started the task.

## Re-escalation

If Luna encounters a new reasoning-heavy problem after a handoff, escalate again.

Example:

```
Sol High:
  discover rendering architecture
  identify ownership problem
  redesign API

Luna High:
  update 18 call sites
  fix straightforward compiler errors

unexpected ABI/linker failure appears

Sol High:
  diagnose ABI mismatch
  establish correct boundary

Luna High:
  finish bindings
  tests
  cleanup
```

Model selection is dynamic.

## Sol XHigh Escalation

Use Sol XHigh only when one or more of these apply:

* Sol High has made a serious attempt and remains unable to establish the root cause.
* The task involves unusually difficult reasoning across many interacting systems.
* Several plausible solutions exist and subtle correctness differences matter.
* A failure could silently corrupt important state.
* A problem remains unresolved after gathering substantial concrete evidence.
* Sol High repeatedly revises its own model of the system.

Before escalating to Sol XHigh, gather evidence.

Do not use higher reasoning merely as a substitute for reading the relevant code, logs, tests, or documentation.

## Phase Boundary Check

After each major discovery or implementation milestone, ask internally:

```
Is the remaining work still reasoning-hard?
```

If NO:

```
de-escalate to Luna High.
```

If YES:

```
remain on or escalate to Sol High.
```

Examples of phase boundaries:

* root cause discovered
* architecture selected
* core interface implemented
* first successful compile
* first passing integration test
* unfamiliar subsystem understood
* subtle failure reduced to mechanical fixes

## Avoid Model Thrashing

Do not switch models for tiny differences in difficulty.

A model switch should correspond to a meaningful change in the nature of the work.

Prefer:

```
reasoning phase -> implementation phase
```

over:

```
hard line -> easy line -> hard line -> easy line
```

Batch nearby mechanical work together.

## Investigation Policy

Before concluding that stronger reasoning is necessary:

* inspect relevant code
* inspect recent changes when relevant
* read test failures carefully
* inspect compiler/runtime errors
* search for existing patterns in the repository

However, if interpreting that evidence is itself difficult, escalate early.

Do not confuse lack of context with lack of intelligence.

## Handoff Requirements

When changing models or delegating to another agent, leave a compact handoff containing:

### Goal

What the task ultimately needs to accomplish.

### Current understanding

The important architecture, invariants, or root cause discovered so far.

### Decisions already made

Design choices that should not be casually revisited.

### Work completed

Files/components already changed.

### Remaining work

Concrete checklist.

### Verification

Commands/tests already run and their results.

### Open problem

Only unresolved issues that genuinely need further reasoning.

Do not require the next model to reconstruct discoveries that have already been made.

## Preserve Good Decisions

A cheaper model taking over from Sol should preserve the established approach unless it finds concrete evidence that the approach is incorrect.

Do not redesign merely because a new model took over.

If the established design appears wrong, escalate rather than silently replacing it with a materially different architecture.

## Time Versus Cost

Human time is more valuable than modest model savings.

Therefore:

* Do not let Luna repeatedly struggle with an obviously difficult problem.
* Do not use Sol for long stretches of mechanical edits.
* Spend expensive reasoning where it eliminates uncertainty.
* Spend cheaper compute where the path is already known.

The desired behavior is:

```
expensive reasoning briefly
+
inexpensive execution extensively
```

rather than:

```
inexpensive struggling
+
retries
+
eventual expensive reasoning
```

## Large Tasks

Do not automatically use Sol merely because a task will take a long time.

Classify the task by uncertainty, not size.

Example:

Updating 60 call sites after an API rename:

```
Luna High
```

Determining what the new API should be:

```
Sol High
```

A large project may therefore begin with a relatively short Sol phase followed by a much longer Luna phase.

## Small Tasks

For obviously trivial changes, use Luna High without additional routing analysis.

Examples:

* rename one symbol
* change a constant
* add a simple field
* update copy
* add an obvious guard
* mirror an existing test
* straightforward dependency/config update

Do not spend more effort choosing a model than performing the task.

## Automatic Routing

When the environment supports model-specific subagents or delegation:

* perform model routing automatically
* do not ask the user for permission for routine escalation/de-escalation
* preserve the same task context and repository state
* provide the receiving agent with the handoff described above

When the environment does NOT permit changing models automatically:

* stop at the next safe task boundary
* clearly emit:

  ROUTE: SOL HIGH

or:

```
ROUTE: LUNA HIGH
```

or:

```
ROUTE: SOL XHIGH
```

* include the compact handoff
* do not continue burning time with an inappropriate model

## Default Bias

When deciding between Luna High and Sol High:

* If the implementation is understood but lengthy -> Luna High.
* If understanding the implementation is the hard part -> Sol High.
* If uncertain whether the current mental model is correct -> Sol High.
* Once uncertainty has been removed -> Luna High.

The goal is not to use the cheapest model.

The goal is to use the **cheapest model that is unlikely to make the task take longer**.

