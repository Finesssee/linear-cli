# v0.3.5 Safety & CRUD Expansion - Implementation Summary

7 improvements across safety hardening, CRUD expansion, and test coverage.

## Safety Hardening (1 fix)
1. **interactive.rs** — Replaced bare `.unwrap()` with `.expect("selection out of range")` on menu action lookup (line 104)

## CRUD Expansion (4 new subcommands)
2. **notifications archive** — `notificationArchive` mutation for single notification
3. **notifications archive-all** — Batch archive with bounded concurrency (`buffer_unordered(10)`)
4. **cycles create** — `cycleCreate` mutation with `--team`, `--name`, `--description`, `--starts-at`, `--ends-at`
5. **cycles update** — `cycleUpdate` mutation with `--name`, `--description`, `--starts-at`, `--ends-at`, `--dry-run`

## Updated Help Text
- Cycles: added create/update examples
- Notifications: added archive/archive-all examples

## Test Results
- **167 unit tests** passing
- **66 integration tests** passing (was 58)
- **233 total** (was 225)
- **0 warnings** in build output

## Files Changed
5 files, 384 insertions, 3 deletions.
