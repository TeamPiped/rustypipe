# Cold-start vs. real PoToken — the first SABR request

The full `base.js` is at `../base.js.full` (2.4MB, 8872 lines, player_es6 build `ac678d18`).

## TL;DR

- **The browser does send a "dummy" 10-byte placeholder on the first SABR request.** It's a `PoTokenMsg{field_4: 8 random/XOR bytes}`, identical in shape to what `downloader/src/lib.rs:1514-1520` produces.
- **The placeholder is generated synchronously by `dP4.D()`** (line 6023) — no BotGuard round-trip, no challenge, no GenerateIT.
- **The placeholder is used because the SWPO shared minter (`M.j`) isn't built yet** when `videodatachange` fires. The on-demand minter `KJ4` (line 1064) is installed as a fallback that produces placeholders inline.
- **On `sps=2`/`sps=3` response (the server returning `spsumpreject`), the player triggers the full `cV` (challenge) + `LT` (GenerateIT) pipeline** to mint a real token, then retries.

## Code path

```text
emf (PoToken manager class, line 7780)
  ├─ constructor (line 7780):
  │   ├─ this.useLivingRoomPoToken = false
  │   ├─ this.S = new g.BM          // init promise
  │   ├─ this.U = false             // SWPO ready
  │   ├─ this.V = false             // "we have a token"
  │   ├─ this.D = null              // on-demand (KJ4) minter
  │   └─ csiinitialized handler (line 7783):
  │       ├─ if html5_web_po_on_demand_init → this.D = KJ4()
  │       └─ else → f0H(this) → eG(this) (build SWPO)
  │
  ├─ s0(M) (line 7784):             // content token mint
  │   if (this.j || this.D) {
  │     M.Xk = jG(this, M.videoId);
  │     if (!this.U) { this.S.promise.then(() => M.Xk = jG(...)) }
  │   }
  │
  └─ jG(M, k) (line 4889):          // actual mint dispatcher
      if (!M.j) {                    // SWPO not built
        if (M.D) try { return M.D(k) } catch { ... }  // ← on-demand path
        return ""                   // empty token
      }
      try { return M.j.z3(z) }      // real SWPO path
      catch { return "", g.N9(z) }
```

## dP4.D — the cold-start placeholder (line 6023-6024)

```js
dP4=class extends Tt{
  constructor(M,k,z){ super(M); this.S=k; this.clientState=z; this.U="S"; this.j="q" }
  D(){
    var M = Math.floor(Date.now()/1E3),                  // unix seconds
        k = [Math.random()*255, Math.random()*255],       // 2-byte XOR key
        z = k.concat([this.S&255, this.clientState],     // counter (0), clientState (1)
                     [M>>24&255, M>>16&255, M>>8&255, M&255]); // ts (4 bytes)
    M = new Uint8Array(2+z.length);
    M[0] = 34;                                           // 0x22 = field-4 tag
    M[1] = z.length;                                     // 0x08 = length
    M.set(z, 2);
    z = M.subarray(2);
    k = k.length;
    for (let T=k; T<z.length; ++T) z[T] ^= z[T%k];       // XOR with 2-byte key
    return M;                                            // 10 bytes total
  }
}
```

10 bytes total: `0x22 0x08 <8 bytes>`. The 8-byte payload is `[rand1, rand2, counter=0, clientState=1, unix_ts_4B]` XORed with `[rand1, rand2]` (a 2-byte repeating key). The XOR is mild obfuscation — the server trivially decodes it.

## KJ4 — the on-demand inline minter (line 1064)

```js
KJ4=function(){
  var M=0, k;
  return z => {
    k || (k = new kI);                          // kI = empty logger
    var T = new dP4(k, M, 1),                   // M = counter, hardcoded clientState=1
        Y = T.Zq(() => dz(z), !0);              // dz = TextEncoder().encode()
    T.dispose();
    M++;
    return Y;                                   // returns dP4.D() output
  };
};
```

`KJ4` is a **factory** that returns a callable minter which produces a placeholder for any identifier `z` with no BotGuard round-trip.

## What rustypipe does

- `mint_attestation_po_token` (line 1393) is only called *after* `sps=2/3` is returned.
- The **initial** PoToken is selected at line 962:
  1. Override file (`RUSTYPIPE_SABR_PO_TOKEN_FILE`) — typically empty
  2. `player_data.po_token` — from the `/youtubei/v1/player` response (this is the chromey-minted content token returned alongside the player response)
  3. Mint a fresh token via botguard (or chromey with `get_po_token_watch_bound`)
  4. **Fallback to cold-start** (line 1512-1523): 8 random bytes wrapped in `0x22 0x08`

The comment at line 944-955 already recognises that the cold-start placeholder should be used as the *initial* request, not the chromey-minted field-6 token:

> The browser sends a *cold-start* PoToken for the initial SABR request: 10 bytes, structured as a `PoTokenMsg` with field 4 holding 8 random bytes (`0x22 0x08 <8 random>`). The server's `sps=2` (status 2) gives us a ~1–2 MB grace window for this kind of placeholder. After that, the server demands a refreshed token (status 3).
> 
> Without this, the botguard's content token (which is always field 6 — 87 bytes) gets misidentified as a refresh token, the server still gives us ~60s of grace, and then it rejects our refresh attempts because they are bound to a different session.

## The discrepancy

The current rustypipe logic prefers a "real" (field-6, 87-byte) BotGuard-attested token for the first request, with the cold-start as a fallback. The browser prefers the cold-start for the first request, with a "real" token as a fallback once `sps=2` is received and the full pipeline has been triggered.

**Practical implication for rustypipe:** the chromey-minted real token currently sent for the first SABR request may be (a) classified by GVS as a refresh attempt rather than a cold-start, getting a shorter grace window, and (b) bound to the wrong session, causing subsequent refresh attempts to be rejected. Sending the cold-start placeholder first, then triggering `mint_attestation_po_token` (which performs the full `cV+LT+GenerateIT` cycle) on the `sps=2` response, would more closely mirror the browser's behavior.

## Fix implemented (commit-by-commit)

The downloader now mirrors the browser's exact flow:

1. **First SABR request uses the cold-start placeholder** (`0x22 0x08 <8 random bytes>` — identical to `dP4.D` at line 6023 of `base.js.full`). See `downloader/src/lib.rs:997-1008` and `1024-1055` for the placeholder build and prewarm spawn.

2. **A real BotGuard-attested PoToken is pre-warmed in parallel** via `tokio::spawn(Self::mint_attestation_po_token(...))`. The browser does the same on `csiinitialized` (line 7783 of `base.js.full`).

3. **On the first `attestation_required` response**, the pre-warmed token is awaited (with a 30s timeout). If it landed in time, it's used directly. Otherwise, `mint_attestation_po_token` is called inline to re-mint. See `downloader/src/lib.rs:1928-1984`.

4. **Subsequent retries** (attempts ≥ 2) call `mint_attestation_po_token` directly, bypassing the prewarm.

5. **Env-var overrides** (`RUSTYPIPE_SABR_PO_TOKEN_B64` / `RUSTYPIPE_SABR_PO_TOKEN_FILE`) still work — they win over the cold-start, so the diagnostic path with a captured browser token continues to function.

## Code sketch (post-fix)

```rust
// Build cold-start placeholder
let cold_start_bytes: Vec<u8> = {
    let mut rand = [0u8; 8];
    rand::rng().fill(&mut rand[..]);
    let mut po = vec![0x22u8, 0x08];
    po.extend_from_slice(&rand);
    po
};

// Pick initial bytes: env-var override wins, otherwise always cold-start
let initial_bytes: Vec<u8> = if let Some(b64) = override_b64.clone() {
    data_encoding::BASE64URL.decode(b64.as_bytes())
        .unwrap_or_else(|_| cold_start_bytes.clone())
} else {
    tracing::info!("sending cold-start PoToken (10 bytes) for initial SABR request");
    cold_start_bytes.clone()
};

// Pre-warm the chromey mint in parallel
let prewarmed = tokio::spawn(async move {
    Self::mint_attestation_po_token(&rp, ...).await.ok()
});

// Pass to download_sabr
self.download_sabr(..., Some(prewarmed)).await
```

In `download_sabr`'s attestation retry loop:

```rust
let new_token = if attestation_attempts == 1 {
    // Wait up to 30s for the prewarm
    let prewarmed_result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        prewarmed_po_token.take().unwrap(),
    ).await;
    match prewarmed_result {
        Ok(Ok(Some(bytes))) => bytes,  // use prewarmed
        _ => Self::mint_attestation_po_token(...).await?,  // fall back
    }
} else {
    Self::mint_attestation_po_token(...).await?
};
```

## Other PoToken-relevant details from base.js

- `iRf` class (line 6021) is the "no VM, no fallback" path — produces a 0-byte token (the worst case). Used when `Ph(k,3)` (integrityToken) is missing.
- `Xuj` class (line 6021) is the "no VM, websafe fallback" path — uses `TQ(k,4)` (websafeFallbackToken, 64 bytes) directly as a Uint8Array. Used when there's no integrity token but there is a websafe fallback.
- `Bgb` class (the real WebPoMinter) is used when `Ph(k,3)` is present and `webPoSignalOutput[0]` (getMinter) returns a function. This is the path that produces 87-byte field-6 tokens.
- `dP4` (line 6023) is the "synchronous, no BotGuard" path — used only for `html5_web_po_on_demand_init`. Produces 10-byte field-4 placeholders.

## BotGuard VM invocation parameters (`vm.a` / `asyncSnapshotFunction`)

Beyond the integrity-token + binding flow, the botguard VM also expects:

