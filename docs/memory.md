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

## Process architecture — fork + privilege separation

The memory build uses a **privileged child process** that stays root permanently,
allowing future kernel-level operations (page-cache reads, eBPF, etc.) without
re-elevating. The parent (Tauri UI) drops to the real user immediately after
fork so D-Bus / portal / Wayland connections work normally.

```
sudo marie
    │
    ├─ [parent, euid=0]  create socketpair(AF_UNIX, SOCK_STREAM|CLOEXEC)
    │                     fork()
    │                     ──────────────────────────────────
    │                                   │
    │                          [child, stays root]
    │                           apply seccomp denylist
    │                           enter request loop
    │                                   │
    │◄──── REQ_SCAN_ACCOUNT ────────────┤
    │──── RESP (AccountInfo JSON) ──────►│
    │                                   │ (waiting for next request)
    │  drop_privileges() → real UID
    │  tauri::Builder::default()...
    │  D-Bus / portal / GTK connect OK
```

## IPC protocol

Wire format over the anonymous `SOCK_STREAM` socketpair:

**Request** (parent → child):
```
[u8: tag] [u8: payload_len] [payload: 0..255 bytes]
```

**Response** (child → parent):
```
[u8: status] [u16le: body_len] [body bytes]
status: 0x00 = OK, 0x01 = Err
```

**Defined request tags:**

| Tag | Name | Payload | Response body |
|-----|------|---------|---------------|
| `0x01` | `ScanAccount` | (empty) | JSON `AccountInfo` |
| `0x02` | `ReadLogBuffer` | (empty) | 16 KiB raw bytes from VA `0x589000` |

Future tags (planned): `EBpfProbe`, etc.

## IPC hardening

### Why the channel is inherently authenticated

`socketpair()` creates an anonymous kernel-managed pair — no filesystem path,
no external process can connect or discover it. Only the two file descriptors
(one per side) exist, shared solely between parent and child via fork. No
authentication primitives (HMAC, tokens) are needed.

### `SOCK_CLOEXEC`

Both FDs are created with `SOCK_CLOEXEC`. If either process ever `exec()`s,
the FDs are not inherited by the new image.

### Zombie prevention — `SIGCHLD = SIG_IGN`

Parent sets `SIGCHLD` to `SIG_IGN` immediately after fork. Linux auto-reaps
child exit status when the disposition is `SIG_IGN`, preventing zombie
accumulation for long-running sessions.

### Child exits on parent disconnect

Child's request loop calls `read_exact()`. When the parent process exits (or
drops its socket FD), the read returns 0 bytes (EOF) and the child calls
`process::exit(0)`. No orphaned root process lingers.

### Protocol hardening

- Fixed max request size (257 bytes). Child reads exactly `payload_len` bytes,
  rejects oversized frames.
- Command allowlist in child's match arm. Unknown tag → send `RESP_ERR`,
  stay alive (no panic, no state corruption).
- No shell execution or `exec` anywhere in the child. Enforced additionally
  by seccomp (see below).

### Warframe PID re-verification

Before reading `/proc/<pid>/mem`, child re-checks `/proc/<pid>/cmdline` still
contains `Warframe.x64`. Defeats PID reuse attacks (Warframe exits → attacker
races to claim the PID before the child reads memory).

### seccomp denylist

Applied in the child immediately on entry, before the request loop. Uses a
classic BPF denylist — the explicitly blocked syscalls receive
`SECCOMP_RET_KILL_PROCESS`; everything else is allowed. Denylist (not
allowlist) because Rust's stdlib and allocator use a broad set of syscalls that
is difficult to enumerate exhaustively.

**Blocked syscalls:**

| Syscall | Threat prevented |
|---------|-----------------|
| `execve`, `execveat` | Shell / binary execution |
| `fork`, `vfork`, `clone`, `clone3` | Process / thread spawning |
| `ptrace` | Tracing other processes |
| `socket`, `socketpair` | Creating new network/IPC channels |
| `connect`, `bind`, `listen` | Network connections |
| `sendmsg`, `sendto` | Data exfiltration via existing sockets |

The child can still read `/proc` files, write to its existing socket FD, and
use the allocator — none of those require the blocked syscalls.

## Privilege drop (parent side)

After the initial synchronous `ScanAccount` response is received,
`drop_privileges()` calls `setresgid(real, real, real)` then
`setresuid(real, real, real)` to restore the invoking user's UID/GID (the
real-uid fields from `/proc/self/status`, preserved by sudo). This is required
because the D-Bus session bus rejects root connections via `SO_PEERCRED`.

## Data collected at startup

| Field | Source |
|-------|--------|
| `username` | EE.log login line |
| `account_id` | EE.log login line |
| `nonce` | process memory scan |

### Account ID — EE.log

Last `Logged in` line wins (handles relog):

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

Scan procedure:

1. Parse `/proc/<pid>/maps` — keep only readable (`r`) anonymous/heap/stack
   regions; skip file-backed mappings and regions > 64 MiB.
2. Open `/proc/<pid>/mem` with `O_RDONLY`.
3. Read in 64 KiB chunks with a 256-byte overlap (catches patterns straddling
   chunk boundaries). Use `pread()` — no seek, no state.
4. Search for needle bytes. Extract alphanumeric chars after `&nonce=` until
   a non-alphanumeric terminator.

## Build

```bash
# dev
bun run tauri dev -- --features memory

# production
bun run tauri build -- --features memory
```

Both variants share the same binary name.

## Tauri command

```ts
import { invoke } from "@tauri-apps/api/core";

const info = await invoke<{
  username: string;
  account_id: string;
  nonce: string | null;
} | null>("get_account_info");
```

Returns `null` in the standard build or when the scan was skipped.

## EE.log ring buffer

`ReadLogBuffer` (tag `0x02`) reads 16 KiB at VA `0x589000` — the wine/Proton
pre-reserved region that holds Warframe's in-process EE.log ring buffer. This
region is stable across restarts (wine bypasses ASLR for its auxiliary area).

The parent polls this every 150 ms via `account_memory::read_log_buffer()`.
`ee_log::start_memory_watcher` processes the raw bytes with edge detection
(not line-by-line), emitting the same Tauri events as the disk watcher but with
sub-200 ms latency instead of up to 10 s flush delay.

See [RE docs](../../games/warframe/dump/RE/docs/riven-screen-detection.md) for
full reverse-engineering notes, address stability validation, and buffer layout.

## Future work

- Re-scan on EE.log `Logged in` events (handles mid-session relog).
- Use `account_id` + `nonce` to call Warframe account API for mastery data.
- Mastery overview UI module.
