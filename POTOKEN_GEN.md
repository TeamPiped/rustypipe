## From-scratch reverse engineering: how `player.js` mints PoTokens

Looking at `https://www.youtube.com/s/player/ac678d18/player_es6.vflset/en_US/base.js`, the BotGuard flow is implemented as a small state machine with four functions. Here is the complete path, in code order, with the obfuscated names from this file mapped to the names you already know from BgUtils.

### The four entry points

```text
cV(M)         // attestation challenge fetch + VM setup  (lines 2242-2243)
LT(M)         // full attestation attempt: snapshot → GenerateIT → build minter (lines 1048-1055)
xPy(M,k,z,T)  // build a WebPoMinter from integrityToken + getMinter fn (line 1058)
OE(M,k)       // mint a token for one contentBinding (line 1066)
```

Plus three transport helpers:

```text
hpq(k,M,V,T,U) → Irn(...)        // Create: POST /Waa/Create (lines 1010, 1013)
QbW(M,T,y)    → mnj(...)          // GenerateIT: POST /Waa/GenerateIT (lines 1012, 1047)
```

### 1. `Create` — get the BotGuard VM and program (line 2242)

```js
// cV: fetch the attestation challenge (5 attempts with exponential backoff)
cV = async function(M) {
  var k = wH(void 0, Tm().U);                  // engagementType = "ENGAGEMENT_TYPE_UNBOUND", maybe interpreterHash
  try { var z = await oxj(M, k) }               // oxj calls HV → CBq → DNq up to 5 times
  catch (x) { /* disable for 24h, return empty */ }

  k = z.iQ;                                     // challenge string (the "a=...&c=...&t=..." URL)
  var T = z.S2;                                 // parsed query (Kt(challenge) → {a, b, c, t, c1a, ...})

  if ("c1a" in T && z.bgChallenge) {            // c1a = botguard version flag
    var Y = new ys;                             // ys = BotguardChallenge proto

    if (z.interpreterJavascript) {              // newer path: interpreter source inline
      var Q = rVW(z.interpreterJavascript);
      Q = g_(Q).toString();
      var y = new YI;                           // YI = AttestChallenge proto
      XV(y, 6, Q);                              // YI.field 6 = interpreter javascript bytes
      q$(Y, YI, 1, y, fR);                      // Y.field 1 (oneof) = AttestChallenge
    } else if (z.interpreterUrl) {              // legacy path: load from URL
      var Q = RQ(z.interpreterUrl);
      Q = UJ(Q).toString();
      var y = new Qs;                           // Qs = AttestChallengeUrl proto
      XV(y, 4, Q);                              // Qs.field 4 = trusted resource URL
      q$(Y, Qs, 2, y, fR);                      // Y.field 2 (oneof) = AttestChallengeUrl
    }

    z.interpreterHash             && iS(Y, 3, z.interpreterHash,             fR);
    z.program                     && iS(Y, 4, z.program,                     fR);
    z.globalName                  && iS(Y, 5, z.globalName,                  fR);
    z.clientExperimentsStateBlob  && iS(Y, 7, z.clientExperimentsStateBlob,  fR);

    try { await GHz(Tm(), Y) }                   // Tm = "VM registry" singleton
    catch (x) { return {challenge:k, lD:T, Fr:void 0, bgChallenge:Y} }

    try { M = new im({challenge:Y, Wl:{Tm:"aGIf"}}); await M.S1 }   // im = Minter, S1 = setup()
    catch (x) { M = void 0 }
  }
  return {challenge:k, lD:T, Fr:M, bgChallenge:Y};
};
```

Things to note:

- `wH(undefined, undefined)` returns `{engagementType:"ENGAGEMENT_TYPE_UNBOUND"}` — that's the default binding-less fetch. The third arg `M.eacrToken` (login session ID) and fourth `interpreterHash` are only added by the caller.
- `Tm()` is the VM registry singleton (`zm.instance`). `GHz(Tm(), Y)` calls `Sgy(Tm(), interpreterJS, interpreterURL, interpreterHash)` which loads the interpreter JS into `window[globalName]` (using `FXy` for inline, `WXj` for URL). After this returns, `window[globalName].a(program, …)` is callable.
- `new im({challenge:Y, Wl:{Tm:"aGIf"}})` is the **attestation session object** (Minter in newer naming). `M.S1` waits for it to be ready. The `Wl.Tm` flag (`"aGIf"` = base64url?) controls how identifiers are encoded.
- The result `Fr` is the Minter itself; `bgChallenge` is the raw challenge proto.

