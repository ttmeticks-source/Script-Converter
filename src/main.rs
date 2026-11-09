use macroquad::prelude::*;
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

const PASSWORD_ROUNDS: u32 = 120_000;
const SEA_LEVEL: i32 = 4;
const WORLD_RADIUS: i32 = 18;

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    CreateAccount,
    Login,
    Menu,
    WorldSetup,
    World,
}

#[derive(Clone, Copy)]
struct Block {
    x: i32,
    y: i32,
    z: i32,
    kind: u8,
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
        high_dpi: true,
        fullscreen: false,
        sample_count: 4,
        window_resizable: true,
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
    blocks: &[Block],
) -> Result<(), String> {
    fs::create_dir_all(data_directory())
        .map_err(|_| "Could not create the world save folder.".to_owned())?;

    let mut contents = format!("{}\n{}\n{}\n", name.replace('\n', " "), seed, style);
    for block in blocks {
        contents.push_str(&format!(
            "{},{},{},{}\n",
            block.x, block.y, block.z, block.kind
        ));
    }
    fs::write(world_path(), contents)
        .map_err(|_| "Could not write the world save file.".to_owned())
}

fn load_world() -> Option<(String, u64, u8, Vec<Block>)> {
    let contents = fs::read_to_string(world_path()).ok()?;
    let mut lines = contents.lines();
    let name = lines.next()?.to_owned();
    let seed = lines.next()?.parse::<u64>().ok()?;
    let style = lines.next()?.parse::<u8>().ok()?.min(2);
    let mut blocks = Vec::new();

    for line in lines {
        let mut values = line.split(',');
        let x = values.next()?.parse::<i32>().ok()?;
        let y = values.next()?.parse::<i32>().ok()?;
        let z = values.next()?.parse::<i32>().ok()?;
        let kind = values.next()?.parse::<u8>().ok()?;
        if kind <= 7 {
            blocks.push(Block { x, y, z, kind });
        }
    }
    Some((name, seed, style, blocks))
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

fn hash01(seed: u64, x: i32, z: i32) -> f32 {
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

fn value_noise(seed: u64, x: f32, z: f32) -> f32 {
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

fn terrain_height(seed: u64, style: u8, x: i32, z: i32) -> i32 {
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

fn push_block(
    blocks: &mut Vec<Block>,
    occupied: &mut HashSet<(i32, i32, i32)>,
    x: i32,
    y: i32,
    z: i32,
    kind: u8,
) {
    if occupied.insert((x, y, z)) {
        blocks.push(Block { x, y, z, kind });
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
    let mut image = Image::gen_image_color(32, 32, WHITE);
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

    for y in 0..32_u32 {
        for x in 0..32_u32 {
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
            let mut shade = 0.78 + grain * 0.34 + fine * 0.12;

            match kind {
                0 => {
                    if grain > 0.88 {
                        shade *= 1.18;
                    }
                }
                1 => {
                    if (x + y * 3) % 11 == 0 {
                        shade *= 0.72;
                    }
                }
                2 => {
                    let vein = value_noise(0x1928374655aaee11, x as f32 * 0.19, y as f32 * 0.19);
                    if (vein - 0.5).abs() < 0.045 {
                        shade *= 0.68;
                    }
                    if grain > 0.94 {
                        shade *= 1.24;
                    }
                }
                3 => {
                    if (y % 7 == 0) && grain > 0.25 {
                        shade *= 0.88;
                    }
                }
                4 => {
                    if x % 8 <= 1 {
                        shade *= 0.62;
                    } else if x % 8 == 2 {
                        shade *= 0.82;
                    }
                }
                5 => {
                    if grain > 0.78 {
                        shade *= 0.70;
                    } else if grain < 0.18 {
                        shade *= 1.18;
                    }
                }
                6 => {
                    let ripple = ((y as f32 * 0.8 + x as f32 * 0.18).sin() + 1.0) * 0.08;
                    shade += ripple;
                }
                _ => {
                    if grain > 0.91 {
                        shade *= 0.83;
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

#[macroquad::main(window_conf)]
async fn main() {
    let mut account = load_account();
    let mut screen = if account.is_some() {
        Screen::Login
    } else {
        Screen::CreateAccount
    };
    let mut player_name = account
        .as_ref()
        .map(|saved| saved.username.clone())
        .unwrap_or_default();
    let mut username_input = player_name.clone();
    let mut password_input = String::new();
    let mut confirm_input = String::new();
    let mut auth_field = 0_u8;
    let mut auth_status = String::new();

    let mut world_name = "New World".to_owned();
    let mut world_seed = random_seed();
    let mut seed_input = world_seed.to_string();
    let mut world_style = 0_u8;
    let mut world_name_field = 0_u8;
    let mut world_status = String::new();
    let mut blocks = Vec::new();
    let mut occupied = HashSet::new();
    let mut has_world = false;

    if let Some((saved_name, saved_seed, saved_style, saved_blocks)) = load_world() {
        world_name = saved_name;
        world_seed = saved_seed;
        seed_input = world_seed.to_string();
        world_style = saved_style;
        for block in saved_blocks {
            occupied.insert((block.x, block.y, block.z));
            blocks.push(block);
        }
        has_world = true;
    }

    let textures: [Texture2D; 8] =
        std::array::from_fn(|index| material_texture(index as u8));

    let mut camera_position = spawn_position(world_seed, world_style);
    let mut yaw = 0.0_f32;
    let mut pitch = -0.34_f32;
    let mut selected_block = 0_u8;
    let mut world_message = "Welcome to your world.".to_owned();

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
                let box_w = 620.0_f32.min(width - 28.0);
                let left = (width - box_w) * 0.5;
                label("REALISMCRAFT", left, height * 0.21, 54.0, WHITE);
                label(
                    "EXPLORE. BUILD. MAKE IT YOURS.",
                    left + 4.0,
                    height * 0.21 + 32.0,
                    16.0,
                    Color::new(0.68, 0.78, 1.0, 1.0),
                );

                let top = height * 0.34;
                panel(left, top, box_w, 350.0);
                label(
                    &format!("Welcome, {}", player_name),
                    left + 34.0,
                    top + 49.0,
                    25.0,
                    WHITE,
                );
                label(
                    "LOCAL PLAYER PROFILE  /  OFFLINE",
                    left + 36.0,
                    top + 75.0,
                    13.0,
                    Color::new(0.60, 0.72, 1.0, 1.0),
                );

                if has_world {
                    label(
                        &format!("Last world: {}", world_name),
                        left + 36.0,
                        top + 105.0,
                        15.0,
                        Color::new(0.82, 0.87, 1.0, 1.0),
                    );
                    if button("PLAY SAVED WORLD", left + 34.0, top + 125.0, box_w - 68.0, 50.0) {
                        camera_position = spawn_position(world_seed, world_style);
                        yaw = 0.0;
                        pitch = -0.34;
                        screen = Screen::World;
                    }
                } else {
                    label(
                        "No world created yet. Start with a fresh landscape.",
                        left + 36.0,
                        top + 111.0,
                        15.0,
                        Color::new(0.82, 0.87, 1.0, 1.0),
                    );
                }

                if button("CREATE NEW WORLD", left + 34.0, top + 190.0, box_w - 68.0, 50.0) {
                    world_name = "New World".to_owned();
                    world_seed = random_seed();
                    seed_input = world_seed.to_string();
                    world_name_field = 0;
                    world_status.clear();
                    screen = Screen::WorldSetup;
                }
                if button("SIGN OUT", left + 34.0, top + 260.0, box_w - 68.0, 42.0) {
                    password_input.clear();
                    auth_status.clear();
                    username_input = player_name.clone();
                    auth_field = 1;
                    screen = Screen::Login;
                }
                label(
                    "REALISMCRAFT  |  ORIGINAL VOXEL SANDBOX",
                    left + 4.0,
                    height * 0.90,
                    13.0,
                    Color::new(0.55, 0.65, 0.90, 1.0),
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
                let top = (height - 468.0) * 0.5;
                panel(left, top, box_w, 468.0);
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
                if button("GENERATE WORLD  /  PLAY", left + 36.0, top + 359.0, field_w, 49.0) {
                    let parsed_seed = seed_input.trim().parse::<u64>();
                    world_seed = parsed_seed.unwrap_or_else(|_| random_seed());
                    seed_input = world_seed.to_string();
                    if world_name.trim().is_empty() {
                        world_name = "New World".to_owned();
                    }

                    let generated = generate_world(world_seed, world_style);
                    blocks = generated.0;
                    occupied = generated.1;
                    has_world = true;
                    camera_position = spawn_position(world_seed, world_style);
                    yaw = 0.0;
                    pitch = -0.34;
                    selected_block = 0;
                    world_message = format!(
                        "Generated {} terrain. Seed: {}",
                        terrain_style_name(world_style),
                        world_seed
                    );
                    match save_world(&world_name, world_seed, world_style, &blocks) {
                        Ok(()) => world_status.clear(),
                        Err(message) => world_status = message,
                    }
                    screen = Screen::World;
                }
                if button("BACK TO MENU", left + 36.0, top + 417.0, field_w, 34.0) {
                    screen = Screen::Menu;
                }
                label(
                    &world_status,
                    left + 38.0,
                    top + 459.0,
                    12.0,
                    Color::new(1.0, 0.62, 0.56, 1.0),
                );
            }
            Screen::World => {
                if is_key_pressed(KeyCode::Escape) {
                    if let Err(message) = save_world(&world_name, world_seed, world_style, &blocks) {
                        world_message = message;
                    }
                    screen = Screen::Menu;
                }

                if is_key_down(KeyCode::Left) {
                    yaw -= 1.8 * dt;
                }
                if is_key_down(KeyCode::Right) {
                    yaw += 1.8 * dt;
                }
                if is_key_down(KeyCode::Up) {
                    pitch = (pitch + 1.2 * dt).clamp(-1.2, 1.2);
                }
                if is_key_down(KeyCode::Down) {
                    pitch = (pitch - 1.2 * dt).clamp(-1.2, 1.2);
                }

                let forward = vec3(yaw.sin(), 0.0, -yaw.cos());
                let right = vec3(yaw.cos(), 0.0, yaw.sin());
                let speed = if is_key_down(KeyCode::LeftShift) { 10.0 } else { 5.0 };
                if is_key_down(KeyCode::W) {
                    camera_position += forward * speed * dt;
                }
                if is_key_down(KeyCode::S) {
                    camera_position -= forward * speed * dt;
                }
                if is_key_down(KeyCode::D) {
                    camera_position += right * speed * dt;
                }
                if is_key_down(KeyCode::A) {
                    camera_position -= right * speed * dt;
                }
                if is_key_down(KeyCode::Space) {
                    camera_position.y += speed * dt;
                }
                if is_key_down(KeyCode::LeftControl) {
                    camera_position.y -= speed * dt;
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

                for block in &blocks {
                    let dx = block.x as f32 - camera_position.x;
                    let dy = block.y as f32 - camera_position.y;
                    let dz = block.z as f32 - camera_position.z;
                    if dx * dx + dy * dy + dz * dz > 1_000.0 {
                        continue;
                    }

                    let position = vec3(block.x as f32, block.y as f32, block.z as f32);
                    let tint = if block.kind == 6 {
                        Color::new(0.80, 0.91, 1.0, 0.72)
                    } else {
                        WHITE
                    };
                    draw_cube(
                        position,
                        vec3(1.0, 1.0, 1.0),
                        Some(&textures[block.kind as usize]),
                        tint,
                    );
                    draw_cube_wires(
                        position,
                        vec3(1.002, 1.002, 1.002),
                        Color::new(0.025, 0.035, 0.045, 0.35),
                    );
                }

                let target = camera_position + look * 3.0;
                let bx = target.x.round() as i32;
                let by = target.y.round() as i32;
                let bz = target.z.round() as i32;

                if is_key_pressed(KeyCode::Key1) { selected_block = 0; }
                if is_key_pressed(KeyCode::Key2) { selected_block = 1; }
                if is_key_pressed(KeyCode::Key3) { selected_block = 2; }
                if is_key_pressed(KeyCode::Key4) { selected_block = 3; }
                if is_key_pressed(KeyCode::Key5) { selected_block = 4; }
                if is_key_pressed(KeyCode::Key6) { selected_block = 5; }
                if is_key_pressed(KeyCode::Key7) { selected_block = 6; }
                if is_key_pressed(KeyCode::Key8) { selected_block = 7; }

                if is_mouse_button_pressed(MouseButton::Left) {
                    if occupied.insert((bx, by, bz)) {
                        blocks.push(Block {
                            x: bx,
                            y: by,
                            z: bz,
                            kind: selected_block,
                        });
                        world_message = format!("Placed {}.", material_name(selected_block));
                        if let Err(message) =
                            save_world(&world_name, world_seed, world_style, &blocks)
                        {
                            world_message = message;
                        }
                    }
                }
                if is_mouse_button_pressed(MouseButton::Right) {
                    if let Some(index) = blocks
                        .iter()
                        .position(|block| block.x == bx && block.y == by && block.z == bz)
                    {
                        let removed = blocks.remove(index);
                        occupied.remove(&(removed.x, removed.y, removed.z));
                        world_message = format!("Removed {}.", material_name(removed.kind));
                        if let Err(message) =
                            save_world(&world_name, world_seed, world_style, &blocks)
                        {
                            world_message = message;
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
                    &format!("{} blocks   |   WASD move   |   arrows look", blocks.len()),
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
                    "LMB place  |  RMB remove  |  1-8 materials  |  ESC save & menu",
                    screen_w - 420.0,
                    screen_h - 28.0,
                    12.0,
                    Color::new(0.95, 0.97, 1.0, 1.0),
                );
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
