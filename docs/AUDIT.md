# Dependency audit — `bnri-cosmic`

**At commit:** `d33f0cb` · **Status:** Gate 1 PASS · Gate 2 FAIL-BY-DESIGN (every
advisory owned, none in the shipped surface)

Two gates, because one tool cannot answer both questions.

---

## Why two gates

The standing order is: *"`cargo audit` runs against the default feature set, and
no vulnerable dependency may sit in default."*

**`cargo audit` cannot do that.** It scans `Cargo.lock`:

```
$ cargo audit
    Scanning Cargo.lock for vulnerabilities (888 crate dependencies)
```

`Cargo.lock` is the union of *all* resolutions — feature-agnostic by design — so
`optional = true` does not remove a crate from it. And `cargo audit --help`
offers `--target-arch`, `--target-os`, `--ignore`, `--file`, and **no
`--features`**. The order's intent is sound; its named metric structurally cannot
see the thing it is about.

So the intent is measured by the tool that can see it, and the lock is reported
raw rather than laundered:

- **Gate 1 — the shipped surface.** `cargo tree -e normal` on default features.
  This is what actually compiles and ships. It must contain no crate carrying a
  known advisory. **This gate is binding.**
- **Gate 2 — the lock.** `cargo audit`, raw and unedited. It will report
  advisories reachable only through default-off features. Every one is listed
  below with a named owner. **Deferred is not dismissed** — an owner clears it
  before the feature that reaches it is turned on.

`--ignore` is deliberately **not** used. Suppressing these would blind whoever
builds the GUI to a live 7.5 HIGH, which is precisely the outcome "deferred, not
dismissed" forbids. A plain `cargo audit` must keep showing them.

---

## Gate 1 — shipped surface (BINDING) · **PASS**

```
$ cargo tree -e normal | grep -c ethers
0
$ cargo tree -e normal | grep -c libcosmic
0
$ cargo tree -e normal --prefix none | sort -u | wc -l
137
```

**137 crates in the default build, against 888 in the lock.** Zero carry a known
advisory. The vulnerable trees are reachable only by explicitly enabling a
feature.

---

## Gate 2 — lock scan (RAW) · 6 advisories, all owned, none shipped

```
$ cargo audit
    Scanning Cargo.lock for vulnerabilities (888 crate dependencies)
error: 6 vulnerabilities found!
warning: 8 allowed warnings found
```

| Advisory | Crate | Severity | Reached via | Owner |
|---|---|---|---|---|
| RUSTSEC-2026-0194 | `quick-xml 0.39.4` | **7.5 high** | `gui` → `libcosmic` | first Linux GUI build |
| RUSTSEC-2026-0195 | `quick-xml 0.39.4` | **7.5 high** | `gui` → `libcosmic` | first Linux GUI build |
| RUSTSEC-2025-0009 | `ring 0.16.20` | — | `evm` → `ethers` → `jsonwebtoken 8.3.0` | C-3 |
| RUSTSEC-2026-0098 | `rustls-webpki 0.101.7` | — | `evm` → `ethers` → `reqwest 0.11.27` → `rustls 0.21.12` | C-3 |
| RUSTSEC-2026-0099 | `rustls-webpki 0.101.7` | — | `evm` → `ethers` → `reqwest 0.11.27` → `rustls 0.21.12` | C-3 |
| RUSTSEC-2026-0104 | `rustls-webpki 0.101.7` | — | `evm` → `ethers` → `reqwest 0.11.27` → `rustls 0.21.12` | C-3 |

Traced, not assumed:

```
$ cargo tree -i ring@0.16.20
ring v0.16.20
└── jsonwebtoken v8.3.0
    └── ethers-providers v2.0.14
        └── ethers v2.0.14
            └── bnri-cosmic v0.1.0

$ cargo tree -i rustls-webpki@0.101.7
rustls-webpki v0.101.7
└── rustls v0.21.12
    └── hyper-rustls v0.24.2
        └── reqwest v0.11.27
            └── ethers-etherscan v2.0.14
                └── ethers v2.0.14
                    └── bnri-cosmic v0.1.0
```

### The four `ethers` advisories — owner: C-3

`cargo update` **cannot** clear these. `ethers 2.0.14` pins `jsonwebtoken 8.3.0`
and `reqwest 0.11.27` at major versions; the fixes need `ring ≥0.17.12` and
`rustls-webpki ≥0.103.12`. Only an `ethers` release could move them.

`evm` is default-off on **"vulnerable + unused"**, not on "ethers is sunset" —
that premise is **UNVERIFIED** and is deliberately not load-bearing. Today the
whole tree is pulled in by `src/wallet.rs`: 8 TODOs and four never-read fields,
i.e. the C-3 dispatch. Vulnerable code imported by a stub that is not wired.

**C-3's task one:** verify the `ethers` sunset against a real source, then settle
ethers-vs-alloy with the evidence real signing supplies. Turning `evm` on without
clearing these is a regression.

### The two `quick-xml` HIGHs — owner: first Linux GUI build

Reached only through `gui` → `libcosmic`, pinned at
`rev = 511384f6206527cf87369da67a9164831afbabef`. Fix is `quick-xml ≥0.41.0`,
which is libcosmic's to take. Whoever first builds the GUI on Linux should
confirm this revision is the one that produced 25/0 there and re-check the
advisory against the pinned rev.

### 8 unmaintained warnings

`fxhash`, `instant`, `paste`, `proc-macro-error2`, `rustls-pemfile`, `rustybuzz`,
`ttf-parser`, and one other. Warnings, not vulnerabilities; all reachable only
through the same two features. Recorded, not actioned.

---

## Other §4 items at this commit

| Item | Status |
|---|---|
| §4-C — no keys/mnemonics in tracked files | **PASS** |
| §4-C — `.gitignore` covers `target/` | **PASS** |
| §4-D — LICENSE in root (AGPL-3.0, byte-identical to the kernel's) | **PASS** |
| §4-D — SPDX header on every `.rs` | **PASS** — 10/10 |
| §4-B — no `*` versions; `Cargo.lock` tracked | **PASS** |
| §4-B — `libcosmic` pinned to a commit SHA | **PASS** — `511384f…` |
| §4-D — DCO sign-off on every commit | **DEVIATION** — see CONTRIBUTING.md |
| §4-B — `cargo audit` clean | **See Gate 2** — cannot pass while any optional dep carries an advisory; the lock is feature-agnostic |

## Honest limits of this document

- The `libcosmic` SHA pin is verified to **resolve**, not to **build**. libcosmic
  does not compile on the Windows host that produced this audit (`accesskit_winit`
  E0782) — a wrong-host artifact, not a defect, but it means no one has yet
  confirmed that this revision builds the GUI.
- `tests/sidecar_ipc.rs` cannot run here: no Python interpreter. 3 of its 4 tests
  fail by design rather than pass vacuously. **The 25/0 figure quoted elsewhere
  has never been reproduced on a host this project still has.**
- Gate 2's advisory data is as of the `cargo audit` run above, against RustSec's
  database at that moment (1160 advisories loaded). Re-run it; do not trust this
  table's age.
