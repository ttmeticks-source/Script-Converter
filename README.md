# RealismCraft

<p align="center"><img src="assets/realismcraft-banner.svg" alt="RealismCraft — Build your own universe" width="100%"></p>

**An original voxel sandbox for macOS and Windows.**

RealismCraft is a standalone block-building game project written in Rust and Macroquad. It is its own game, not a Minecraft client, and does not include Mojang code or assets.

![RealismCraft](https://img.shields.io/badge/Rust-powered-orange?logo=rust)
![Platforms](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-blue)
![Status](https://img.shields.io/badge/status-playable%20prototype-yellow)

## Current features

- **Account screen:** first launch opens a create-account flow. Returning players sign in. The profile is stored locally and works offline; there is no online account server.
- **Password handling:** passwords are stored as salted PBKDF2-HMAC-SHA256 hashes, not as readable text.
- **World setup:** choose a world name, enter a seed, or generate a random seed.
- **Random terrain:** seeded Natural, Highlands, and Islands terrain presets, with layered ground, beaches, water, snowy areas, and scattered trees.
- **World saves:** the current world and placed/removed blocks are saved locally.
- **Material textures:** procedural 128×128 surface textures for grass, dirt, stone, sand, wood, leaves, water, and snow. These are generated materials inspired by real surfaces, not photo-scanned decal packs.
- **Chunk renderer:** 16×16 chunk meshes batch visible block faces by material; buried faces are omitted.
- **Parallel loading:** terrain and CPU mesh construction run on worker threads with Rayon while a loading screen stays responsive.
- **Performance HUD:** FPS and frame-time display, with F3 counters for visible chunks and mesh draw calls.
- **First-person camera:** mouse-look with cursor capture, WASD movement, and vertical controls.
- **Chat commands:** `!set GameMode creative`, `survival`, `adventure`, or `spectator`.
- **Creative building:** place and remove blocks with a quick-select hotbar.
- **Galaxy-style menus:** a custom first-run and main menu interface.

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
- **Mouse** — look around (cursor is captured in-world)
- **Enter** — open chat; type `!set GameMode creative` to switch modes
- **F3** — show/hide renderer counters
- **Space / Left Ctrl** — move up / down
- **Left Shift** — move faster
- **Left click** — place selected block
- **Right click** — remove a nearby block
- **1–8** — select a material
- **Escape** — save and return to the menu

## Local data

The account and world save are kept in the operating system's application data folder:

- **macOS:** `~/Library/Application Support/RealismCraft/`
- **Windows:** `%APPDATA%\\RealismCraft\\`
- **Linux:** `$XDG_DATA_HOME/RealismCraft/` (or `~/.local/share/RealismCraft/`)

The local profile is not connected to an authentication server and cannot be used to sign into a website or multiplayer service.

## Planned next milestones

- Real multiplayer sessions with an explicit protocol/server bridge; current remote avatars are only a visual simulation.
- Further performance work: chunk streaming, frustum culling, mesh reuse, and GPU-side rendering.
- Higher-detail material maps, lighting, shadows, and ambient effects.
- World selection and multiple save slots.
- More biomes, caves, ores, and structures.
- Skin editor and user-created skins.
- A sandboxed WebAssembly mod runtime. Mods will not receive unrestricted access to the user's computer.
- Optional Pumpkin server adapter; Pumpkin is a separate GPLv3 server, not a renderer dependency. See [Pumpkin integration notes](docs/PUMPKIN_BACKEND.md).

## License

The source code is intended to be released under the MIT License. Third-party libraries and user-provided assets remain under their own licenses.
