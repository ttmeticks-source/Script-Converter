# RealismCraft mods

This folder is reserved for community mods and examples.

## Planned format

RealismCraft is designed around a WebAssembly (WASM) mod sandbox. A mod will declare:
- its name and version;
- the RealismCraft API version it needs;
- requested capabilities;
- its entry point.

The host game will expose a limited, versioned API for supported actions such as registering blocks, adding recipes, and responding to gameplay events.

## Language support

WASM is a compilation target, not a promise that every language works automatically. Languages with a compatible WASM toolchain can be supported, including Rust, C, C++, and AssemblyScript. Additional languages may use a separately installed runtime or a bridge when one is implemented.

Native executables and unrestricted scripts will not be loaded into the game process. This protects players from mods silently accessing files, credentials, or other applications.

The mod API and loader are not implemented in the current prototype yet.
