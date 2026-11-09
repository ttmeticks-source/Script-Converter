mod fast_render;
mod systems;
mod world_stream;

use macroquad::audio::{load_sound_from_bytes, play_sound_once};
use macroquad::prelude::*;
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};

const PASSWORD_ROUNDS: u32 = 120_000;
pub(crate) const SEA_LEVEL: i32 = 4;
const WORLD_RADIUS: i32 = 18;

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    CreateAccount,
    Login,
    Menu,
    WorldSetup,
    Loading,
    World,
}

#[derive(Clone, Copy)]
pub(crate) struct Block {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) z: i32,
    pub(crate) kind: u8,
    pub(crate) placed: bool,
}

struct LocalAccount {
    username: String,
    salt: [u8; 16],
    password_hash: [u8; 32],
}

fn window_conf() -> Conf {
    Conf {
        window_title: "RealismCraft".to_owned(),
        window_width: 1280,
        window_height: 800,
        high_dpi: false,
        fullscreen: false,
        sample_count: 1,
        window_resizable: true,
        platform: miniquad::conf::Platform {
            // Ask the driver for an uncapped presentation interval and use
            // Apple's native Metal backend where supported.
            apple_gfx_api: miniquad::conf::AppleGfxApi::Metal,
            swap_interval: Some(0),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn data_directory() -> PathBuf {
    if cfg!(target_os = "macos") {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        home.join("Library")
            .join("Application Support")
            .join("RealismCraft")
    } else if cfg!(target_os = "windows") {
        let root = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        root.join("RealismCraft")
    } else {
        let root = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(".local").join("share"))
            })
            .unwrap_or_else(|| PathBuf::from("."));
        root.join("RealismCraft")
    }
}

fn account_path() -> PathBuf {
    data_directory().join("account.dat")
}

fn world_path() -> PathBuf {
    data_directory().join("world.dat")
}

fn session_path() -> PathBuf {
    data_directory().join("session.dat")
}

fn remember_session(username: &str) {
    if fs::create_dir_all(data_directory()).is_ok() {
        // This remembers the local session, never the password.
        let _ = fs::write(session_path(), username.trim());
    }
}

fn session_is_remembered(account: &LocalAccount) -> bool {
    fs::read_to_string(session_path())
        .map(|saved| saved.trim().eq_ignore_ascii_case(&account.username))
        .unwrap_or(false)
}

fn forget_session() {
    let _ = fs::remove_file(session_path());
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn decode_hex<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N * 2 {
        return None;
    }
    let mut result = [0_u8; N];
    for index in 0..N {
        result[index] = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(result)
}

fn load_account() -> Option<LocalAccount> {
    let contents = fs::read_to_string(account_path()).ok()?;
    let mut lines = contents.lines();
    let username = lines.next()?.to_owned();
    let salt = decode_hex::<16>(lines.next()?)?;
    let password_hash = decode_hex::<32>(lines.next()?)?;
    Some(LocalAccount {
        username,
        salt,
        password_hash,
    })
}

fn valid_username(username: &str) -> bool {
    let length = username.chars().count();
    (3..=18).contains(&length)
        && username
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn create_local_account(username: &str, password: &str) -> Result<LocalAccount, String> {
    let username = username.trim();
    if !valid_username(username) {
        return Err("Use 3-18 letters, numbers, _ or - for the username.".to_owned());
    }
    if password.len() < 8 {
        return Err("Your password needs at least 8 characters.".to_owned());
    }
    if account_path().exists() {
        return Err("A local account already exists. Sign in instead.".to_owned());
    }

    let mut salt = [0_u8; 16];
    getrandom::getrandom(&mut salt)
        .map_err(|_| "Could not create a secure local account.".to_owned())?;

    let mut password_hash = [0_u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, PASSWORD_ROUNDS, &mut password_hash);

    fs::create_dir_all(data_directory())
        .map_err(|_| "Could not create the RealismCraft data folder.".to_owned())?;
    let contents = format!(
        "{}\n{}\n{}\n",
        username,
        encode_hex(&salt),
        encode_hex(&password_hash)
    );
    fs::write(account_path(), contents)
        .map_err(|_| "Could not save the local account.".to_owned())?;

    Ok(LocalAccount {
        username: username.to_owned(),
        salt,
        password_hash,
    })
}

fn verify_login(account: &LocalAccount, username: &str, password: &str) -> bool {
    if !account.username.eq_ignore_ascii_case(username.trim()) {
        return false;
    }

    let mut candidate = [0_u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), &account.salt, PASSWORD_ROUNDS, &mut candidate);
    let difference = account
        .password_hash
        .iter()
        .zip(candidate.iter())
        .fold(0_u8, |value, (left, right)| value | (left ^ right));
    difference == 0
}

fn random_seed() -> u64 {
    let mut bytes = [0_u8; 8];
    if getrandom::getrandom(&mut bytes).is_ok() {
        u64::from_ne_bytes(bytes)
    } else {
        (get_time() * 1_000_000.0) as u64 ^ 0x9e3779b97f4a7c15
    }
}

fn save_world(
    name: &str,
    seed: u64,
    style: u8,
    game_mode: systems::GameMode,
    blocks: &[Block],
) -> Result<(), String> {
    fs::create_dir_all(data_directory())
        .map_err(|_| "Could not create the world save folder.".to_owned())?;

    let mut contents = format!(
        "{}\n{}\n{}\n{}\n",
        name.replace('\n', " "),
        seed,
        style,
        game_mode.label()
    );
    for block in blocks {
        contents.push_str(&format!(
            "{},{},{},{},{}\n",
            block.x, block.y, block.z, block.kind, u8::from(block.placed)
        ));
    }
    fs::write(world_path(), contents)
        .map_err(|_| "Could not write the world save file.".to_owned())
}

fn load_world() -> Option<(String, u64, u8, systems::GameMode, Vec<Block>)> {
    let contents = fs::read_to_string(world_path()).ok()?;
    let mut lines = contents.lines();
    let name = lines.next()?.to_owned();
    let seed = lines.next()?.parse::<u64>().ok()?;
    let style = lines.next()?.parse::<u8>().ok()?.min(2);
    let remaining: Vec<&str> = lines.collect();

    // Older saves had no game-mode row, so keep them loadable as Survival.
    let (game_mode, block_lines) = match remaining.first() {
        Some(value) => match systems::GameMode::parse(value) {
            Some(mode) => (mode, &remaining[1..]),
            None => (systems::GameMode::Survival, &remaining[..]),
        },
        None => (systems::GameMode::Survival, &remaining[..]),
    };
    let mut blocks = Vec::new();

    for line in block_lines {
        let mut values = line.split(',');
        let x = values.next()?.parse::<i32>().ok()?;
        let y = values.next()?.parse::<i32>().ok()?;
        let z = values.next()?.parse::<i32>().ok()?;
        let kind = values.next()?.parse::<u8>().ok()?;
        let placed = values.next().and_then(|value| value.parse::<u8>().ok()).unwrap_or(0) != 0;
        if kind <= 7 {
            blocks.push(Block { x, y, z, kind, placed });
        }
    }
    Some((name, seed, style, game_mode, blocks))
}

fn panel(x: f32, y: f32, w: f32, h: f32) {
    draw_rectangle(x, y, w, h, Color::new(0.018, 0.026, 0.065, 0.94));
    draw_rectangle_lines(x, y, w, h, 1.5, Color::new(0.34, 0.48, 0.95, 0.82));
}

fn label(text: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text(text, x, y, size, color);
}

fn button(text: &str, x: f32, y: f32, w: f32, h: f32) -> bool {
    let (mx, my) = mouse_position();
    let hovered = mx >= x && mx <= x + w && my >= y && my <= y + h;
    let fill = if hovered {
        Color::new(0.20, 0.31, 0.72, 1.0)
    } else {
        Color::new(0.085, 0.13, 0.34, 0.98)
    };
    draw_rectangle(x, y, w, h, fill);
    draw_rectangle_lines(x, y, w, h, 1.0, Color::new(0.48, 0.61, 1.0, 0.9));
    let size = 19.0;
    let measured = measure_text(text, None, size as u16, 1.0);
    draw_text(
        text,
        x + (w - measured.width) * 0.5,
        y + (h + size * 0.32) * 0.5,
        size,
        WHITE,
    );
    hovered && is_mouse_button_pressed(MouseButton::Left)
}

fn draw_input(
    title: &str,
    value: &str,
    x: f32,
    y: f32,
    width: f32,
    active: bool,
    password: bool,
) {
    label(title, x + 2.0, y - 9.0, 14.0, Color::new(0.73, 0.80, 1.0, 1.0));
    let fill = if active {
        Color::new(0.12, 0.19, 0.44, 1.0)
    } else {
        Color::new(0.045, 0.065, 0.16, 1.0)
    };
    draw_rectangle(x, y, width, 44.0, fill);
    draw_rectangle_lines(
        x,
        y,
        width,
        44.0,
        1.0,
        Color::new(0.39, 0.53, 0.95, 0.9),
    );
    let shown = if password {
        "*".repeat(value.chars().count())
    } else {
        value.to_owned()
    };
    label(
        if shown.is_empty() { "Type here..." } else { &shown },
        x + 13.0,
        y + 28.0,
        18.0,
        Color::new(0.94, 0.96, 1.0, 1.0),
    );
}

fn galaxy_background() {
    clear_background(Color::new(0.008, 0.012, 0.045, 1.0));
    let width = screen_width();
    let height = screen_height();

    for i in 0..18 {
        let t = i as f32 / 18.0;
        draw_rectangle(
            0.0,
            height * t,
            width,
            height / 18.0 + 1.0,
            Color::new(0.015 + t * 0.025, 0.018 + t * 0.012, 0.075 + t * 0.10, 0.35),
        );
    }

    let time = get_time() as f32;
    for i in 0..160 {
        let seed = i as f32;
        let x = (seed * 127.1).sin().abs() * width;
        let y = (seed * 311.7).cos().abs() * height;
        let pulse = ((time * 1.4 + seed).sin() * 0.5 + 0.5) * 0.8 + 0.2;
        let radius = if i % 13 == 0 { 2.0 } else { 1.0 };
        draw_circle(x, y, radius, Color::new(0.65, 0.75, 1.0, pulse));
    }
}

pub(crate) fn hash01(seed: u64, x: i32, z: i32) -> f32 {
    let mut value = seed
        ^ (x as i64 as u64).wrapping_mul(0x9e3779b185ebca87)
        ^ (z as i64 as u64)
            .wrapping_mul(0xc2b2ae3d27d4eb4f)
            .rotate_left(29);
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58476d1ce4e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d049bb133111eb);
    value ^= value >> 31;
    ((value >> 40) as f32) / 16_777_215.0
}

