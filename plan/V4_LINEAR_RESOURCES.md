# V4 Decision Record: Fallible Release of Foreign Resources

> **Status: decided 2026-08-22.** A C handle is modelled as a **linear**
> resource that must be consumed exactly once by an explicit, fallible release
> (option C below). `[fin]value` remains available as an explicit
> discard-the-failure consumer, so the unsafe choice is spelled in the source
> rather than being the default. A scope still holding an unreleased linear
> resource **may not `report`** (resolution 4 below): it must release first.
>
> `fin` finalization was ruled out because releasing a C handle can fail and
> scope-exit finalization has no caller to report the failure to. The analysis
> that led here is retained below; sections 1 to 5 are the reasoning, section 6
> is the decision, and section 7 is what it changes.
>
> Open detail: the capability is spelled `lin` throughout this record as a
> placeholder. The name is not part of the decision.
>
> `plan/V4_PLAN.md` remains the implementation authority. Section 4.8 and
> milestone M7 are specified from this record.

## 1. The constraint

A C resource — `FILE*`, `sqlite3*`, a socket, an `mmap` region — has a release
operation that **can fail and whose failure is meaningful**:

```c
fclose(f)            /* EOF on flush failure: data you wrote is gone */
close(fd)            /* EIO; the write may not have reached the disk */
sqlite3_close(db)    /* SQLITE_BUSY while statements are outstanding */
```

A caller that ignores those statuses silently loses data. So the release must
be able to return a status to somebody who can act on it.

## 2. Verified facts

Each was confirmed against the tree, not assumed.

- A custom finalizer is fixed at `pro (T)finalize(): non`. Returning an error
  is not expressible; `fol-typecheck/src/decls.rs:220` enforces the `pro` form
  and the contract carries no error channel.
- `fin` is **affine, not linear**. A `fin` value that is never consumed is
  accepted and finalized at scope exit. There is no must-consume rule.
- FOL has no must-use rule at all: discarding a returned value is accepted.
- `[fin]value` already runs the finalizer early and invalidates the source, and
  the scope-exit finalizer then does not run again
  (`examples/mem_fin_early_m1`). The machinery for *explicit* consumption
  exists.
- `dfr` blocks **do** run on the `report` path. A routine that defers a block
  and then reports still runs the block. Verified by observation, not by
  reading the lowering.
- A `fin` value may not be a top-level binding
  (`fol-typecheck/src/exprs/bindings.rs:447`) and may not be partially moved
  out of (`:1007`), so the finalizer always sees a whole value in a routine
  scope.

## 3. Why `fin` is ruled out

The problem is not that `finalize` happens to be typed `: non`. Widening it to
`: non / E` would not help, because **scope-exit cleanup has no caller**. When
a scope ends there is no expression waiting on a result and no `||` or `check`
to route a failure into. The same applies to `dfr`, which runs on the error
path but cannot propagate either.

This is the wall every mainstream language hits: Rust's `Drop` cannot fail,
C++ destructors must not throw, and Go's `defer f.Close()` conventionally
discards the error. None of them is a model to copy here; each is a known
defect that FOL's recoverable-error surface makes both more tractable and more
visibly wrong if fudged.

What a C handle needs is a different discipline from what `fin` provides:

```text
fin       consumed AT MOST once, implicitly, infallibly
handle    consumed EXACTLY once, explicitly, fallibly
```

The second is linear. FOL implements the first.

## 4. Options

### A. `fin` with a fallible `close`, finalizer as best-effort backstop

The handle stays `fin`. The idiomatic API adds `pro (T)close(): non / E` taking
`[mov]self`, which performs the real release and suppresses the backstop the
way `[fin]` already does. `finalize` remains a leak-guard that discards errors.

- No new type-system machinery; buildable today.
- Leaks are impossible, because the backstop always runs.
- **A caller who forgets `close()` silently discards the failure.** That is the
  defect this record exists to avoid, merely made idiomatic rather than fixed.

### B. A linear capability

A new conformance — spelling to be decided — marks a type as must-consume. The
compiler proves that every path through a scope consumes the value exactly once
by an explicit consuming call. There is no implicit cleanup, so `close` is an
ordinary fallible method and its error propagates like any other.

