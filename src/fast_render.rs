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


fn append_world_quad(
    mesh: &mut MaterialMeshData,
    corners: [(f32, f32, f32); 4],
    uvs: [(f32, f32); 4],
    shade: f32,
) {
    if mesh.vertices.len() > u16::MAX as usize - 4 {
        return;
    }
    let base = mesh.vertices.len() as u16;
    let tint = Color::new(shade, shade, shade, 1.0);
    for index in 0..4 {
        let p = corners[index];
        mesh.vertices.push(Vertex::new(
            p.0, p.1, p.2,
            uvs[index].0, uvs[index].1,
            tint,
        ));
    }
    mesh.indices.extend_from_slice(&[
        base, base + 1, base + 2, base, base + 2, base + 3,
    ]);
}

fn material_for(chunk: &mut ChunkMeshData, kind: u8) -> &mut MaterialMeshData {
    let index = chunk.materials.iter().position(|material| material.kind == kind);
    if let Some(index) = index {
        return &mut chunk.materials[index];
    }
    chunk.materials.push(MaterialMeshData {
        kind,
        vertices: Vec::new(),
        indices: Vec::new(),
    });
    chunk.materials.last_mut().expect("material was just inserted")
}

/// Adds a continuous height-field surface for natural terrain. The underlying
/// terrain blocks remain in the world data for edits and saves, but only
/// constructed blocks, trees and water are rendered as block models.
fn add_smooth_ground_surface(
    chunk: &mut ChunkMeshData,
    seed: u64,
    style: u8,
    occupied: &HashSet<(i32, i32, i32)>,
) {
    let start_x = chunk.chunk_x * CHUNK_SIZE;
    let start_z = chunk.chunk_z * CHUNK_SIZE;
    let chunk_scale = CHUNK_SIZE as f32;

    for local_x in 0..CHUNK_SIZE {
        for local_z in 0..CHUNK_SIZE {
            let x = start_x + local_x;
            let z = start_z + local_z;
            let h00 = crate::terrain_height_smooth(seed, style, x, z) + 0.035;
            let h01 = crate::terrain_height_smooth(seed, style, x, z + 1) + 0.035;
            let h11 = crate::terrain_height_smooth(seed, style, x + 1, z + 1) + 0.035;
            let h10 = crate::terrain_height_smooth(seed, style, x + 1, z) + 0.035;

            let discrete_height = crate::terrain_height(seed, style, x, z);
            if !occupied.contains(&(x, discrete_height, z)) {
                continue;
            }

            let temperature = crate::value_noise(
                seed ^ 0x13a5ba1d4e7c902f,
                x as f32 * 0.035,
                z as f32 * 0.035,
            );
            let kind = if discrete_height >= 15 || (temperature < 0.17 && discrete_height > 9) {
                7
            } else if discrete_height <= crate::SEA_LEVEL + 1 {
                3
            } else {
                0
            };

            // A directional sun term gives hillsides light and shadow instead
            // of darkening every slope equally.
            let dx = h10 - h00;
            let dz = h01 - h00;
            let normal = vec3(-dx, 1.0, -dz).normalize_or_zero();
            let sun = vec3(-0.42, 0.82, 0.38).normalize();
            let diffuse = normal.dot(sun).max(0.0);
            let local_roughness = ((h11 - h10).abs() + (h11 - h01).abs()) * 0.018;
            let shade = (0.68 + diffuse * 0.34 - local_roughness).clamp(0.58, 1.0);

            let corners = [
                (x as f32, h00, z as f32),
                (x as f32, h01, (z + 1) as f32),
                ((x + 1) as f32, h11, (z + 1) as f32),
                ((x + 1) as f32, h10, z as f32),
            ];

            // One 128px material tile covers a whole 16x16m chunk. This gives
            // grass and sand natural variation without a repeated square stamp
            // on every single terrain cell.
            let u0 = local_x as f32 / chunk_scale;
            let v0 = local_z as f32 / chunk_scale;
            let u1 = (local_x + 1) as f32 / chunk_scale;
            let v1 = (local_z + 1) as f32 / chunk_scale;
            let uvs = [(u0, v0), (u0, v1), (u1, v1), (u1, v0)];
            append_world_quad(material_for(chunk, kind), corners, uvs, shade);
        }
    }

    // Water is one continuous plane at sea level, not a stack of visible
    // cube tops. This removes the striped, rectangular shoreline appearance.
    let water_y = crate::SEA_LEVEL as f32 + 0.025;
    for local_x in 0..CHUNK_SIZE {
        for local_z in 0..CHUNK_SIZE {
            let x = start_x + local_x;
            let z = start_z + local_z;
            let ground_height = crate::terrain_height(seed, style, x, z);
            if ground_height >= crate::SEA_LEVEL || !occupied.contains(&(x, crate::SEA_LEVEL, z)) {
                continue;
            }

            let corners = [
                (x as f32, water_y, z as f32),
                (x as f32, water_y, (z + 1) as f32),
                ((x + 1) as f32, water_y, (z + 1) as f32),
                ((x + 1) as f32, water_y, z as f32),
            ];
            let u0 = local_x as f32 / chunk_scale;
            let v0 = local_z as f32 / chunk_scale;
            let u1 = (local_x + 1) as f32 / chunk_scale;
            let v1 = (local_z + 1) as f32 / chunk_scale;
            let uvs = [(u0, v0), (u0, v1), (u1, v1), (u1, v0)];
            append_world_quad(material_for(chunk, 6), corners, uvs, 0.96);
        }
    }
}

