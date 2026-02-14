let
  pkgs = import <nixpkgs> {
    config = {
      allowUnfree = true;
      android_sdk.accept_license = true;
    };
  };

  platformVersion = "31"; # Android 12

  androidComposition = pkgs.androidenv.composeAndroidPackages {
    includeNDK = true;
    platformVersions = [ platformVersion ];
    abiVersions = [ "x86_64" ];
    includeSystemImages = true;
    systemImageTypes = [ "default" ];
  };

  emulatorScript = pkgs.androidenv.emulateApp {
    name = "emulate-tauri";
    platformVersion = platformVersion;
    abiVersion = "x86_64";
    systemImageType = "default";
  };

  androidSdk = androidComposition.androidsdk;
in
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    pkg-config
    wrapGAppsHook4
    cargo
    cargo-tauri
    bun
  ];

  buildInputs = with pkgs; [
    librsvg
    webkitgtk_4_1
    androidSdk
    emulatorScript # Add the generated script to the shell environment
  ];

  shellHook = ''
    # Standard Tauri / Android paths
    export ANDROID_HOME="${androidSdk}/libexec/android-sdk"
    export NDK_HOME="$ANDROID_HOME/ndk-bundle"
    export JAVA_HOME="${pkgs.zulu.home}"
    export XDG_DATA_DIRS="$GSETTINGS_SCHEMAS_PATH"

    echo "--- Tauri Android Environment ---"
    echo "Declarative emulator ready."
    echo "To start the emulator, simply run: run-test-emulator"
    echo "---------------------------------"
  '';
}