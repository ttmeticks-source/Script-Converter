use macroquad::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Setup,
    Menu,
    World,
}

#[derive(Clone, Copy)]
struct Block {
    x: i32,
    y: i32,
    z: i32,
    kind: u8,
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

fn panel(x: f32, y: f32, w: f32, h: f32) {
    draw_rectangle(x, y, w, h, Color::new(0.025, 0.035, 0.09, 0.88));
    draw_rectangle_lines(x, y, w, h, 1.5, Color::new(0.35, 0.48, 0.95, 0.8));
}

fn button(label: &str, x: f32, y: f32, w: f32, h: f32) -> bool {
    let (mx, my) = mouse_position();
    let hovered = mx >= x && mx <= x + w && my >= y && my <= y + h;
    let fill = if hovered {
        Color::new(0.22, 0.32, 0.72, 0.98)
    } else {
        Color::new(0.10, 0.15, 0.38, 0.96)
    };
    draw_rectangle(x, y, w, h, fill);
    draw_rectangle_lines(x, y, w, h, 1.0, Color::new(0.55, 0.66, 1.0, 0.9));
    let size = 22.0;
    let measured = measure_text(label, None, size as u16, 1.0);
    draw_text(
        label,
        x + (w - measured.width) * 0.5,
        y + (h + size * 0.32) * 0.5,
        size,
        WHITE,
    );
    hovered && is_mouse_button_pressed(MouseButton::Left)
}

fn galaxy_background() {
    clear_background(Color::new(0.008, 0.012, 0.045, 1.0));
    let width = screen_width();
    let height = screen_height();

    for i in 0..18 {
        let t = i as f32 / 18.0;
        let y = height * t;
        draw_rectangle(
            0.0,
            y,
            width,
            height / 18.0 + 1.0,
            Color::new(0.015 + t * 0.025, 0.018 + t * 0.012, 0.075 + t * 0.10, 0.35),
        );
    }

    let time = get_time() as f32;
    for i in 0..145 {
        let seed = i as f32;
        let x = (seed * 127.1).sin().abs() * width;
        let y = (seed * 311.7).cos().abs() * height;
        let pulse = ((time * 1.4 + seed).sin() * 0.5 + 0.5) * 0.8 + 0.2;
        let radius = if i % 13 == 0 { 2.0 } else { 1.0 };
        draw_circle(x, y, radius, Color::new(0.65, 0.75, 1.0, pulse));
    }

    draw_circle(
        width * 0.78,
        height * 0.18,
        height * 0.20,
        Color::new(0.20, 0.18, 0.65, 0.045),
    );
    draw_circle(
        width * 0.20,
        height * 0.82,
        height * 0.24,
        Color::new(0.10, 0.45, 0.72, 0.035),
    );
}

fn label(text: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text(text, x, y, size, color);
}

fn add_block(blocks: &mut Vec<Block>, x: i32, y: i32, z: i32, kind: u8) {
    if !blocks.iter().any(|b| b.x == x && b.y == y && b.z == z) {
        blocks.push(Block { x, y, z, kind });
    }
}

fn make_world() -> Vec<Block> {
    let mut blocks = Vec::new();
    for x in -12..=12 {
        for z in -12..=12 {
            add_block(&mut blocks, x, 0, z, 0);
            add_block(&mut blocks, x, -1, z, 1);
            if (x * 7 + z * 11).rem_euclid(9) == 0 {
                add_block(&mut blocks, x, 1, z, 2);
            }
        }
    }
    blocks
}

fn block_color(kind: u8) -> Color {
    match kind {
        0 => Color::new(0.25, 0.58, 0.28, 1.0),
        1 => Color::new(0.34, 0.22, 0.14, 1.0),
        2 => Color::new(0.38, 0.40, 0.44, 1.0),
        3 => Color::new(0.25, 0.52, 0.78, 1.0),
        4 => Color::new(0.72, 0.65, 0.48, 1.0),
        _ => Color::new(0.58, 0.40, 0.70, 1.0),
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut screen = Screen::Setup;
    let mut player_name = String::new();
    let mut world_name = String::from("My World");
    let mut active_field = 0_u8;
    let mut blocks = make_world();
    let mut camera_position = vec3(0.0, 3.0, 8.0);
    let mut yaw = 0.0_f32;
    let mut pitch = -0.18_f32;
    let mut selected_block = 0_u8;
    let mut world_message = String::from("Creative mode - build anything.");

    loop {
        let dt = get_frame_time().min(0.05);
        galaxy_background();

        match screen {
            Screen::Setup => {
                let w = screen_width();
                let h = screen_height();
                let box_w = 560.0_f32.min(w - 32.0);
                let box_x = (w - box_w) * 0.5;
                let box_y = (h - 390.0) * 0.5;

                panel(box_x, box_y, box_w, 390.0);
                label("REALISMCRAFT", box_x + 38.0, box_y + 62.0, 42.0, WHITE);
                label("A new world. Your rules.", box_x + 40.0, box_y + 92.0, 19.0,
                      Color::new(0.66, 0.75, 1.0, 1.0));

                label("CHOOSE YOUR PLAYER NAME", box_x + 40.0, box_y + 145.0, 15.0,
                      Color::new(0.72, 0.79, 1.0, 1.0));
                let name_color = if active_field == 0 {
                    Color::new(0.16, 0.22, 0.48, 1.0)
                } else {
                    Color::new(0.07, 0.10, 0.24, 1.0)
                };
                draw_rectangle(box_x + 38.0, box_y + 158.0, box_w - 76.0, 48.0, name_color);
                draw_rectangle_lines(box_x + 38.0, box_y + 158.0, box_w - 76.0, 48.0, 1.0,
                                     Color::new(0.48, 0.60, 1.0, 0.9));
                label(
                    if player_name.is_empty() { "Type a name..." } else { &player_name },
                    box_x + 54.0, box_y + 190.0, 21.0, WHITE,
                );

                label("WORLD NAME", box_x + 40.0, box_y + 245.0, 15.0,
                      Color::new(0.72, 0.79, 1.0, 1.0));
                let world_color = if active_field == 1 {
                    Color::new(0.16, 0.22, 0.48, 1.0)
                } else {
                    Color::new(0.07, 0.10, 0.24, 1.0)
                };
                draw_rectangle(box_x + 38.0, box_y + 258.0, box_w - 76.0, 42.0, world_color);
                draw_rectangle_lines(box_x + 38.0, box_y + 258.0, box_w - 76.0, 42.0, 1.0,
                                     Color::new(0.48, 0.60, 1.0, 0.9));
                label(&world_name, box_x + 54.0, box_y + 286.0, 19.0, WHITE);

                let (mx, my) = mouse_position();
                if is_mouse_button_pressed(MouseButton::Left) {
                    if mx >= box_x + 38.0 && mx <= box_x + box_w - 38.0 {
                        if my >= box_y + 158.0 && my <= box_y + 206.0 {
                            active_field = 0;
                        } else if my >= box_y + 258.0 && my <= box_y + 300.0 {
                            active_field = 1;
                        }
                    }
                }

                while let Some(ch) = get_char_pressed() {
                    if !ch.is_control() {
                        let target = if active_field == 0 { &mut player_name } else { &mut world_name };
                        if target.chars().count() < 22 {
                            target.push(ch);
                        }
                    }
                }
                if is_key_pressed(KeyCode::Backspace) {
                    let target = if active_field == 0 { &mut player_name } else { &mut world_name };
                    target.pop();
                }

                if button("CONTINUE", box_x + 38.0, box_y + 324.0, box_w - 76.0, 46.0) {
                    if player_name.trim().is_empty() {
                        player_name = "Player".to_owned();
                    }
                    if world_name.trim().is_empty() {
                        world_name = "My World".to_owned();
                    }
                    screen = Screen::Menu;
                }
            }
            Screen::Menu => {
                let w = screen_width();
                let h = screen_height();
                let left = (w - 620.0) * 0.5;
                label("REALISMCRAFT", left, h * 0.27, 58.0, WHITE);
                label("BUILD YOUR OWN UNIVERSE", left + 4.0, h * 0.27 + 34.0, 17.0,
                      Color::new(0.67, 0.77, 1.0, 1.0));
                panel(left, h * 0.39, 620.0, 260.0);
                label(&format!("Welcome, {}", player_name), left + 34.0, h * 0.39 + 48.0, 24.0, WHITE);
                label(&format!("Selected world: {}", world_name), left + 34.0, h * 0.39 + 82.0, 17.0,
                      Color::new(0.72, 0.79, 1.0, 1.0));
                if button("PLAY", left + 34.0, h * 0.39 + 112.0, 552.0, 54.0) {
                    blocks = make_world();
                    camera_position = vec3(0.0, 3.0, 8.0);
                    yaw = 0.0;
                    pitch = -0.18;
                    screen = Screen::World;
                }
                if button("CHANGE PLAYER / WORLD NAME", left + 34.0, h * 0.39 + 180.0, 552.0, 44.0) {
                    screen = Screen::Setup;
                }
                label("EARLY PROTOTYPE  |  CREATIVE SANDBOX", left + 4.0, h * 0.88, 14.0,
                      Color::new(0.54, 0.63, 0.86, 1.0));
            }
            Screen::World => {
                if is_key_pressed(KeyCode::Escape) {
                    screen = Screen::Menu;
                }

                if is_key_down(KeyCode::Left) { yaw -= 1.8 * dt; }
                if is_key_down(KeyCode::Right) { yaw += 1.8 * dt; }
                if is_key_down(KeyCode::Up) { pitch = (pitch + 1.2 * dt).clamp(-1.2, 1.2); }
                if is_key_down(KeyCode::Down) { pitch = (pitch - 1.2 * dt).clamp(-1.2, 1.2); }

                let forward = vec3(yaw.sin(), 0.0, -yaw.cos());
                let right = vec3(yaw.cos(), 0.0, yaw.sin());
                let speed = if is_key_down(KeyCode::LeftShift) { 10.0 } else { 5.0 };
                if is_key_down(KeyCode::W) { camera_position += forward * speed * dt; }
                if is_key_down(KeyCode::S) { camera_position -= forward * speed * dt; }
                if is_key_down(KeyCode::D) { camera_position += right * speed * dt; }
                if is_key_down(KeyCode::A) { camera_position -= right * speed * dt; }
                if is_key_down(KeyCode::Space) { camera_position.y += speed * dt; }
                if is_key_down(KeyCode::LeftControl) { camera_position.y -= speed * dt; }

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

                draw_grid(
                    40,
                    1.0,
                    Color::new(0.15, 0.20, 0.28, 0.25),
                    Color::new(0.15, 0.20, 0.28, 0.25),
                );
                for block in &blocks {
                    let p = vec3(block.x as f32, block.y as f32, block.z as f32);
                    draw_cube(p, vec3(1.0, 1.0, 1.0), None, block_color(block.kind));
                    draw_cube_wires(p, vec3(1.002, 1.002, 1.002), Color::new(0.035, 0.045, 0.06, 0.7));
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

                if is_mouse_button_pressed(MouseButton::Left) {
                    add_block(&mut blocks, bx, by, bz, selected_block);
                    world_message = "Block placed.".to_owned();
                }
                if is_mouse_button_pressed(MouseButton::Right) {
                    if let Some(index) = blocks.iter().position(|b| b.x == bx && b.y == by && b.z == bz) {
                        blocks.remove(index);
                        world_message = "Block removed.".to_owned();
                    }
                }

                set_default_camera();
                let hud_w = screen_width();
                draw_rectangle(16.0, 16.0, 360.0, 104.0, Color::new(0.015, 0.025, 0.06, 0.78));
                label(&format!("{}  |  {}", player_name, world_name), 30.0, 43.0, 19.0, WHITE);
                label("WASD move  |  arrows look  |  space / ctrl up-down", 30.0, 70.0, 13.0,
                      Color::new(0.76, 0.82, 1.0, 1.0));
                label("LMB place  |  RMB remove  |  1-5 select block  |  ESC menu", 30.0, 94.0, 13.0,
                      Color::new(0.76, 0.82, 1.0, 1.0));
                draw_rectangle(hud_w - 240.0, 16.0, 224.0, 72.0, Color::new(0.015, 0.025, 0.06, 0.78));
                label(&format!("BLOCKS: {}", blocks.len()), hud_w - 222.0, 45.0, 18.0, WHITE);
                label(&format!("SELECTED: {}", selected_block + 1), hud_w - 222.0, 71.0, 14.0,
                      Color::new(0.76, 0.82, 1.0, 1.0));
                let _ = &world_message;
            }
        }

        next_frame().await;
    }
}
