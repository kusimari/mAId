# kdevkit phase tools

`phase` reads git and the feature spec to answer factual questions about
where a feature stands, and refuses stage changes the repository
contradicts. It also owns the map: a phase module states what it must
achieve and asks `phase advance --next` where that leads, so adding a
stage never means editing the module before it.

`advance` moves forward only, along a closed table. Going back is
`return`, which accepts any earlier stage but demands the fault, the
issue, the expected fix and the acceptance criterion. The two hooks let git do the recording, so the stage is
never something an agent has to remember to write.

Nothing here is installed into the project being worked on. `phase
install` points the checkout's `core.hooksPath` at this directory,
chaining any hook that was already there.

    phase install                     wire git to these hooks
    phase show                        where does this feature stand?
    phase facts                       every fact, one key=value per line
    phase next                        what follows the current stage?
    phase advance --next              move on, without naming where
    phase check --to review           may we move there?
    phase advance --to review         record a named move
    phase return --to planning \
        --fault-entered requirements \
        --issue ... --expected-fix ... --acceptance ...
    phase verify                      run the project's checks, record the tree
    phase uninstall                   unwire and clear kdevkit state

Two settings per project, read from git config, because every project
names its own commands:

    git config kdevkit.qualityCommand 'just lint'
    git config kdevkit.testCommand    'just test'

Design and rationale: `specs/feature/kdevkit-deterministic-phasing.md`.
