# pleme-io/tameshi-attest

Compute BLAKE3 attestation hashes for release artifacts (files or directory trees).

```yaml
- id: attest
  uses: pleme-io/tameshi-attest@v1
  with:
    artifacts: dist/my-tool-linux-amd64,dist/my-tool-darwin-arm64
    git-sha: ${{ github.sha }}
    release-tag: ${{ github.ref_name }}
- run: |
    echo "Records: $RECORDS"
  env:
    RECORDS: ${{ steps.attest.outputs.records }}
```

## Inputs

| Name | Required | Description |
|---|---|---|
| `artifacts` | yes | Comma-separated paths (files or dirs) |
| `git-sha` | no | Git SHA stamped on every record |
| `release-tag` | no | Release tag (vX.Y.Z) stamped on every record |

## Outputs

| Name | Description |
|---|---|
| `records` | JSON array `[{artifact, blake3, bytes, git_sha, release_tag}]` |
| `count` | Number of artifacts hashed |

## Directory hashing

For directory artifacts, the hash is computed by sorting paths, hashing each as `<rel-path>\0<bytes>`, and finalizing — matches forge's directory hashing convention.

## Part of the pleme-io action library

This action is one of 11 in [`pleme-io/pleme-actions`](https://github.com/pleme-io/pleme-actions) — discovery hub, version compat matrix, contributing guide, and reusable SDLC workflows shared across the library.
