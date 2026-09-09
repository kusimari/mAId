# kdevkit feature-loop tools

Split by role, so each part can be reviewed on its own. Read them in this
order — each depends only on the ones above it.

Three files, split by **who invokes them**:

| File | Invoked by | Role |
|---|---|---|
| `driver` | the coding agent | `install`, and every verb a builder runs. Decides; refuses an inconsistent record, never a judgement. |
| `hooks/*` | git | records the stage as a side effect of committing; gates the push |
| `state` | both of the above | the record itself — read, store, update. The only place that knows the record is commit trailers. |

The split matters for one reason: **the stage is recorded by git, not by the
agent.** `install` makes that possible, `hooks/*` do it, `driver` is how a
builder asks about it, and `lib/state.sh` is the single answer all of them
agree on.

## Using it

```sh
# once per checkout
"$K/driver" install

# afterwards, named from the repository — install records where the tools are
"$(git config kdevkit.tools)/driver" show
```

## The prose path

The same five verbs can be carried out by hand, recorded in the spec's
handoff section with an append-only `### Crossings` list. The two paths are
equals, and a feature is finished on the one it started on — `driver install`
refuses on a branch already keeping its record in prose. See `SKILL.md`,
*Crossing a stage boundary*.

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