fn smooth(value: f32) -> f32 {
    value * value * (3.0 - 2.0 * value)
}

pub(crate) fn value_noise(seed: u64, x: f32, z: f32) -> f32 {
    let x0 = x.floor() as i32;
    let z0 = z.floor() as i32;
    let tx = smooth(x - x.floor());
    let tz = smooth(z - z.floor());

    let a = hash01(seed, x0, z0);
    let b = hash01(seed, x0 + 1, z0);
    let c = hash01(seed, x0, z0 + 1);
    let d = hash01(seed, x0 + 1, z0 + 1);

    let top = a + (b - a) * tx;
    let bottom = c + (d - c) * tx;
    top + (bottom - top) * tz
}

pub(crate) fn terrain_height(seed: u64, style: u8, x: i32, z: i32) -> i32 {
    let broad = value_noise(seed, x as f32 * 0.055, z as f32 * 0.055);
    let detail = value_noise(
        seed ^ 0x5a17d3c49b82e601,
        x as f32 * 0.17,
        z as f32 * 0.17,
    );
    let shape = match style {
        1 => 2.0 + broad * 14.0 + detail * 5.0,
        2 => {
            let island = ((broad - 0.25) / 0.75).clamp(0.0, 1.0);
            1.0 + island * 12.0 + detail * 2.5
        }
        _ => 3.0 + broad * 8.0 + detail * 3.0,
    };
    shape.round() as i32
}

pub(crate) fn terrain_height_smooth(seed: u64, style: u8, x: i32, z: i32) -> f32 {
    let broad = value_noise(seed, x as f32 * 0.055, z as f32 * 0.055);
    let detail = value_noise(
        seed ^ 0x5a17d3c49b82e601,
        x as f32 * 0.17,
        z as f32 * 0.17,
    );
    match style {
        1 => 2.0 + broad * 14.0 + detail * 5.0,
        2 => {
            let island = ((broad - 0.25) / 0.75).clamp(0.0, 1.0);
            1.0 + island * 12.0 + detail * 2.5
        }
        _ => 3.0 + broad * 8.0 + detail * 3.0,
    }
}

