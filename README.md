# Echelon

A (hopefully) good crossplatform client for matrix servers.

## Contributing

Currently, this is just a personal project with some uni friends, but we are always open for feature requests and bug reports.

## Building

### Pre requisites

- Rust
- Bun (Nodejs probably works too, but we use bun for development)
- Android studio with android SDK 35. (if you plan on building for android)
- Xcode (if you plan on building for macOS/iOS)

If you are on NixOS, you can use the provided `shell.nix` to get a development environment with all the necessary dependencies.

Run `nix-shell shell.nix` to enter the development environment.

### Installing dependencies

To install dependencies, run:

```sh
bun install
```

### Building for desktop

To build for desktop, run:

```sh
bun run tauri build
```

### Building for android

NOTE: ANDROID BUILDS ARE BROKEN FOR NOW (on nix at least, no android 36 for now, on stable)

To build for android, run:

```sh
bun run tauri android build
```