### 2. `Irn` / `hpq` — actually POST `/Waa/Create` (line 1010)

```js
// Irn: POST Create, return descrambled BotguardChallenge
Irn = async function(M, k, z, T, Y) {                          // M = client, k=interpreterHash, z=engagementType, T=logger, Y=extra headers
  k = $nn(ar4(Unj(new bQF, M.requestKey), k), z);              // build CreatePayload [requestKey, interpreterHash, engagementType]

  z = M.U();                                                   // auth headers
  M = await T.S("c", M.requestKey, Y, M.client.create(k, z));  // POST → returns raw proto bytes

  var Q = Rb(Fs(M, 2));                                        // field 2 = scrambled bytes (each +97)
  if (Q.length) return vK(T, () => {
    var y = new Uint8Array(Q.length);
    for (var x = 0; x < Q.length; x++) y[x] = Q[x] + 97;      // ← +97 descramble (matches chromey.rs)
    if (g.tO.TextDecoder) y = (new TextDecoder).decode(y);
    else if (y.length <= 8192) y = String.fromCharCode.apply(null, y);
    else { /* chunked fromCharCode */ }
    return AjH(y);                                              // parse JSON
  }, "U");

  T = pB(M, ys, 1);                                             // field 1 = bgChallenge (proto)
  if (!T) throw Error("Missing field");
  return T;
};

hpq = function(M, k, z, T, Y) {                                // M = client, k=logger, z=attempts, T=interpreterHash, Y=engagementType
  return M instanceof xI
    ? Irn(M, T, Y, k, z)                                        // new client → pass through
    : Irn(M, T, Y, new kI, 0);                                 // legacy → attempt 0, no logger
};
```

The `+97` per-byte XOR-descramble is **literally the same line that lives in your `chromey.rs`** (`for b in bytes.iter_mut() { *b = b.wrapping_add(97); }`), just inlined here.

### 3. `GHz` and `Tm()` — load the interpreter and attach the VM (lines 1001-1006)

```js
GHz = function(M, k) {                                          // M = Tm() singleton, k = BotguardChallenge proto
  return Sgy(M, pB(k, YI, 1, fR), pB(k, Qs, 2, fR), TQ(k, 3, void 0, fR));
};

Sgy = function(M, k, z, T) {                                   // k=interpreterJS, z=interpreterURL, T=interpreterHash
  if (!k && !z) return Promise.resolve();
  if (!T) return OQy(k, z);                                    // no hash → just load
  var Y;
  (Y = M.j)[T] || (Y[T] = new Promise((Q, y) => {
    OQy(k, z).then(() => { M.U = T; Q(); }, x => { delete M.j[T]; y(x); });
  }));
  return M.j[T];
};

OQy = function(M, k) {                                          // M = JS source, k = trusted URL
  return k ? WXj(k) : M ? FXy(M) : Promise.resolve();
};

WXj = function(M) {                                             // legacy: load via <script src=trustedUrl>
  return new Promise((k, z) => {
    var T = g.kp("SCRIPT"), Y = LXb(M);                         // LXb extracts TrustedResourceUrl → script URL
    g.qx(T, Y);
    T.onload  = () => { g.yQ(T); k(); };
    T.onerror = () => { g.yQ(T); z(Error("EWLS")); };
    (g.op("HEAD")[0] || document.documentElement).appendChild(T);
  });
};

FXy = function(M) {                                             // new: inline script
  return new Promise(k => {
    var z = g.kp("SCRIPT");
    if (M) { var T = xH(M, 6); T = T == null ? null : Nx(T); }   // M.field 6 = interpreter JS bytes
    else T = null;
    z.textContent = g_(T);
    jo(z);                                                       // add nonce for CSP
    (g.op("HEAD")[0] || document.documentElement).appendChild(z);
    g.yQ(z);
    k();
  });
};
```

After `GHz` resolves, the interpreter has run and attached the VM to `window[globalName]`. Then `new im({challenge:Y, …}).S1` constructs the Minter that wraps that VM.

