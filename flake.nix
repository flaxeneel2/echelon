{
  description = "Tauri Android Development Environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          config = {
            allowUnfree = true;
            android_sdk.accept_license = true;
          };
        };

        platformVersion = "36";

        androidComposition = pkgs.androidenv.composeAndroidPackages {
          includeNDK = true;
          platformVersions = [ platformVersion ];
          abiVersions = [ "x86_64" ];
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
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            rustc
            openssl
            pkg-config
            wrapGAppsHook4
            cargo
            cargo-tauri
            xdg-utils
            bun
          ];

          buildInputs = with pkgs; [
            librsvg
            webkitgtk_4_1
            androidSdk
            emulatorScript
          ];

          shellHook = ''
            # Standard Tauri / Android paths
            export ANDROID_HOME="${androidSdk}/libexec/android-sdk"
            export NDK_HOME="$ANDROID_HOME/ndk-bundle"
            export JAVA_HOME="${pkgs.zulu.home}"
            export XDG_DATA_DIRS="$GSETTINGS_SCHEMAS_PATH"

            # Fix for WebKitGTK/Wayland crashes without disabling Wayland entirely
            export WEBKIT_DISABLE_DMABUF_RENDERER=1

            # Telling it to look at the sdk we installed up above
            export GRADLE_OPTS="-Dorg.gradle.project.android.sdk.channel=0"
            export GRADLE_OPTS="$GRADLE_OPTS -Dorg.gradle.project.android.builder.sdkDownload=false"
            export PATH="$ANDROID_HOME/build-tools/35.0.0:$PATH"

            echo "--- Tauri Android Flake Environment ---"
            echo "Declarative emulator ready."
            echo "To start the emulator, simply run: emulate-tauri"
            echo "---------------------------------------"
          '';
        };
      }
    );
}
