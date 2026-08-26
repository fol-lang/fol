# V4 C Boundary — Gap Scan and Closure Plan

The C boundary passes every lane in `make verify` and cannot bind Lua, SQLite,
or zlib. This plan is about that distance.

`V4_CONTINUE.md` closed 35 of 35 tasks and every claim in it is true. What it
never asked was whether the shapes it supports are the shapes real headers
contain. They are not. A scan of 40 constructs drawn from ordinary C found
**four blockers that stop most real libraries at the door**, one of which
accepts or refuses the same declaration depending on what its neighbours are.

**Nothing here merges to `develop` until a real third-party header binds.**

---

# 1. How this was measured

Each construct is a minimal header, a definition that compiles, and an overlay;
each is run through `fol tool bind c` and the verdict recorded. The harness is
`/home/bresilla/foltmp/abiscan/run.sh` during the scan and becomes a test in
M17. A construct is only listed below if it was **run**, never if it was read.

Three verdicts matter and are not the same:

- **ok** — binds. Not the same as usable: M16 found a raw pointer that binds
  and is then refused at mount, so a shape is only proven when a FOL program
  calls it.
- **REFUSED (FOL)** — FOL's own projection declined it. FOL can change this.
- **REFUSED (GERC)** — failed in `raw binding generation`, before FOL's
  projection ran. FOL cannot fix these alone, and §5 is about that.

---

# 2. What the scan found

## 2.1 Blockers — these stop real libraries

| # | Construct | Verdict | Layer |
|---|---|---|---|
| B1 | `const struct S *s` with `S` defined | ~~REFUSED~~ **closed (const)** | FOL selection |
| B2 | `typedef struct T T;` (opaque handle idiom) | REFUSED | GERC |
| B3 | `typedef int32_t (*F)(void*, int32_t);` as a callback | ~~REFUSED~~ **closed** | FOL |
| B4 | callback with no context (`int (*)(lua_State*)`) | REFUSED | FOL |
| B5 | `const char *s` as a parameter | ~~unmountable~~ **closed** | FOL |

**B1 is the worst.** `const struct S *s` is refused with *"S is incomplete"*
when `S` is defined in the same header. GERC only materialises a complete
record when something takes it by value, so the pointee arrives classified
`Opaque` and FOL reports "incomplete" — accurate about what FOL received, false
about the header, and it sends a reader looking for a definition that is right
there.

It is also **incoherent at the bind stage**: add an unrelated by-value use of
`S` elsewhere in the header and the same declaration binds. That is not a
workaround, and an earlier draft of this plan wrongly implied it was — the
corpus's mount check corrected it. The by-value neighbour gets the declaration
past `bind` and it is *still* refused when a package mounts it, with `T1099`.
Struct-by-pointer does not work by any route.

Passing a struct by pointer is how most C libraries pass structs at all.

*Closed, for `const`.* The cause was **FOL's**, not a sibling's. PARC minimises
its closure deliberately: `visit_type` demotes a `RecordRef` reached through a
pointer to `Need::OpaqueSufficient`, because holding an address does not need a
layout. `Selection::Only` grants `Need::Definition` to whatever it names, and
FOL named only the routines. Naming every complete supported record too asks
for what FOL actually needs, and the opaque view stops arriving.

The FOL side then maps a `const` pointer-to-record to that record: C uses a
pointer to avoid a copy, not to say anything about ownership, and FOL's struct
has FOL's layout — so the adapter rebuilds the provider's struct from the
fields and lends *that*, the same rebuild a by-value record already does.

A **mutable** pointer still does not mount. It is an out-parameter whose writes
have to come back, and nothing copies them back; its corpus row says so.

Two things fell out of the fix. `struct N { struct N *next; }` is now refused
as *"refers to itself, so it has no finite FOL shape"* rather than the false
*"is incomplete"* — FOL receives the definition and its own cycle guard gives
the real reason. And a flexible-array member is refused earlier, by the probe
profile, because naming the record asks PARC for a definition it will not give.

**B2** is the dominant spelling of an opaque handle: `typedef struct lua_State
lua_State`, `typedef struct sqlite3 sqlite3`. The bare `struct T;` form works,
which is why `v4_c_opaque_handle` passes and nothing noticed.

**B3** means a callback only binds when its type is written inline in the
parameter list. Real headers typedef callback types. FOL is not resolving the
alias before asking "is this a function pointer" — the same `resolve_alias` the
scalar path already uses.

