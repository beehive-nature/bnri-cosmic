# Security Policy

## Reporting a vulnerability

Email **beehivenature@protonmail.com**.

**Do not open a public GitHub issue for security vulnerabilities.**

Include:

- Description of the vulnerability
- Steps to reproduce
- Affected versions / commit SHAs
- Proposed fix (optional)

We do not publish a PGP key at this time. When one exists, its fingerprint will
be listed here — and not before. A placeholder fingerprint would be worse than
none: it invites people to encrypt to a key that cannot decrypt.

## Response timeline

- Acknowledgement: within 48 hours
- Initial assessment: within 7 days
- Fix or mitigation: within 30 days (severity-dependent)
- Public disclosure: after a fix is released, coordinated with the reporter

## Scope

- `bnri-cosmic` (this repo)
- `beehive-nature` (the kernel)
- `bnri-design` (the spec) — when it lands
- `bnri-contracts`, `bnri-explorer` — when they land

## Out of scope

- Vulnerabilities in dependencies (report upstream). Known advisories carried by
  this crate's optional features are tracked in [`docs/AUDIT.md`](docs/AUDIT.md)
  with named owners — those are already on the record, not new findings.
- Social engineering
- Physical attacks on hardware wallets
- Issues requiring compromised root keys

## Known-issue transparency

This project treats a document asserting a property its mechanism does not have
as a defect in itself, not as documentation of one. If you find a claim in this
repo that the code does not honour — a comment, a README line, an audit row —
that is a security-relevant finding and we want it, even where nothing is
exploitable. Several have already been fixed on exactly that basis.

## Rewards

We do not operate a bug bounty at this time. Reporters are credited in release
notes unless they prefer otherwise.
