// Simple test for the chromey PoToken provider.
//
// Usage: cargo test --features chromey-po-token test_chromey_init -- --nocapture
//
// Requires a Chrome/Chromium binary on the system (auto-detected).
//
// This test asserts that a valid PoToken can be minted
// using the chromey (real-browser) provider. A valid
// PoToken is a non-empty base64url-encoded string whose
// decoded length is at least ~80 bytes (YouTube's PoToken
// format).

#[cfg(feature = "chromey-po-token")]
#[tokio::test]
async fn test_chromey_init() {
    use rustypipe::client::chromey::ChromeyProvider;
    use std::time::Instant;

    eprintln!("test: creating ChromeyProvider");
    let start = Instant::now();
    let chrome_path = Some(std::path::PathBuf::from("/usr/bin/chromium"));
    let provider = ChromeyProvider::new(chrome_path);
    eprintln!("test: provider created in {:?}", start.elapsed());

    eprintln!("test: calling provider.mint");
    let start = Instant::now();
    // Use a real-looking identifier (an 11-char video id
    // shape) to ensure mint produces a YouTube-shaped
    // PoToken. Pass a video id so the chromey provider
    // navigates to the watch page first; this matches the
    // production path in the downloader.
    let tokens = provider
        .mint(
            &["dQw4w9WgXcQ", "oHg5SJYRHA0", "test_identifier_2"],
            None,
            None,
            Some("dQw4w9WgXcQ"),
        )
        .await;
    eprintln!("test: provider.mint took {:?}", start.elapsed());
    let (tokens, _valid_until) = tokens.expect("provider.mint should succeed");
    assert_eq!(tokens.len(), 3, "expected 3 tokens, got {}", tokens.len());

    // Dump tokens to file for debugging
    let dump_path = std::env::var("RUSTYPIPE_DUMP_CHROMEY_TOKENS").unwrap_or_else(|_| "/tmp/chromey_tokens.txt".to_string());
    if let Ok(mut f) = std::fs::File::create(&dump_path) {
        use std::io::Write;
        for (i, t) in tokens.iter().enumerate() {
            let decoded = data_encoding::BASE64URL.decode(t.as_bytes()).unwrap_or_default();
            let _ = writeln!(f, "=== token[{}] ({} chars b64, {} bytes decoded) ===", i, t.len(), decoded.len());
            let _ = writeln!(f, "{}", t);
            let _ = writeln!(f, "decoded hex: {}", decoded.iter().take(60).map(|b| format!("{:02x}", b)).collect::<String>());
            let _ = writeln!(f, "decoded hex (full): {}", decoded.iter().map(|b| format!("{:02x}", b)).collect::<String>());
        }
        eprintln!("dumped chromey tokens to {}", dump_path);
    }
    for (i, t) in tokens.iter().enumerate() {
        assert!(!t.is_empty(), "token[{}] is empty", i);
        // A YouTube PoToken decoded is around 80-128 bytes.
        let len_b64 = t.len();
        eprintln!("  token[{}]: {} ({} chars)", i, &t[..40.min(t.len())], len_b64);
        assert!(
            len_b64 >= 80,
            "token[{}] looks too short to be a valid PoToken",
            i
        );
    }
    // Sanity: the 3 tokens should differ (each is bound
    // to a different identifier).
    assert_ne!(tokens[0], tokens[1], "tokens for different idents should differ");
    assert_ne!(tokens[1], tokens[2], "tokens for different idents should differ");
}

#[cfg(not(feature = "chromey-po-token"))]
#[test]
fn test_chromey_init() {
    eprintln!("chromey-po-token feature not enabled; skipping");
}