**B5 was invisible until M17 checked mounting.** `const char *s` binds, writes
a manifest, and is then refused when a package mounts it: *"imported routine
uses a pointer type, which the C import path does not surface to FOL"*. So does
a returned `const char *`, `char **argv`, and the pointer+out-length result
convention. **There is no way to pass a string to an imported C routine** —
`fol_str_view_t` is the export direction, and the `buffer`/`buffer_length`
pairing needs a length parameter a `const char *` does not have.
`sqlite3_open(const char *filename, ...)` cannot cross.

This is the shape of defect this plan exists for: four green lanes, a written
manifest, and nothing a program can call. Every `ok` in a bind-only scan is
suspect until a package mounts it.

*Closed.* The overlay declares which parameters are text --
`string = ["filename"]` -- because `const char *` is equally how C spells a
pointer to one byte, and inferring it would be the guess the buffer pairing
already exists to avoid. A declared parameter projects as `AbiType::CString`
and reaches FOL as borrowed `str`. FOL's strings carry no NUL, so the call site
builds one that lives exactly as long as the call. Text containing a NUL is a
fault rather than a truncation: a C string ends at its first NUL, so the
provider would read something shorter than FOL holds.
`examples/v4_c_string_arg` passes `"hello"` and C returns 5 and `h`.

An *undeclared* `char *` still binds and still cannot be mounted, and a test
holds that: the declaration is the mechanism, not a formality.

**B4** is a shape decision, not a bug: V4 requires a `void *` context as the
callback's own first parameter. `lua_CFunction` and `qsort`'s comparator have
no context at all. §4 argues this one is worth revisiting rather than fixing.

## 2.2 Refusals that are correct and documented

Unions anywhere, bitfields, packed layouts, flexible-array members, variadics,
`long double`, self-referential structs by value, a struct returned by value,
nested and anonymous struct members, arrays as parameters or fields, a function
pointer inside a struct, and a callback whose context is last. Each is refused
by name with a reason. These are the boundary V4 chose, and this plan does not
reopen them.

## 2.3 What already works

Every integer and float width both directions and at both range edges,
`size_t`, `bool`, `char`, enums including explicit values, pointer-to-pointer,
`const char *`, a returned C string, pointer/length pairs, provider-allocated
buffers, structs **by value**, typedefs of complete structs and of scalars,
opaque handles through the bare-struct spelling, inline context-first
callbacks, status/out-parameter error mapping, and contained panics.

That is a real boundary. It is not the boundary a real header needs.

## 2.4 One more, found by the scan

`volatile int32_t` as a parameter fails in raw binding generation. FOL already
refuses volatile in its own projection with a clear reason; the GERC failure
arrives first and says less.

---

# 3. M17 — Lock the scan

Goal: the corpus becomes a test, so the boundary's shape is measured on every
run rather than assumed.

- [x] Move the 40-construct corpus into `test/v4_c_shapes.rs`, each with its
  verdict and, for a refusal, the phrase that must appear.
- [x] Assert **no panics** across the corpus.
- [x] Mark each blocker's row `Blocker`, so closing one fails the test
  until its row is updated. A gap that quietly starts working is a gap nobody
  documented. `the_blocker_count_is_what_the_gap_plan_records` pins the list.
- [x] For every `ok` row, prove it **mounts**, not just binds. This is what
  found B5, and it corrected B1: the by-value neighbour that made
  struct-by-pointer bind does *not* make it mount.
- [ ] Prove the mounting rows are **callable**, not only mountable. Mounting is
  checked by building a package that imports the alias; calling each shape
  needs a bespoke program per row, and the shapes with examples
  (`v4_c_import_scalar`, `v4_c_record`, `v4_c_buffer`, `v4_c_callback`,
  `v4_c_opaque_handle`) are the only ones proven that far today.

**STOP:** an `ok` row that no FOL program calls is not evidence.

---

# 4. M18 — The two FOL-side blockers

Goal: B3 and B4 close without touching a sibling.

- [x] **B3**: resolve type aliases before classifying a callback parameter.
  `resolve_alias` already exists and the scalar path already uses it;
  `callback_positions` did not. Both halves are resolved now -- a typedef'd
  function pointer *and* a typedef'd context -- and both have corpus rows.
- [ ] **B4**: decide, with the owner, whether a **context-free callback** is
  supportable. It is not a small question: FOL's trampoline recovers the
  closure from a thread-local slot keyed by the call, and the context pointer
  is currently how a provider identifies the closure. A context-free callback
  can still use the thread-local slot — the context was never load-bearing for
  recovery, only for identity — so the mechanism may already support it.
  - If yes: a `callback_context = "none"` spelling, and the null-context check
    becomes a slot check alone.
  - If no: record why, and that `lua_CFunction`-shaped APIs need a C shim.
- [x] Refuse a typedef'd function pointer used as an **ordinary parameter**
  with the same message the inline form gets, so B3's fix does not turn one
  confusing refusal into two. Held by the corpus's `function_pointer_result`
  row, which is a typedef and still refused by name.
