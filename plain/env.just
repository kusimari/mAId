# Where plain keeps and finds its things, passed to plain in the
# environment. Imported by plain's Justfile and by the ghost's, which runs
# plain's install as a library.

export MAID_PROFILE := env("MAID_PROFILE", env("XDG_STATE_HOME", env("HOME") / ".local/state") / "maid/profile")
# Every package's install manifest, one file each; mAId's is maid.json.
export INSTALL_RECORDS := env("INSTALL_RECORDS", env("XDG_STATE_HOME", env("HOME") / ".local/state") / "install-records")
# The old part lists (backlog drop-old-part-lists), read for one release
# (mAId's old maid-harness.json is migrated on install), are where older
# writers put them: under the state dir's old name, my-harness. The ghost's
# own state dir is ghost-claude's to find (ghost::config), so it is not
# exported here.
export GHOST_IN_SHELL_PARTS := env("GHOST_IN_SHELL_PARTS", env("MY_HARNESS_PARTS", env("MY_HARNESS_STATE", env("XDG_STATE_HOME", env("HOME") / ".local/state") / "my-harness") / "parts"))
# mAId's user-owned config: each skill's state dir (Claude may read and write those only), the
# browser allowlist and the memory store's file.
export MAID_CONFIG := env("MAID_CONFIG", env("XDG_CONFIG_HOME", env("HOME") / ".config") / "maid")
# The file naming mAId's personal memory store.
export MAID_MEMORY_STORE := env("MAID_MEMORY_STORE", MAID_CONFIG / "memory-store")
