# Backlog: ghost-status-closed-pipe

## What

`ghost status` exits quietly when its output pipe closes (as when piped
into `head`), instead of panicking.

## Why

The final Macro run saw the panic. Cosmetic, but a status command a person
pipes should not print a backtrace.

## Open questions

- None: handle a broken pipe on stdout as a normal end.
