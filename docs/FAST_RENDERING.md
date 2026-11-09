# RealismCraft rendering performance

The renderer now batches visible block faces into chunk meshes. Each chunk
contains one mesh per material, rather than issuing one cube draw for every
block on every frame.

## Current performance-oriented choices

- Chunk size: 16 x 16 columns.
- Faces touching another occupied block are omitted from the mesh.
- CPU mesh construction is parallelized with Rayon.
- World generation and mesh construction run on a worker thread while the
  loading screen keeps the main frame loop alive.
- The render loop skips chunks outside a fixed horizontal distance.
- The default window disables MSAA and high-DPI rendering to reduce GPU fill
  cost on older integrated graphics.
- The HUD shows FPS and frame time. F3 toggles visible chunk and draw-call
  counters.

## Controls

- Mouse: look around (captured while in the world).
- WASD: move; Space and Left Control: vertical movement.
- Left click: place; right click: remove.
- 1-8: select a material.
- Enter: open chat; Escape: close chat or save and return to menu.
- F3: toggle renderer counters.

Chat command example:

```text
!set GameMode creative
!set GameMode survival
!set GameMode adventure
!set GameMode spectator
```

These modes are currently parsed and displayed by the prototype; full survival
rules, inventory restrictions and spectator collision rules still need to be
implemented.

## Benchmarking

Compare builds on the same machine, at the same window size, with the same
world seed and camera path. Record average FPS, 1% low FPS, frame-time
percentiles, world-load time, chunk count, mesh draw calls, and memory use.
Do not compare one machine's FPS to another machine's FPS as a direct measure
of renderer quality.

This is a first optimization pass, not a promise of Horizon-equivalent
performance. Large worlds will need chunk streaming, frustum culling, mesh
reuse, save I/O workers, and a more advanced GPU backend.

## Optional local CPU tuning

For a binary used only on the Mac that built it, you can let LLVM target that
machine's CPU features:

```sh
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

Do not use this flag for a general download or cross-platform release: the
result may use CPU instructions that another computer does not support. The
GitHub release workflow intentionally builds portable target binaries.