### 4. `LT` — full attestation attempt: snapshot → GenerateIT → minter (lines 1048-1055)

```js
LT = async function(M) {                                       // M = PoTokenRequest state machine
  M.S++;                                                        // attempt counter

  var z = new g.BM;                                             // BM = Deferred
  M.Fr instanceof um && M.Fr.Z.push(z.promise);                 // if old minter is a um, await its disposal
  if (M.q3) { /* q3 = some pre-snapshot delay flag */
    let Q = new g.BM;
    setTimeout(() => void Q.resolve());
    await Q.promise;
  }

  var T = M.logger.share();
  try {
    M.state = 5;                                                // STATE: snapshotting

    let Q = [],                                                 // Q = signal output array
        y = await Xb(
          M.Fr.snapshot({Uu: {}, N3: Q}),                       // ← call asyncSnapshotFunction via the VM
          M.P5.b9,                                               // timeout
          () => Promise.reject(new Ex(15, "MDA:Timeout"))
        );
    yWH(M, "MDA:Disposed");                                     // check disposed
    let x = Q[0];                                               // x = getMinter factory function (webPoSignalOutput[0])

    M.state = 6;                                                // STATE: GenerateIT

    let X = await Xb(
      QbW(M.fj, T, y),                                         // POST /Waa/GenerateIT
      M.P5.qu,                                                  // timeout
      () => Promise.reject(new Ex(10, "BWB:Timeout"))
    );
    yWH(M, "BWB:Disposed");

    M.state = 7;                                                // STATE: building minter

    k = vK(T, () => {                                           // vK = run inside the logger's error context
      var B = xPy(M, X, z, x);                                  // build the WebPoMinter
      B.Z.promise.then(() => void M.O());
      return B;
    }, "i");
  }
  catch (Q) {
    k?.dispose();
    if (!M.j) {
      let y = sbb(M, Q);
      z.resolve();
      var Y;
      if (Y = M.Fr instanceof um && M.S < 2)                    // retry at most 2 times
        a: if (Q instanceof Ex) Y = Q.code !== 32 && Q.code !== 20 && Q.code !== 10;
           else { if (Q instanceof KT) switch (Q.code) {
             case 2: case 13: case 14: case 4: break;
             default: Y = !1; break a; }
             Y = !0; }
      if (Y) {
        let x = setTimeout(() => void M.O(),
                           (1 + Math.random() * .25) * (M.U ? 6E4 : 1E3));  // 1-1.25s or 60-75s
        M.addOnDisposeCallback(() => void clearTimeout(x));
        return;
      }
      M.j = y;
    }
    T.TU(M.U ? 13 : 14);
    M.Z.reject(M.j);
    return;
  }
  finally { T.dispose(); }

  M.state = 8;                                                  // STATE: ready
  M.S = 0;
  M.U?.dispose();
  M.U = k;                                                      // ← install new minter as the active one
  M.Z.resolve();                                                // ← resolve any pending PoToken promise
};
```

Things worth highlighting:

- `M.state` is the state-machine value (5=snapshot, 6=GenerateIT, 7=build, 8=ready). The class itself logs these transitions via `M.options.ZXD?.(k)` (the `sE` callback).
- `Uu: {}` in `M.Fr.snapshot({Uu: {}, N3: Q})` — the empty object is the **contentBinding** the caller hasn't filled in yet. The actual binding (visitorData / DSID / video_id) is added later by the consumer of `M.U` (which is the resulting minter).
- `Q = []` is the `webPoSignalOutput` array the VM fills as a side effect. After `snapshot` returns, `Q[0]` is the `getMinter` factory.
- `QbW(M.fj, T, y)` — `M.fj` is the botguard client; the second arg is the snapshot string; the third is the logger.

### 5. `xPy` — build a `Bgb` from integrityToken + getMinter (line 1058)

