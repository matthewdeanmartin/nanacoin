# Security policy

## Supported versions

NanaCoin is a pre-release household currency project maintained on a
best-effort basis. Security fixes target the current `main` branch; there is
no supported release maintenance matrix yet.

| Component | Receives fixes |
| --- | --- |
| Current Rust server and firmware (`nanacoin_rs`) | Yes |
| Current Angular client and public demo (`nanacoin_ui`) | Yes |
| Current MicroPython static host (`nanacoin_web`) | Yes |
| Older commits and archived Go/TinyGo implementation | No; update to the active implementation |

## Reporting a vulnerability

Use [GitHub private vulnerability reporting](https://github.com/matthewdeanmartin/nanacoin/security/advisories/new).
If that is unavailable, email **matthewdeanmartin@gmail.com**.
Keep exploitable findings private while a fix is being coordinated.

Include the affected commit, component, desktop OS or board profile, browser,
relevant configuration, reproduction steps, expected and actual behavior,
and the impact. A minimal proof of concept or suggested fix is useful.
Sanitize logs and examples: omit passwords, PINs, session/OAuth tokens, Wi-Fi
credentials, private certificate keys and personal household ledger entries.

The maintainer will review reports and coordinate a fix and disclosure on a
best-effort basis. This project has no dedicated security team, guaranteed
response deadline or bug bounty. Discuss disclosure timing and attribution
with the maintainer; anonymous credit is welcome.

## Scope and safe investigation

Relevant findings include authentication or Nana-role bypass, cross-site
scripting, unauthorized access to private messages or account data, forged or
duplicate payments, ledger corruption, unsafe recovery, and exposed credentials.
Dependency vulnerabilities are relevant when they affect NanaCoin's use of the
dependency; describe the reachable path when possible.

Nana is the trusted household administrator. Her documented ability to issue,
retire and reverse currency is intentional; another user gaining that authority
is a vulnerability. Test against your own local instance and synthetic data.
Ask an instance owner before investigating their board or accounts. The public
demo uses disposable browser-local data; it does not authorize testing other
people's services or accounts.

## Deployment and dependency hygiene

Protect household deployments with HTTPS and a trusted certificate. Keep them
on the intended household network; do not assume a local hostname is access
control. Plain HTTP development proxies belong on trusted local machines.
Keep ledger backups, private keys and local credentials private. Firmware may
embed Wi-Fi configuration: review artifacts before sharing them.

Use the committed npm and Cargo lockfiles. `npm audit` in `nanacoin_ui` reports
known npm advisories. For Rust, install `cargo-audit` and run `cargo audit` from
`nanacoin_rs`. Evaluate applicability and test dependency updates rather than
blindly forcing major upgrades. These checks do not replace a security review.
Development-data resets are permitted by [AGENTS.md](AGENTS.md), but current
accounting and durable-write invariants still require protection.

## Policy reference

The private reporting and reproduction guidance follows the structure of
[GitHub's organization SECURITY.md](https://github.com/github/.github/blob/main/SECURITY.md),
adapted to NanaCoin's maintainer and support scope. GitHub's corporate reporting
address, bounty program and safe-harbor policy do not apply to this project.
