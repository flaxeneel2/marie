# Riven Memory Reading (`--features memory`)

External reimplementation of the riven detection pipeline from `gep_warframeext.dll`,
as a pure `/proc/<pid>/mem` reader running in the privileged child process.
No DLL injection. No ptrace. No EAC interaction.

For the full DLL RE notes, see `gep-warframe/docs/detection-pipeline.md`.

---

## What the DLL Actually Does

The DLL **does not read riven stat values or stat names from Warframe memory.**

The complete JSON it emits is:
```json
{"name":"<weapon_name>","riven_details":[]}
```

`riven_details` is always an empty array. The weapon name is extracted from a Lua
debug string that Warframe writes to heap memory when the riven reroll UI opens:

```
"ThemedDetailedPurchaseDialog.lua: PopulateInfo-><WeaponName>"
```

Everything after the `->` up to the first non-printable byte is the weapon name.

**Riven stat names and values are not available via this approach.** Marie obtains
them through the WFM riven attributes API after identifying the weapon.

---

## High-Level Design

Two phases, same as the DLL:

```
Phase 1 — one-time code scan (~50–200 ms)
    Parse /proc/<pid>/maps → find Warframe.x64.exe .text base + size
    Read .text section via /proc/<pid>/mem
    Scan for Pattern C (CMP guard) → walk to 3rd LEA → heap_scan_va
    Scan for Pattern D-2 (byte null-check) → validity_flag_va
    Cache: (heap_scan_va, validity_flag_va) per Warframe PID

Phase 2 — poll loop (~0.2 ms per tick)
    pread validity_flag_va → 0 = riven UI closed, >0 = open
    if open: pread 4096 bytes from heap_scan_va
             search buffer for trigger string
             extract weapon name
    emit weapon name to parent
```

IPC request tags:

| Tag | Name | Payload | Response |
|-----|------|---------|----------|
| `0x03` | `ScanRivenCode` | (empty) | JSON `RivenAddresses` or error |
| `0x04` | `PollRivenState` | `[u64le heap_scan_va][u64le validity_flag_va]` | JSON `Option<String>` (weapon name) |

---

## Phase 1 — Locating Warframe's `.text` Section

```rust
fn find_warframe_text(pid: i32) -> Result<(u64, usize), String> {
    let maps = fs::read_to_string(format!("/proc/{pid}/maps"))?;
    for line in maps.lines() {
        let parts: Vec<&str> = line.splitn(6, ' ').collect();
        if parts.len() < 6 { continue; }
        let perms = parts[1];
        let path  = parts[5].trim();
        if !perms.contains('r') || !perms.contains('x') { continue; }
        if !path.ends_with("Warframe.x64.exe") { continue; }
        let (start_s, end_s) = parts[0].split_once('-').ok_or("bad range")?;
        let start = u64::from_str_radix(start_s, 16)?;
        let end   = u64::from_str_radix(end_s, 16)?;
        return Ok((start, (end - start) as usize));
    }
    Err("Warframe.x64.exe not found in maps".into())
}
```

On Linux/Proton, Wine loads `Warframe.x64.exe` typically at `0x140000000`; always
read the actual address — never assume.

---

## Phase 1 — Pattern Scanner

```rust
/// mask[i] == 0 → wildcard; mask[i] != 0 → must match
fn scan_pattern(buf: &[u8], pattern: &[u8], mask: &[u8]) -> Option<usize> {
    assert_eq!(pattern.len(), mask.len());
    let plen = pattern.len();
    'outer: for i in 0..buf.len().saturating_sub(plen - 1) {
        for j in 0..plen {
            if mask[j] != 0 && buf[i + j] != pattern[j] { continue 'outer; }
        }
        return Some(i);
    }
    None
}
```

---

## Phase 1 — Pattern C: Heap Scan Address

Finds the `cmp qword [rip+disp32]; je short; test byte` guard that gates riven UI visibility:

```
Pattern (11 bytes):  48 83 3d ?? ?? ?? ?? 74 ?? 84 ??
Mask   (11 bytes):   01 01 01 00 00 00 00 01 00 01 00
```

After the match, walk forward (using the embedded instruction walker or fixed offsets)
to find the **3rd `lea` instruction**. The `lea` encoding is:

```
48/4c 8d <ModRM=05> <disp32>    (7 bytes, RIP-relative)
resolved_va = instr_va + 7 + i32::from_le_bytes(disp32_bytes) as i64
```

Store as `heap_scan_va`. This is the address in Warframe's data segment where the
Lua debug output buffer resides.

---

## Phase 1 — Pattern D-2: Validity Flag

Finds `cmp byte [rip+disp32], 0; mov reg,[mem]; jne rel32`:

