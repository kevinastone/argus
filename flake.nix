{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
    treefmt-nix.url = "github:numtide/treefmt-nix";
    treefmt-nix.inputs.nixpkgs.follows = "nixpkgs";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      utils,
      treefmt-nix,
      crane,
      ...
    }:
    utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        cargoToml = fromTOML (builtins.readFile ./Cargo.toml);
        craneLib = crane.mkLib pkgs;
        treefmtStack = treefmt-nix.lib.evalModule pkgs {
          projectRootFile = "flake.nix";
          programs.rustfmt = {
            enable = true;
            edition = "2024";
          };
          # Nix formatters
          programs.nixfmt.enable = true;
          programs.statix.enable = true;
          programs.deadnix.enable = true;
          settings.formatter = {
            deadnix.priority = 1;
            statix.priority = 2;
            nixfmt.priority = 3;
          };
        };

        src = craneLib.cleanCargoSource (craneLib.path ./.);

        commonArgs = {
          inherit src;
          strictDeps = true;
          doCheck = false;
          SSL_CERT_FILE = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";
        }
        // craneLib.crateNameFromCargoToml { inherit src; };

        # Build only dependencies to cache them
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        getTargetPkgs =
          targetSystem:
          if targetSystem == system then
            pkgs
          else
            import nixpkgs {
              inherit system;
              crossSystem = targetSystem;
            };

        # Helper to compile the binary for a target system (either natively or cross-compiled)
        makeBinary =
          targetSystem:
          let
            targetPkgs = getTargetPkgs targetSystem;

            inherit (targetPkgs) stdenv lib;
            targetCraneLib = crane.mkLib targetPkgs;
            isCross = stdenv.hostPlatform != stdenv.buildPlatform;
            isBuildDarwin = stdenv.buildPlatform.isDarwin;

            args =
              commonArgs
              // lib.optionalAttrs (isCross && isBuildDarwin) {
                depsBuildBuild = [
                  targetPkgs.libiconv
                ];
                "NIX_LDFLAGS" = "-L${targetPkgs.buildPackages.libiconv}/lib";
              };

            cargoArtifacts = targetCraneLib.buildDepsOnly args;
          in
          targetCraneLib.buildPackage (
            args
            // {
              inherit cargoArtifacts;
              passthru = {
                inherit targetSystem;
              };
              meta.mainProgram = args.pname;
            }
          );

        # Helper to compile the container image for Linux using a pre-built crossSystem package
        makeImage =
          bin:
          let
            inherit (bin) targetSystem;
            targetPkgs = getTargetPkgs targetSystem;
          in
          with targetPkgs;
          dockerTools.buildLayeredImage {
            name = cargoToml.package.name;
            contents = with dockerTools; [
              caCertificates
              fakeNss
            ];
            config.Entrypoint = [ (lib.getExe bin) ];
            config.Labels = with cargoToml; {
              "org.opencontainers.image.title" = package.name;
              "org.opencontainers.image.source" = package.repository or "";
              "org.opencontainers.image.description" = package.description or "";
            };
          };

        crossTargets = {
          amd64 = "x86_64-unknown-linux-musl";
          arm64 = "aarch64-unknown-linux-musl";
          amd64-glibc = "x86_64-linux";
          arm64-glibc = "aarch64-linux";
        };

        push-multiarch = pkgs.writeShellApplication {
          name = "push-multiarch";
          runtimeInputs = with pkgs; [
            regctl
            gzip
            coreutils
          ];
          text = ''
            if [ "$#" -lt 3 ]; then
              echo "Usage: push-multiarch <registry-repo> <amd64-image-tar> <arm64-image-tar>"
              exit 1
            fi

            REPO=$(echo "$1" | tr '[:upper:]' '[:lower:]')
            AMD64_IMAGE="$2"
            ARM64_IMAGE="$3"

            if [ -z "''${TAGS:-}" ]; then
              echo "Error: TAGS environment variable is not set"
              exit 1
            fi

            TMP_DIR=$(mktemp -d)
            trap 'rm -rf "$TMP_DIR"' EXIT

            # Import images into local OCI layout directories directly from Nix build outputs
            regctl image import "ocidir://$TMP_DIR/amd64" "$AMD64_IMAGE"
            regctl image import "ocidir://$TMP_DIR/arm64" "$ARM64_IMAGE"

            # Get the digests of the imported OCI layouts
            AMD64_DIGEST=$(regctl image digest "ocidir://$TMP_DIR/amd64")
            ARM64_DIGEST=$(regctl image digest "ocidir://$TMP_DIR/arm64")

            # Push single-architecture layers and manifests by digest
            echo "Pushing AMD64 digest: $AMD64_DIGEST to $REPO..."
            regctl image copy "ocidir://$TMP_DIR/amd64" "$REPO@$AMD64_DIGEST"

            echo "Pushing ARM64 digest: $ARM64_DIGEST to $REPO..."
            regctl image copy "ocidir://$TMP_DIR/arm64" "$REPO@$ARM64_DIGEST"

            # Create and push the multi-architecture manifest index for each tag
            # Since TAGS is multiline, we read it line by line
            echo "$TAGS" | while read -r tag || [ -n "$tag" ]; do
              if [ -n "$tag" ]; then
                echo "Creating and pushing multi-arch index for $tag..."
                regctl index create "$tag" \
                  --ref "$REPO@$AMD64_DIGEST" \
                  --platform linux/amd64 \
                  --ref "$REPO@$ARM64_DIGEST" \
                  --platform linux/arm64
              fi
            done
          '';
        };

        crossBins = pkgs.lib.mapAttrs' (
          name: target: pkgs.lib.nameValuePair "bin-${name}" (makeBinary target)
        ) crossTargets;

        crossImages = pkgs.lib.mapAttrs' (
          name: _: pkgs.lib.nameValuePair "image-${name}" (makeImage crossBins."bin-${name}")
        ) crossTargets;
      in
      rec {
        packages = rec {
          bin = makeBinary system;
          default = bin;

          clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );

          test = craneLib.cargoTest (
            commonArgs
            // {
              doCheck = true;
              inherit cargoArtifacts;
            }
          );

          image = makeImage bin;

          inherit push-multiarch;
        }
        // crossBins
        // crossImages;

        checks = {
          build = packages.bin;
          inherit (packages) clippy test;
          formatting = treefmtStack.config.build.check self;
        };

        formatter = treefmtStack.config.build.wrapper;
        devShells.default =
          with pkgs;
          craneLib.devShell {
            checks = self.checks.${system};
            packages = [
              cargo-outdated
              cargo-release
              git-cliff
            ];

            RUST_SRC_PATH = "${rustPlatform.rustLibSrc}";
          };
      }
    );
}
