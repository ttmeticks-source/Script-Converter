# Creating RealismCraft Mods

RealismCraft mods are designed around sandboxed WebAssembly modules. A mod should
not receive direct filesystem, process, or network access. The host will expose
a versioned API for registering blocks, items, recipes, world-generation hooks,
and UI panels.

## Starter layout

```
my-mod/
  mod.json
  src/
    lib.rs
  build/
    my_mod.wasm
```

Copy the example manifest and change the id, name, version, and entry. The entry
must point to a compiled .wasm module. Use a unique lowercase id.

Any language that can compile to WebAssembly can be used, including Rust, C/C++,
AssemblyScript, Go, and Zig. The exact host API is still under development;
mods should not assume unrestricted system access.

## Current status

The repository contains a mod manifest format and validation foundation.
The in-game mod browser, WASM runtime, permission prompts, and stable mod API
are not yet wired into the game. Do not install untrusted WASM mods.
