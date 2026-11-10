# Where plain keeps and finds its things, passed to plain in the
# environment. Imported by plain's Justfile and by the ghost's, which runs
# plain's install as a library.

export MAID_PROFILE := env("MAID_PROFILE", env("XDG_STATE_HOME", env("HOME") / ".local/state") / "maid/profile")
# Every package's install manifest, one file each; mAId's is maid.json.
export INSTALL_RECORDS := env("INSTALL_RECORDS", env("XDG_STATE_HOME", env("HOME") / ".local/state") / "install-records")
# My harness's state (the core's delivery record and pending decisions), with
# the old part lists under it, read for one release (mAId's old
# maid-harness.json is migrated on install). ghost-claude reads the same three,
# with the same defaults.
export MY_HARNESS_STATE := env("MY_HARNESS_STATE", env("XDG_STATE_HOME", env("HOME") / ".local/state") / "my-harness")
export MY_HARNESS_PARTS := env("MY_HARNESS_PARTS", MY_HARNESS_STATE / "parts")
# mAId's user-owned config: each skill's state dir (Claude may read and write those only), the
# browser allowlist and the memory store's file.
export MAID_CONFIG := env("MAID_CONFIG", env("XDG_CONFIG_HOME", env("HOME") / ".config") / "maid")
# The file naming mAId's personal memory store.
export MAID_MEMORY_STORE := env("MAID_MEMORY_STORE", MAID_CONFIG / "memory-store")
