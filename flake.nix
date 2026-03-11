{
  description = "Tauri Android Development Environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            (import rust-overlay)
          ];
          config = {
            allowUnfree = true;
            android_sdk.accept_license = true;
          };
        };

        platformVersion = "36";

        androidComposition = pkgs.androidenv.composeAndroidPackages {
          includeNDK = true;
          platformVersions = [ platformVersion ];
          abiVersions = [ "x86_64" "arm64-v8a" ];
          buildToolsVersions = [ "35.0.0" ];
          includeSystemImages = true;
          systemImageTypes = [ "google_apis" ];
        };

        emulatorScript = pkgs.androidenv.emulateApp {
          name = "emulate-tauri";
          platformVersion = platformVersion;
          abiVersion = "x86_64";
          systemImageType = "google_apis";
        };

        androidSdk = androidComposition.androidsdk;

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analysis" "clippy" "rustfmt" "rust-analyzer" ];
          targets = [
            "aarch64-linux-android"
            "x86_64-unknown-linux-gnu"
            "armv7-linux-androideabi"
            "i686-linux-android"
            "x86_64-linux-android"
          ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            openssl
            pkg-config
            wrapGAppsHook4
            cargo
            cargo-tauri
            xdg-utils
            bun
          ];

          buildInputs = with pkgs; [
            rustToolchain
            librsvg
            webkitgtk_4_1
            androidSdk
            emulatorScript
          ];

          shellHook = ''
            # 1. Environment Paths
            export ANDROID_HOME="${androidSdk}/libexec/android-sdk"
            export NDK_HOME="$ANDROID_HOME/ndk-bundle"
            export JAVA_HOME="${pkgs.zulu.home}"
            export XDG_DATA_DIRS="$GSETTINGS_SCHEMAS_PATH"

            # 2. Linux/Tauri Fixes
            export WEBKIT_DISABLE_DMABUF_RENDERER=1

            # 3. Gradle System Options
            export GRADLE_OPTS="-Dorg.gradle.project.android.sdk.channel=0 -Dorg.gradle.project.android.builder.sdkDownload=false"
            
            # 4. Declarative AAPT2 Sync
            # Injects the Nix store path directly into gradle.properties to ensure 
            # AGP worker daemons use the patched binary.
            PROP_FILE="src-tauri/gen/android/gradle.properties"
            AAPT2_NIX_PATH="${androidSdk}/libexec/android-sdk/build-tools/35.0.0/aapt2"

            if [ -f "$PROP_FILE" ]; then
              # Remove existing override and ensure file ends with a newline
              sed -i '/android.aapt2FromMavenOverride/d' "$PROP_FILE"
              sed -i '$a\' "$PROP_FILE"
              
              # Append the current Nix store path
              echo "android.aapt2FromMavenOverride=$AAPT2_NIX_PATH" >> "$PROP_FILE"
              echo "Synced AAPT2: $PROP_FILE"
            fi

            # 5. Path Update
            export PATH="$ANDROID_HOME/build-tools/35.0.0:$PATH"
          '';
        };
      }
    );
}
