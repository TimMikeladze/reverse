# Evals: does the toolchain make a model write better Verse?

Ten small tasks. Each has a prompt (`task.md`), hidden tests (`tests.verse`) and a reference
solution (`reference.verse`). A solution counts only if it checks without errors and passes
every test.

## Protocol

Run each model in two conditions and compare the solve rate:

1. **Bare.** Give the model only `task.md`. Save its answer as `<task>.verse`.
2. **With tools.** Give the model `task.md` plus the reverse MCP server (or shell access to
   the `reverse` binary) and the skill in `agents/skills/verse/`. Let it iterate.

Score a directory of answers:

```
python3 evals/score.py path/to/answers          # human-readable
python3 evals/score.py path/to/answers --json   # for dashboards
```

Useful measurements beyond the solve rate: errors on the first attempt by diagnostic code
(this shows which mistakes to target with better diagnostics or guide entries), and the
number of check calls needed to reach a clean compile.

## What the tasks cover

| Task | Verse feature under test |
| --- | --- |
| `clamp` | `if` / `else` as an expression |
| `safe_average` | `<decides>`, int to float conversion |
| `first_even` | options, failure inside `option{}` |
| `fizzbuzz` | `Mod[]`, string interpolation |
| `bank_account` | classes, `var` fields, rollback on failure |
| `char_counts` | maps, element assignment in a failure context |
| `traffic_light` | enums, exhaustive `case` |
| `shapes` | abstract classes, `<override>` |
| `stack` | value-type arrays inside a class, failable methods |
| `pairs_with_sum` | `for` with several generators and filters |

The tasks avoid Fortnite APIs on purpose: they measure the language, which is what this
toolchain can verify. The reference solutions are checked by the test suite.
