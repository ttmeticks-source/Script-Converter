//! Reusable RealismCraft gameplay systems.
//! These are deliberately local/simulated systems; they do not claim to provide
//! network multiplayer. A future transport can feed RemoteAvatar snapshots.

use macroquad::prelude::*;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameMode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "survival" | "0" => Some(Self::Survival),
            "creative" | "1" => Some(Self::Creative),
            "adventure" | "2" => Some(Self::Adventure),
            "spectator" | "3" => Some(Self::Spectator),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Survival => "Survival",
            Self::Creative => "Creative",
            Self::Adventure => "Adventure",
            Self::Spectator => "Spectator",
        }
    }
}

/// Parses Minecraft-style /gamemode commands and the earlier !set alias.
pub fn parse_chat_command(message: &str) -> Result<Option<GameMode>, &'static str> {
    let parts: Vec<&str> = message.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(None);
    }

    let value = if parts[0].eq_ignore_ascii_case("!set") {
        if parts.len() != 3 || !parts[1].eq_ignore_ascii_case("GameMode") {
            return Err("Use: /gamemode survival|creative|adventure|spectator");
        }
        parts[2]
    } else if parts[0].eq_ignore_ascii_case("/gamemode")
        || parts[0].eq_ignore_ascii_case("gamemode")
    {
        if parts.len() != 2 {
            return Err("Use: /gamemode survival|creative|adventure|spectator");
        }
        parts[1]
    } else {
        return Ok(None);
    };

    GameMode::parse(value)
        .map(Some)
        .ok_or("Unknown mode. Choose survival, creative, adventure or spectator.")
}

/// A remote-avatar snapshot. A networking layer can update target_position;
/// rendering interpolates toward it so remote movement appears smooth.
#[derive(Clone, Debug)]
pub struct RemoteAvatar {
    pub username: String,
    pub position: Vec3,
    pub target_position: Vec3,
    pub tint: Color,
}

impl RemoteAvatar {
    pub fn new(username: impl Into<String>, position: Vec3, tint: Color) -> Self {
        Self { username: username.into(), position, target_position: position, tint }
    }

    pub fn update(&mut self, dt: f32) {
        let blend = (dt * 12.0).clamp(0.0, 1.0);
        self.position += (self.target_position - self.position) * blend;
    }

    pub fn draw_ghost(&self) {
        // A simple original placeholder avatar, not a real player's live model.
        draw_cube(self.position + vec3(0.0, 0.9, 0.0), vec3(0.62, 1.25, 0.38), None, self.tint);
        draw_cube(self.position + vec3(0.0, 1.72, 0.0), vec3(0.48, 0.48, 0.48), None, self.tint);
        draw_text(
            &self.username,
            self.position.x - self.username.len() as f32 * 0.045,
            self.position.y + 2.15,
            18.0,
            WHITE,
        );
    }
}

/// Draws ten evenly spaced, non-overlapping hearts above the status bar.
pub fn draw_hearts(health: u8, max_health: u8) {
    let max_health = max_health.max(1).min(40);
    let health = health.min(max_health);
    let hearts = (max_health as usize + 1) / 2;
    let start_x = 22.0;
    let y = screen_height() - 143.0;

    draw_rectangle(
        14.0,
        y - 8.0,
        hearts as f32 * 29.0 + 18.0,
        36.0,
        Color::new(0.012, 0.020, 0.055, 0.88),
    );
    draw_rectangle_lines(
        14.0,
        y - 8.0,
        hearts as f32 * 29.0 + 18.0,
        36.0,
        1.0,
        Color::new(0.20, 0.30, 0.56, 0.78),
    );

    for index in 0..hearts {
        let x = start_x + index as f32 * 29.0;
        let filled_units = health.saturating_sub((index * 2) as u8);
        let base = Color::new(0.20, 0.23, 0.33, 1.0);
        let fill = if filled_units >= 2 {
            Color::new(0.94, 0.16, 0.27, 1.0)
        } else {
            Color::new(0.94, 0.16, 0.27, if filled_units == 1 { 0.52 } else { 0.0 })
        };

        // A heart made from two lobes and a lower point avoids the old
        // concave-polygon fan triangulation that produced broken shapes.
        for color in [base, fill] {
            if color.a <= 0.0 {
                continue;
            }
            draw_triangle(
                vec2(x + 1.0, y + 8.0),
                vec2(x + 25.0, y + 8.0),
                vec2(x + 13.0, y + 25.0),
                color,
            );
            draw_circle(x + 7.0, y + 8.0, 6.8, color);
            draw_circle(x + 19.0, y + 8.0, 6.8, color);
            if color == base && filled_units > 0 {
                // Keep the empty heart visible underneath a half/full fill.
            }
        }
    }
}