```js
xPy = function(M, k, z, T) {                                   // M=request, k=IntegrityTokenResponse, z=Deferred, T=getMinter
  var Y = (Lk(Ph(k, 2)) ?? 0) * 1E3;                           // field 2 of response = TTL seconds → ms
  if (Y <= 0) throw new Ex(31, "TTM:Invalid");                 // "Invalid TTL"

  if (TQ(k, 4))                                                 // field 4 = websafeFallbackToken (64-byte placeholder)
    return new Xuj(M.logger, TQ(k, 4), Y);                      // Xuj = "no VM needed, only websafe fallback"

  if (!(Lk(Ph(k, 3)) ?? 0))                                     // field 3 = integrityToken (the real attestation)
    return new iRf(M.logger, Rb(Fs(k, 1)), Y);                 // iRf = "no VM needed, no fallback" → cold-start token

  if (!T) throw new Ex(4, "PMD:Undefined");                    // ← missing getMinter function (matches BgUtils)
  T = T(Rb(Fs(k, 1)));                                          // ← getMinter(integrityTokenBytes) → mintCallback
  if (typeof T !== "function") throw new Ex(16, "APF:Failed"); // ← getMinter returned non-function (matches BgUtils)

  M.V = Math.floor((Date.now() + Y) / 1E3);                    // record expiration unix seconds

  M = new Bgb(M.logger, T, Lk(Ph(k, 3)) ?? 0, Y);              // ← Bgb = WebPoMinter (logger, mintCb, integrityToken, ttlMs)
  M.addOnDisposeCallback(() => void z.resolve());
  return M;
};
```

Note the **exact** error-code mapping to BgUtils:

| player.js code | BgUtils error | Meaning |
|---|---|---|
| `Ex(31, "TTM:Invalid")` | `BGError('VM_ERROR', 'TTM:Invalid')` | TTL was 0 or negative |
| `Ex(4, "PMD:Undefined")` | `BGError('VM_ERROR', 'PMD:Undefined')` | `webPoSignalOutput[0]` was missing |
| `Ex(16, "APF:Failed")` | `BGError('VM_ERROR', 'APF:Failed')` | `getMinter(tokenBytes)` didn't return a function |

`Bgb` is the minter class. After this returns, `M.U = new Bgb(...)` is the active minter that every subsequent contentBinding mint will go through.

### 6. `mnj` / `QbW` — `POST /Waa/GenerateIT` (lines 1012, 1047)

```js
mnj = async function(M, k, z, T) {                            // M=client, k=snapshot, z=postProcessor, T=timeout?
  var Y = M.U();                                                // headers
  var Q = new HQW;
  Q = iS(Q, 1, M.requestKey);                                  // field 1 = requestKey (e.g. "O43z0dpjhgX20SCx4KAo")
  var y = iS(Q, 2, k);                                         // field 2 = the BotGuard snapshot string

  k = z.S;                                                      // save the response callback
  Q = M.requestKey;
  M = M.client;
  Y = wPq(M.U, M.j + "/$rpc/google.internal.waa.v1.Waa/GenerateIT",
          y, Y || {}, cj4);                                    // POST it (with x-goog-api-key + x-user-agent via `cj4`)
  return k.call(z, "g", Q, T, Y);                              // call back with response
};

QbW = function(M, k, z) {                                      // M=client, k=logger, z=snapshot string
  return M instanceof xI
    ? mnj(M, z, k, 1)                                           // new client (xI) → standard timeout
    : M.Xa(z);                                                  // legacy client → its own transport
};
```

Same wire format as BgUtils: `[requestKey, snapshotString]`, base proto. The `cj4` constant is the header object — it carries `x-goog-api-key: AIzaSyDyT5W0Jh49F30Pqqtyfdf7pDLFKLJoAnw` and `x-user-agent: grpc-web-javascript/0.1` (you can see this same key pair hardcoded in your `chromey.rs`).

### 7. `OE` — mint one PoToken (line 1066)

```js
OE = function(M, k) {                                          // M=logger, k = request {Go?, Uu?, ...}
  return k.Go                                                   // already-minted Uint8Array (cached)?
    ? k.Go
    : k.Uu                                                      // contentBinding string?
      ? vK(M.logger, () => k.Go = dz(k.Uu), "c")               // TextEncoder().encode(Uu) and memoize
      : [];                                                     // nothing to mint
};
```

`dz` is the standard `TextEncoder`. So a single PoToken mint is literally:

```js
const mintCallback = await getMinter(base64ToU8(integrityToken));   // done once in xPy
const result = await mintCallback(new TextEncoder().encode(visitorData));  // done in OE
const poToken = u8ToBase64(result, true);                            // base64url
```

That's exactly the flow BgUtils documents, and it's what your `chromey_runner.js`'s `mint()` function does.