/// Smooth natural terrain plus block models for constructed blocks, trees and
/// water. This preserves the voxel editing model while removing the stepped
/// cube silhouette from the landscape surface.
pub fn build_chunk_data_smooth(
    blocks: &[Block],
    occupied: &HashSet<(i32, i32, i32)>,
    seed: u64,
    style: u8,
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
            let visible_blocks = chunk_blocks
                .into_iter()
                .filter(|block| {
                    block.placed
                        || block.kind == 4
                        || block.kind == 5
                        || (!occupied.contains(&(block.x, block.y + 1, block.z))
                            && block.y < crate::terrain_height(seed, style, block.x, block.z))
                })
                .collect();
            let mut chunk = build_one_chunk(chunk_x, chunk_z, visible_blocks, occupied);
            add_smooth_ground_surface(&mut chunk, seed, style, occupied);
            chunk
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

/// Replace one edited chunk without rebuilding every loaded chunk on the
/// main thread. Neighbor occupancy is still read from the complete world set.
pub fn rebuild_one_chunk(
    chunks: &mut Vec<FastChunk>,
    blocks: &[Block],
    occupied: &HashSet<(i32, i32, i32)>,
    seed: u64,
    style: u8,
    chunk_x: i32,
    chunk_z: i32,
    textures: &[Texture2D; 8],
) {
    let chunk_blocks: Vec<Block> = blocks
        .iter()
        .copied()
        .filter(|block| {
            block.x.div_euclid(CHUNK_SIZE) == chunk_x
                && block.z.div_euclid(CHUNK_SIZE) == chunk_z
        })
        .collect();

    chunks.retain(|chunk| chunk.chunk_x != chunk_x || chunk.chunk_z != chunk_z);
    let data = build_chunk_data_smooth(&chunk_blocks, occupied, seed, style);
    chunks.extend(upload_chunk_data(data, textures));
}

/// Draw only nearby chunks that are inside a loose horizontal camera frustum.
/// The coarse test avoids spending draw calls on terrain behind the player.
pub fn draw_visible_chunks(
    chunks: &[FastChunk],
    camera_position: Vec3,
    camera_forward: Vec3,
    render_distance: f32,
) -> (usize, usize) {
    let limit_squared = render_distance * render_distance;
    let forward = vec2(camera_forward.x, camera_forward.z).normalize_or_zero();
    let chunk_radius = CHUNK_SIZE as f32 * 0.72;
    let mut visible_chunks = 0;
    let mut draw_calls = 0;

    for chunk in chunks {
        let center_x = chunk.chunk_x as f32 * CHUNK_SIZE as f32 + CHUNK_SIZE as f32 * 0.5;
        let center_z = chunk.chunk_z as f32 * CHUNK_SIZE as f32 + CHUNK_SIZE as f32 * 0.5;
        let dx = center_x - camera_position.x;
        let dz = center_z - camera_position.z;
        let distance_squared = dx * dx + dz * dz;
        if distance_squared > limit_squared {
            continue;
        }

        let distance = distance_squared.sqrt();
        if forward.length_squared() > 0.5 && distance > chunk_radius {
            let forward_distance = dx * forward.x + dz * forward.y;
            if forward_distance < distance * 0.55 - chunk_radius {
                continue;
            }
        }

        visible_chunks += 1;
        for mesh in &chunk.meshes {
            draw_mesh(mesh);
            draw_calls += 1;
        }
    }

    (visible_chunks, draw_calls)
}
