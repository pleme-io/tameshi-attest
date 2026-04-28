//! `pleme-io/tameshi-attest` — emit BLAKE3 attestation hashes for a release.
//!
//! Lightweight standalone of forge's `commands/attestation.rs`. Computes
//! BLAKE3 hashes of the source tree (sha-stamped), per-artifact build
//! outputs, and optional image digests; emits them as JSON output the
//! consumer can attach to a GitHub Release body or sekiban annotation.
//!
//! When the full forge attestation chain (source + build + image + chart +
//! deployment, composed into a ProductCertification) is needed, fall back
//! to invoking forge directly. This action is for the common case: "give
//! me a content-addressable hash for my release artifact."

use std::path::PathBuf;

use pleme_actions_shared::{ActionError, Input, Output, StepSummary};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct Inputs {
    /// Comma-separated list of files or directories to hash. Each entry
    /// becomes one row in the attestation table.
    artifacts: String,
    /// Optional git SHA stamped onto every artifact's row. Useful for
    /// downstream verification.
    #[serde(default)]
    git_sha: Option<String>,
    /// Optional release tag (e.g. v1.2.3) stamped likewise.
    #[serde(default)]
    release_tag: Option<String>,
}

#[derive(Debug, Serialize)]
struct AttestationRecord {
    artifact: String,
    blake3: String,
    bytes: u64,
    git_sha: Option<String>,
    release_tag: Option<String>,
}

fn main() {
    pleme_actions_shared::log::init();
    if let Err(e) = run() {
        e.emit_to_stdout();
        if e.is_fatal() {
            std::process::exit(1);
        }
    }
}

fn run() -> Result<(), ActionError> {
    let inputs = Input::<Inputs>::from_env()?;
    let artifacts: Vec<&str> = inputs.artifacts.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
    if artifacts.is_empty() {
        return Err(ActionError::error("input `artifacts` is empty (comma-separated paths required)"));
    }

    let mut records = Vec::new();
    for artifact in &artifacts {
        let path = PathBuf::from(artifact);
        if !path.exists() {
            return Err(ActionError::error(format!(
                "artifact `{}` does not exist", artifact
            )));
        }
        let (hash, bytes) = blake3_path(&path)?;
        records.push(AttestationRecord {
            artifact: artifact.to_string(),
            blake3: hash,
            bytes,
            git_sha: inputs.git_sha.clone(),
            release_tag: inputs.release_tag.clone(),
        });
    }

    let json = serde_json::to_string(&records)
        .map_err(|e| ActionError::error(format!("failed to serialize records: {e}")))?;

    let output = Output::from_runner_env()?;
    output.set_json("records", &records)?;
    output.set("count", records.len().to_string())?;

    let mut summary = StepSummary::from_runner_env()?;
    summary.heading(2, "tameshi-attest");
    let rows: Vec<Vec<String>> = records.iter().map(|r| {
        vec![
            r.artifact.clone(),
            r.blake3[..16.min(r.blake3.len())].to_string() + "…",
            r.bytes.to_string(),
        ]
    }).collect();
    summary.table(&["Artifact", "BLAKE3 (truncated)", "Bytes"], rows);
    if let Some(tag) = &inputs.release_tag {
        summary.paragraph(&format!("Release tag: `{tag}`"));
    }
    if let Some(sha) = &inputs.git_sha {
        summary.paragraph(&format!("Git SHA: `{sha}`"));
    }
    summary.commit()?;

    eprintln!("tameshi-attest records: {json}");
    Ok(())
}

fn blake3_path(path: &PathBuf) -> Result<(String, u64), ActionError> {
    let metadata = std::fs::metadata(path)
        .map_err(|e| ActionError::error(format!("failed to stat `{}`: {e}", path.display())))?;
    if metadata.is_file() {
        let bytes = std::fs::read(path)
            .map_err(|e| ActionError::error(format!("failed to read `{}`: {e}", path.display())))?;
        let hash = blake3::hash(&bytes);
        Ok((hex::encode(hash.as_bytes()), metadata.len()))
    } else if metadata.is_dir() {
        // Recursive sorted-path BLAKE3 — matches forge's directory hashing
        // convention so consumers can verify against a `forge attest` result.
        let mut hasher = blake3::Hasher::new();
        let mut total_bytes: u64 = 0;
        let mut paths: Vec<PathBuf> = walkdir(path)?;
        paths.sort();
        for p in &paths {
            let rel = p.strip_prefix(path).unwrap_or(p);
            hasher.update(rel.to_string_lossy().as_bytes());
            hasher.update(b"\0");
            let bytes = std::fs::read(p)
                .map_err(|e| ActionError::error(format!("failed to read `{}`: {e}", p.display())))?;
            hasher.update(&bytes);
            total_bytes += bytes.len() as u64;
        }
        Ok((hex::encode(hasher.finalize().as_bytes()), total_bytes))
    } else {
        Err(ActionError::error(format!(
            "artifact `{}` is neither a file nor a directory", path.display()
        )))
    }
}

fn walkdir(path: &PathBuf) -> Result<Vec<PathBuf>, ActionError> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(path)
        .map_err(|e| ActionError::error(format!("failed to read `{}`: {e}", path.display())))?;
    for entry in entries {
        let entry = entry.map_err(|e| ActionError::error(format!("dir-entry error: {e}")))?;
        let p = entry.path();
        if p.is_dir() {
            out.extend(walkdir(&p)?);
        } else if p.is_file() {
            out.push(p);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn blake3_file_matches_known_hash() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.txt");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"hello").unwrap();
        drop(f);
        let (hash, bytes) = blake3_path(&path.to_path_buf()).unwrap();
        // Reference: blake3("hello") = ea8f163db38682925e4491c5e58d4bb3506ef8c14eb78a86e908c5624a67200f
        assert_eq!(hash, "ea8f163db38682925e4491c5e58d4bb3506ef8c14eb78a86e908c5624a67200f");
        assert_eq!(bytes, 5);
    }

    #[test]
    fn blake3_directory_is_deterministic() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("a.txt"), b"alpha").unwrap();
        std::fs::write(root.join("b.txt"), b"beta").unwrap();
        let (hash1, _) = blake3_path(&root.clone()).unwrap();
        let (hash2, _) = blake3_path(&root.clone()).unwrap();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn blake3_directory_total_bytes_sums() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("a.txt"), b"123").unwrap();
        std::fs::write(root.join("b.txt"), b"4567").unwrap();
        let (_, bytes) = blake3_path(&root).unwrap();
        assert_eq!(bytes, 7);
    }
}

#[cfg(test)]
mod test_deps {
    // Ensure tempfile is available as a dev-dep
    #[allow(unused_imports)]
    use tempfile::tempdir as _;
}