| Parameter | Source | player.js / BgUtils | Old rustypipe | New rustypipe |
|---|---|---|---|---|
| `vm.a` arg 4: `userInteractionElement` | DOM element for interaction signals | `this.userInteractionElement` (BgUtils v3.1+); `M.Or` (`undefined`) for the `cV` challenge path | `undefined` | **`document.body`** — the chromey page is a real YouTube page so we have plenty of real DOM. The botguard VM attaches `passEvent` listeners and factors interaction entropy into the snapshot; without it GVS classifies the token as a low-quality refresh. |
| `asyncSnapshotFunction` arg 0: `contentBinding` | visitor_data / DSID / video_id | `Uu: {}` (empty) at snapshot time; binding is supplied at *mint time* via `OE(M,k)` → `dz(k.Uu)` | `(visitor_data, deobf.sts)` from the caller — **baked the binding into the integrityToken** | **`undefined` at snapshot time** — the binding is forwarded at mint time by `__rustypipeMint`, matching player.js. The old behaviour was producing integrityTokens GVS treated as "stale" because the binding didn't match the SABR request's `StreamerContext.binding`. |
| `asyncSnapshotFunction` arg 1: `signedTimestamp` | `deobf.sts` | same as above — supplied at mint time | `(visitor_data, deobf.sts)` baked into the snapshot | **`undefined` at snapshot time**. |
| `Create` response field 3: `interpreterHash` | server-provided hash of the challenge script | captured into `bgChallenge`, forwarded on the next Create | dropped | **captured and logged** — not needed by rustypipe's single-Create-per-init flow, but kept for diagnostics. |
| `Create` response field 7: `clientExperimentsStateBlob` | stringified `V9O` proto describing YouTube's experiment state | forwarded into `im.M.U` (line 6000) and read back via `TQ(this.U, 5)` at VM-call time | dropped | **captured and stashed on `globalThis.__rustypipeClientExperimentsStateBlob`** for future use. (Today we call `vm.a` directly without an `im` wrapper, so we can't pass it through the `TQ(this.U, 5)` path — the player wraps the VM in `im` and reads experiments from its `this.U`. rustypipe bypasses `im`, so the VM uses its internal default. Capturing it is the first step toward either rebuilding an `im`-equivalent wrapper or having the botguard VM consume it from a known global.) |
| `GenerateIT` response field 2: `mintRefreshThreshold` | seconds after which the player should re-attest | read by `xPy` and used by `ryy`'s poll loop | dropped | **captured and logged**. (The player uses this as `g.Ed(0, T, mintRefreshThreshold)` for its 2-hour refresh scheduler. rustypipe's refresh cadence is driven by GVS's `attestation_required` events instead, so this is currently informational.) |
| `GenerateIT` response field 3: `websafeFallbackToken` | 64-byte placeholder the player falls back to when no VM is available | `xPy` uses `if (TQ(k, 4)) return new Xuj(...)` (line 1058) | dropped | **captured and logged**. (rustypipe's chromey path always has a VM so this fallback isn't triggered in practice, but it's wired up so a future minter-rebuild path can use it instead of failing.) |

### Why these changes matter

The chromey PoTokens were producing 87-byte field-6 tokens (the same outer shape as the player's), but every refresh attempt was rejected with `StreamProtectionStatus = 3 (attestation_required)`. Three independent discrepancies with the browser's flow made GVS treat our tokens as "bad refreshes":

1. **Baked binding.** The previous code baked `visitor_data` + `deobf.sts` into the snapshot, which got baked into the integrityToken. Subsequent SABR requests used a *different* binding (the video id), which didn't match. The browser never bakes the binding at snapshot time — it only sets it when calling `OE(M, k)` to mint the final PoToken.

2. **No interaction signals.** Passing `undefined` for the 4th `vm.a` argument means the botguard VM has no DOM element to attach interaction listeners to. The resulting snapshot lacks the interaction entropy that GVS's freshness check looks for. `document.body` gives the VM a real, large, interactive subtree.

3. **Missing context.** Player.js also threads `clientExperimentsStateBlob` (field 7) through to the VM; rustypipe was discarding it. While we can't yet consume it without rebuilding an `im`-equivalent wrapper, capturing it is the prerequisite.

Together these should make rustypipe's chromey tokens indistinguishable from the browser's at the SABR layer.

## Code locations

- `src/client/chromey.rs:885-942` — `Create` response parsing (now reads fields 3 + 7).
- `src/client/chromey.rs:1043-1058` — stash `clientExperimentsStateBlob` on `globalThis` before `runSnapshot`.
- `src/client/chromey.rs:1078-1095` — snapshot expression (no longer stashes binding/sts on `globalThis`).
- `src/client/chromey.rs:1185-1232` — `GenerateIT` response parsing (now reads fields 2 + 3).
- `src/client/chromey_runner.js:152-180` — `vm.a` invocation with `document.body` as arg 4.
- `src/client/chromey_runner.js:196-209` — `asyncSnapshotFunction` invocation with `undefined` for contentBinding/signedTimestamp.
