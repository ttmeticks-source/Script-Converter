use rayon::prelude::*;
use std::collections::HashSet;

use crate::{fast_render, Block, SEA_LEVEL};

fn push(
    blocks: &mut Vec<Block>,
    occupied: &mut HashSet<(i32, i32, i32)>,
    x: i32,
    y: i32,
    z: i32,
    kind: u8,
) {
    if occupied.insert((x, y, z)) {
        blocks.push(Block { x, y, z, kind, placed: false });
    }
}

/// Generates one 16x16 terrain chunk using absolute world coordinates.
/// The generator is deterministic: revisiting a coordinate recreates the same
/// terrain from its seed, so the world has no fixed outer edge.
pub fn generate_chunk(seed: u64, style: u8, chunk_x: i32, chunk_z: i32) -> (Vec<Block>, HashSet<(i32, i32, i32)>) {
    let mut blocks = Vec::with_capacity(4_500);
    let mut occupied = HashSet::with_capacity(4_500);
    let start_x = chunk_x * fast_render::CHUNK_SIZE;
    let start_z = chunk_z * fast_render::CHUNK_SIZE;

    for local_x in 0..fast_render::CHUNK_SIZE {
        for local_z in 0..fast_render::CHUNK_SIZE {
            let x = start_x + local_x;
            let z = start_z + local_z;
            let height = crate::terrain_height(seed, style, x, z);
            let temperature = crate::value_noise(
                seed ^ 0x13a5ba1d4e7c902f,
                x as f32 * 0.035,
                z as f32 * 0.035,
            );

            let surface_kind = if height >= 15 || (temperature < 0.17 && height > 9) {
                7
            } else if height <= SEA_LEVEL + 1 {
                3
            } else {
                0
            };

            for y in -5..=height {
                let kind = if y == height {
                    surface_kind
                } else if y >= height - 2 && surface_kind == 0 {
                    1
                } else if y >= height - 2 && surface_kind == 3 {
                    3
                } else {
                    2
                };
                push(&mut blocks, &mut occupied, x, y, z, kind);
            }

            if height < SEA_LEVEL {
                for y in height + 1..=SEA_LEVEL {
                    push(&mut blocks, &mut occupied, x, y, z, 6);
                }
            }

            let tree_roll = crate::hash01(seed ^ 0x6f2a91c5d3e84017, x, z);
            if surface_kind == 0 && height > SEA_LEVEL + 1 && tree_roll > 0.988 {
                for y in height + 1..=height + 4 {
                    push(&mut blocks, &mut occupied, x, y, z, 4);
                }
                for dy in 2..=4 {
                    let radius: i32 = if dy == 4 { 1 } else { 2 };
                    for dx in -radius..=radius {
                        for dz in -radius..=radius {
                            if dx.abs() + dz.abs() <= radius + 1 {
                                // Keep each tree inside its owning chunk. The next
                                // chunk will generate its own trees deterministically.
                                let tx = x + dx;
                                let tz = z + dz;
                                if tx >= start_x && tx < start_x + fast_render::CHUNK_SIZE
                                    && tz >= start_z && tz < start_z + fast_render::CHUNK_SIZE
                                {
                                    push(&mut blocks, &mut occupied, tx, height + dy, tz, 5);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    (blocks, occupied)
}

/// Generates and meshes a batch of chunks in parallel on a worker thread.
pub fn build_region(
    seed: u64,
    style: u8,
    coordinates: Vec<(i32, i32)>,
) -> (Vec<Block>, HashSet<(i32, i32, i32)>, Vec<fast_render::ChunkMeshData>) {
    let built: Vec<_> = coordinates
        .into_par_iter()
        .map(|(chunk_x, chunk_z)| {
            let (blocks, occupied) = generate_chunk(seed, style, chunk_x, chunk_z);
            let mesh = fast_render::build_chunk_data_smooth(&blocks, &occupied, seed, style);
            (blocks, occupied, mesh)
        })
        .collect();

    let mut all_blocks = Vec::new();
    let mut all_occupied = HashSet::new();
    let mut all_meshes = Vec::new();

    for (blocks, occupied, meshes) in built {
        all_blocks.extend(blocks);
        all_occupied.extend(occupied);
        all_meshes.extend(meshes);
    }

    (all_blocks, all_occupied, all_meshes)
}
