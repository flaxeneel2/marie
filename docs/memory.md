# Memory feature (`--features memory`)

Linux-only. Requires root. Unlocks account-gated features by extracting the
Warframe authentication nonce from the game's process memory.

## Why root, why no ptrace

Reading `/proc/<pid>/mem` via `pread()` as root triggers the kernel's
`ptrace_may_access(PTRACE_MODE_READ_FSCREDS)` check, which passes for euid=0
without an actual ptrace-attach. The `ptrace()` syscall is **never invoked**, so
EAC's ptrace hook is not triggered.

Running without root: the scan is silently skipped; the app falls back to
standard (EAC-safe) mode and `get_account_info` returns null.

## Data collected at startup

| Field | Source |
|-------|--------|
| `username` | EE.log login line |
| `account_id` | EE.log login line |
| `nonce` | process memory scan |

### Account ID — EE.log

Pattern matched on every `Logged in` line; the last match wins (handles
relog):

```
12.928 Sys [Info]: Logged in flaxeneel2 (5b911d7cf2f2ebf3da0de6ab)
                              ^^^^^^^^^^  ^^^^^^^^^^^^^^^^^^^^^^^^
                              username    account_id (24-char hex)
```

### Nonce — memory scan

The client keeps an active session URL somewhere in heap/anonymous memory:

```
?accountId=5b911d7cf2f2ebf3da0de6ab&nonce=<value>
```

Scan procedure (`account_memory::scan_nonce`):

1. Parse `/proc/<pid>/maps` — keep only readable (`r`) anonymous/heap/stack
   regions; skip file-backed mappings and regions > 64 MiB.
2. Open `/proc/<pid>/mem` with `O_RDONLY`.
3. For each region, read in 64 KiB chunks with a 256-byte overlap (so patterns
   straddling a chunk boundary are caught). Use `pread()` — no seek, no state.
4. Search each buffer for the needle bytes. Extract alphanumeric characters
   after `&nonce=` until a non-alphanumeric terminator.

## Build

```bash
# dev
bun run tauri dev -- --features memory

# production
bun run tauri build -- --features memory
```

The standard build (`bun run tauri dev`) compiles without this feature; both
variants share the same binary name.

## Tauri command

```ts
import { invoke } from "@tauri-apps/api/core";

const info = await invoke<{
  username: string;
  account_id: string;
  nonce: string | null;
} | null>("get_account_info");
```

Returns `null` in the standard build or when the scan was skipped (not root /
game not running at startup).

## Future work

- Re-scan on EE.log `Logged in` events (handles mid-session relog).
- Use `account_id` + `nonce` to call the Warframe account API for mastery data.
- Mastery overview UI module.