- [x] **B5**: declare text in the overlay and carry it as `AbiType::CString`.
  Found by M17's mount check, not by the gap scan, which only bound.

**STOP:** do not widen the callback contract to context-last. That was
measured, refused deliberately, and guessing a provider's context position is
the mistake the pairing exists to prevent.

---

# 5. M19 — The GERC-side blockers

Goal: B1 and B2 close, or FOL states exactly what it needs and from whom.

B1 and B2 fail before FOL's projection runs. §3.5 of `V4_CONTINUE.md` records
the last time a sibling limit blocked V4 and the pin was already at HEAD — the
same check is the first task here.

- [ ] Re-measure the pin. `V4_CONTINUE.md` §M15 found all three siblings at
  upstream HEAD on 2026-08-25; confirm that is still true before assuming a fix
  must be written.
- [x] Establish, in GERC's own source, **why** a pointee arrives `Opaque`.
  It is PARC, not GERC, and it is deliberate: `complete.rs:412` demotes a
  `RecordRef` behind a pointer to `OpaqueSufficient`. It is a closure-scope
  decision, so FOL can widen what it asks for -- which closed B1.
  Whether it is a projection-scope decision (only by-value uses materialise a
  definition) or a contract limit decides everything downstream.
- [x] If it is scope: find whether FOL can widen what it asks GERC to project.
  It can, and does: every complete supported record joins the selection.
  FOL controls the selection it hands the pipeline, and B1's own workaround —
  an unrelated by-value use — suggests the definition is reachable and simply
  not requested.
- [ ] If it is a contract limit: write the sibling-side issue with the
  reproduction, and record B1/B2 as blocked in this plan the way §3.5 is.
- [x] Fix FOL's message either way. *"'S' is incomplete"* is false when the
  header defines `S`. It no longer arises for a defined struct; the one case
  that still reports it is a genuinely incomplete one. It should say FOL received an opaque view and name the
  by-value workaround, so a reader is not hunting a definition that exists.

**STOP:** do not add a by-value decoy declaration to make B1 pass. It works,
and shipping a workaround as a fix would put a lie in the examples.

---

# 5b. Found while proving B1: inbound records were transposed

Proving B1 meant *calling* the routine, not mounting it — and the call returned
the wrong answer. `struct Point { int32_t zulu; int32_t alpha; }` passed from
FOL to C arrived with its fields **swapped**. Silently: no diagnostic, a wrong
number, and a program that runs.

The adapter's parameter list was built in the provider's declaration order
while the call site emitted from FOL's own lowered record, whose fields are
sorted by name. The two disagreed for every struct whose declaration order was
not already alphabetical.

**This was not new.** It shipped with M10's inbound records and had been live
ever since. The test that covers that feature used `struct point { int x; int
y; }` and multiplied the fields — alphabetical already, and a product that
cannot see a swap. It could not have caught this.

Fixed by sorting the adapter's parameter list by field name, so both sides
agree; the provider's own field order is untouched, because the struct is
rebuilt by name. The fixture is now `zulu, alpha` computing `zulu * 100 +
alpha`, which fails if either the order or the arithmetic is wrong.

---

# 6. M20 — Bind something real

Goal: the boundary is proven by a header FOL did not write.

Nothing above is evidence until a third-party header binds. Each candidate is
chosen for what it exercises, in increasing order of demand:

- [ ] **zlib** — `compress2`/`uncompress`: pointer/length buffers, status
  codes, `z_stream` by pointer (B1).
- [ ] **SQLite** — `sqlite3_open`/`_close`/`_exec`: `typedef struct sqlite3
  sqlite3` (B2), an opaque handle with a paired destroy, and a callback (B3/B4).
- [ ] **Lua** — `luaL_newstate`/`lua_pcall`: B2 plus `lua_CFunction` (B4). If
  B4 stays refused, prove the C-shim path instead and document it as the
  supported way.
- [ ] Each lands as an example that **builds, links, and runs**, with the
  library vendored or its absence a skip that `FOL_H7_REQUIRED=1` makes fatal.

**STOP:** this milestone does not close on a header that binds. It closes on a
program that calls the library and produces a result C agrees with.

---

# 7. Order, and what "done" means

M17 first — it costs little and every later claim leans on it. Then M18, which
needs no one else. M19 in parallel, because its first task is a question to
another repository and the answer sets the schedule. M20 last, and it is the
only milestone whose completion means anything to a user.

**The C ABI is done when a program written against a real C library's real
header compiles, runs, and agrees with a C program doing the same thing.** Not
when the lanes are green — they already are.
