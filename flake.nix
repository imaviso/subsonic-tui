{
  description = "TUI music player client for OpenSubsonic-compatible servers";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {self, ...} @ inputs:
    inputs.flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import inputs.nixpkgs {
        inherit system;
        overlays = [
          inputs.self.overlays.default
        ];
      };
      naersk' = pkgs.callPackage inputs.naersk {
        cargo = pkgs.rustToolchain;
        rustc = pkgs.rustToolchain;
      };

      # Platform-specific build inputs
      buildInputs = with pkgs;
        [
          openssl
        ]
        ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
          alsa-lib
        ]
        ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          darwin.apple_sdk.frameworks.AudioUnit
          darwin.apple_sdk.frameworks.CoreAudio
          darwin.apple_sdk.frameworks.CoreServices
        ];

      nativeBuildInputs = with pkgs; [
        pkg-config
      ];

      # Runtime libraries needed on Linux
      runtimeLibs = with pkgs; pkgs.lib.optionals pkgs.stdenv.isLinux [
        alsa-lib
      ];

      # Unwrapped package built by naersk
      unwrapped = naersk'.buildPackage {
        src = ./.;
        inherit buildInputs nativeBuildInputs;
      };
    in {
      packages.default =
        if pkgs.stdenv.isLinux
        then
          pkgs.stdenv.mkDerivation {
            pname = "subsonic-tui";
            version = unwrapped.version or "0.1.0";
            src = unwrapped;
            nativeBuildInputs = [pkgs.makeWrapper];
            installPhase = ''
              mkdir -p $out/bin
              cp $src/bin/subsonic-tui $out/bin/subsonic-tui
              wrapProgram $out/bin/subsonic-tui \
                --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath runtimeLibs}
            '';
            meta = with pkgs.lib; {
              description = "TUI music player for OpenSubsonic servers";
              homepage = "https://github.com/imaviso/subsonic-tui";
              license = licenses.mit;
            };
          }
        else unwrapped;

      packages.unwrapped = unwrapped;

      devShells.default = pkgs.mkShell {
        packages = with pkgs;
          [
            rustToolchain
            openssl
            pkg-config
            cargo-deny
            cargo-edit
            cargo-watch
            rust-analyzer
          ]
          ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            alsa-lib
          ]
          ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            darwin.apple_sdk.frameworks.AudioUnit
            darwin.apple_sdk.frameworks.CoreAudio
            darwin.apple_sdk.frameworks.CoreServices
          ];

        env = {
          # Required by rust-analyzer
          RUST_SRC_PATH = "${pkgs.rustToolchain}/lib/rustlib/src/rust/library";
          OPENSSL_NO_VENDOR = "1";
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
        };

        shellHook = ''
          echo "subsonic-tui development environment"
          echo "Rust version: $(rustc --version)"
        '';
      };

      # Main build check
      checks.default = unwrapped;

      # Clippy check using naersk
      checks.clippy = naersk'.buildPackage {
        src = ./.;
        inherit buildInputs nativeBuildInputs;
        mode = "clippy";
      };
    })
    // {
      overlays.default = final: prev: {
        rustToolchain = with inputs.fenix.packages.${prev.stdenv.hostPlatform.system};
          combine (
            with stable; [
              clippy
              rustc
              cargo
              rustfmt
              rust-src
            ]
          );
      };
    };
}