/// Progress state for a staged world load. Call advance after each batch so
/// the UI can render between batches instead of freezing on one large loop.
#[derive(Clone, Debug)]
pub struct WorldLoadProgress {
    pub completed: usize,
    pub total: usize,
    pub stage: String,
}

impl WorldLoadProgress {
    pub fn new(total: usize) -> Self {
        Self { completed: 0, total: total.max(1), stage: "Preparing world".into() }
    }

    pub fn fraction(&self) -> f32 {
        (self.completed as f32 / self.total as f32).clamp(0.0, 1.0)
    }

    pub fn advance(&mut self, amount: usize, stage: impl Into<String>) {
        self.completed = self.completed.saturating_add(amount).min(self.total);
        self.stage = stage.into();
    }

    pub fn draw(&self) {
        clear_background(Color::new(0.012, 0.018, 0.055, 1.0));
        let w = screen_width();
        let h = screen_height();
        draw_text("REALISMCRAFT", w * 0.5 - 150.0, h * 0.43, 42.0, WHITE);
        draw_text(&self.stage, w * 0.5 - 100.0, h * 0.51, 19.0, LIGHTGRAY);
        let bar_w = (w * 0.55).min(560.0);
        let x = (w - bar_w) * 0.5;
        draw_rectangle(x, h * 0.56, bar_w, 13.0, Color::new(0.12, 0.15, 0.24, 1.0));
        draw_rectangle(x, h * 0.56, bar_w * self.fraction(), 13.0, Color::new(0.32, 0.54, 1.0, 1.0));
        draw_text(&format!("{:.0}%", self.fraction() * 100.0), x, h * 0.61, 16.0, WHITE);
    }
}

/// Face visibility test for a voxel. Only render exposed faces; this is the
/// first step toward chunk meshes and avoids drawing buried blocks.
pub fn exposed_face_count(position: (i32, i32, i32), occupied: &HashSet<(i32, i32, i32)>) -> u8 {
    let (x, y, z) = position;
    let neighbors = [
        (x + 1, y, z), (x - 1, y, z), (x, y + 1, z),
        (x, y - 1, z), (x, y, z + 1), (x, y, z - 1),
    ];
    neighbors.iter().filter(|neighbor| !occupied.contains(neighbor)).count() as u8
}

/// A shape descriptor for non-cubic world geometry. Rendering code can map
/// these to custom meshes instead of forcing every material into a cube.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockShape {
    Cube,
    Slab,
    Slope,
    IrregularRock,
    CrossedLeaves,
}

#[derive(Clone, Debug)]
pub struct ModManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub entry: String,
    pub runtime: String,
}

impl ModManifest {
    pub fn validate(&self) -> Result<(), &'static str> {
        let valid_id = !self.id.is_empty()
            && self.id.len() <= 48
            && self.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
        if !valid_id { return Err("Mod id must use lowercase letters, numbers, _ or - (max 48 chars)."); }
        if self.name.trim().is_empty() { return Err("Mod name cannot be empty."); }
        if self.version.trim().is_empty() { return Err("Mod version cannot be empty."); }
        if !self.entry.ends_with(".wasm") { return Err("Mods must use a .wasm entry module."); }
        if self.runtime != "wasm32-wasi" && self.runtime != "wasm32-unknown-unknown" {
            return Err("Unsupported mod runtime.");
        }
        Ok(())
    }
}
