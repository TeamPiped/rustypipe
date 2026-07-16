// BotGuard runner for the chromey (real-browser) PoToken provider.
//
// Runs in a real Chrome page on `https://www.youtube.com/` launched
// by chromiumoxide. The Rust side fetches the `jnn/v1/Create`
// challenge from YouTube, which returns an `interpreterJavascript`
// (the botguard VM source) and a `globalName`. We evaluate the
// interpreter in the page's real Chrome context to attach the VM
// to `window[globalName]`, then run the program on the VM.
//
// The point of using real Chrome (not Deno+JSDOM like
// `rustypipe-botguard`) is that GVS's server-side environment
// fingerprinting accepts tokens minted in a real browser but
// rejects the botguard binary's tokens.
//
// Three globals are exposed:
//   - `loadInterpreter(interpreterJavascript, globalName)` — eval
//     the interpreter source and attach the resulting VM to
//     `window[globalName]`.
//   - `runSnapshot(program)` — assumes the VM is already loaded
//     onto `window[globalName]`. Returns
//     `[snapshotString, webPoSignalOutput]`.
//   - `newMinter(integrityToken, webPoSignalOutput)` — uses
//     `webPoSignalOutput[0]` (the minter factory) with the
//     integrityToken to build a minter function. Stores it on
//     `globalThis.__rustypipeMint`.
//   - `mint(identifier)` — base64url-encodes the minter's output
//     for a given identifier (Video ID, Visitor Data, etc).
//
// Registered with `Page.add_script_to_evaluate_on_new_document`
// so it runs as a classic script on the youtube.com main frame
// (and any same-origin iframe if we ever decide to). We do not
// use top-level `await` because chromey injects this as a
// classic script.

