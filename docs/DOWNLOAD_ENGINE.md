# Download engine (`sulfur-download`)

Installing Minecraft means one large jar, a few hundred libraries and
thousands of tiny assets (26.3: 5,216 files, 573 MiB). The engine is built for
both shapes of traffic at once.

## Pipeline per batch

1. **Deduplicate** tasks by destination and sort **largest first**, so long
   transfers start early and small files fill the remaining capacity.
2. **Check existing files**: a file with the right size and checksum is
   skipped (hashing runs on the blocking pool).
3. **Reuse**: if a verified copy exists in another game directory, it is
   hard-linked (or copied across volumes) instead of downloaded.
4. **Download** with a global connection budget; up to 2× as many files are
   in flight as connections, so hashing and disk I/O overlap the network.
5. Write to a unique `.part` file, **verify**, then atomically **rename**.

## Concurrency model

- One `Semaphore` caps concurrent HTTP requests (`max_connections`, default
  256, adjustable in Settings). Every request, small file or range, takes a
  permit.
- Downloads speak **HTTP/1.1** (`http1_only`) through one client whose pool
  keeps a connection per request in flight. HTTP/2 would multiplex many
  downloads over few TCP connections, which then share one congestion window
  and, behind a proxy, one tunnel: a single loss or stall slows all of them.
  Separate connections ramp up and recover independently, and the warm pool
  serves the next request without a new TLS handshake (the metadata fetched
  before an install went from ~5 s to ~0.5 s). `http_clients` spreads requests
  over several pools, which only matters with HTTP/2.
- Every file runs on its own Tokio task, so TLS decryption and inline hashing
  scale across cores.

## Large files: ranges with work stealing

The first request for a possibly large file asks for `bytes=0-`. If the server
answers `206` and the file is above `segment_threshold` (4 MiB):

- The file is preallocated, and the first response keeps streaming as range
  `[0, total)`.
- Helpers join as permits become free. Each one **splits the largest
  remaining range** in half and requests the second half (with the final,
  post-redirect URL).
- A worker that finishes picks up more work the same way, so the tail of the
  download is always shared.
- **Stall takeover**: a range that has not received data for 4 s is taken over
  whole by the next idle worker; the stalled owner notices its range is empty
  and moves on.
- Byte reservations go through one small mutex per file, so ranges never
  overlap even while they shrink.
- A failed range is released for another worker. If the file is still
  incomplete after a round, the next round **resumes** the remaining ranges
  instead of restarting the file.

Servers without range support simply stream the whole body.

## Reliability

- Retries with exponential backoff and jitter (250 ms → 8 s, `max_retries`).
- Sources rotate on transient errors; a source that refuses a file (404/403)
  is dropped immediately in favour of the next mirror.
- A connection with no data for `read_timeout` (10 s) is abandoned and retried.
- Cancellation is checked at every await point via `CancellationToken`.
- Progress is lock-free atomics (`Progress::snapshot()`); bytes from failed
  attempts are rolled back so the bar never overshoots.

## Mirrors

`MirrorSet` rewrites URL prefixes. `sulfur-minecraft::sources` maps Mojang's
hosts to BMCLAPI:

| Setting | Order |
|---|---|
| Automatic | official → mirror |
| Official | official only |
| Mirror | mirror → official |

## Tests

`cargo test -p sulfur-download` runs unit tests for range splitting and
stealing, plus end-to-end tests against a local range-capable HTTP server:
byte-exact segmented downloads, 200 small files with fallback, skip and reuse,
checksum failures leaving no files behind, and prompt cancellation.

## Tuning notes

Measured on 2026-10-30 (x86-64 desktop, through a local proxy) by installing
26.3 (582 MiB, 5,217 files) into an empty folder with `--no-reuse`:

| Setup | Official | BMCLAPI |
|---|---|---|
| HTTP/2, 128 connections over 32 clients (previous default) | 36.7 s | 91.0 s |
| HTTP/2, 256 connections over 64 clients | 29.6 s | |
| HTTP/2, 64 connections over 8 clients | 167.8 s (stalls) | |
| HTTP/1.1, 128 connections, one client | 30.0 s | 81.4 s |
| **HTTP/1.1, 256 connections, one client** (default) | **22.4 s** | **49.9 s** |

At 256 connections the official servers run close to the line's bandwidth.
Starting small files alongside the large ones (instead of largest first)
made no measurable difference. Useful tools:

```bash
cargo run --release -p sulfur-download --example fetch_list -- urls.txt out 256 1
```

```bash
cargo run --release -p sulfur-minecraft --example install -- 26.3 ./mc --official --no-reuse
```