```
Pattern (13 bytes):  80 3d ?? ?? ?? ?? 00 48 8b ?? ?? 0f 85
Mask   (13 bytes):   01 01 00 00 00 00 01 01 01 00 00 01 01
```

Resolve the `cmp byte [rip+disp32]` operand:

```rust
let disp32 = i32::from_le_bytes(buf[match_off+2..match_off+6].try_into().unwrap());
let validity_flag_va = (text_va + match_off as u64 + 7).wrapping_add(disp32 as i64 as u64);
```

`validity_flag_va` is a single byte in Warframe's data segment:
- `0` → riven reroll UI is **closed**
- non-zero → riven reroll UI is **open**

---

## Result Struct

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RivenAddresses {
    pub heap_scan_va:     u64,   // VA to read 4096-byte buffer from
    pub validity_flag_va: u64,   // VA of UI visibility byte
    pub warframe_pid:     i32,
}
```

---

## Phase 2 — Polling

```rust
const TRIGGER: &[u8] = b"ThemedDetailedPurchaseDialog.lua: PopulateInfo->";
const TRIGGER_LEN: usize = 48;

fn handle_poll_riven(addrs: &RivenAddresses) -> Result<Option<String>, String> {
    if !pid_is_warframe(addrs.warframe_pid) {
        return Err("PID stale".into());
    }
    let mem = fs::File::open(format!("/proc/{}/mem", addrs.warframe_pid))?;

    // 1. Check visibility flag
    let mut flag = [0u8; 1];
    mem.read_at(&mut flag, addrs.validity_flag_va)?;
    if flag[0] == 0 {
        return Ok(None);
    }

    // 2. Read heap buffer
    let mut buf = [0u8; 0x1000];
    mem.read_at(&mut buf, addrs.heap_scan_va)?;

    // 3. Search for trigger string
    let pos = buf.windows(TRIGGER_LEN)
        .position(|w| w == TRIGGER)?;

    // 4. Extract weapon name (text after "->", up to first non-printable)
    let name_start = pos + TRIGGER_LEN;
    let name_bytes = buf[name_start..]
        .iter()
        .take_while(|&&b| b >= 0x20)
        .copied()
        .collect::<Vec<u8>>();

    if name_bytes.is_empty() {
        return Ok(None);
    }

    Ok(Some(String::from_utf8_lossy(&name_bytes).into_owned()))
}
```

---

## Integration with Marie

### EE.log trigger

The `riven-reroll` event from `ee_log.rs` drives polling:

1. Parent sends `ScanRivenCode` (once per PID); receives and caches `RivenAddresses`.
2. Spawns a 1 Hz Tokio interval that sends `PollRivenState`.
3. On `Some(weapon_name)` response:
   - Cancel the poll task.
   - Emit `riven-memory-data` Tauri event with weapon name.
   - Overlay looks up WFM riven listings for that weapon and renders prices.

### Tauri command

```ts
const weaponName = await invoke<string | null>("get_riven_memory_state");
```

```rust
#[tauri::command]
async fn get_riven_memory_state() -> Option<String> {
    account_memory::poll_riven_name()
}
```

---

## Scan Performance Estimate

| Step | Cost |
|------|------|
| `/proc/<pid>/maps` parse | < 1 ms |
| `.text` read (80 MB typical) | 20–80 ms cold, < 5 ms warm |
| Pattern scan (2 patterns × 80 MB) | 10–40 ms naive, < 5 ms with SIMD leading-byte filter |
| `pread` validity_flag (1 byte) | < 0.05 ms |
| `pread` heap buffer (4096 bytes) | < 0.1 ms |
| `memchr`+`memcmp` for trigger | < 0.05 ms |

---

## Error Cases

| Condition | Handling |
|-----------|---------|
| Pattern not found | Return error; Warframe was patched |
| PID stale | Re-run `ScanRivenCode` |
| `pread` fault | Re-scan |
| Trigger not in buffer | `None`; UI open but string not yet written |
| Name bytes empty after `->` | `None`; retry next poll |

---

## Known Limits

- **Stat values not available:** The DLL does not read riven stat floats or stat names from
  Warframe memory. Marie must use the WFM riven attributes API to get stat metadata and
  the WFM listings API to get prices.
- **Pattern D-1 (`riven_ptr`) not used:** The CB D pattern 1 result points into Warframe's
  UI widget tree (used by the DLL's chat feature), not a riven stat struct. Do not implement
  pointer-chain traversal for stat data.
- **Reroll detection:** `validity_flag_va` is non-zero while the riven UI is open, not
  only on reroll. Compare weapon names between polls to detect a new roll.
