# A second central bank in one household

Status: the second bank runs (September 2026). Interbank transfers are not
designed yet; this note only explains why the second bank exists and what it
is.

## Why would one household have two central banks?

Nobody does this on purpose, which is the point. NanaCoin exists to teach how
money works, and one bank can only teach money *inside* one economy. Some of
the most interesting lessons happen at the border between two:

- **Exchange rates.** "How many NanaCoins is one S2-coin worth?" has no answer
  until two groups of people start trading. A price that nobody decrees is a
  much better lesson than a fixed peg.
- **Monetary policy has consequences next door.** If one bank's Nana issues
  generously, its coin should get cheaper against the other. That is
  inflation you can see, without anyone announcing it.
- **Trust is not free.** A payment inside one bank is one atomic ledger write.
  A payment between banks crosses two ledgers, two clocks and a Wi-Fi network
  that can fail at the worst moment. "The money left here but did not arrive
  there" is a real problem, and a household can watch it being solved.
- **Engineering honesty.** Local atomicity does not make two boards atomic.
  A second, separately powered bank is the cheapest way to prove the
  protocol handles that, before anyone relies on it.

A cheap single-core ESP32-S2 Mini is enough hardware for the second bank, and
it keeps the first bank's ledger out of every experiment.

## What the second bank is

| | First bank | Second bank |
|---|---|---|
| Address | `https://nanacoin.local` | `https://nanacoin-s2.local` |
| Board | ESP32-S3-N16R8 (2 cores, 8 MiB PSRAM, 16 MiB flash) | ESP32-S2 Mini (1 core, 2 MiB PSRAM, 4 MiB flash) |
| Build profile | default (`s3`) | `--features board-s2` |
| Household, members, ledger | its own | its own, created fresh |
| Currency | its own | **its own**, a separate currency |
| TLS | leaf for `nanacoin.local` | leaf for `nanacoin-s2.local`, signed by the same household CA |

Each bank is a complete, independent NanaCoin: the same application and the
same Angular client, with every feature. Neither bank is a replica, mirror or
client of the other. Being a member of one says nothing about the other.

The S2's smaller memory means smaller *retention and connection bounds*, not
fewer features (`src/board.rs`):

| Bound | S3 | S2 |
|---|---:|---:|
| Recent transactions kept in RAM | 3,000 | 300 |
| Journal records before a checkpoint is forced | 4,096 | 512 |
| Ledger corrections / audit cache | 4,096 / 1,024 | 512 / 128 |
| Physical fulfilment records | 128 | 32 |
| Archive pages (4 KiB) | 1,024 | 128 |
| Largest API response | 512 KiB | 192 KiB |
| Concurrent TLS / HTTP clients | 8 / 4 | 3 / 2 |
| Web assets | identity + gzip | gzip only |
| HTTP/API worker | its own task on core 1 | the main task, with the payment tick between turns |
| TLS/diagnostics/LED task stacks | internal RAM | PSRAM (they never write flash) |
| Status light | WS2812 RGB (GPIO48) | blue LED (GPIO15), same Morse rotation |

Members, listings, offers, loans, lottos, quotes and the catalogue keep the
S3's limits. Older history on the S2 is still archived to flash, just less of
it. The S3 build never uses these smaller bounds.

## Money between the banks

Each bank issues its own currency. Money does not "move" between banks; it is
**exchanged**. Holding the other bank's coin is holding a foreign currency,
not local money, and it never counts toward a bank's own issuance.

How an exchange settles between two ledgers (reservations, acceptance,
timeouts, replay protection and reconciliation) is deliberately not designed
here. The constraints already listed under *Federation* in `../../roadmap.md`
apply, and a separate reviewed spec comes before any code.