### 8. `nFq` — the public mint API with cache lookup (lines 1060-1066)

```js
nFq = function(M, k, z) {                                      // M=request, k=PoTokenRequest (one identifier), z=callback
  try {
    if (M.A3()) throw new Ex(21, "BNT:disposed");              // bail if disposed
    if (!M.U && M.j) throw M.j;                                // no active minter but a permanent error
    return VAq(M, k, z)                                        // try active minter
        ?? ZR4(M, k, z)                                        // try cache
        ?? JWF(M, k, z);                                       // fall back to on-demand minter
  }
  catch (T) {
    if (!k.Wi) throw P3q(M, T);                                // Wi = "don't report"
    return tAy(M, z, T);                                       // return error to caller
  }
};

VAq = function(M, k, z) {                                      // active minter path
  return M.U?.Zq(() => OE(M, k), z, T => {                     // Zq = "queue", runs the producer once, returns the value
    if (M.U instanceof Bgb && k.Hy?.ga)                        // ga = "cache to disk"
      try { M.cache?.U(OE(M, k), T, k.Hy.Ib, M.V - 120) }      // cache by contentBinding (Ib) with 2-min pre-expiry
      catch (Y) { M.reportError(new Ex(24, "ELX:write", Y)) }
  });
};

ZR4 = function(M, k, z) {                                      // cache lookup path
  if (k.Hy?.VV)                                                // VV = "verify cache"
    try {
      let T = M.cache?.j(OE(M, k), k.Hy.Ib);                    // j = cache.get(key)
      return T ? z
        ? vK(M.logger, () => g.Wu(T, 2), "a")                  // Wu(T, 2) = base64url encode (offset 2 in `g` is base64url helpers)
        : T
        : void 0;
    }
    catch (T) { M.reportError(new Ex(23, "RXO:read", T)) }
};

JWF = function(M, k, z) {                                      // one-shot fallback (creates a fresh minter inline)
  var T = {stack:[], error:void 0, hasError:!1};
  try {
    if (!k.r6) throw new Ex(29, "SDF:notready");
    return mq(T, new dP4(M.logger, 0, M.state))                // dP4 = inline one-shot Minter
              .Zq(() => OE(M, k), z);                          // runs OE to mint, returns Uint8Array
  }
  catch (Y) { T.error = Y; T.hasError = !0; }
  finally { EW(T) }
};

OE = function(M, k) {
  return k.Go ? k.Go : k.Uu
    ? vK(M.logger, () => k.Go = dz(k.Uu), "c")                 // TextEncoder().encode(Uu)
    : [];
};
```

This three-tier fallback is the cache hierarchy:

