use macroquad::prelude::*;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::Block;

pub const CHUNK_SIZE: i32 = 16;

pub struct MaterialMeshData {
    pub kind: u8,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

pub struct ChunkMeshData {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub materials: Vec<MaterialMeshData>,
}

pub struct FastChunk {
    chunk_x: i32,
    chunk_z: i32,
    meshes: Vec<Mesh>,
}

const FACES: [((i32, i32, i32), [(f32, f32, f32); 4], f32); 6] = [
    ((1, 0, 0), [(0.5,-0.5,-0.5),(0.5,0.5,-0.5),(0.5,0.5,0.5),(0.5,-0.5,0.5)], 0.82),
    ((-1,0,0),[(-0.5,-0.5,0.5),(-0.5,0.5,0.5),(-0.5,0.5,-0.5),(-0.5,-0.5,-0.5)],0.74),
    ((0,1,0), [(-0.5,0.5,-0.5),(-0.5,0.5,0.5),(0.5,0.5,0.5),(0.5,0.5,-0.5)], 1.0),
    ((0,-1,0),[(-0.5,-0.5,0.5),(-0.5,-0.5,-0.5),(0.5,-0.5,-0.5),(0.5,-0.5,0.5)],0.56),
    ((0,0,1), [(-0.5,-0.5,0.5),(0.5,-0.5,0.5),(0.5,0.5,0.5),(-0.5,0.5,0.5)],0.90),
    ((0,0,-1),[(0.5,-0.5,-0.5),(-0.5,-0.5,-0.5),(-0.5,0.5,-0.5),(0.5,0.5,-0.5)],0.68),
];

fn append_face(mesh: &mut MaterialMeshData, x: i32, y: i32, z: i32, corners: [(f32,f32,f32);4], shade: f32) {
    if mesh.vertices.len() > (u16::MAX as usize - 4) {
        return;
    }
    let base = mesh.vertices.len() as u16;
    let uvs = [vec2(0.0, 1.0), vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(1.0, 1.0)];
    let tint = Color::new(shade, shade, shade, 1.0);
    for index in 0..4 {
        let p = corners[index];
        mesh.vertices.push(Vertex::new(
            x as f32 + p.0, y as f32 + p.1, z as f32 + p.2,
            uvs[index].x, uvs[index].y, tint,
        ));
    }
    mesh.indices.extend_from_slice(&[
        base, base + 1, base + 2, base, base + 2, base + 3,
    ]);
}

fn build_one_chunk(
    chunk_x: i32,
    chunk_z: i32,
    chunk_blocks: Vec<Block>,
    occupied: &HashSet<(i32, i32, i32)>,
) -> ChunkMeshData {
    let mut materials: [MaterialMeshData; 8] = std::array::from_fn(|kind| MaterialMeshData {
        kind: kind as u8,
        vertices: Vec::new(),
        indices: Vec::new(),
    });

    for block in chunk_blocks {
        let kind = (block.kind.min(7)) as usize;

        for (neighbor, corners, shade) in FACES {
            let adjacent = (
                block.x + neighbor.0,
                block.y + neighbor.1,
                block.z + neighbor.2,
            );
            if occupied.contains(&adjacent) {
                continue;
            }

            // One grass block: a turf cap over a soil body, with a narrow
            // grassy lip on each exposed vertical side.
            if kind == 0 && neighbor.1 == 0 {
                append_face(&mut materials[1], block.x, block.y, block.z, corners, shade);
                let mut grass_lip = corners;
                grass_lip[0].1 = 0.34;
                grass_lip[3].1 = 0.34;
                append_face(&mut materials[0], block.x, block.y, block.z, grass_lip, shade * 1.04);
            } else {
                let face_material = if kind == 0 && neighbor.1 == -1 { 1 } else { kind };
                append_face(
                    &mut materials[face_material],
                    block.x,
                    block.y,
                    block.z,
                    corners,
                    shade,
                );
            }
        }
    }

    ChunkMeshData {
        chunk_x,
        chunk_z,
        materials: materials.into_iter().filter(|mesh| !mesh.indices.is_empty()).collect(),
    }
}

/// Build CPU-only chunk meshes. Safe to run on worker threads; GPU texture
/// handles are attached later on the main thread.
pub fn build_chunk_data(
    blocks: &[Block],
    occupied: &HashSet<(i32, i32, i32)>,
) -> Vec<ChunkMeshData> {
    let mut grouped: HashMap<(i32, i32), Vec<Block>> = HashMap::new();
    for block in blocks {
        grouped
            .entry((block.x.div_euclid(CHUNK_SIZE), block.z.div_euclid(CHUNK_SIZE)))
            .or_default()
            .push(*block);
    }

    let mut chunks: Vec<ChunkMeshData> = grouped
        .into_par_iter()
        .map(|((chunk_x, chunk_z), chunk_blocks)| {
            build_one_chunk(chunk_x, chunk_z, chunk_blocks, occupied)
        })
        .collect();

    chunks.sort_by_key(|chunk| (chunk.chunk_x, chunk.chunk_z));
    chunks
}

/// Attach textures on the render thread after worker meshing is complete.
pub fn upload_chunk_data(
    data: Vec<ChunkMeshData>,
    textures: &[Texture2D; 8],
) -> Vec<FastChunk> {
    data.into_iter()
        .map(|chunk| {
            let meshes = chunk
                .materials
                .into_iter()
                .map(|material| Mesh {
                    vertices: material.vertices,
                    indices: material.indices,
                    texture: Some(textures[material.kind as usize].clone()),
                })
                .collect();
            FastChunk {
                chunk_x: chunk.chunk_x,
                chunk_z: chunk.chunk_z,
                meshes,
            }
        })
        .collect()
}

/// Draw only nearby chunks; each mesh contains many visible block faces.
pub fn draw_visible_chunks(
    chunks: &[FastChunk],
    camera_position: Vec3,
    render_distance: f32,
) -> (usize, usize) {
    let limit_squared = render_distance * render_distance;
    let mut visible_chunks = 0;
    let mut draw_calls = 0;

    for chunk in chunks {
        let center_x = chunk.chunk_x as f32 * CHUNK_SIZE as f32 + CHUNK_SIZE as f32 * 0.5;
        let center_z = chunk.chunk_z as f32 * CHUNK_SIZE as f32 + CHUNK_SIZE as f32 * 0.5;
        let dx = center_x - camera_position.x;
        let dz = center_z - camera_position.z;
        if dx * dx + dz * dz > limit_squared {
            continue;
        }

        visible_chunks += 1;
        for mesh in &chunk.meshes {
            draw_mesh(mesh);
            draw_calls += 1;
        }
    }

    (visible_chunks, draw_calls)
}
