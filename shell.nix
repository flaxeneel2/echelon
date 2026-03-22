let

  rust_overlay_src = builtins.fetchTarball {
    url = "https://github.com/oxalica/rust-overlay/archive/master.tar.gz";
    sha256 = "0qgrkgc695a7gja83dngxrcx4gdg9056gvg5325i5yyjxg0ni6c9";
  };

  pkgs = import <nixpkgs> {
    overlays = [
      (import rust_overlay_src)
    ];
    config = {
      allowUnfree = true;
      android_sdk.accept_license = true;
    };
  };

  # Tauri seems to default to min sdk 24 so this will work for all versions from android 7 onwards
  platformVersion = "36";
  buildToolsVersion = "35.0.0";

  androidComposition = pkgs.androidenv.composeAndroidPackages {
    includeNDK = true;
    platformVersions = [ platformVersion ];
    abiVersions = [ "x86_64" "arm64-v8a" ];
    buildToolsVersions = [ buildToolsVersion ];
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

  rustToolchain = pkgs.rust-bin.stable."1.93.0".default.override {
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
pkgs.mkShell {
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
    emulatorScript # Add the generated script to the shell environment
  ];

  shellHook = ''
    # Android and Java Paths
    export ANDROID_HOME="${androidSdk}/libexec/android-sdk"
    export NDK_HOME="$ANDROID_HOME/ndk-bundle"
    export JAVA_HOME="${pkgs.zulu.home}"
    export XDG_DATA_DIRS="$GSETTINGS_SCHEMAS_PATH"

    # Exports the android build tools to path
    export PATH="$ANDROID_HOME/build-tools/${buildToolsVersion}:$PATH"

    # Disables the DMA-BUF renderer in webkit
    # It causes crashes/weird behaviour otherwise
    export WEBKIT_DISABLE_DMABUF_RENDERER=1

    # Exports Gradle System Options
    # Below does two important things:
    # 1. Stops gradle from trying to download its own tools
    # 2. Forces gradle to use the nix version of aapt2, so nix does not crash out
    export GRADLE_OPTS="-Dorg.gradle.project.android.sdk.channel=0 -Dorg.gradle.project.android.builder.sdkDownload=false -Dorg.gradle.project.android.aapt2FromMavenOverride=${androidSdk}/libexec/android-sdk/build-tools/${buildToolsVersion}/aapt2"
  '';
}
