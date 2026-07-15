# Contributing

## DCO sign-off

Every commit must be signed off:

```
Signed-off-by: Your Name <your.email@example.com>
```

This certifies you wrote the contribution and have the right to submit it under
AGPL-3.0-only. `git commit -s` adds it automatically.

### Deviation on the record: this history is not signed

**No commit in this repository carries a `Signed-off-by:` trailer yet.** That is
a known, deliberate deviation from the policy stated above, recorded here rather
than left for a contributor to discover.

Why it happened: the repo was built by machine seats under human direction. The
DCO is a **legal certification** — *"I have the right to submit this"* — and a
model cannot make it. Signing as `Seat 3 (Claude Code)` would have been a machine
attesting; signing as the human without explicit authority would have been a
certification made in a person's name by something that is not that person.
Neither is acceptable, so the seats signed nothing and raised it instead.

How it is resolved (standing ruling, Seat 0):

- Machine seats **never** emit `Signed-off-by:` under their own identity.
- Seat 3 appends `Signed-off-by: loVis waTer <loviswater44@gmail.com>` **only**
  to merges the founder has explicitly gated. This is a standing, scoped
  authorization — not inference, and not per-commit consent.
- Seats are credited via `Co-authored-by:` in the commit body, or in prose where
  a seat has no real address. No address is invented to carry credit.

**Fixed forward, never retroactively.** Rewriting this history to look compliant
would make the record say something that was not true when it happened, which is
the same defect the policy exists to prevent. Human contributors sign off from
the start; that is the point of stating this rather than quietly back-filling it.

Note for auditors reading `beehive-nature`: three commits there carry
`Signed-off-by: Lovis Lobster <lovis-lobster@beehive-nature>` — an address on a
domain that does not exist, predating the ruling above. Those stay, for the same
reason. They are the precedent the ruling ends, not an example of it.

## Branch policy

- `main` is the only merge target
- Branch off `main`, PR back to `main`
- `beehive-nature` (kernel): Seat 3 is the sole merger
- `bnri-cosmic`, `bnri-design`: the founder or a designated maintainer merges

## Receipt rule

Every PR touching code includes, in its description:

- the command(s) run
- their **real, unedited** output
- what that output proves

No ✅ without a receipt. "Looks correct" is not a receipt, and neither is a
summary table standing in for a raw dump. A negative finding needs a receipt too:
if something could not be verified, say so and show the failure.

## Test policy

```
cargo test          # default features: lib + tests, no GUI, no EVM
cargo clippy --lib --tests
```

Expected at HEAD: **21 passing** (lib 6, `hex_fixtures` 5, `hexrect_codec` 10).

`tests/sidecar_ipc.rs` spawns a real `python3` sidecar. On a host without a
working interpreter, **3 of its 4 tests fail, and that is correct** — they are
built so a dead interpreter fails the liveness probe loudly rather than passing
vacuously. Do not "fix" them by weakening the assertions; fix the host.

Clippy must be clean **except** for one warning:

```
warning: field `context_path` is never read
```

That is a stub marker for the C-3 dispatch and **must remain visible**. Do not
add `#[allow(dead_code)]` and do not rename it to `_context_path`. An unfinished
stub should look unfinished to the toolchain; when C-3 implements the field, the
warning goes to zero on its own.

## Features, and why `default` is empty

`default = []` builds the library and its tests only. Both optional features
carry known RUSTSEC advisories today, so neither may sit in `default` — see
[`docs/AUDIT.md`](docs/AUDIT.md) for the two gates and the named owners.

- `gui` — the COSMIC view layer (`src/main.rs`)
- `evm` — the exSat EVM client (`src/wallet.rs`)

An unused vulnerability should not be in the shipped surface at all.

## License

By contributing, you agree your contributions are licensed AGPL-3.0-only (code)
or CC0 1.0 (sprite pixel data).
