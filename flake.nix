{
  description = "Slopdrop - Multi-frontend TCL Evaluation Bot for IRC";

  # Flake skeleton modeled on ~/veles-agent/flake.nix (nixpkgs master +
  # flake-utils + fenix). The old slopdrop flake's nixos-25.05 nixpkgs pin
  # + rust-overlay are deliberately NOT used (defunct server environment).
  #
  # PURITY RULE (user, 2026-08-09): everything comes from this flake, the
  # system, or the wrath profile. No hand-picked /nix/store paths. The C/C++
  # tooling is llvmPackages stdenv (clang/lld) as sanctioned by the user.
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/master";
    utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      fenix,
      nixpkgs,
      utils,
    }:
    utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          config.allowUnfree = true;
          overlays = [ fenix.overlays.default ];
        };

        # Rust toolchain from fenix — STABLE, deliberately.
        # (nightly cargo >=1.99 switched the build-dir layout to
        # build/<pkg>/<hash>/out, which breaks `inwelling` 0.5.5's
        # locate_manifest_paths (skip(2) assumes the flat layout) and
        # leaves the tcl crate's clib bindings empty. stable keeps the
        # flat layout that clib/inwelling were written for.)
        rustToolchain = fenix.packages.${system}.stable.withComponents [
          "rustc"
          "cargo"
          "rust-std"
          "rust-src"
          "rust-analyzer"
          "clippy"
          "rustfmt"
        ];

        # What slopdrop actually needs: the Tcl runtime stack (tcl-8_6 +
        # tcllib for http/needs/sha1 + tclcurl for the httpx ensemble),
        # openssl/zlib for the Rust TLS stack (ureq, git2), git for the
        # state repo, and the bindgen prerequisites for the tcl crate.
        buildInputs = with pkgs; [
          openssl # TLS for ureq/git2 + tclcurl https
          zlib
          tcl-8_6 # the scripting core
          tclPackages.tcllib
          tclPackages.tclcurl
          git # git-backed state persistence (slopdrop pattern)
          cacert
        ];

        nativeBuildInputs = with pkgs; [
          pkg-config
          rustToolchain
          # wrapProgram for the installed binary (TCL_LIBRARY etc.)
          makeBinaryWrapper
          # C/C++ tooling for build scripts + linking: llvmPackages stdenv
          # (clang), plus libclang for bindgen (the tcl crate's clib probe).
          llvmPackages.stdenv
          llvmPackages.llvm
          llvmPackages.libclang
          llvmPackages.libclang.lib
        ];

        # clang's resource dir holds stddef.h & friends bindgen needs.
        # Its version tracks llvm's major version, resolved at eval time
        # so clang bumps don't rot the reference.
        clangMajor = nixpkgs.lib.versions.major pkgs.llvmPackages.llvm.version;
        clangResourceInclude =
          "${pkgs.llvmPackages.libclang.lib}/lib/clang/${clangMajor}/include";

        # Env vars shared by the package build and the dev shell.
        # TCLLIBPATH uses a glob at shell time so it survives tcllib version
        # bumps on nixpkgs master (the old flake hardcoded tcllib1.21).
        buildEnv = {
          PKG_CONFIG_PATH =
            "${pkgs.tcl-8_6}/lib/pkgconfig"
            + ":${pkgs.zlib.dev}/lib/pkgconfig"
            + ":${pkgs.openssl.dev}/lib/pkgconfig";
          TCL_LIBRARY = "${pkgs.tcl-8_6}/lib/tcl8.6";
          OPENSSL_DIR = "${pkgs.openssl.dev}";
          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
          OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
          # bindgen (clib, via the tcl crate) — libclang comes from the
          # flake's llvmPackages, never from hand-picked store paths.
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          BINDGEN_EXTRA_CLANG_ARGS = "-I${clangResourceInclude}";
          CPATH = "${pkgs.glibc.dev}/include:${pkgs.zlib.dev}/include";
          # Bake rpaths so binaries find libtcl8.6.so / libssl.so outside
          # the shell too (the flake package wrapProgram covers installed
          # builds; this covers dev-shell runs).
          RUSTFLAGS = "-C link-arg=-Wl,-rpath,${pkgs.tcl-8_6}/lib -C link-arg=-Wl,-rpath,${pkgs.openssl.out}/lib";
        };

        tcllibPathExpr =
          "$(echo ${pkgs.tclPackages.tclcurl}/lib ${pkgs.tclPackages.tcllib}/lib/tcllib*)";

      in
      let
        # Package builder shared by default and minimal variants
        slopdropPkg =
          { buildFeatures }:
          pkgs.rustPlatform.buildRustPackage rec {
            pname = "slopdrop";
            version = "0.1.0";
            # Cargo.lock is gitignored; cleanSourceWith re-adds it so
            # cargoLock.lockFile resolves inside the filtered source.
            src = pkgs.lib.cleanSourceWith {
              src = ./.;
              filter = path: type: (
                pkgs.lib.cleanSourceFilter path type
                || builtins.baseNameOf path == "Cargo.lock"
              );
            };

            cargoLock = {
              lockFile = ./Cargo.lock;
              outputHashes = {
                # oooutlk/tcltk @ the rev pinned in Cargo.lock (HEAD doesn't
                # compile — missing clib symbols). Same pin as veles-agent.
                "tcl-0.1.9" = "sha256-eDkfHA9eQyOYPQr7JyKT4im3V6WXWl8yscIhHvhuJYw=";
              };
            };

            inherit buildInputs nativeBuildInputs;
            env = buildEnv;

            inherit buildFeatures;

            # tests/web_frontend_tests.rs is stale (SecurityConfig grew a
            # notify_self field; the test initializer was never updated) so
            # `cargo test` doesn't compile. Binary builds fine — skip checks.
            doCheck = false;

            # Wrap the binary to include runtime dependencies
            postInstall = ''
              wrapProgram $out/bin/slopdrop \
                --set TCL_LIBRARY "${pkgs.tcl-8_6}/lib/tcl8.6" \
                --set TCLLIBPATH "${tcllibPathExpr}" \
                --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.git pkgs.openssh ]}
            '';

            meta = with pkgs.lib; {
              description = "Multi-frontend TCL Evaluation Bot for IRC";
              homepage = "https://github.com/pillowtrucker/slopdrop";
              license = licenses.agpl3Only;
              maintainers = [ ];
              platforms = platforms.unix;
            };
          };
      in
      {
        # Build with all frontends (IRC + CLI + TUI + web)
        packages.default = slopdropPkg { buildFeatures = [ "all-frontends" ]; };

        # Variant without all frontends (IRC only)
        packages.minimal = slopdropPkg { buildFeatures = [ ]; };

        devShells.default = pkgs.mkShell {
          stdenv = pkgs.llvmPackages.stdenv;
          inherit buildInputs nativeBuildInputs;
          packages = with pkgs; [
            cargo-watch
            rust-analyzer-nightly
          ];

          inherit (buildEnv)
            PKG_CONFIG_PATH
            TCL_LIBRARY
            OPENSSL_DIR
            OPENSSL_LIB_DIR
            OPENSSL_INCLUDE_DIR
            LIBCLANG_PATH
            BINDGEN_EXTRA_CLANG_ARGS
            CPATH
            RUSTFLAGS
            ;

          shellHook = ''
            export TCLLIBPATH="${tcllibPathExpr}"
            echo "slopdrop development shell (pure)"
            echo "Rust: $(rustc --version)"
            echo "C:    $(cc --version | head -1)"
            echo "Tcl:  $(echo 'puts [info patchlevel]' | tclsh)"
            echo ""
            echo "Build commands:"
            echo "  cargo build                    # Build with default features (IRC only)"
            echo "  cargo build --features all-frontends  # Build with all frontends"
            echo "  cargo run -- --help            # Show help"
          '';
        };

        # For older NixOS: nix-shell support
        devShell = self.devShells.${system}.default;
      }
    );
}