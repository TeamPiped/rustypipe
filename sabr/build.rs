use std::{
    env,
    fs,
    io,
    path::{Path, PathBuf},
};

/// List of proto files relative to the `protos/` directory that this crate
/// actually uses. Limiting the input avoids collisions between unrelated
/// enums declared in the same `package` (proto2's C++-scoping rules).
const WANTED: &[&str] = &[
    "misc/common.proto",
    "video_streaming/ump_part_id.proto",
    "video_streaming/time_range.proto",
    "video_streaming/media_header.proto",
    "video_streaming/client_abr_state.proto",
    "video_streaming/streamer_context.proto",
    "video_streaming/format_initialization_metadata.proto",
    "video_streaming/playback_cookie.proto",
    "video_streaming/next_request_policy.proto",
    "video_streaming/sabr_error.proto",
    "video_streaming/sabr_redirect.proto",
    "video_streaming/sabr_context_update.proto",
    "video_streaming/sabr_context_sending_policy.proto",
    "video_streaming/stream_protection_status.proto",
    "video_streaming/video_playback_abr_request.proto",
];

fn main() -> io::Result<()> {
    let _out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let proto_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("protos");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=protos/");

    let proto_files: Vec<PathBuf> = WANTED
        .iter()
        .map(|rel| proto_dir.join(rel))
        .filter(|p| p.exists())
        .collect();

    if proto_files.is_empty() {
        return Err(io::Error::other(format!(
            "no proto files found in {}",
            proto_dir.display()
        )));
    }

    let mut cfg = prost_build::Config::new();
    // Use `bytes::Bytes` for byte fields throughout for cheap zero-copy sharing.
    cfg.bytes(["."]);
    // proto3 default value semantics are fine; proto2 `optional` fields become
    // `Option<T>` regardless.
    if let Err(e) = cfg.compile_protos(&proto_files, &[&proto_dir]) {
        eprintln!("prost_build failed: {e}");
        // Fall through and return a clean error
        return Err(io::Error::other(format!("prost_build failed: {e}")));
    }

    Ok(())
}

#[allow(dead_code)]
fn read_dir(path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(path)? {
        let p = entry?.path();
        if p.is_dir() {
            out.extend(read_dir(&p)?);
        } else {
            out.push(p);
        }
    }
    Ok(out)
}
