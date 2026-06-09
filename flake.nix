{
  description = "pleme-io/tameshi-attest — BLAKE3 source/build/artifact attestation for a release";

  inputs = {
    nixpkgs.follows = "substrate/nixpkgs";
    crate2nix = { url = "github:nix-community/crate2nix"; inputs.nixpkgs.follows = "nixpkgs"; };
    flake-utils.url = "github:numtide/flake-utils";
    substrate = { url = "github:pleme-io/substrate";};
  };

  outputs = inputs @ { self, nixpkgs, crate2nix, flake-utils, substrate, ... }:
    (import "${substrate}/lib/rust-action-release-flake.nix" {
      inherit nixpkgs crate2nix flake-utils;
    }) {
      toolName = "tameshi-attest";
      src = self;
      repo = "pleme-io/tameshi-attest";
      action = {
        description = "Compute BLAKE3 hashes for release artifacts (files or sorted-path directory hashes). Lightweight standalone of forge's full attestation chain — for the common case of 'give me a content-addressable hash for this artifact.' Outputs JSON the consumer attaches to a GitHub Release body or sekiban annotation.";
        inputs = [
          { name = "artifacts"; description = "Comma-separated list of files or directories to hash"; required = true; }
          { name = "git-sha"; description = "Git SHA to stamp onto each record"; }
          { name = "release-tag"; description = "Release tag (vX.Y.Z) to stamp onto each record"; }
        ];
        outputs = [
          { name = "records"; description = "JSON array of {artifact, blake3, bytes, git_sha, release_tag}"; }
          { name = "count"; description = "Number of artifacts hashed"; }
        ];
      };
    };
}
