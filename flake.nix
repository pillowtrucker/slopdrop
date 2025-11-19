{
  description = "Slopdrop - Multi-frontend TCL Evaluation Bot for IRC";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, flake-compat }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Rust toolchain
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        # Common build inputs for both package and devShell
        buildInputs = with pkgs; [
          tcl-8_6
          tclPackages.tclcurl
          tcllib
          openssl
          zlib
          pkg-config
          git
          cacert
          llvmPackages.llvm
          llvmPackages.stdenv
          llvmPackages.stdenv.cc
          llvmPackages.stdenv.cc.cc.lib
          llvmPackages.libclang
          llvmPackages.libclang.lib
        ];

        # Native build inputs
        nativeBuildInputs = with pkgs; [
          pkg-config
          rustToolchain
          llvmPackages.libclang
          llvmPackages.stdenv.cc
        ];

        # Environment variables needed for building
        buildEnvVars = {
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig:${pkgs.tcl-8_6}/lib/pkgconfig:${pkgs.zlib.dev}/lib/pkgconfig";
          TCL_LIBRARY = "${pkgs.tcl-8_6}/lib/tcl8.6";
          TCLLIBPATH = "${pkgs.tclPackages.tclcurl}/lib ${pkgs.tcllib}/lib/tcllib1.21";
          OPENSSL_DIR = "${pkgs.openssl.dev}";
          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
          OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          CPATH = "${pkgs.glibc.dev}/include:${pkgs.zlib.dev}/include";
        };

      in
      {
        packages = {
          default = pkgs.rustPlatform.buildRustPackage rec {
            pname = "slopdrop";
            version = "0.1.0";
            src = ./.;

            cargoLock = {
              lockFile = ./Cargo.lock;
              outputHashes = {
                # If the tcl crate from git needs a hash, add it here
                # "tcl-0.x.x" = "sha256-...";
              };
            };

            inherit buildInputs nativeBuildInputs;

            # Set environment variables for build
            env = {
              PKG_CONFIG_PATH = buildEnvVars.PKG_CONFIG_PATH;
              TCL_LIBRARY = buildEnvVars.TCL_LIBRARY;
              TCLLIBPATH = buildEnvVars.TCLLIBPATH;
              OPENSSL_DIR = buildEnvVars.OPENSSL_DIR;
              OPENSSL_LIB_DIR = buildEnvVars.OPENSSL_LIB_DIR;
              OPENSSL_INCLUDE_DIR = buildEnvVars.OPENSSL_INCLUDE_DIR;
              LIBCLANG_PATH = buildEnvVars.LIBCLANG_PATH;
              CPATH = buildEnvVars.CPATH;
            };

            # Build with all frontends
            buildFeatures = [ "all-frontends" ];

            # Wrap the binary to include runtime dependencies
            postInstall = ''
              wrapProgram $out/bin/slopdrop \
                --set TCL_LIBRARY "${pkgs.tcl-8_6}/lib/tcl8.6" \
                --set TCLLIBPATH "${pkgs.tclPackages.tclcurl}/lib ${pkgs.tcllib}/lib/tcllib1.21" \
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

          # Variant without all frontends (IRC only)
          minimal = self.packages.${system}.default.override {
            buildFeatures = [ ];
          };
        };

        devShells.default = pkgs.mkShell {
          inherit buildInputs;
          nativeBuildInputs = nativeBuildInputs ++ (with pkgs; [
            # Development tools
            cargo-watch
            cargo-edit
            clippy
            rustfmt
          ]);

          # Environment variables
          PKG_CONFIG_PATH = buildEnvVars.PKG_CONFIG_PATH;
          TCL_LIBRARY = buildEnvVars.TCL_LIBRARY;
          TCLLIBPATH = buildEnvVars.TCLLIBPATH;
          OPENSSL_DIR = buildEnvVars.OPENSSL_DIR;
          OPENSSL_LIB_DIR = buildEnvVars.OPENSSL_LIB_DIR;
          OPENSSL_INCLUDE_DIR = buildEnvVars.OPENSSL_INCLUDE_DIR;
          LIBCLANG_PATH = buildEnvVars.LIBCLANG_PATH;
          CPATH = buildEnvVars.CPATH;

          shellHook = ''
            export LIBCLANG_PATH="${pkgs.llvmPackages.libclang.lib}/lib"
            echo "Slopdrop development shell"
            echo "Rust version: $(rustc --version)"
            echo "TCL version: $(echo 'puts [info patchlevel]' | tclsh)"
            echo ""
            echo "Build commands:"
            echo "  cargo build                    # Build with default features (IRC only)"
            echo "  cargo build --features all-frontends  # Build with all frontends"
            echo "  cargo run -- --help            # Show help"
            echo ""
          '';
        };

        # For older NixOS: nix-shell support
        devShell = self.devShells.${system}.default;
      }
    );
}