1. **Active minter (`VAq`)** — `M.U` is the `Bgb` instance built by the most recent `LT` run. Every call to `OE(M, k)` mints a fresh token through it. The result is memoized on `k.Go` and the binding→token mapping is also written to `M.cache` (a disk-backed cache) with key `k.Hy.Ib` (binding hash) and TTL `M.V - 120` (two minutes before integrity-token expiry).
2. **Disk cache (`ZR4`)** — `M.cache.j(OE(M,k), k.Hy.Ib)` looks up the same binding's token by content. If `z=true`, base64url-encodes the result; otherwise returns the raw bytes.
3. **One-shot (`JWF`)** — instantiates a fresh `dP4` minter inline (without going through `LT`'s full pipeline). Used as a last resort when the active minter hasn't been built yet but a token is needed *right now*.

### 9. The challenge lifecycle (lines 2251, 2247-2248)

```js
// cV → returns {challenge, lD, Fr (Minter), bgChallenge (proto)}
// pWH: replace the cached Minter with a freshly-attested one
pWH = async function(M) {
  var k = await Promise.race([M.U, null]),     // wait for current promise or none
      z = cV(M);                                // fetch new challenge + build new Minter
  M.U = z;                                      // ← new promise
  k?.Fr?.dispose();                             // dispose old Minter
};

// ryy: schedule refresh
ryy = function(M, k) {                          // k = ms until refresh
  var z = Date.now() + k,
      T = async () => {
        var Y = z - Date.now();
        Y < 1E3
          ? await pWH(M)                        // expired → refresh now
          : g.Ed(0, T, Math.min(Y, 6E4));       // else poll every ≤60s
      };
  T();
};
```

`ryy` is called from `cV` with `(Number(T.t) || 7200) * 1E3` — so the default refresh cadence is **2 hours** (7200 s) or whatever `t` field the challenge URL carries, whichever is longer. `pWH` always runs the **full** `cV` flow (challenge fetch + new Minter) — it doesn't just refresh the integrity token, it re-fetches the challenge entirely.

### Summary: end-to-end call graph

```
cV(M)                              ← attestation challenge fetch + VM load (line 2242)
  └→ oxj(M, k)                     ← retry loop, 5 attempts
      └→ HV(M, k)                  ← wait for publicytnetworkstatus-online
          └→ CBq(M, k)
              └→ HV(M.network, k)  ← raw network fetch
      └→ DNq(Y)                    ← normalize response → {iQ, S2, bgChallenge}
  └→ GHz(Tm(), Y)                  ← load interpreter JS into window[globalName]
  └→ new im({challenge:Y, ...}).S1 ← construct Minter wrapping the loaded VM

LT(M)                              ← full attestation attempt (line 1048)
  ├→ M.Fr.snapshot({Uu:{}, N3:Q})  ← asyncSnapshotFunction(contentBinding, signedTimestamp, webPoSignalOutput, skipPrivacyBuffer)
  ├→ QbW(M.fj, T, y)               ← mnj: POST /Waa/GenerateIT with [requestKey, snapshot]
  └→ xPy(M, X, z, x)               ← build Bgb (WebPoMinter) from integrityToken + Q[0]=getMinter
       └→ x = Q[0]                 ← getMinter factory from webPoSignalOutput
       └→ T = T(Rb(Fs(k,1)))       ← getMinter(base64ToU8(integrityToken)) → mintCallback
       └→ new Bgb(logger, T, integrityToken, ttlMs)

OE(M, k)                           ← mint one token (line 1066)
  └→ dz(k.Uu)                      ← TextEncoder().encode(visitorData | videoId)

nFq(M, k, z)                       ← public API with cache lookup (line 1060)
  ├→ VAq:  active Bgb → OE() → mintCallback(identifierBytes)
  ├→ ZR4:  disk cache lookup by binding hash
  └→ JWF:  inline one-shot minter (dP4) → OE()

pWH(M)                             ← scheduled refresh (line 2251)
  └→ cV(M)                         ← fetch new challenge + new Minter, dispose old
ryy(M, k)                          ← poll-and-refresh scheduler (line 2247)
  └→ pWH(M) when remaining < 1s
```

The clean match to your existing rustypipe chromey implementation:

| player.js | rustypipe equivalent |
|---|---|
| `M.Fr.snapshot({Uu:{}, N3:Q})` (line 1049) | `chromey_runner.js` `runSnapshot()` |
| `QbW(M.fj, T, y)` → `mnj` (line 1050) | `chromey.rs` `inner.http.post("https://www.youtube.com/api/jnn/v1/GenerateIT")` |
| `xPy(M, X, z, x)` → `new Bgb(...)` (line 1058) | `chromey_runner.js` `newMinter()` |
| `T = T(Rb(Fs(k,1)))` (line 1058) | `globalThis.__rustypipeMint = await getMinter(tokenBytes)` |
| `OE(M, k)` → `dz(k.Uu)` (line 1066) | `chromey_runner.js` `mint()` → `mintCallback(new TextEncoder().encode(identifier))` |
| `g.Kz("VISITOR_DATA")` (line 1582) | The `visitorData` you pass in as `contentBinding` |
| `ryy` 2-hour refresh (line 2247) | The chromey provider's `valid_until` + minter-reuse logic |

The error-code mapping (`PMD:Undefined`, `APF:Failed`, `TTM:Invalid`) is **identical** to BgUtils — that's the strongest evidence the player.js code and BgUtils describe the exact same protocol. The only meaningful difference is the player's lifetime policy: `ryy` schedules a full re-attestation at the challenge URL's `t` parameter (default 7200s = 2h), whereas your chromey provider reuses the `Bgb`/`__rustypipeMint` until the server-reported TTL expires. Both are valid; the player's approach pays a full challenge round-trip every 2h to stay fresh against BotGuard updates, while yours amortizes that cost by reusing one challenge for the full 12h integrity-token lifetime.