(async () => {
    // `webPoSignalOutput` and `mintCallback` are stashed on
    // globalThis because the sandbox is discarded after
    // `evaluate_function` returns. The Rust side keeps the page
    // alive, so this state survives across calls.
    const base64UrlToBytes = (s) => {
        // Convert standard base64url to base64, then decode.
        const std = s.replace(/-/g, "+").replace(/_/g, "/");
        const pad = std.length % 4;
        const padded = pad ? std + "=".repeat(4 - pad) : std;
        const bin = atob(padded);
        const out = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
        return out;
    };
    const bytesToBase64Url = (u8) => {
        let s = "";
        for (let i = 0; i < u8.length; i++) s += String.fromCharCode(u8[i]);
        return btoa(s).replace(/\+/g, "-").replace(/\//g, "_");
    };

    // Evaluate the botguard interpreter source in the page's
    // real Chrome context. The interpreter source is JS that
    // defines an `a()` function returning a VM, and (depending
    // on version) attaches it to a global name. Some versions
    // do not auto-attach; we make sure `window[globalName]`
    // holds the VM.
    globalThis.loadInterpreter = async (interpreterJavascript, globalName) => {
        if (typeof interpreterJavascript !== "string" || interpreterJavascript.length === 0) {
            throw new Error("interpreterJavascript missing");
        }
        if (typeof globalName !== "string" || globalName.length === 0) {
            throw new Error("globalName missing");
        }
        // Eval the interpreter in the page's context. The
        // interpreter uses `(0,eval)(...)` to invoke itself, so
        // it works in indirect-eval mode and produces a VM
        // that's accessible from the page's global scope.
        // We wrap the interpreter in a closure so we can
        // optionally access the VM it returns.
        //
        // The botguard interpreter expects to find itself on
        // the global scope with a specific name. Looking at
        // BgUtils, the interpreter assigns the VM to
        // `window[globalName]` if it can; otherwise we have to
        // do it ourselves by capturing what `(0,eval)` returns.
        //
        // In practice the most reliable approach is to simply
        // `eval` the interpreter, which will run its
        // `(0,eval)(function(...){...})(...)` IIFE that
        // returns a VM, and then access the VM via
        // `window[globalName]`. If the interpreter doesn't
        // auto-attach, we fall back to capturing the IIFE
        // return value.
        const wrapper = `(function() {
            ${interpreterJavascript}
        })`;
        // The interpreter source ends with a `(...)` that calls
        // itself; we just eval it. The interpreter's IIFE will
        // attach the VM to window[globalName] as a side effect
        // (this is how YouTube's own page does it).
        (0, eval)(wrapper);
        // Wait for the VM to land.
        const deadline = Date.now() + 30000;
        while (Date.now() < deadline) {
            const vm = globalThis[globalName];
            if (vm && typeof vm.a === "function") {
                return vm;
            }
            await new Promise((r) => setTimeout(r, 50));
        }
        throw new Error(
            "botguard VM not found at window." + globalName + " after 30s"
        );
    };

    // Compute a snapshot using the botguard VM attached to
    // `window[globalName]`. Returns
    // `[snapshot, webPoSignalOutput]`.
    //
    // The VM puts the getMinter function into
    // `webPoSignalOutput[0]`. That function is a non-
    // serialisable JS function — we MUST keep a live
    // reference to it on `globalThis` so the subsequent
    // `newMinter` call (in the same execution context)
    // can read it. We also return it in the array, but
    // when the Rust side serialises the array as JSON
    // the function becomes `null`/`{}` — that's why the
    // subsequent call must read from `globalThis`, not
    // from the JSON payload.
    globalThis.runSnapshot = async (program, globalName) => {
        const vm = globalThis[globalName];
        if (!vm || typeof vm.a !== "function") {
            throw new Error("botguard VM not loaded; call loadInterpreter first");
        }
        // The VM's `vmFunctionsCallback` is called by the VM
        // with (asyncSnapshotFunction, shutdownFunction,
        // passEventFunction, checkCameraFunction). We pass our
        // own callback that stashes `asyncSnapshotFunction` on
        // a deferred promise.
        let resolveAsync;
        const asyncSnapshotDeferred = new Promise((res) => {
            resolveAsync = res;
        });
        const vmFunctionsCallback = (asyncSnapshotFunction) => {
            resolveAsync(asyncSnapshotFunction);
        };
        // The VM's `vm.a` returns `[syncSnapshot, ...]`. We
        // don't use the sync snapshot — we only need
        // `asyncSnapshotFunction`.
        //
        // The 6th arg `signalOutputTuple` is `[ [], [] ]`
        // (contentSignals, miscSignals). The VM may
        // optionally populate those arrays in addition to
        // the webPoSignalOutput passed to
        // asyncSnapshotFunction. We keep a reference to it
        // for diagnostics.
        //
        // The 4th arg is the `userInteractionElement` (a DOM
        // element). BgUtils v3.1+ and NewPipe's working
        // `po_token.html` pass a real DOM element here so the
        // botguard VM can attach mouse / pointer / keyboard
        // listeners and factor user-interaction signals into
        // the snapshot. Without it, the VM's snapshot lacks
        // interaction entropy and GVS classifies the
        // resulting token as a "bad refresh" — every refresh
        // comes back with `StreamProtectionStatus = 3
        // (attestation_required)`. The chromey page is a
        // real YouTube page so we have plenty of real DOM
        // elements; `document.body` is the safest choice
        // (always present, large, and the VM can attach
        // listeners to descendants).
        //
        // The 5th arg is the no-op snapshot callback. The
        // 7th–9th args are VM-internal: the experiments-state
        // field (TQ(this.U, 5) in player.js), a boolean, and
        // a logger-callback array. Player.js passes these
        // explicitly; BgUtils/NewPipe only pass 6 args
        // because their botguard VM version doesn't read
        // them. We pass 6 args to match the working
        // upstream.
        const userInteractionElement =
            (typeof document !== "undefined" && document.body) || null;
        const signalOutputTuple = [[], []];
        vm.a(
            program,
            vmFunctionsCallback,
            true,
            userInteractionElement,
            () => {},
            signalOutputTuple
        );
        const asyncSnapshotFunction = await asyncSnapshotDeferred;
        if (typeof asyncSnapshotFunction !== "function") {
            throw new Error("botguard VM did not provide asyncSnapshotFunction");
        }
        const webPoSignalOutput = [];
        // The botguard `asyncSnapshotFunction` takes four
        // positional args: `[contentBinding, signedTimestamp,
        // webPoSignalOutput, skipPrivacyBuffer]`. Player.js
        // (line 1046) calls the snapshot with `Uu: {}` —
        // an EMPTY content binding — and supplies the real
        // binding **at mint time** via `Bgb.D` →
        // `OE(M,k)` → `dz(k.Uu)`. BgUtils and NewPipe do
        // the same (snapshot with `undefined` for the first
        // two args; identifier is passed to
        // `mintCallback` later).
        //
        // Passing a real `contentBinding` /
        // `signedTimestamp` to the snapshot — as the
        // previous implementation did — bakes the binding
        // into the resulting integrityToken. When GVS then
        // sees a PoToken minted with a *different* binding
        // (e.g. the player request uses visitorData but the
        // SABR request uses videoId), it rejects the token
        // as a bad refresh. We now leave the snapshot's
        // binding slots empty and let the mint path supply
        // the real binding.
        const snapshot = await new Promise((resolve) => {
            asyncSnapshotFunction(resolve, [
                undefined,        // contentBinding (per-mint only)
                undefined,        // signedTimestamp (per-mint only)
                webPoSignalOutput, // populated as a side effect
                undefined,        // skipPrivacyBuffer
            ]);
        });
        if (typeof snapshot !== "string" || snapshot.length === 0) {
            throw new Error("botguard snapshot returned empty/non-string");
        }
        // Stash the array (with the live getMinter
        // function at index 0) on globalThis so the next
        // call (in the same execution context) can find
        // it. The Rust side reads this live reference
        // from globalThis in the newMinter call rather
        // than round-tripping the array through JSON,
        // which would lose the function.
        globalThis.__rustypipeWebPoSignalOutput = webPoSignalOutput;
        return [snapshot, webPoSignalOutput];
    };

    // Build a minter from the integrityToken (returned by
    // `jnn/v1/GenerateIT`) and the `webPoSignalOutput` produced
    // by `runSnapshot`. `webPoSignalOutput[0]` is a function
    // returned by the VM that, given the integrityToken bytes,
    // returns a `mintCallback(identifierBytes) -> Uint8Array`.
    //
    // IMPORTANT: `webPoSignalOutput[0]` is a non-serialisable
    // JS function. We read it from `globalThis` (where
    // `runSnapshot` stashed it) so the function reference
    // survives across CDP `page.execute` calls within the same
    // execution context.
    globalThis.newMinter = async (integrityToken) => {
        if (typeof integrityToken !== "string" || integrityToken.length === 0) {
            throw new Error("integrityToken missing");
        }
        const webPoSignalOutput = globalThis.__rustypipeWebPoSignalOutput;
        if (!webPoSignalOutput || !Array.isArray(webPoSignalOutput) || webPoSignalOutput.length === 0) {
            throw new Error("webPoSignalOutput missing/empty (runSnapshot must be called first)");
        }
        const getMinter = webPoSignalOutput[0];
        if (typeof getMinter !== "function") {
            throw new Error(
                "webPoSignalOutput[0] is not a function (PMD:Undefined); " +
                "type=" + typeof getMinter +
                " len=" + webPoSignalOutput.length
            );
        }
        const tokenBytes = base64UrlToBytes(integrityToken);
        const mintCallback = await getMinter(tokenBytes);
        if (typeof mintCallback !== "function") {
            throw new Error("getMinter did not return a function (APF:Failed)");
        }
        globalThis.__rustypipeMint = (identifier) => {
            const idBytes = new TextEncoder().encode(identifier);
            const tokenU8 = mintCallback(idBytes);
            if (!(tokenU8 instanceof Uint8Array)) {
                throw new Error("minter returned non-Uint8Array (ODM:Invalid)");
            }
            // eprintln-style debug to /dev/stderr so we can see the
            // raw mint output size in the CLI's stderr stream.
            console.log(
                "[chromey mint] identifier=" + identifier +
                " raw_len=" + tokenU8.length +
                " hex=" + Array.from(tokenU8)
                    .map((b) => b.toString(16).padStart(2, "0")).join("")
            );
            return bytesToBase64Url(tokenU8);
        };
    };

    // Mint a PoToken for the given identifier using the minter
    // built by `newMinter`.
    globalThis.mint = async (identifier) => {
        if (typeof globalThis.__rustypipeMint !== "function") {
            throw new Error("minter not initialised; call newMinter() first");
        }
        // decodeURIComponent mirrors the botguard binary's
        // behaviour: identifiers may come in URL-encoded form
        // and we want to mint on the decoded string.
        let decoded;
        try {
            decoded = decodeURIComponent(identifier);
        } catch (_) {
            decoded = identifier;
        }
        return globalThis.__rustypipeMint(decoded);
    };

    globalThis.__rustypipeRunnerReady = true;
})();
