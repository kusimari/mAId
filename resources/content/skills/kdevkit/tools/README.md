# kdevkit feature-loop tools

Split by role, so each part can be reviewed on its own. Read them in this
order — each depends only on the ones above it.

| File | Role | Writes? |
|---|---|---|
| `lib/state.sh` | **What the repository shows.** Facts, and whether a stage change is legal. | never |
| `driver` | **What a coding agent invokes.** Asks where work stands, moves it on, goes back, proceeds on the record. | the intent file only |
| `hooks/prepare-commit-msg`, `hooks/pre-push` | **What git invokes.** Records the stage as a side effect of committing; gates the push. | commit messages |
| `install` | **Wiring git for one checkout.** Points git at the hooks; records where these tools are. | git config |
| `feature-loop` | compatibility shim forwarding to `driver` / `install` | — |

The split matters for one reason: **the stage is recorded by git, not by the
agent.** `install` makes that possible, `hooks/*` do it, `driver` is how a
builder asks about it, and `lib/state.sh` is the single answer all of them
agree on.

## Using it

```sh
# once per checkout
"$K/install"

# afterwards, named from the repository — install records where the tools are
"$(git config kdevkit.tools)/driver" show
```

Verbs, by what you are doing:

```
ASKING   show · facts · next · check --to <stage>
MOVING   advance --next · advance --to <stage>
         return --to <stage> --fault-entered L --issue T --expected-fix T --acceptance T
         except --skipping W --why W
GATES    verify
```

## Per-project configuration

Declared in `specs/project.md` under `### kdevkit`, where a project already
declares its reviewer — not in git config, which is per-clone and would leave
a fresh clone with nothing:

```
### kdevkit
- `gates:`
  - `dev:`
    - `quality: just lint`
    - `tests: just test`
```

Design and rationale: `specs/feature/kdevkit-deterministic-phasing.md`.
