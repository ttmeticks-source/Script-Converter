# RealismCraft

<p align="center"><img src="assets/realismcraft-banner.svg" alt="RealismCraft — Build your own universe" width="100%"></p>

**A standalone voxel sandbox for macOS and Windows.**

RealismCraft is an original block-building game project written in Rust. It is not Minecraft, does not bundle Mojang assets, and does not require a Minecraft account.

![RealismCraft](https://img.shields.io/badge/Rust-powered-orange?logo=rust)
![Platforms](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-blue)
![Status](https://img.shields.io/badge/status-early%20prototype-yellow)

## What is planned

- A custom galaxy-inspired launcher and first-run player-name setup.
- Optional online wallpaper gallery, with a built-in galaxy background and user-selectable backgrounds.
- Creative building with an expanding library of original blocks.
- Procedural worlds and local save files.
- High-quality lighting, materials, and optional visual effects.
- Player skins and a skin editor using user-created or properly licensed assets.
- Optional multiplayer with user-hosted servers.
- A modding SDK with a sandboxed WebAssembly interface. Mods can be written in languages that compile to WASM; native arbitrary code is not loaded into the game.

## Current prototype

The first milestone is a small, playable 3D voxel sandbox. It establishes the Rust project, rendering loop, movement, and block interaction. The launcher, persistent worlds, networking, skin editor, and mod SDK are planned milestones, not finished features yet.

## Build from source

Install a current stable Rust toolchain from [rustup.rs](https://rustup.rs/).

### macOS

```sh
git clone https://github.com/ttmeticks-source/Script-Converter.git
cd Script-Converter
cargo run --release
```

### Windows

Install Rust, then run in PowerShell:

```powershell
git clone https://github.com/ttmeticks-source/Script-Converter.git
cd Script-Converter
cargo run --release
```

## Controls

- **WASD** — move
- **Arrow keys** — look around
- **Space / Left Shift** — move up / down
- **Left click** — place a block
- **Right click** — remove a nearby block
- **Escape** — return to the menu

## Project structure

```
src/main.rs        Game prototype
assets/            Original game assets (added as the project grows)
mods/              Local mod examples and documentation
.github/workflows/ Cross-platform build workflow
```

## Modding direction

The planned mod API uses WebAssembly (WASM) as its portable sandbox. Rust, C/C++, and other languages with a WASM toolchain can target it. A future SDK will define supported hooks, permissions, and version compatibility. Mods will not receive unrestricted access to the user's computer.

## License

The source code is intended to be released under the MIT License. Third-party libraries and user-provided assets remain under their own licenses.