- The honest model: the type system states the real obligation.
- Real work. Every control-flow path must be proven to consume: branches, loop
  breaks, early `return`, and `report`.
- Interacts with existing rules that assume a value may simply go out of scope.

### C. Linear by default, `[fin]` as an auditable escape hatch

Option B, plus `[fin]handle` retained as an explicit "best-effort release,
discard the error" consumer that satisfies the linear obligation.

- Safe by default; the dangerous choice is spelled in the source and greppable.
- Reuses `[fin]`, which already consumes and invalidates, so the escape hatch
  costs nothing new.
- Matches the posture the interop stack already takes, where GERC refuses a
  declaration rather than emitting an opaque type.

## 5. The sub-question: release on the error path

This is the part that decides how hard B and C are, and it is a language-level
call rather than an implementation detail.

If a routine holds an open handle and then reports an error, the handle must
still be released, and **the release can fail too**. Two errors then exist at
once and one result channel is available:

1. the body's reported error wins, and the release error is discarded;
2. the release error wins, and the body's error is discarded;
3. the two combine into an aggregate, which requires an error shape that can
   hold both;
4. the language refuses the situation — a scope holding an unreleased linear
   resource may not `report` — forcing an explicit release before the error
   path, which is sound but restrictive.

Option 4 is the most conservative and the most in keeping with V4's
fail-closed rules; options 1 and 2 silently lose information, which is the
original complaint in a new place; option 3 is the most expressive and the most
work.

`dfr` running on the `report` path is relevant but not sufficient: it gives a
hook that already fires on both exits, and it still cannot carry a status out.

## 6. Decision

**Option C**, with sub-question resolution **4**.

A foreign resource type declares the linear capability (spelled `lin` here as a
placeholder). The compiler proves every path through a scope consumes the value
exactly once, by one of:

- an explicit fallible release, whose error propagates like any other:

  ```fol
  close([mov]handle) || report;
  ```

- or the existing explicit discard, which satisfies the obligation while
  stating plainly that the failure is being thrown away:

  ```fol
  [fin]handle;
  ```

Omitting both is a compile error. There is no implicit scope-exit release, so
no failure is ever discarded without a token in the source saying so, and
`[fin]` is greppable when auditing a codebase for ignored release failures.

A scope that still holds an unreleased linear resource may not `report`. The
resource must be released first, on the error path as on the success path:

```fol
fun[] work(): int / E = {
    var handle: File = open("x");
    when(bad) {
        case(true) { report 9; }   // rejected: handle still unreleased
        * { }
    };
    close([mov]handle) || report;
    return 7;
};
```

That rule is chosen because it refuses the case it cannot represent honestly
rather than silently choosing which of two errors to lose. It is the most
restrictive of the four, deliberately: relaxing it later to an aggregate error
(resolution 3) is a widening, and widenings do not break existing programs,
whereas shipping resolution 1 or 2 first would be a narrowing to undo.

The cost is real and should be expected: the diagnostic for a rejected
`report` has to name the resource still held and point at where it was
acquired, or the rule will read as arbitrary.

## 7. Consequences for `plan/V4_PLAN.md`

- **§4.8** stated that "ownership, escape, and destructor provenance are
  signature/manifest metadata." Destructor provenance is not metadata: it is a
  type obligation carried by the linear capability. That subsection is
  rewritten from this record.
- **M7** ("Records, Entries, Errors, Views, Buffers, and Handles") gains a
  linear-resource design. Contrary to an earlier assessment during this review,
  M7 **grows** rather than shrinks.

  M7 already leans that way without naming it. Section 12.4 asks to "consume
  transferred handles exactly once in ownership checking" and to "diagnose leak
  paths where an owned foreign resource exits scope without transfer or
  destruction" — which is linearity, described operationally. What it never
  says is that the destruction can fail. Options B and C give that existing
  intent a type-level name and an error channel.
- **M4** is unaffected and still shrinks. Ownership transfer and duplication at
  the boundary really are covered by the existing `[mov]`/`[bor]` operations
  and the `copy`/`clone` capability claims.
- The annotation overlay in **§4.13** loses "destructor pairing" as a metadata
  item and gains a type-level obligation instead. Pointer/length pairing,
  direction, nullability, and the imported error convention remain metadata:
  C cannot express them and FOL has no equivalent.
