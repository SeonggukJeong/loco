# Forensic: verification-reach transcripts (pre length-salvage)

## Scope
Post-hoc on local `.loco/eval` stamps that ran `run_command` (n≥269 of ~335 scanned). Focus: fd-1873-path-sep treatment smokes.

## B1/B2 relevance (fd-1873)

| stamp | outcome | turns | rc | pipes | same_read max | B1/B2 note |
|---|---|---|---|---|---|---|
| 145615Z brevity | rep_stop | 29 | 3 | 2 | **5× tests.rs@2790** | **B1 flagship** after `cargo test \| tail` exit 0 with FAILED in body |
| 131902Z first smoke | max_turns | 24 | 12 | 7 | 1 | **B2 flagship** — almost all test cmds piped; exit 0 masks fail |
| 135116Z t1800 | rep_stop | 18 | 3 | 3 | 1 | all 3 tests piped (`head`/`tail`/`grep`) |
| 133940Z | timeout | 8 | 2 | 2 | 1 | both tests piped |
| 151524Z B1/B2 code | timeout | 4 | **0** | 0 | 1 | **never reached verify** — length loop first |

## Failure modes (ordered by how often they kill the run)

1. **Pipe-verify (B2)** — `cargo test … | tail` / `| head` → exit 0 of pager; body still has FAILED. Model treats as green or confuses.
2. **Stuck re-read (B1)** — after failed tests visible in log, re-read same offset (e.g. tests.rs:2790×5) instead of grepping failed names.
3. **Length / JSON stutter (pre-B1)** — completion maxes out by *repeating* a short valid JSON turn (~192–2kB object × N) until 4096 tokens. `LENGTH_RECOVERY` de-dupes and asks for shorter; each unit is already short → loop burns wall-clock → Timeout. **Salvage first complete JSON would recover the turn.**
4. Wrong fix content (message without `('/')` etc.) — orientation/solution accuracy; not harness nav.

## Length blob salvage probe (151524Z)

Using streaming JSON decode on transcript assistant blobs:

| blob | bytes | first complete object | tool |
|---|---|---|---|
| 1 | 15813 | 192 B | grep |
| 2+ | 16154 | 1986 B | edit_file (search/replace complete) |

Conclusion: length path should **try `parse_turn` before discard**.

## Follow-up (same session)

Implemented: salvage (`d12756d`), prose cap (`14c57c3`), stutter cap keep (`84ed9a9`).
See parent `README.md` smoke table for post-fix stamps.