fn push_block(
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

fn generate_world(
    seed: u64,
    style: u8,
) -> (Vec<Block>, HashSet<(i32, i32, i32)>) {
    let mut blocks = Vec::with_capacity(15_000);
    let mut occupied = HashSet::with_capacity(15_000);

    for x in -WORLD_RADIUS..=WORLD_RADIUS {
        for z in -WORLD_RADIUS..=WORLD_RADIUS {
            let height = terrain_height(seed, style, x, z);
            let temperature = value_noise(
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
                push_block(&mut blocks, &mut occupied, x, y, z, kind);
            }

            if height < SEA_LEVEL {
                for y in height + 1..=SEA_LEVEL {
                    push_block(&mut blocks, &mut occupied, x, y, z, 6);
                }
            }

            let tree_roll = hash01(seed ^ 0x6f2a91c5d3e84017, x, z);
            if surface_kind == 0 && height > SEA_LEVEL + 1 && tree_roll > 0.988 {
                for y in height + 1..=height + 4 {
                    push_block(&mut blocks, &mut occupied, x, y, z, 4);
                }
                for dy in 2..=4 {
                    let radius: i32 = if dy == 4 { 1 } else { 2 };
                    for dx in -radius..=radius {
                        for dz in -radius..=radius {
                            if dx.abs() + dz.abs() <= radius + 1 {
                                push_block(
                                    &mut blocks,
                                    &mut occupied,
                                    x + dx,
                                    height + dy,
                                    z + dz,
                                    5,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    (blocks, occupied)
}

fn material_texture(kind: u8) -> Texture2D {
    let mut image = Image::gen_image_color(128, 128, WHITE);
    let base = match kind {
        0 => (83.0, 133.0, 56.0),
        1 => (112.0, 76.0, 49.0),
        2 => (118.0, 124.0, 132.0),
        3 => (194.0, 174.0, 119.0),
        4 => (119.0, 77.0, 43.0),
        5 => (52.0, 111.0, 48.0),
        6 => (45.0, 116.0, 174.0),
        _ => (215.0, 224.0, 231.0),
    };

    for y in 0..128_u32 {
        for x in 0..128_u32 {
            let grain = hash01(
                0x7b3d91e5a6c2048f ^ kind as u64,
                x as i32,
                y as i32,
            );
            let fine = hash01(
                0x42c8d179b5e603af ^ (kind as u64).rotate_left(11),
                x as i32 * 3,
                y as i32 * 3,
            );
            let broad_grain = value_noise(
                0x8db24c719e35a601 ^ kind as u64,
                x as f32 * 0.045,
                y as f32 * 0.045,
            );
            let mut shade =
                0.77 + grain * 0.24 + fine * 0.10 + (broad_grain - 0.5) * 0.24;

            match kind {
                0 => {
                    if grain > 0.90 {
                        shade *= 1.24;
                    } else if grain < 0.10 {
                        shade *= 0.72;
                    }
                }
                1 => {
                    let soil_patch = value_noise(
                        0x43a91c7e25b608df,
                        x as f32 * 0.11,
                        y as f32 * 0.11,
                    );
                    shade *= 0.86 + soil_patch * 0.25;
                    if grain > 0.94 {
                        shade *= 0.66;
                    }
                }
                2 => {
                    let vein = value_noise(
                        0x1928374655aaee11,
                        x as f32 * 0.075,
                        y as f32 * 0.075,
                    );
                    if (vein - 0.5).abs() < 0.025 {
                        shade *= 0.62;
                    }
                    if grain > 0.965 {
                        shade *= 1.30;
                    }
                }
                3 => {
                    let ripple =
                        ((y as f32 * 0.21 + x as f32 * 0.035).sin() + 1.0) * 0.045;
                    shade += ripple;
                    if grain > 0.94 {
                        shade *= 1.12;
                    }
                }
                4 => {
                    let bark_wave =
                        ((x as f32 * 0.22 + (y as f32 * 0.017).sin() * 2.0).sin() + 1.0)
                            * 0.08;
                    shade += bark_wave;
                    if x % 19 <= 1 {
                        shade *= 0.60;
                    }
                }
                5 => {
                    if grain > 0.80 {
                        shade *= 0.65;
                    } else if grain < 0.16 {
                        shade *= 1.22;
                    }
                }
                6 => {
                    let ripple =
                        ((y as f32 * 0.18 + x as f32 * 0.045).sin() + 1.0) * 0.09;
                    shade += ripple;
                }
                _ => {
                    if grain > 0.93 {
                        shade *= 0.78;
                    } else if grain < 0.08 {
                        shade *= 1.10;
                    }
                }
            }

            let red = (base.0 * shade).clamp(0.0, 255.0) as u8;
            let green = (base.1 * shade).clamp(0.0, 255.0) as u8;
            let blue = (base.2 * shade).clamp(0.0, 255.0) as u8;
            image.set_pixel(x, y, Color::from_rgba(red, green, blue, 255));
        }
    }

    let texture = Texture2D::from_image(&image);
    texture.set_filter(FilterMode::Linear);
    texture
}

fn material_name(kind: u8) -> &'static str {
    match kind {
        0 => "Grass",
        1 => "Dirt",
        2 => "Stone",
        3 => "Sand",
        4 => "Wood",
        5 => "Leaves",
        6 => "Water",
        _ => "Snow",
    }
}

fn terrain_style_name(style: u8) -> &'static str {
    match style {
        1 => "Highlands",
        2 => "Islands",
        _ => "Natural",
    }
}

fn spawn_position(seed: u64, style: u8) -> Vec3 {
    let ground = terrain_height(seed, style, 0, 0) as f32;
    vec3(0.0, ground + 7.0, 15.0)
}

fn block_cell(point: Vec3) -> (i32, i32, i32) {
    (
        (point.x + 0.5).floor() as i32,
        (point.y + 0.5).floor() as i32,
        (point.z + 0.5).floor() as i32,
    )
}

/// Marches a short ray through the voxel grid and returns the first occupied
/// cell plus the last empty cell before it (the normal Minecraft place target).
fn raycast_block(
    origin: Vec3,
    direction: Vec3,
    occupied: &HashSet<(i32, i32, i32)>,
    max_distance: f32,
) -> Option<((i32, i32, i32), (i32, i32, i32))> {
    let direction = direction.normalize_or_zero();
    if direction.length_squared() < 0.5 {
        return None;
    }

    let mut previous = block_cell(origin);
    let steps = (max_distance / 0.05).ceil() as usize;
    for step in 1..=steps {
        let point = origin + direction * (step as f32 * 0.05);
        let cell = block_cell(point);
        if cell != previous {
            if occupied.contains(&cell) {
                return Some((cell, previous));
            }
            previous = cell;
        }
    }
    None
}

/// Makes short original PCM effects so sounds are bundled in the executable
/// and do not depend on the app's current working directory.
fn make_effect_wav(frequency: f32, duration: f32, grit: f32, sweep: f32) -> Vec<u8> {
    const SAMPLE_RATE: u32 = 22_050;
    let sample_count = (duration * SAMPLE_RATE as f32).max(1.0) as usize;
    let data_size = (sample_count * 2) as u32;
    let mut wav = Vec::with_capacity(44 + data_size as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36_u32 + data_size).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());

    let mut noise_state = 0x51f2_a39du32;
    for index in 0..sample_count {
        let t = index as f32 / SAMPLE_RATE as f32;
        let progress = index as f32 / sample_count as f32;
        let envelope = (1.0 - progress).max(0.0).powf(2.1);
        let phase = std::f32::consts::TAU * frequency * t * (1.0 + sweep * progress);
        noise_state = noise_state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = ((noise_state >> 16) as i16) as f32 / i16::MAX as f32;
        let tone = phase.sin() * (1.0 - grit) + noise * grit;
        let sample = (tone * envelope * 0.24 * i16::MAX as f32)
            .clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    wav
}

type WorldBuildResult = (
    Vec<Block>,
    HashSet<(i32, i32, i32)>,
    Vec<fast_render::ChunkMeshData>,
);

fn spawn_mesh_build(
    blocks: Vec<Block>,
    occupied: HashSet<(i32, i32, i32)>,
    seed: u64,
    style: u8,
) -> Receiver<WorldBuildResult> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let meshes = fast_render::build_chunk_data_smooth(&blocks, &occupied, seed, style);
        let _ = sender.send((blocks, occupied, meshes));
    });
    receiver
}

fn region_coordinates(center_x: i32, center_z: i32, radius: i32) -> Vec<(i32, i32)> {
    let mut coordinates = Vec::new();
    for chunk_x in center_x - radius..=center_x + radius {
        for chunk_z in center_z - radius..=center_z + radius {
            coordinates.push((chunk_x, chunk_z));
        }
    }
    coordinates
}

fn spawn_chunk_batch_build(
    seed: u64,
    style: u8,
    coordinates: Vec<(i32, i32)>,
) -> Receiver<WorldBuildResult> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let result = world_stream::build_region(seed, style, coordinates);
        let _ = sender.send(result);
    });
    receiver
}

fn spawn_generated_world_build(seed: u64, style: u8) -> Receiver<WorldBuildResult> {
    // Start with a 7x7 area around spawn. The rest is streamed as the player moves.
    spawn_chunk_batch_build(seed, style, region_coordinates(0, 0, 3))
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut account = load_account();
    let remembered = account.as_ref().map(session_is_remembered).unwrap_or(false);
    let mut screen = if remembered {
        Screen::Menu
    } else if account.is_some() {
        Screen::Login
    } else {
        Screen::CreateAccount
    };
    let mut player_name = if remembered {
        account.as_ref().map(|saved| saved.username.clone()).unwrap_or_default()
    } else {
        String::new()
    };
    if !remembered {
        player_name = account.as_ref().map(|saved| saved.username.clone()).unwrap_or_default();
    }
    let mut username_input = player_name.clone();
    let mut password_input = String::new();
    let mut confirm_input = String::new();
    let mut auth_field = 0_u8;
    let mut auth_status = String::new();

    let mut world_name = "New World".to_owned();
    let mut world_seed = random_seed();
    let mut seed_input = world_seed.to_string();
    let mut world_style = 0_u8;
    let mut game_mode = systems::GameMode::Survival;
    let mut world_name_field = 0_u8;
    let mut world_status = String::new();
    let mut blocks = Vec::new();
    let mut occupied = HashSet::new();
    let mut fast_chunks: Vec<fast_render::FastChunk> = Vec::new();
    let mut requested_chunks: HashSet<(i32, i32)> = HashSet::new();
    let mut pending_chunks: HashSet<(i32, i32)> = HashSet::new();
    let mut stream_receiver: Option<Receiver<WorldBuildResult>> = None;
    let mut loading_receiver: Option<Receiver<WorldBuildResult>> = None;
    let mut loading_status = "Preparing world...".to_owned();
    let mut has_world = false;

    if let Some((saved_name, saved_seed, saved_style, saved_mode, saved_blocks)) = load_world() {
        world_name = saved_name;
        world_seed = saved_seed;
        seed_input = world_seed.to_string();
        world_style = saved_style;
        game_mode = saved_mode;
        for block in saved_blocks {
            occupied.insert((block.x, block.y, block.z));
            requested_chunks.insert((
                block.x.div_euclid(fast_render::CHUNK_SIZE),
                block.z.div_euclid(fast_render::CHUNK_SIZE),
            ));
            blocks.push(block);
        }
        has_world = true;
    }

    let textures: [Texture2D; 8] =
        std::array::from_fn(|index| material_texture(index as u8));

    // Audio is optional at runtime: unsupported devices simply leave effects off.
    let place_sound = load_sound_from_bytes(&make_effect_wav(620.0, 0.075, 0.10, -0.18)).await.ok();
    let break_sound = load_sound_from_bytes(&make_effect_wav(145.0, 0.13, 0.34, -0.62)).await.ok();
    let ui_sound = load_sound_from_bytes(&make_effect_wav(880.0, 0.055, 0.02, 0.08)).await.ok();

    let mut camera_position = spawn_position(world_seed, world_style);
    let mut yaw = 0.0_f32;
    let mut pitch = -0.34_f32;
    let mut selected_block = 0_u8;
    let mut world_message = "Welcome to your world.".to_owned();
    let mut health = 20_u8;
    let mut chat_open = false;
    let mut chat_input = String::new();
    let mut debug_overlay = false;

    loop {
        let dt = get_frame_time().min(0.05);
        if screen == Screen::World {
            clear_background(Color::new(0.40, 0.65, 0.88, 1.0));
        } else {
            galaxy_background();
        }

        match screen {
            Screen::CreateAccount => {
                let width = screen_width();
                let height = screen_height();
                let box_w = 590.0_f32.min(width - 28.0);
                let box_x = (width - box_w) * 0.5;
                let box_y = (height - 548.0) * 0.5;

                panel(box_x, box_y, box_w, 548.0);
                label("CREATE YOUR ACCOUNT", box_x + 36.0, box_y + 59.0, 34.0, WHITE);
                label(
                    "Your profile is saved locally on this computer.",
                    box_x + 39.0,
                    box_y + 89.0,
                    15.0,
                    Color::new(0.67, 0.77, 1.0, 1.0),
                );

                let field_x = box_x + 38.0;
                let field_w = box_w - 76.0;
                draw_input(
                    "USERNAME",
                    &username_input,
                    field_x,
                    box_y + 132.0,
                    field_w,
                    auth_field == 0,
                    false,
                );
                draw_input(
                    "PASSWORD",
                    &password_input,
                    field_x,
                    box_y + 210.0,
                    field_w,
                    auth_field == 1,
                    true,
                );
                draw_input(
                    "CONFIRM PASSWORD",
                    &confirm_input,
                    field_x,
                    box_y + 288.0,
                    field_w,
                    auth_field == 2,
                    true,
                );

                let (mx, my) = mouse_position();
                if is_mouse_button_pressed(MouseButton::Left) {
                    if mx >= field_x && mx <= field_x + field_w {
                        if my >= box_y + 132.0 && my <= box_y + 176.0 {
                            auth_field = 0;
                        } else if my >= box_y + 210.0 && my <= box_y + 254.0 {
                            auth_field = 1;
                        } else if my >= box_y + 288.0 && my <= box_y + 332.0 {
                            auth_field = 2;
                        }
                    }
                }

                while let Some(ch) = get_char_pressed() {
                    if !ch.is_control() && ch.is_ascii() {
                        let target = match auth_field {
                            0 => &mut username_input,
                            1 => &mut password_input,
                            _ => &mut confirm_input,
                        };
                        let limit = if auth_field == 0 { 18 } else { 64 };
                        if target.len() < limit {
                            if auth_field != 0 || ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                                target.push(ch);
                            }
                        }
                    }
                }
                if is_key_pressed(KeyCode::Backspace) {
                    match auth_field {
                        0 => { username_input.pop(); }
                        1 => { password_input.pop(); }
                        _ => { confirm_input.pop(); }
                    }
                }

                if button("CREATE ACCOUNT", box_x + 38.0, box_y + 365.0, field_w, 49.0) {
                    if password_input != confirm_input {
                        auth_status = "The passwords do not match.".to_owned();
                    } else {
                        match create_local_account(&username_input, &password_input) {
                            Ok(saved) => {
                                player_name = saved.username.clone();
                                username_input = player_name.clone();
                                remember_session(&saved.username);
                                account = Some(saved);
                                password_input.clear();
                                confirm_input.clear();
                                auth_status.clear();
                                screen = Screen::Menu;
                            }
                            Err(message) => auth_status = message,
                        }
                    }
                }

                label(
                    "No online account is required. This profile works offline.",
                    box_x + 40.0,
                    box_y + 455.0,
                    13.0,
                    Color::new(0.62, 0.71, 0.93, 1.0),
                );
                label(
                    &auth_status,
                    box_x + 40.0,
                    box_y + 510.0,
                    14.0,
                    Color::new(1.0, 0.61, 0.57, 1.0),
                );
            }
            Screen::Login => {
                let width = screen_width();
                let height = screen_height();
                let box_w = 560.0_f32.min(width - 28.0);
                let box_x = (width - box_w) * 0.5;
                let box_y = (height - 450.0) * 0.5;

                panel(box_x, box_y, box_w, 450.0);
                label("WELCOME BACK", box_x + 36.0, box_y + 65.0, 38.0, WHITE);
                label(
                    "Sign in to your local RealismCraft profile.",
                    box_x + 39.0,
                    box_y + 95.0,
                    15.0,
                    Color::new(0.67, 0.77, 1.0, 1.0),
                );

                let field_x = box_x + 38.0;
                let field_w = box_w - 76.0;
                draw_input(
                    "USERNAME",
                    &username_input,
                    field_x,
                    box_y + 153.0,
                    field_w,
                    auth_field == 0,
                    false,
                );
                draw_input(
                    "PASSWORD",
                    &password_input,
                    field_x,
                    box_y + 230.0,
                    field_w,
                    auth_field == 1,
                    true,
                );

                let (mx, my) = mouse_position();
                if is_mouse_button_pressed(MouseButton::Left) {
                    if mx >= field_x && mx <= field_x + field_w {
                        if my >= box_y + 153.0 && my <= box_y + 197.0 {
                            auth_field = 0;
                        } else if my >= box_y + 230.0 && my <= box_y + 274.0 {
                            auth_field = 1;
                        }
                    }
                }

                while let Some(ch) = get_char_pressed() {
                    if !ch.is_control() && ch.is_ascii() {
                        let target = if auth_field == 0 {
                            &mut username_input
                        } else {
                            &mut password_input
                        };
                        if target.len() < 64 {
                            target.push(ch);
                        }
                    }
                }
                if is_key_pressed(KeyCode::Backspace) {
                    if auth_field == 0 {
                        username_input.pop();
                    } else {
                        password_input.pop();
                    }
                }

                if button("SIGN IN", box_x + 38.0, box_y + 305.0, field_w, 50.0) {
                    let valid = account
                        .as_ref()
                        .map(|saved| verify_login(saved, &username_input, &password_input))
                        .unwrap_or(false);
                    if valid {
                        player_name = account.as_ref().unwrap().username.clone();
                        remember_session(&player_name);
                        password_input.clear();
                        auth_status.clear();
                        screen = Screen::Menu;
                    } else {
                        auth_status = "Incorrect username or password.".to_owned();
                    }
                }

                label(
                    "Your password is stored as a salted hash on this device.",
                    box_x + 40.0,
                    box_y + 388.0,
                    13.0,
                    Color::new(0.62, 0.71, 0.93, 1.0),
                );
                label(
                    &auth_status,
                    box_x + 40.0,
                    box_y + 420.0,
                    14.0,
                    Color::new(1.0, 0.61, 0.57, 1.0),
                );
            }
            Screen::Menu => {
                let width = screen_width();
                let height = screen_height();
                let box_w = 940.0_f32.min(width - 36.0);
                let left = (width - box_w) * 0.5;
                let main_w = (box_w * 0.64).min(600.0);
                let gap = 16.0;
                let side_x = left + main_w + gap;
                let side_w = box_w - main_w - gap;
                let top = height * 0.30;
                let panel_h = (height * 0.55).min(440.0);

                label("REALISMCRAFT", left + 2.0, height * 0.16, 49.0, WHITE);
                label(
                    "A WORLD THAT GROWS WITH YOU",
                    left + 5.0,
                    height * 0.16 + 31.0,
                    15.0,
                    Color::new(0.58, 0.75, 1.0, 1.0),
                );
                draw_rectangle(left, height * 0.16 + 43.0, 185.0, 2.0, Color::new(0.31, 0.48, 0.96, 0.9));

                panel(left, top, main_w, panel_h);
                label("PLAYER PROFILE", left + 30.0, top + 38.0, 13.0, Color::new(0.55, 0.71, 1.0, 1.0));
                label(
                    &format!("Welcome back, {}", player_name),
                    left + 30.0,
                    top + 78.0,
                    25.0,
                    WHITE,
                );
                label(
                    "LOCAL ACCOUNT  /  SESSION REMEMBERED",
                    left + 32.0,
                    top + 105.0,
                    12.0,
                    Color::new(0.62, 0.80, 0.72, 1.0),
                );

                if has_world {
                    draw_rectangle(left + 28.0, top + 128.0, main_w - 56.0, 82.0, Color::new(0.045, 0.075, 0.17, 0.96));
                    draw_rectangle_lines(left + 28.0, top + 128.0, main_w - 56.0, 82.0, 1.0, Color::new(0.22, 0.36, 0.72, 0.8));
                    label("CONTINUE YOUR WORLD", left + 44.0, top + 153.0, 12.0, Color::new(0.60, 0.76, 1.0, 1.0));
                    label(&world_name, left + 44.0, top + 185.0, 23.0, WHITE);
                    if button("PLAY WORLD", left + 28.0, top + 226.0, main_w - 56.0, 51.0) {
                        loading_status = "Building visible chunk meshes...".to_owned();
                        loading_receiver = Some(spawn_mesh_build(
                            blocks.clone(),
                            occupied.clone(),
                            world_seed,
                            world_style,
                        ));
                        screen = Screen::Loading;
                    }
                } else {
                    draw_rectangle(left + 28.0, top + 128.0, main_w - 56.0, 82.0, Color::new(0.045, 0.075, 0.17, 0.96));
                    label("YOUR WORLD IS WAITING", left + 44.0, top + 157.0, 18.0, WHITE);
                    label("Create a seed and explore a new landscape.", left + 44.0, top + 185.0, 13.0, Color::new(0.69, 0.78, 0.96, 1.0));
                }

                if button("CREATE NEW WORLD", left + 28.0, top + 296.0, main_w - 56.0, 51.0) {
                    world_name = "New World".to_owned();
                    game_mode = systems::GameMode::Survival;
                    world_seed = random_seed();
                    seed_input = world_seed.to_string();
                    world_name_field = 0;
                    world_status.clear();
                    screen = Screen::WorldSetup;
                }
                if button("SIGN OUT", left + 28.0, top + panel_h - 66.0, main_w - 56.0, 38.0) {
                    forget_session();
                    password_input.clear();
                    auth_status.clear();
                    username_input = player_name.clone();
                    auth_field = 1;
                    screen = Screen::Login;
                }

                panel(side_x, top, side_w, panel_h);
                label("WORLD ENGINE", side_x + 24.0, top + 40.0, 18.0, WHITE);
                label("RUST  /  NATIVE BUILD", side_x + 24.0, top + 63.0, 11.0, Color::new(0.55, 0.73, 1.0, 1.0));
                let features = [
                    ("01", "LIVE CHUNK GENERATION"),
                    ("02", "SMOOTH TERRAIN MESH"),
                    ("03", "PARALLEL WORLD BUILD"),
                    ("04", "LOCAL WORLD SAVES"),
                ];
                for (index, (number, feature)) in features.iter().enumerate() {
                    let y = top + 112.0 + index as f32 * 48.0;
                    draw_rectangle(side_x + 22.0, y - 17.0, side_w - 44.0, 36.0, Color::new(0.035, 0.060, 0.14, 0.88));
                    label(number, side_x + 32.0, y + 6.0, 11.0, Color::new(0.44, 0.65, 1.0, 1.0));
                    label(feature, side_x + 61.0, y + 6.0, 10.0, Color::new(0.83, 0.88, 1.0, 1.0));
                }
                draw_rectangle(side_x + 22.0, top + panel_h - 93.0, side_w - 44.0, 1.0, Color::new(0.20, 0.32, 0.62, 0.8));
                label(&format!("TERRAIN  {}", terrain_style_name(world_style).to_uppercase()), side_x + 24.0, top + panel_h - 65.0, 12.0, WHITE);
                label(&format!("SEED  {}", world_seed), side_x + 24.0, top + panel_h - 43.0, 10.0, Color::new(0.65, 0.75, 0.96, 1.0));

                label(
                    "REALISMCRAFT  /  ORIGINAL PROCEDURAL SANDBOX",
                    left + 3.0,
                    height * 0.91,
                    12.0,
                    Color::new(0.49, 0.60, 0.86, 1.0),
                );
            }
            Screen::WorldSetup => {
                if is_key_pressed(KeyCode::Escape) {
                    screen = Screen::Menu;
                }

                let width = screen_width();
                let height = screen_height();
                let box_w = 620.0_f32.min(width - 28.0);
                let left = (width - box_w) * 0.5;
                let setup_height = 540.0;
                let top = (height - setup_height) * 0.5;
                panel(left, top, box_w, setup_height);
                label("CREATE A WORLD", left + 36.0, top + 55.0, 36.0, WHITE);
                label(
                    "A new landscape is generated from your seed.",
                    left + 39.0,
                    top + 83.0,
                    15.0,
                    Color::new(0.67, 0.77, 1.0, 1.0),
                );

                let field_x = left + 36.0;
                let field_w = box_w - 72.0;
                draw_input(
                    "WORLD NAME",
                    &world_name,
                    field_x,
                    top + 143.0,
                    field_w,
                    world_name_field == 0,
                    false,
                );
                draw_input(
                    "WORLD SEED",
                    &seed_input,
                    field_x,
                    top + 230.0,
                    field_w,
                    world_name_field == 1,
                    false,
                );

                let (mx, my) = mouse_position();
                if is_mouse_button_pressed(MouseButton::Left) {
                    if mx >= field_x && mx <= field_x + field_w {
                        if my >= top + 143.0 && my <= top + 187.0 {
                            world_name_field = 0;
                        } else if my >= top + 230.0 && my <= top + 274.0 {
                            world_name_field = 1;
                        }
                    }
                }

                while let Some(ch) = get_char_pressed() {
                    if !ch.is_control() && ch.is_ascii() {
                        if world_name_field == 0 {
                            if world_name.len() < 28 {
                                world_name.push(ch);
                            }
                        } else if ch.is_ascii_digit() && seed_input.len() < 20 {
                            seed_input.push(ch);
                        }
                    }
                }
                if is_key_pressed(KeyCode::Backspace) {
                    if world_name_field == 0 {
                        world_name.pop();
                    } else {
                        seed_input.pop();
                    }
                }

                if button("RANDOMIZE SEED", left + 36.0, top + 300.0, 265.0, 42.0) {
                    seed_input = random_seed().to_string();
                }
                let style_label = format!("TERRAIN: {}", terrain_style_name(world_style));
                if button(&style_label, left + 319.0, top + 300.0, 265.0, 42.0) {
                    world_style = (world_style + 1) % 3;
                }
                let mode_button = format!("GAME MODE: {}  /  CLICK TO CHANGE", game_mode.label().to_uppercase());
                if button(&mode_button, left + 36.0, top + 355.0, field_w, 42.0) {
                    game_mode = match game_mode {
                        systems::GameMode::Survival => systems::GameMode::Creative,
                        systems::GameMode::Creative => systems::GameMode::Adventure,
                        systems::GameMode::Adventure => systems::GameMode::Spectator,
                        systems::GameMode::Spectator => systems::GameMode::Survival,
                    };
                    if let Some(sound) = &ui_sound {
                        play_sound_once(sound);
                    }
                }

                if button("GENERATE WORLD  /  PLAY", left + 36.0, top + 410.0, field_w, 49.0) {
                    let parsed_seed = seed_input.trim().parse::<u64>();
                    world_seed = parsed_seed.unwrap_or_else(|_| random_seed());
                    seed_input = world_seed.to_string();
                    if world_name.trim().is_empty() {
                        world_name = "New World".to_owned();
                    }

                    loading_status = "Generating the spawn region and building chunk meshes...".to_owned();
                    requested_chunks.clear();
                    pending_chunks.clear();
                    stream_receiver = None;
                    let initial_chunks = region_coordinates(0, 0, 3);
                    pending_chunks.extend(initial_chunks.iter().copied());
                    loading_receiver = Some(spawn_chunk_batch_build(world_seed, world_style, initial_chunks));
                    screen = Screen::Loading;
                }
                if button("BACK TO MENU", left + 36.0, top + 468.0, field_w, 34.0) {
                    screen = Screen::Menu;
                }
                label(
                    &world_status,
                    left + 38.0,
                    top + 526.0,
                    12.0,
                    Color::new(1.0, 0.62, 0.56, 1.0),
                );
            }
            Screen::Loading => {
                let completed = match loading_receiver.as_ref() {
                    Some(receiver) => match receiver.try_recv() {
                        Ok(payload) => Some(Ok(payload)),
                        Err(TryRecvError::Empty) => None,
                        Err(TryRecvError::Disconnected) => Some(Err(())),
                    },
                    None => Some(Err(())),
                };

                match completed {
                    Some(Ok((loaded_blocks, loaded_occupied, mesh_data))) => {
                        blocks = loaded_blocks;
                        occupied = loaded_occupied;
                        requested_chunks.extend(mesh_data.iter().map(|mesh| (mesh.chunk_x, mesh.chunk_z)));
                        pending_chunks.clear();
                        fast_chunks = fast_render::upload_chunk_data(mesh_data, &textures);
                        has_world = true;
                        camera_position = spawn_position(world_seed, world_style);
                        yaw = 0.0;
                        pitch = -0.34;
                        selected_block = 0;
                        health = 20;
                        world_message = format!(
                            "{} world ready. Seed: {}",
                            terrain_style_name(world_style),
                            world_seed
                        );
                        if let Err(message) = save_world(&world_name, world_seed, world_style, game_mode, &blocks) {
                            world_message = message;
                        }
                        loading_receiver = None;
                        chat_open = false;
                        set_cursor_grab(true);
                        show_mouse(false);
                        screen = Screen::World;
                    }
                    Some(Err(())) => {
                        loading_status = "World loading failed. Returning to menu.".to_owned();
                        loading_receiver = None;
                        screen = Screen::Menu;
                    }
                    None => {}
                }

                let width = screen_width();
                let height = screen_height();
                let pulse = (get_time() as f32 * 2.0).sin() * 0.5 + 0.5;
                let panel_w = width.min(720.0) - 48.0;
                let panel_x = (width - panel_w) * 0.5;
                let panel_y = height * 0.40;
                draw_rectangle(panel_x, panel_y, panel_w, 170.0, Color::new(0.012, 0.020, 0.060, 0.96));
                draw_rectangle_lines(panel_x, panel_y, panel_w, 170.0, 1.5, Color::new(0.31, 0.48, 0.90, 0.9));
                label("BUILDING YOUR WORLD", panel_x + 28.0, panel_y + 48.0, 30.0, WHITE);
                label(&loading_status, panel_x + 30.0, panel_y + 82.0, 15.0, Color::new(0.72, 0.81, 1.0, 1.0));
                let bar_x = panel_x + 30.0;
                let bar_y = panel_y + 108.0;
                let bar_w = panel_w - 60.0;
                draw_rectangle(bar_x, bar_y, bar_w, 10.0, Color::new(0.10, 0.13, 0.23, 1.0));
                let segment_w = (bar_w * 0.22).max(30.0);
                let segment_x = bar_x + (bar_w - segment_w) * pulse;
                draw_rectangle(segment_x, bar_y, segment_w, 10.0, Color::new(0.36, 0.57, 1.0, 1.0));
                label("Terrain and mesh work runs away from the main frame loop.", panel_x + 30.0, panel_y + 146.0, 13.0, Color::new(0.58, 0.68, 0.88, 1.0));
            }
            Screen::World => {
                // Receive streamed chunks without stopping the render loop.
                let stream_result = stream_receiver.as_ref().and_then(|receiver| {
                    match receiver.try_recv() {
                        Ok(result) => Some(Ok(result)),
                        Err(TryRecvError::Empty) => None,
                        Err(TryRecvError::Disconnected) => Some(Err(())),
                    }
                });
                match stream_result {
                    Some(Ok((new_blocks, new_occupied, new_mesh_data))) => {
                        blocks.extend(new_blocks);
                        occupied.extend(new_occupied);
                        requested_chunks.extend(new_mesh_data.iter().map(|mesh| (mesh.chunk_x, mesh.chunk_z)));
                        fast_chunks.extend(fast_render::upload_chunk_data(new_mesh_data, &textures));
                        pending_chunks.clear();
                        stream_receiver = None;
                    }
                    Some(Err(())) => {
                        pending_chunks.clear();
                        stream_receiver = None;
                        world_message = "A terrain chunk failed to load; it will be retried.".to_owned();
                    }
                    None => {}
                }

                if is_key_pressed(KeyCode::F3) {
                    debug_overlay = !debug_overlay;
                }

                let mut chat_closed_this_frame = false;
                if !chat_open && (is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::Enter)) {
                    chat_open = true;
                    chat_input.clear();
                    set_cursor_grab(false);
                    show_mouse(true);
                } else if chat_open && is_key_pressed(KeyCode::Enter) {
                    let submitted = chat_input.trim().to_owned();
                    if !submitted.is_empty() {
                        if submitted.eq_ignore_ascii_case("/help") {
                            world_message = "Commands: /gamemode <mode>, /seed, /spawn, /help".to_owned();
                        } else if submitted.eq_ignore_ascii_case("/seed") {
                            world_message = format!("World seed: {}", world_seed);
                        } else if submitted.eq_ignore_ascii_case("/spawn") {
                            camera_position = spawn_position(world_seed, world_style);
                            yaw = 0.0;
                            pitch = -0.34;
                            world_message = "Returned to world spawn.".to_owned();
                        } else {
                            match systems::parse_chat_command(&submitted) {
                                Ok(Some(mode)) => {
                                    game_mode = mode;
                                    world_message = format!("Game mode changed to {}.", game_mode.label());
                                    if let Err(message) =
                                        save_world(&world_name, world_seed, world_style, game_mode, &blocks)
                                    {
                                        world_message = message;
                                    }
                                }
                                Ok(None) if submitted.starts_with('/') => {
                                    world_message = "Unknown command. Type /help.".to_owned();
                                }
                                Ok(None) => {
                                    world_message = format!("{}: {}", player_name, submitted);
                                }
                                Err(message) => world_message = message.to_owned(),
                            }
                        }
                    }
                    chat_input.clear();
                    chat_open = false;
                    chat_closed_this_frame = true;
                    set_cursor_grab(true);
                    show_mouse(false);
                } else if chat_open && is_key_pressed(KeyCode::Escape) {
                    chat_open = false;
                    chat_input.clear();
                    chat_closed_this_frame = true;
                    set_cursor_grab(true);
                    show_mouse(false);
                }

                if chat_open {
                    while let Some(ch) = get_char_pressed() {
                        if !ch.is_control() && ch.is_ascii() && chat_input.len() < 120 {
                            chat_input.push(ch);
                        }
                    }
                    if is_key_pressed(KeyCode::Backspace) {
                        chat_input.pop();
                    }
                } else if !chat_closed_this_frame && is_key_pressed(KeyCode::Escape) {
                    if let Err(message) = save_world(&world_name, world_seed, world_style, game_mode, &blocks) {
                        world_message = message;
                    }
                    set_cursor_grab(false);
                    show_mouse(true);
                    screen = Screen::Menu;
                }

                let mouse_delta = if chat_open { Vec2::ZERO } else { mouse_delta_position() };
                if !chat_open {
                    yaw -= mouse_delta.x * 2.3;
                    pitch = (pitch + mouse_delta.y * 2.3).clamp(-1.35, 1.35);
                }

                if !chat_open && is_key_down(KeyCode::Left) {
                    yaw -= 1.8 * dt;
                }
                if !chat_open && is_key_down(KeyCode::Right) {
                    yaw += 1.8 * dt;
                }
                if !chat_open && is_key_down(KeyCode::Up) {
                    pitch = (pitch + 1.2 * dt).clamp(-1.2, 1.2);
                }
                if !chat_open && is_key_down(KeyCode::Down) {
                    pitch = (pitch - 1.2 * dt).clamp(-1.2, 1.2);
                }

                let forward = vec3(yaw.sin(), 0.0, -yaw.cos());
                let right = vec3(yaw.cos(), 0.0, yaw.sin());
                let speed = if is_key_down(KeyCode::LeftShift) { 10.0 } else { 5.0 };
                if !chat_open && is_key_down(KeyCode::W) {
                    camera_position += forward * speed * dt;
                }
                if !chat_open && is_key_down(KeyCode::S) {
                    camera_position -= forward * speed * dt;
                }
                if !chat_open && is_key_down(KeyCode::D) {
                    camera_position += right * speed * dt;
                }
                if !chat_open && is_key_down(KeyCode::A) {
                    camera_position -= right * speed * dt;
                }
                if !chat_open && is_key_down(KeyCode::Space) {
                    camera_position.y += speed * dt;
                }
                if !chat_open && is_key_down(KeyCode::LeftControl) {
                    camera_position.y -= speed * dt;
                }

                // The world has no horizontal border: request any missing chunks
                // within the active 9x9 region around the player's current chunk.
                if stream_receiver.is_none() {
                    let center_chunk_x = (camera_position.x.floor() as i32).div_euclid(fast_render::CHUNK_SIZE);
                    let center_chunk_z = (camera_position.z.floor() as i32).div_euclid(fast_render::CHUNK_SIZE);
                    let mut missing = Vec::new();
                    for chunk_x in center_chunk_x - 4..=center_chunk_x + 4 {
                        for chunk_z in center_chunk_z - 4..=center_chunk_z + 4 {
                            let coordinate = (chunk_x, chunk_z);
                            if !requested_chunks.contains(&coordinate) && !pending_chunks.contains(&coordinate) {
                                missing.push(coordinate);
                            }
                        }
                    }
                    if !missing.is_empty() {
                        pending_chunks.extend(missing.iter().copied());
                        stream_receiver = Some(spawn_chunk_batch_build(world_seed, world_style, missing));
                    }
                }

                let look = vec3(
                    yaw.sin() * pitch.cos(),
                    pitch.sin(),
                    -yaw.cos() * pitch.cos(),
                );
                let camera = Camera3D {
                    position: camera_position,
                    target: camera_position + look,
                    up: Vec3::Y,
                    fovy: 65.0,
                    aspect: None,
                    z_near: 0.05,
                    z_far: 500.0,
                    viewport: None,
                    ..Default::default()
                };
                set_camera(&camera);

                let (visible_chunks, draw_calls) =
                    fast_render::draw_visible_chunks(&fast_chunks, camera_position, look, 78.0);

                if !chat_open && is_key_pressed(KeyCode::Key1) { selected_block = 0; }
                if !chat_open && is_key_pressed(KeyCode::Key2) { selected_block = 1; }
                if !chat_open && is_key_pressed(KeyCode::Key3) { selected_block = 2; }
                if !chat_open && is_key_pressed(KeyCode::Key4) { selected_block = 3; }
                if !chat_open && is_key_pressed(KeyCode::Key5) { selected_block = 4; }
                if !chat_open && is_key_pressed(KeyCode::Key6) { selected_block = 5; }
                if !chat_open && is_key_pressed(KeyCode::Key7) { selected_block = 6; }
                if !chat_open && is_key_pressed(KeyCode::Key8) { selected_block = 7; }

                // Minecraft-style controls: left click breaks, right click places.
                if !chat_open && is_mouse_button_pressed(MouseButton::Left) {
                    if !matches!(game_mode, systems::GameMode::Survival | systems::GameMode::Creative) {
                        world_message = "Block breaking is disabled in this game mode.".to_owned();
                    } else if let Some((hit_cell, _)) =
                        raycast_block(camera_position, look, &occupied, 7.0)
                    {
                        if let Some(index) = blocks.iter().rposition(|block| {
                            (block.x, block.y, block.z) == hit_cell
                        }) {
                            let removed = blocks.remove(index);
                            occupied.remove(&(removed.x, removed.y, removed.z));
                            world_message = format!("Broke {}.", material_name(removed.kind));
                            let chunk_x = removed.x.div_euclid(fast_render::CHUNK_SIZE);
                            let chunk_z = removed.z.div_euclid(fast_render::CHUNK_SIZE);
                            fast_render::rebuild_one_chunk(
                                &mut fast_chunks,
                                &blocks,
                                &occupied,
                                world_seed,
                                world_style,
                                chunk_x,
                                chunk_z,
                                &textures,
                            );
                            if let Some(sound) = &break_sound {
                                play_sound_once(sound);
                            }
                            if let Err(message) =
                                save_world(&world_name, world_seed, world_style, game_mode, &blocks)
                            {
                                world_message = message;
                            }
                        }
                    }
                }

                if !chat_open && is_mouse_button_pressed(MouseButton::Right) {
                    if !matches!(game_mode, systems::GameMode::Survival | systems::GameMode::Creative) {
                        world_message = "Block placing is disabled in this game mode.".to_owned();
                    } else if let Some((_, place_cell)) =
                        raycast_block(camera_position, look, &occupied, 7.0)
                    {
                        if occupied.insert(place_cell) {
                            let (px, py, pz) = place_cell;
                            blocks.push(Block {
                                x: px,
                                y: py,
                                z: pz,
                                kind: selected_block,
                                placed: true,
                            });
                            world_message = format!("Placed {}.", material_name(selected_block));
                            let chunk_x = px.div_euclid(fast_render::CHUNK_SIZE);
                            let chunk_z = pz.div_euclid(fast_render::CHUNK_SIZE);
                            fast_render::rebuild_one_chunk(
                                &mut fast_chunks,
                                &blocks,
                                &occupied,
                                world_seed,
                                world_style,
                                chunk_x,
                                chunk_z,
                                &textures,
                            );
                            if let Some(sound) = &place_sound {
                                play_sound_once(sound);
                            }
                            if let Err(message) =
                                save_world(&world_name, world_seed, world_style, game_mode, &blocks)
                            {
                                world_message = message;
                            }
                        }
                    }
                }

                set_default_camera();
                let screen_w = screen_width();
                let screen_h = screen_height();

                draw_rectangle(
                    16.0,
                    16.0,
                    420.0,
                    104.0,
                    Color::new(0.018, 0.028, 0.065, 0.84),
                );
                label(
                    &format!("{}  /  {}", player_name, world_name),
                    30.0,
                    43.0,
                    18.0,
                    WHITE,
                );
                label(
                    &format!(
                        "{} TERRAIN   |   SEED {}",
                        terrain_style_name(world_style),
                        world_seed
                    ),
                    30.0,
                    68.0,
                    13.0,
                    Color::new(0.77, 0.84, 1.0, 1.0),
                );
                label(
                    &format!("{} blocks   |   {} mode   |   mouse look", blocks.len(), game_mode.label()),
                    30.0,
                    96.0,
                    13.0,
                    Color::new(0.77, 0.84, 1.0, 1.0),
                );

                let center_x = screen_w * 0.5;
                let center_y = screen_h * 0.5;
                draw_rectangle(center_x - 7.0, center_y - 1.0, 14.0, 2.0, WHITE);
                draw_rectangle(center_x - 1.0, center_y - 7.0, 2.0, 14.0, WHITE);

                let slot_width = 48.0;
                let total_width = slot_width * 8.0;
                let start_x = (screen_w - total_width) * 0.5;
                let slot_y = screen_h - 68.0;
                for index in 0..8_u8 {
                    let x = start_x + index as f32 * slot_width;
                    let fill = if index == selected_block {
                        Color::new(0.24, 0.34, 0.68, 0.96)
                    } else {
                        Color::new(0.015, 0.025, 0.06, 0.86)
                    };
                    draw_rectangle(x, slot_y, 44.0, 44.0, fill);
                    draw_rectangle_lines(
                        x,
                        slot_y,
                        44.0,
                        44.0,
                        1.0,
                        Color::new(0.58, 0.68, 0.95, 0.9),
                    );
                    label(
                        &format!("{}", index + 1),
                        x + 5.0,
                        slot_y + 14.0,
                        10.0,
                        Color::new(0.75, 0.82, 1.0, 1.0),
                    );
                    draw_rectangle(
                        x + 17.0,
                        slot_y + 9.0,
                        19.0,
                        19.0,
                        block_color_preview(index),
                    );
                    label(
                        material_name(index),
                        x + 3.0,
                        slot_y + 37.0,
                        8.0,
                        WHITE,
                    );
                }

                draw_rectangle(
                    16.0,
                    screen_h - 96.0,
                    390.0,
                    22.0,
                    Color::new(0.015, 0.025, 0.06, 0.78),
                );
                label(
                    &world_message,
                    24.0,
                    screen_h - 80.0,
                    12.0,
                    Color::new(0.88, 0.91, 1.0, 1.0),
                );
                label(
                    "LEFT CLICK break  |  RIGHT CLICK place  |  1-8 hotbar  |  - chat  |  ESC save",
                    screen_w - 520.0,
                    screen_h - 28.0,
                    12.0,
                    Color::new(0.95, 0.97, 1.0, 1.0),
                );

                // Performance HUD: always visible; F3 shows extra renderer counters.
                let fps = get_fps();
                let frame_ms = get_frame_time() * 1000.0;
                draw_rectangle(
                    screen_w - 158.0, 16.0, 142.0, 54.0,
                    Color::new(0.012, 0.020, 0.055, 0.88),
                );
                draw_rectangle_lines(
                    screen_w - 158.0, 16.0, 142.0, 54.0, 1.0,
                    Color::new(0.30, 0.45, 0.84, 0.82),
                );
                label(&format!("FPS  {}", fps), screen_w - 146.0, 39.0, 18.0, Color::new(0.62, 1.0, 0.72, 1.0));
                label(&format!("{:.1} ms/frame", frame_ms), screen_w - 146.0, 58.0, 12.0, WHITE);
                systems::draw_hearts(health, 20);
                if debug_overlay {
                    draw_rectangle(screen_w - 252.0, 80.0, 236.0, 72.0, Color::new(0.012, 0.020, 0.055, 0.88));
                    label(&format!("Visible chunks: {}", visible_chunks), screen_w - 240.0, 104.0, 13.0, WHITE);
                    label(&format!("Mesh draw calls: {}", draw_calls), screen_w - 240.0, 124.0, 13.0, WHITE);
                    label(&format!("Loaded chunks: {}", fast_chunks.len()), screen_w - 240.0, 144.0, 13.0, WHITE);
                }
                if chat_open {
                    draw_rectangle(16.0, screen_h - 142.0, (screen_w - 32.0).min(760.0), 36.0, Color::new(0.012, 0.020, 0.055, 0.94));
                    draw_rectangle_lines(16.0, screen_h - 142.0, (screen_w - 32.0).min(760.0), 36.0, 1.0, Color::new(0.38, 0.52, 0.92, 0.9));
                    label(&format!("> {}", chat_input), 28.0, screen_h - 118.0, 17.0, WHITE);
                    label("Enter: send  |  Esc: close  |  Try /gamemode creative or /help", 24.0, screen_h - 96.0, 12.0, Color::new(0.72, 0.80, 1.0, 1.0));
                }

            }
        }

        next_frame().await;
    }
}

fn block_color_preview(kind: u8) -> Color {
    match kind {
        0 => Color::new(0.34, 0.58, 0.23, 1.0),
        1 => Color::new(0.43, 0.27, 0.15, 1.0),
        2 => Color::new(0.47, 0.50, 0.54, 1.0),
        3 => Color::new(0.75, 0.66, 0.43, 1.0),
        4 => Color::new(0.48, 0.28, 0.14, 1.0),
        5 => Color::new(0.20, 0.43, 0.17, 1.0),
        6 => Color::new(0.18, 0.48, 0.73, 1.0),
        _ => Color::new(0.84, 0.89, 0.94, 1.0),
    }
}
