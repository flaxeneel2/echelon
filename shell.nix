let
  pkgs = import <nixpkgs> { };
  # Define the android composition so we can reference it in multiple places
  androidComposition = pkgs.androidenv.composeAndroidPackages {
    includeNDK = true;
    # Add specific versions if Tauri complains, e.g., ndkVersion = "25.1.8937393";
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
  ];

  shellHook = ''
    # Set Android SDK path
    export ANDROID_HOME="${androidSdk}/libexec/android-sdk"
    
    # Set NDK path (Tauri specifically looks for this)
    # The path structure in Nix can vary, but this is the standard location:
    export NDK_HOME="$ANDROID_HOME/ndk-bundle"
    
    # Ensure Java is available (Android tools need it)
    export JAVA_HOME="${pkgs.zulu.home}"

    export XDG_DATA_DIRS="$GSETTINGS_SCHEMAS_PATH"
    
    echo "Android SDK Environment Ready"
    echo "ANDROID_HOME: $ANDROID_HOME"
  '';
}
