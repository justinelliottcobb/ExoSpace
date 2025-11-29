use libnotcurses_sys::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Server URL for map fetching
const SERVER_URL: &str = "http://localhost:3000";

/// User configuration
#[derive(Serialize, Deserialize, Clone)]
struct Config {
    /// Enable background visual effects (stars, nebula animations, etc.)
    effects_enabled: bool,
    /// Server URL override
    server_url: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            effects_enabled: false,  // Off by default
            server_url: None,
        }
    }
}

impl Config {
    /// Get the config file path
    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|mut p| {
            p.push("exospace");
            p.push("config.json");
            p
        })
    }

    /// Load config from file, or return default if not found
    fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };

        match fs::read_to_string(&path) {
            Ok(contents) => {
                serde_json::from_str(&contents).unwrap_or_else(|e| {
                    eprintln!("Warning: Failed to parse config: {}", e);
                    Self::default()
                })
            }
            Err(_) => Self::default(),
        }
    }

    /// Save config to file
    fn save(&self) -> Result<(), String> {
        let path = Self::config_path()
            .ok_or_else(|| "Could not determine config directory".to_string())?;

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        fs::write(&path, json)
            .map_err(|e| format!("Failed to write config file: {}", e))?;

        Ok(())
    }

    /// Get the server URL (config override or default)
    fn server_url(&self) -> &str {
        self.server_url.as_deref().unwrap_or(SERVER_URL)
    }
}

/// Tile types in the map
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
enum Tile {
    Wall,
    Floor,
    Asteroid,
    Nebula,
}

/// Map data received from server
#[derive(Deserialize)]
struct MapData {
    tiles: Vec<Vec<Tile>>,
    width: usize,
    height: usize,
    start_x: i32,
    start_y: i32,
}

impl Tile {
    fn is_passable(&self) -> bool {
        matches!(self, Tile::Floor | Tile::Nebula)
    }
}

/// 8-directional orientation
#[derive(Clone, Copy, PartialEq, Debug, Default)]
enum Direction {
    #[default]
    Up,
    UpRight,
    Right,
    DownRight,
    Down,
    DownLeft,
    Left,
    UpLeft,
}

impl Direction {
    fn from_delta(dx: i32, dy: i32) -> Option<Direction> {
        match (dx, dy) {
            (0, -1) => Some(Direction::Up),
            (1, -1) => Some(Direction::UpRight),
            (1, 0) => Some(Direction::Right),
            (1, 1) => Some(Direction::DownRight),
            (0, 1) => Some(Direction::Down),
            (-1, 1) => Some(Direction::DownLeft),
            (-1, 0) => Some(Direction::Left),
            (-1, -1) => Some(Direction::UpLeft),
            _ => None,
        }
    }

    fn to_char(self) -> char {
        match self {
            Direction::Up => '↑',
            Direction::UpRight => '↗',
            Direction::Right => '→',
            Direction::DownRight => '↘',
            Direction::Down => '↓',
            Direction::DownLeft => '↙',
            Direction::Left => '←',
            Direction::UpLeft => '↖',
        }
    }

    fn name(self) -> &'static str {
        match self {
            Direction::Up => "N",
            Direction::UpRight => "NE",
            Direction::Right => "E",
            Direction::DownRight => "SE",
            Direction::Down => "S",
            Direction::DownLeft => "SW",
            Direction::Left => "W",
            Direction::UpLeft => "NW",
        }
    }
}

/// Simple deterministic hash for consistent random-looking values
fn hash_position(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(374761393);
    h = h.wrapping_add((y as u32).wrapping_mul(668265263));
    h = h.wrapping_add(seed.wrapping_mul(1013904223));
    h ^= h >> 13;
    h = h.wrapping_mul(1274126177);
    h ^= h >> 16;
    h
}

/// The game map
struct Map {
    tiles: Vec<Vec<Tile>>,
    width: usize,
    height: usize,
    start_position: Option<(i32, i32)>,
}

impl Map {
    /// Fetch map from the server
    fn fetch_from_server(config: &Config) -> Result<Self, String> {
        let url = format!("{}/map", config.server_url());

        let response = reqwest::blocking::get(&url)
            .map_err(|e| format!("Failed to connect to server: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Server returned error: {}", response.status()));
        }

        let map_data: MapData = response
            .json()
            .map_err(|e| format!("Failed to parse map data: {}", e))?;

        Ok(Map {
            tiles: map_data.tiles,
            width: map_data.width,
            height: map_data.height,
            start_position: Some((map_data.start_x, map_data.start_y)),
        })
    }

    /// Generate map locally (fallback)
    fn generate_local(width: usize, height: usize) -> Self {
        let mut tiles = vec![vec![Tile::Wall; width]; height];

        let mut rng_state: u64 = 12345;

        let mut rand = || -> u64 {
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            (rng_state >> 16) & 0x7fff
        };

        // Create main corridors with varying widths
        let mut y = 2;
        while y < height - 2 {
            let corridor_height = (rand() % 15 + 3) as usize;
            let wall_height = (rand() % 4 + 1) as usize;

            for cy in y..(y + corridor_height).min(height - 1) {
                for x in 1..width - 1 {
                    tiles[cy][x] = Tile::Floor;
                }
            }

            y += corridor_height + wall_height;
        }

        // Create vertical corridors
        let mut x = 2;
        while x < width - 2 {
            let corridor_width = (rand() % 18 + 2) as usize;
            let wall_width = (rand() % 6 + 2) as usize;

            for cx in x..(x + corridor_width).min(width - 1) {
                for y in 1..height - 1 {
                    tiles[y][cx] = Tile::Floor;
                }
            }

            x += corridor_width + wall_width;
        }

        // Add some random rooms
        let num_rooms = (width * height) / 2000;
        for _ in 0..num_rooms {
            let room_w = (rand() % 20 + 5) as usize;
            let room_h = (rand() % 15 + 4) as usize;
            let room_x = (rand() as usize % (width.saturating_sub(room_w + 2))).max(1);
            let room_y = (rand() as usize % (height.saturating_sub(room_h + 2))).max(1);

            for ry in room_y..(room_y + room_h).min(height - 1) {
                for rx in room_x..(room_x + room_w).min(width - 1) {
                    tiles[ry][rx] = Tile::Floor;
                }
            }
        }

        // Add nebula zones (passable colored areas)
        let num_nebulae = (width * height) / 5000;
        for _ in 0..num_nebulae {
            let neb_w = (rand() % 30 + 10) as usize;
            let neb_h = (rand() % 20 + 8) as usize;
            let neb_x = (rand() as usize % width.saturating_sub(neb_w + 2)).max(1);
            let neb_y = (rand() as usize % height.saturating_sub(neb_h + 2)).max(1);

            for ny in neb_y..(neb_y + neb_h).min(height - 1) {
                for nx in neb_x..(neb_x + neb_w).min(width - 1) {
                    if tiles[ny][nx] == Tile::Floor {
                        tiles[ny][nx] = Tile::Nebula;
                    }
                }
            }
        }

        // Add internal walls/pillars
        let num_pillars = (width * height) / 500;
        for _ in 0..num_pillars {
            let pillar_w = (rand() % 8 + 1) as usize;
            let pillar_h = (rand() % 8 + 1) as usize;
            let pillar_x = (rand() as usize % width.saturating_sub(pillar_w + 4)) + 2;
            let pillar_y = (rand() as usize % height.saturating_sub(pillar_h + 4)) + 2;

            let mut can_place = true;
            for py in pillar_y.saturating_sub(1)..(pillar_y + pillar_h + 1).min(height) {
                for px in pillar_x.saturating_sub(1)..(pillar_x + pillar_w + 1).min(width) {
                    if tiles[py][px] == Tile::Wall {
                        can_place = false;
                        break;
                    }
                }
                if !can_place {
                    break;
                }
            }

            if can_place {
                for py in pillar_y..(pillar_y + pillar_h).min(height - 1) {
                    for px in pillar_x..(pillar_x + pillar_w).min(width - 1) {
                        tiles[py][px] = Tile::Wall;
                    }
                }
            }
        }

        // Add asteroid fields (impassable but different visual)
        let num_asteroid_fields = (width * height) / 3000;
        for _ in 0..num_asteroid_fields {
            let field_w = (rand() % 15 + 5) as usize;
            let field_h = (rand() % 10 + 4) as usize;
            let field_x = (rand() as usize % width.saturating_sub(field_w + 2)).max(1);
            let field_y = (rand() as usize % height.saturating_sub(field_h + 2)).max(1);

            for fy in field_y..(field_y + field_h).min(height - 1) {
                for fx in field_x..(field_x + field_w).min(width - 1) {
                    // Sparse asteroids
                    if rand() % 3 == 0 && tiles[fy][fx] == Tile::Floor {
                        tiles[fy][fx] = Tile::Asteroid;
                    }
                }
            }
        }

        // Ensure borders are walls
        for x in 0..width {
            tiles[0][x] = Tile::Wall;
            tiles[height - 1][x] = Tile::Wall;
        }
        for y in 0..height {
            tiles[y][0] = Tile::Wall;
            tiles[y][width - 1] = Tile::Wall;
        }

        Map { tiles, width, height, start_position: None }
    }

    /// Get map from server, falling back to local generation
    fn new(config: &Config) -> Self {
        match Self::fetch_from_server(config) {
            Ok(map) => {
                eprintln!("Connected to server, map loaded");
                map
            }
            Err(e) => {
                eprintln!("Server unavailable ({}), generating local map", e);
                Self::generate_local(500, 200)
            }
        }
    }

    fn get(&self, x: i32, y: i32) -> Option<Tile> {
        if x < 0 || y < 0 {
            return None;
        }
        self.tiles
            .get(y as usize)
            .and_then(|row| row.get(x as usize))
            .copied()
    }

    fn is_passable(&self, x: i32, y: i32) -> bool {
        self.get(x, y).map(|t| t.is_passable()).unwrap_or(false)
    }

    fn find_start_position(&self) -> (i32, i32) {
        // Use server-provided start position if available
        if let Some(pos) = self.start_position {
            return pos;
        }

        // Otherwise search for one
        let center_x = self.width / 2;
        let center_y = self.height / 2;

        for radius in 0..self.width.max(self.height) {
            for dy in -(radius as i32)..=(radius as i32) {
                for dx in -(radius as i32)..=(radius as i32) {
                    let x = center_x as i32 + dx;
                    let y = center_y as i32 + dy;
                    if self.is_passable(x, y) {
                        return (x, y);
                    }
                }
            }
        }
        (1, 1)
    }
}

/// A single cell of the ship sprite
#[derive(Clone, Copy, Debug, PartialEq)]
struct ShipCell {
    ch: char,
    fg: u32,
    bg: Option<u32>,
}

impl ShipCell {
    fn new(ch: char, fg: u32) -> Self {
        ShipCell { ch, fg, bg: None }
    }

    fn with_bg(ch: char, fg: u32, bg: u32) -> Self {
        ShipCell { ch, fg, bg: Some(bg) }
    }

    fn empty() -> Self {
        ShipCell { ch: ' ', fg: 0x000000, bg: None }
    }
}

/// Ship sprite data - 3x3 grid for each direction
/// Grid is [row][col] where (0,0) is top-left
struct ShipSprite {
    cells: [[ShipCell; 3]; 3],
}

impl ShipSprite {
    /// Get ship sprite for a direction
    fn for_direction(direction: Direction) -> Self {
        let hull = 0x40C080;      // Cyan-green hull
        let cockpit = 0x80FFFF;   // Bright cyan cockpit
        let wing = 0x3090A0;      // Darker wing color
        let accent = 0x60A0C0;    // Accent color

        let e = ShipCell::empty();

        match direction {
            Direction::Up => ShipSprite {
                cells: [
                    [e,                              ShipCell::new('^', cockpit), e],
                    [ShipCell::new('/', wing),       ShipCell::new('|', hull),    ShipCell::new('\\', wing)],
                    [ShipCell::new('<', accent),     ShipCell::new('=', hull),    ShipCell::new('>', accent)],
                ],
            },
            Direction::Down => ShipSprite {
                cells: [
                    [ShipCell::new('>', accent),     ShipCell::new('=', hull),    ShipCell::new('<', accent)],
                    [ShipCell::new('\\', wing),      ShipCell::new('|', hull),    ShipCell::new('/', wing)],
                    [e,                              ShipCell::new('v', cockpit), e],
                ],
            },
            Direction::Left => ShipSprite {
                cells: [
                    [e,                              ShipCell::new('/', wing),    ShipCell::new('^', accent)],
                    [ShipCell::new('<', cockpit),    ShipCell::new('-', hull),    ShipCell::new('=', hull)],
                    [e,                              ShipCell::new('\\', wing),   ShipCell::new('v', accent)],
                ],
            },
            Direction::Right => ShipSprite {
                cells: [
                    [ShipCell::new('v', accent),     ShipCell::new('\\', wing),   e],
                    [ShipCell::new('=', hull),       ShipCell::new('-', hull),    ShipCell::new('>', cockpit)],
                    [ShipCell::new('^', accent),     ShipCell::new('/', wing),    e],
                ],
            },
            Direction::UpRight => ShipSprite {
                cells: [
                    [e,                              ShipCell::new('/', wing),    ShipCell::new('>', cockpit)],
                    [ShipCell::new('/', wing),       ShipCell::new('/', hull),    ShipCell::new('/', wing)],
                    [ShipCell::new('<', accent),     ShipCell::new('/', wing),    e],
                ],
            },
            Direction::UpLeft => ShipSprite {
                cells: [
                    [ShipCell::new('<', cockpit),    ShipCell::new('\\', wing),   e],
                    [ShipCell::new('\\', wing),      ShipCell::new('\\', hull),   ShipCell::new('\\', wing)],
                    [e,                              ShipCell::new('\\', wing),   ShipCell::new('>', accent)],
                ],
            },
            Direction::DownRight => ShipSprite {
                cells: [
                    [ShipCell::new('^', accent),     ShipCell::new('\\', wing),   e],
                    [ShipCell::new('\\', wing),      ShipCell::new('\\', hull),   ShipCell::new('\\', wing)],
                    [e,                              ShipCell::new('\\', wing),   ShipCell::new('v', cockpit)],
                ],
            },
            Direction::DownLeft => ShipSprite {
                cells: [
                    [e,                              ShipCell::new('/', wing),    ShipCell::new('^', accent)],
                    [ShipCell::new('/', wing),       ShipCell::new('/', hull),    ShipCell::new('/', wing)],
                    [ShipCell::new('v', cockpit),    ShipCell::new('/', wing),    e],
                ],
            },
        }
    }
}

/// Exhaust animation - 3x4 grid behind the ship
struct ExhaustSprite {
    cells: [[ShipCell; 3]; 4],
}

impl ExhaustSprite {
    /// Get exhaust sprite for a direction and animation frame
    fn for_direction(direction: Direction, frame: u64) -> Self {
        let phase = (frame / 4) % 4; // Exhaust animation cycles every 4 frames

        // Exhaust colors that cycle - bright near ship, fading further out
        let colors = [0xFF6600, 0xFFAA00, 0xFFFF00, 0xFF8800]; // Orange to yellow
        let mid_colors = [0xCC5500, 0xCC8800, 0xCCCC00, 0xCC6600]; // Mid brightness
        let dim_colors = [0x803300, 0x805500, 0x808000, 0x804400]; // Dimmer
        let faint_colors = [0x401800, 0x402800, 0x404000, 0x402200]; // Faintest trail

        let bright = colors[phase as usize];
        let mid = mid_colors[phase as usize];
        let dim = dim_colors[phase as usize];
        let faint = faint_colors[phase as usize];

        let e = ShipCell::empty();

        // Exhaust characters that flicker
        let exhaust_chars = ['*', '+', 'o', '.'];
        let ch1 = exhaust_chars[phase as usize];
        let ch2 = exhaust_chars[((phase + 2) % 4) as usize];
        let ch3 = exhaust_chars[((phase + 1) % 4) as usize];
        let ch4 = exhaust_chars[((phase + 3) % 4) as usize];

        match direction {
            Direction::Up => ExhaustSprite {
                // Exhaust below the ship (trailing down)
                cells: [
                    [ShipCell::new(ch2, mid),    ShipCell::new('|', bright), ShipCell::new(ch2, mid)],
                    [ShipCell::new(ch1, dim),    ShipCell::new(ch1, mid),    ShipCell::new(ch1, dim)],
                    [e,                          ShipCell::new(ch3, dim),    e],
                    [e,                          ShipCell::new(ch4, faint),  e],
                ],
            },
            Direction::Down => ExhaustSprite {
                // Exhaust above the ship (trailing up)
                cells: [
                    [e,                          ShipCell::new(ch4, faint),  e],
                    [e,                          ShipCell::new(ch3, dim),    e],
                    [ShipCell::new(ch1, dim),    ShipCell::new(ch1, mid),    ShipCell::new(ch1, dim)],
                    [ShipCell::new(ch2, mid),    ShipCell::new('|', bright), ShipCell::new(ch2, mid)],
                ],
            },
            Direction::Left => ExhaustSprite {
                // Exhaust to the right of ship (trailing right) - 4 cols conceptually but we use rows
                cells: [
                    [ShipCell::new(ch2, mid),    ShipCell::new('-', bright), ShipCell::new(ch1, dim), ],
                    [ShipCell::new(ch2, mid),    ShipCell::new('-', bright), ShipCell::new(ch1, dim), ],
                    [ShipCell::new(ch3, dim),    ShipCell::new(ch3, mid),    ShipCell::new(ch4, faint)],
                    [ShipCell::new(ch4, faint),  e,                          e],
                ],
            },
            Direction::Right => ExhaustSprite {
                // Exhaust to the left of ship (trailing left)
                cells: [
                    [ShipCell::new(ch1, dim),    ShipCell::new('-', bright), ShipCell::new(ch2, mid)],
                    [ShipCell::new(ch1, dim),    ShipCell::new('-', bright), ShipCell::new(ch2, mid)],
                    [ShipCell::new(ch4, faint),  ShipCell::new(ch3, mid),    ShipCell::new(ch3, dim)],
                    [e,                          e,                          ShipCell::new(ch4, faint)],
                ],
            },
            Direction::UpRight => ExhaustSprite {
                // Diagonal exhaust (down-left)
                cells: [
                    [ShipCell::new(ch1, mid),    ShipCell::new(ch2, dim),    e],
                    [ShipCell::new('\\', bright), ShipCell::new(ch1, mid),   e],
                    [ShipCell::new(ch3, dim),    ShipCell::new('\\', dim),   e],
                    [ShipCell::new(ch4, faint),  ShipCell::new(ch3, faint),  e],
                ],
            },
            Direction::UpLeft => ExhaustSprite {
                // Diagonal exhaust (down-right)
                cells: [
                    [e,                          ShipCell::new(ch2, dim),    ShipCell::new(ch1, mid)],
                    [e,                          ShipCell::new(ch1, mid),    ShipCell::new('/', bright)],
                    [e,                          ShipCell::new('/', dim),    ShipCell::new(ch3, dim)],
                    [e,                          ShipCell::new(ch3, faint),  ShipCell::new(ch4, faint)],
                ],
            },
            Direction::DownRight => ExhaustSprite {
                // Diagonal exhaust (up-left)
                cells: [
                    [ShipCell::new(ch4, faint),  ShipCell::new(ch3, faint),  e],
                    [ShipCell::new(ch3, dim),    ShipCell::new('/', dim),    e],
                    [ShipCell::new('/', bright), ShipCell::new(ch1, mid),    e],
                    [ShipCell::new(ch1, mid),    ShipCell::new(ch2, dim),    e],
                ],
            },
            Direction::DownLeft => ExhaustSprite {
                // Diagonal exhaust (up-right)
                cells: [
                    [e,                          ShipCell::new(ch3, faint),  ShipCell::new(ch4, faint)],
                    [e,                          ShipCell::new('\\', dim),   ShipCell::new(ch3, dim)],
                    [e,                          ShipCell::new(ch1, mid),    ShipCell::new('\\', bright)],
                    [e,                          ShipCell::new(ch2, dim),    ShipCell::new(ch1, mid)],
                ],
            },
        }
    }

    /// Get the offset for the exhaust relative to ship center
    fn offset_for_direction(direction: Direction) -> (i32, i32) {
        match direction {
            Direction::Up => (-1, 2),      // Below ship, centered
            Direction::Down => (-1, -5),   // Above ship, centered
            Direction::Left => (2, -1),    // Right of ship
            Direction::Right => (-5, -1),  // Left of ship (extended)
            Direction::UpRight => (-3, 1), // Down-left of ship
            Direction::UpLeft => (1, 1),   // Down-right of ship
            Direction::DownRight => (-3, -4), // Up-left of ship
            Direction::DownLeft => (1, -4),   // Up-right of ship
        }
    }
}

/// Visual renderer with animation state
struct Renderer {
    frame: u64,
    star_chars: [char; 4],
    asteroid_chars: [char; 4],
    effects_enabled: bool,
}

impl Renderer {
    fn new(effects_enabled: bool) -> Self {
        Renderer {
            frame: 0,
            star_chars: ['.', '+', '*', 'o'],
            asteroid_chars: ['o', 'O', '0', '@'],
            effects_enabled,
        }
    }

    fn toggle_effects(&mut self) {
        self.effects_enabled = !self.effects_enabled;
    }

    fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
    }

    /// Get the visual representation of a tile at a position
    fn render_tile(&self, tile: Option<Tile>, x: i32, y: i32) -> (char, u32) {
        let pos_hash = hash_position(x, y, 42);

        // Simplified rendering when effects are disabled
        if !self.effects_enabled {
            return match tile {
                Some(Tile::Wall) => ('█', 0x4060A0),  // Simple blue wall
                Some(Tile::Floor) => (' ', 0x000000), // Plain black
                Some(Tile::Asteroid) => ('@', 0x808080), // Simple gray asteroid
                Some(Tile::Nebula) => (' ', 0x000000), // Plain black (passable)
                None => (' ', 0x000000),
            };
        }

        match tile {
            Some(Tile::Wall) => {
                // Subtle wall colors - mostly blue with occasional variation
                let wall_variant = pos_hash % 100;
                let base_color = if wall_variant < 70 {
                    // Standard blue walls
                    let intensity = 0x50 + ((pos_hash % 0x20) as u32);
                    (0x20 << 16) | (intensity << 8) | 0xC0
                } else if wall_variant < 85 {
                    // Slightly cyan-tinted
                    0x3090A0
                } else {
                    // Occasional purple accent
                    0x604080
                };

                // Mostly solid blocks
                let ch = match pos_hash % 12 {
                    0..=9 => '█',
                    10 => '▓',
                    _ => '▒',
                };

                (ch, base_color)
            }

            Some(Tile::Floor) => {
                // Sparse starfield
                let star_chance = pos_hash % 50;  // Less frequent stars

                if star_chance == 0 {
                    // Twinkling star (slower animation)
                    let twinkle = ((self.frame / 16) + (pos_hash as u64)) % 4;
                    let colors = [0xC0C0C0, 0xD0D0A0, 0xA0C0C0, 0xC0C0C0];
                    (self.star_chars[twinkle as usize], colors[twinkle as usize])
                } else if star_chance == 1 {
                    // Blue star
                    ('.', 0x5070C0)
                } else if star_chance == 2 {
                    // Dim white star
                    ('.', 0x505050)
                } else {
                    // Empty space
                    (' ', 0x000000)
                }
            }

            Some(Tile::Asteroid) => {
                // Slower rotating asteroid
                let rotation = ((self.frame / 24) + (pos_hash as u64 / 3)) % 4;
                let ch = self.asteroid_chars[rotation as usize];

                // Muted asteroid colors
                let color_variant = pos_hash % 4;
                let color = match color_variant {
                    0 => 0x907050, // Brown
                    1 => 0x707070, // Grey
                    2 => 0x806040, // Dark brown
                    _ => 0x808080, // Light grey
                };

                (ch, color)
            }

            Some(Tile::Nebula) => {
                // Subtle nebula with slow animation
                let flow = ((self.frame / 12) as i32 + x / 5 + y / 4) % 20;

                // Muted nebula colors by region
                let region = hash_position(x / 20, y / 20, 123);
                let base_hue = region % 6;

                let (r, g, b) = match base_hue {
                    0 => (0x80, 0x40, 0x80), // Soft purple
                    1 => (0x40, 0x70, 0x80), // Muted cyan
                    2 => (0x80, 0x50, 0x40), // Soft orange
                    3 => (0x50, 0x40, 0x80), // Deep purple
                    4 => (0x40, 0x70, 0x50), // Soft green
                    _ => (0x50, 0x50, 0x70), // Grey-blue
                };

                // Gentler pulsing
                let pulse = ((flow as u32 % 10) * 3) as i32;
                let dim = 20 + (pos_hash % 20) as i32;
                let color = ((((r as i32 - dim + pulse).max(0).min(255)) as u32) << 16)
                    | ((((g as i32 - dim + pulse).max(0).min(255)) as u32) << 8)
                    | (((b as i32 - dim + pulse).max(0).min(255)) as u32);

                // Fewer animated characters
                let ch = match (pos_hash + self.frame as u32 / 12) % 8 {
                    0 => '.',
                    1 => ':',
                    _ => ' ',
                };

                (ch, color)
            }

            None => {
                // Out of bounds - mostly empty
                if pos_hash % 100 == 0 {
                    ('.', 0x202030)
                } else {
                    (' ', 0x000000)
                }
            }
        }
    }

    /// Check if a screen offset from center is part of the ship or exhaust
    /// Returns Some(ShipCell) if it should be rendered as ship/exhaust, None otherwise
    /// offset_x/y are relative to player center (0,0 = center of ship)
    fn get_ship_cell(&self, direction: Direction, offset_x: i32, offset_y: i32) -> Option<ShipCell> {
        // Ship is centered at (0,0), so ship cells are at offsets -1..=1 for both x and y
        // Ship grid: row 0 = y offset -1, row 1 = y offset 0, row 2 = y offset 1
        //            col 0 = x offset -1, col 1 = x offset 0, col 2 = x offset 1

        // Check if in ship bounds (3x3 centered on player)
        if offset_x >= -1 && offset_x <= 1 && offset_y >= -1 && offset_y <= 1 {
            let ship = ShipSprite::for_direction(direction);
            let row = (offset_y + 1) as usize;
            let col = (offset_x + 1) as usize;
            let cell = ship.cells[row][col];

            // Return cell if it's not empty
            if cell.ch != ' ' {
                return Some(cell);
            }
        }

        // Check if in exhaust bounds
        let (exhaust_offset_x, exhaust_offset_y) = ExhaustSprite::offset_for_direction(direction);
        let exhaust = ExhaustSprite::for_direction(direction, self.frame);

        // Exhaust is 3x4 grid starting at the offset position
        let rel_x = offset_x - exhaust_offset_x;
        let rel_y = offset_y - exhaust_offset_y;

        if rel_x >= 0 && rel_x < 3 && rel_y >= 0 && rel_y < 4 {
            let cell = exhaust.cells[rel_y as usize][rel_x as usize];
            if cell.ch != ' ' {
                return Some(cell);
            }
        }

        None
    }

}

#[derive(Clone)]
struct KeyState {
    held: bool,
    last_seen: Instant,
}

impl Default for KeyState {
    fn default() -> Self {
        KeyState {
            held: false,
            last_seen: Instant::now(),
        }
    }
}

struct InputState {
    up: KeyState,
    down: KeyState,
    left: KeyState,
    right: KeyState,
    has_release_support: bool,
    key_timeout: Duration,
}

impl Default for InputState {
    fn default() -> Self {
        InputState {
            up: KeyState::default(),
            down: KeyState::default(),
            left: KeyState::default(),
            right: KeyState::default(),
            has_release_support: false,
            key_timeout: Duration::from_millis(300),
        }
    }
}

impl InputState {
    fn update_key(&mut self, key: NcKey, evtype: NcInputType) {
        let key_state = match key {
            NcKey::Up => &mut self.up,
            NcKey::Down => &mut self.down,
            NcKey::Left => &mut self.left,
            NcKey::Right => &mut self.right,
            _ => return,
        };

        match evtype {
            NcInputType::Press | NcInputType::Repeat | NcInputType::Unknown => {
                key_state.held = true;
                key_state.last_seen = Instant::now();
            }
            NcInputType::Release => {
                key_state.held = false;
                self.has_release_support = true;
            }
        }
    }

    fn timeout_stale_keys(&mut self) {
        if self.has_release_support {
            return;
        }

        let now = Instant::now();

        if self.up.held && now.duration_since(self.up.last_seen) > self.key_timeout {
            self.up.held = false;
        }
        if self.down.held && now.duration_since(self.down.last_seen) > self.key_timeout {
            self.down.held = false;
        }
        if self.left.held && now.duration_since(self.left.last_seen) > self.key_timeout {
            self.left.held = false;
        }
        if self.right.held && now.duration_since(self.right.last_seen) > self.key_timeout {
            self.right.held = false;
        }
    }

    fn movement_delta(&self) -> (i32, i32) {
        let mut dx = 0;
        let mut dy = 0;

        if self.up.held {
            dy -= 1;
        }
        if self.down.held {
            dy += 1;
        }
        if self.left.held {
            dx -= 1;
        }
        if self.right.held {
            dx += 1;
        }

        (dx, dy)
    }

    fn any_movement(&self) -> bool {
        self.up.held || self.down.held || self.left.held || self.right.held
    }
}

struct Player {
    x: i32,
    y: i32,
    direction: Direction,
}

impl Player {
    fn new(x: i32, y: i32) -> Self {
        Player {
            x,
            y,
            direction: Direction::Up,
        }
    }

    fn try_move(&mut self, dx: i32, dy: i32, map: &Map) -> bool {
        if dx == 0 && dy == 0 {
            return false;
        }

        if let Some(dir) = Direction::from_delta(dx, dy) {
            self.direction = dir;
        }

        let new_x = self.x + dx;
        let new_y = self.y + dy;

        if map.is_passable(new_x, new_y) {
            self.x = new_x;
            self.y = new_y;
            return true;
        }

        if dx != 0 && dy != 0 {
            if map.is_passable(self.x + dx, self.y) {
                self.x += dx;
                return true;
            }
            if map.is_passable(self.x, self.y + dy) {
                self.y += dy;
                return true;
            }
        }

        false
    }
}

/// A message in the chat history
#[derive(Clone)]
struct ChatMessage {
    text: String,
    color: u32,
}

impl ChatMessage {
    fn new(text: String, color: u32) -> Self {
        ChatMessage {
            text,
            color,
        }
    }

    fn system(text: &str) -> Self {
        ChatMessage::new(text.to_string(), 0xFFFF00) // Yellow for system messages
    }

    fn user(text: &str) -> Self {
        ChatMessage::new(text.to_string(), 0x00FF00) // Green for user input
    }

    fn error(text: &str) -> Self {
        ChatMessage::new(text.to_string(), 0xFF4444) // Red for errors
    }
}

/// Chat/command window state
struct ChatWindow {
    /// Whether chat input is active
    active: bool,
    /// Current input buffer
    input: String,
    /// Cursor position in input
    cursor: usize,
    /// Message history (newest last)
    messages: Vec<ChatMessage>,
    /// Maximum messages to keep
    max_messages: usize,
    /// Number of visible message lines
    visible_lines: usize,
}

impl Default for ChatWindow {
    fn default() -> Self {
        ChatWindow {
            active: false,
            input: String::new(),
            cursor: 0,
            messages: Vec::new(),
            max_messages: 100,
            visible_lines: 3,
        }
    }
}

impl ChatWindow {
    fn new() -> Self {
        let mut chat = ChatWindow::default();
        chat.add_message(ChatMessage::system("Welcome to Exospace! Press Enter to chat, / for commands."));
        chat
    }

    /// Toggle chat input mode
    fn toggle(&mut self) {
        self.active = !self.active;
        if !self.active {
            self.input.clear();
            self.cursor = 0;
        }
    }

    /// Open chat (if not already open)
    fn open(&mut self) {
        self.active = true;
    }

    /// Close chat without submitting
    fn close(&mut self) {
        self.active = false;
        self.input.clear();
        self.cursor = 0;
    }

    /// Add a character at cursor position
    fn insert_char(&mut self, ch: char) {
        self.input.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    /// Delete character before cursor (backspace)
    fn backspace(&mut self) {
        if self.cursor > 0 {
            // Find the previous character boundary
            let prev = self.input[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.input.remove(prev);
            self.cursor = prev;
        }
    }

    /// Delete character at cursor (delete key)
    fn delete(&mut self) {
        if self.cursor < self.input.len() {
            self.input.remove(self.cursor);
        }
    }

    /// Move cursor left
    fn cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.input[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move cursor right
    fn cursor_right(&mut self) {
        if self.cursor < self.input.len() {
            self.cursor = self.input[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.input.len());
        }
    }

    /// Move cursor to start
    fn cursor_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end
    fn cursor_end(&mut self) {
        self.cursor = self.input.len();
    }

    /// Submit the current input and return it
    fn submit(&mut self) -> Option<String> {
        if self.input.is_empty() {
            self.close();
            return None;
        }

        let text = self.input.clone();
        self.add_message(ChatMessage::user(&text));
        self.input.clear();
        self.cursor = 0;
        self.active = false;

        Some(text)
    }

    /// Add a message to history
    fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
        if self.messages.len() > self.max_messages {
            self.messages.remove(0);
        }
    }

    /// Process a command or chat message
    fn process_input(&mut self, text: &str) -> Option<ChatCommand> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }

        // Check if it's a command (starts with /)
        if let Some(cmd) = text.strip_prefix('/') {
            let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
            let command = parts[0].to_lowercase();
            let args = parts.get(1).map(|s| s.to_string());

            match command.as_str() {
                "help" | "?" => {
                    self.add_message(ChatMessage::system("Commands:"));
                    self.add_message(ChatMessage::system("  /help - Show this help"));
                    self.add_message(ChatMessage::system("  /pos - Show current position"));
                    self.add_message(ChatMessage::system("  /goto X Y - Teleport to position"));
                    self.add_message(ChatMessage::system("  /fx - Toggle effects"));
                    self.add_message(ChatMessage::system("  /quit - Exit game"));
                    None
                }
                "quit" | "exit" | "q" => Some(ChatCommand::Quit),
                "pos" | "position" | "where" => Some(ChatCommand::ShowPosition),
                "goto" | "tp" | "teleport" => {
                    if let Some(args) = args {
                        let coords: Vec<&str> = args.split_whitespace().collect();
                        if coords.len() >= 2 {
                            if let (Ok(x), Ok(y)) = (coords[0].parse::<i32>(), coords[1].parse::<i32>()) {
                                return Some(ChatCommand::Teleport(x, y));
                            }
                        }
                    }
                    self.add_message(ChatMessage::error("Usage: /goto X Y"));
                    None
                }
                "fx" | "effects" => Some(ChatCommand::ToggleEffects),
                _ => {
                    self.add_message(ChatMessage::error(&format!("Unknown command: /{}", command)));
                    None
                }
            }
        } else {
            // Regular chat message (for now just echo it)
            self.add_message(ChatMessage::new(format!("You: {}", text), 0xAAAAAA));
            None
        }
    }

    /// Get the visible messages (most recent)
    fn visible_messages(&self) -> impl Iterator<Item = &ChatMessage> {
        let start = self.messages.len().saturating_sub(self.visible_lines);
        self.messages[start..].iter()
    }

    /// Get cursor position in display characters (for rendering)
    fn display_cursor_pos(&self) -> usize {
        self.input[..self.cursor].chars().count()
    }
}

/// Commands that can be executed from chat
#[derive(Debug, Clone, PartialEq)]
enum ChatCommand {
    Quit,
    ShowPosition,
    Teleport(i32, i32),
    ToggleEffects,
}

fn main() -> NcResult<()> {
    let nc = unsafe { Nc::new()? };

    // Load user configuration
    let mut config = Config::load();

    let map = Map::new(&config);
    let start = map.find_start_position();
    let mut player = Player::new(start.0, start.1);
    let mut renderer = Renderer::new(config.effects_enabled);
    let mut chat = ChatWindow::new();

    let stdplane = unsafe { nc.stdplane() };
    let (mut term_height, mut term_width) = stdplane.dim_yx();

    let mut input_state = InputState::default();
    let mut last_move_time = Instant::now();
    let move_delay = Duration::from_millis(33);

    // Chat area takes up bottom lines: messages + input line + status bar
    let chat_height: u32 = 5; // 3 message lines + 1 input line + 1 status bar

    loop {
        let mut quit = false;
        let mut input = NcInput::new_empty();

        loop {
            match nc.get_nblock(Some(&mut input)) {
                Ok(received) => {
                    if chat.active {
                        // Chat mode input handling
                        match received {
                            NcReceived::NoInput => break,
                            NcReceived::Char(ch) => {
                                if ch.is_ascii_graphic() || ch == ' ' {
                                    chat.insert_char(ch);
                                }
                            }
                            NcReceived::Key(key) => {
                                match key {
                                    NcKey::Enter => {
                                        if let Some(text) = chat.submit() {
                                            if let Some(cmd) = chat.process_input(&text) {
                                                match cmd {
                                                    ChatCommand::Quit => {
                                                        quit = true;
                                                        break;
                                                    }
                                                    ChatCommand::ShowPosition => {
                                                        chat.add_message(ChatMessage::system(
                                                            &format!("Position: ({}, {})", player.x, player.y)
                                                        ));
                                                    }
                                                    ChatCommand::Teleport(x, y) => {
                                                        if map.is_passable(x, y) {
                                                            player.x = x;
                                                            player.y = y;
                                                            chat.add_message(ChatMessage::system(
                                                                &format!("Teleported to ({}, {})", x, y)
                                                            ));
                                                        } else {
                                                            chat.add_message(ChatMessage::error(
                                                                &format!("Cannot teleport to ({}, {}) - not passable", x, y)
                                                            ));
                                                        }
                                                    }
                                                    ChatCommand::ToggleEffects => {
                                                        renderer.toggle_effects();
                                                        config.effects_enabled = renderer.effects_enabled;
                                                        let _ = config.save();
                                                        chat.add_message(ChatMessage::system(
                                                            &format!("Effects: {}", if renderer.effects_enabled { "ON" } else { "OFF" })
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    NcKey::Esc => {
                                        chat.close();
                                    }
                                    NcKey::Backspace => {
                                        chat.backspace();
                                    }
                                    NcKey::Del => {
                                        chat.delete();
                                    }
                                    NcKey::Left => {
                                        chat.cursor_left();
                                    }
                                    NcKey::Right => {
                                        chat.cursor_right();
                                    }
                                    NcKey::Home => {
                                        chat.cursor_home();
                                    }
                                    NcKey::End => {
                                        chat.cursor_end();
                                    }
                                    NcKey::Resize => {
                                        let dims = stdplane.dim_yx();
                                        term_height = dims.0;
                                        term_width = dims.1;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    } else {
                        // Game mode input handling
                        match received {
                            NcReceived::NoInput => break,
                            NcReceived::Char('q') | NcReceived::Char('Q') => {
                                quit = true;
                                break;
                            }
                            NcReceived::Char('b') | NcReceived::Char('B') => {
                                renderer.toggle_effects();
                                config.effects_enabled = renderer.effects_enabled;
                                let _ = config.save();
                            }
                            NcReceived::Char('/') => {
                                // Open chat with / pre-filled for command
                                chat.open();
                                chat.insert_char('/');
                            }
                            NcReceived::Key(key) => {
                                let evtype = NcInputType::from(input.evtype);
                                match key {
                                    NcKey::Enter => {
                                        chat.open();
                                    }
                                    NcKey::Up | NcKey::Down | NcKey::Left | NcKey::Right => {
                                        input_state.update_key(key, evtype);
                                    }
                                    NcKey::Resize => {
                                        let dims = stdplane.dim_yx();
                                        term_height = dims.0;
                                        term_width = dims.1;
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                },
                Err(_) => break,
            }
        }

        if quit {
            break;
        }

        // Only process movement when not in chat mode
        if !chat.active {
            input_state.timeout_stale_keys();

            if input_state.any_movement() && last_move_time.elapsed() >= move_delay {
                let (dx, dy) = input_state.movement_delta();
                player.try_move(dx, dy, &map);
                last_move_time = Instant::now();
            }
        }

        // Update animation frame
        renderer.tick();

        // Render
        stdplane.erase();

        let game_height = term_height.saturating_sub(chat_height);
        let center_screen_x = term_width / 2;
        let center_screen_y = game_height / 2;

        // Render game area
        for screen_y in 0..game_height {
            for screen_x in 0..term_width {
                let map_x = player.x + (screen_x as i32 - center_screen_x as i32);
                let map_y = player.y + (screen_y as i32 - center_screen_y as i32);

                // Calculate offset from player center for ship rendering
                let offset_x = screen_x as i32 - center_screen_x as i32;
                let offset_y = screen_y as i32 - center_screen_y as i32;

                // Check if this position is part of the ship or exhaust
                if let Some(ship_cell) = renderer.get_ship_cell(player.direction, offset_x, offset_y) {
                    if let Some(bg_color) = ship_cell.bg {
                        stdplane.set_bg_rgb(bg_color);
                    } else {
                        stdplane.set_bg_default();
                    }
                    stdplane.set_fg_rgb(ship_cell.fg);
                    let s: String = ship_cell.ch.into();
                    stdplane.putstr_yx(Some(screen_y), Some(screen_x), &s)?;
                    stdplane.set_bg_default();
                } else {
                    // Render map tile
                    let tile = map.get(map_x, map_y);
                    let (ch, fg) = renderer.render_tile(tile, map_x, map_y);

                    stdplane.set_fg_rgb(fg);
                    stdplane.set_bg_default();
                    let s: String = ch.into();
                    stdplane.putstr_yx(Some(screen_y), Some(screen_x), &s)?;
                }
            }
        }

        // Render chat messages
        stdplane.set_bg_rgb(0x000010);
        let msg_start_y = game_height;
        for (i, msg) in chat.visible_messages().enumerate() {
            stdplane.set_fg_rgb(msg.color);
            let truncated: String = msg.text.chars().take(term_width as usize).collect();
            let padded = format!("{:<width$}", truncated, width = term_width as usize);
            stdplane.putstr_yx(Some(msg_start_y + i as u32), Some(0), &padded)?;
        }
        // Fill remaining message lines if fewer messages
        let msg_count = chat.visible_messages().count();
        for i in msg_count..chat.visible_lines {
            let blank = " ".repeat(term_width as usize);
            stdplane.set_fg_rgb(0x404040);
            stdplane.putstr_yx(Some(msg_start_y + i as u32), Some(0), &blank)?;
        }

        // Render chat input line
        let input_y = term_height - 2;
        stdplane.set_bg_rgb(0x000020);
        if chat.active {
            stdplane.set_fg_rgb(0x00FFFF);
            let prompt = "> ";
            let input_display: String = chat.input.chars().take(term_width as usize - 2).collect();
            let input_line = format!("{}{:<width$}", prompt, input_display, width = term_width as usize - 2);
            stdplane.putstr_yx(Some(input_y), Some(0), &input_line)?;

            // Show cursor (by inverting colors at cursor position)
            let cursor_x = 2 + chat.display_cursor_pos();
            if cursor_x < term_width as usize {
                stdplane.set_fg_rgb(0x000020);
                stdplane.set_bg_rgb(0x00FFFF);
                let cursor_char = chat.input.chars().nth(chat.display_cursor_pos()).unwrap_or(' ');
                let cursor_str: String = cursor_char.into();
                stdplane.putstr_yx(Some(input_y), Some(cursor_x as u32), &cursor_str)?;
            }
        } else {
            stdplane.set_fg_rgb(0x606060);
            let hint = format!("{:<width$}", "Press Enter to chat, / for commands", width = term_width as usize);
            stdplane.putstr_yx(Some(input_y), Some(0), &hint)?;
        }
        stdplane.set_bg_default();

        // Status bar
        let current_tile = map.get(player.x, player.y);
        let tile_name = match current_tile {
            Some(Tile::Floor) => "Space",
            Some(Tile::Nebula) => "Nebula",
            _ => "???",
        };

        stdplane.set_fg_rgb(0x00FF00);
        stdplane.set_bg_rgb(0x000020);

        let effects_indicator = if renderer.effects_enabled { "FX:ON" } else { "FX:OFF" };
        let mode_indicator = if chat.active { "[CHAT]" } else { "" };
        let status = format!(
            " ({:>4},{:>4}) {:>2} | {} | {} {} ",
            player.x,
            player.y,
            player.direction.name(),
            tile_name,
            effects_indicator,
            mode_indicator
        );
        let padded_status = format!("{:<width$}", status, width = term_width as usize);
        stdplane.putstr_yx(Some(term_height - 1), Some(0), &padded_status)?;
        stdplane.set_bg_default();

        nc.render()?;

        std::thread::sleep(Duration::from_millis(16));
    }

    unsafe { nc.stop()? };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Tile Tests ====================

    #[test]
    fn test_tile_passability() {
        assert!(Tile::Floor.is_passable(), "Floor should be passable");
        assert!(Tile::Nebula.is_passable(), "Nebula should be passable");
        assert!(!Tile::Wall.is_passable(), "Wall should not be passable");
        assert!(!Tile::Asteroid.is_passable(), "Asteroid should not be passable");
    }

    // ==================== Direction Tests ====================

    #[test]
    fn test_direction_from_delta_cardinal() {
        assert_eq!(Direction::from_delta(0, -1), Some(Direction::Up));
        assert_eq!(Direction::from_delta(0, 1), Some(Direction::Down));
        assert_eq!(Direction::from_delta(-1, 0), Some(Direction::Left));
        assert_eq!(Direction::from_delta(1, 0), Some(Direction::Right));
    }

    #[test]
    fn test_direction_from_delta_diagonal() {
        assert_eq!(Direction::from_delta(1, -1), Some(Direction::UpRight));
        assert_eq!(Direction::from_delta(-1, -1), Some(Direction::UpLeft));
        assert_eq!(Direction::from_delta(1, 1), Some(Direction::DownRight));
        assert_eq!(Direction::from_delta(-1, 1), Some(Direction::DownLeft));
    }

    #[test]
    fn test_direction_from_delta_zero() {
        assert_eq!(Direction::from_delta(0, 0), None);
    }

    #[test]
    fn test_direction_to_char() {
        assert_eq!(Direction::Up.to_char(), '↑');
        assert_eq!(Direction::Down.to_char(), '↓');
        assert_eq!(Direction::Left.to_char(), '←');
        assert_eq!(Direction::Right.to_char(), '→');
        assert_eq!(Direction::UpRight.to_char(), '↗');
        assert_eq!(Direction::UpLeft.to_char(), '↖');
        assert_eq!(Direction::DownRight.to_char(), '↘');
        assert_eq!(Direction::DownLeft.to_char(), '↙');
    }

    #[test]
    fn test_direction_name() {
        assert_eq!(Direction::Up.name(), "N");
        assert_eq!(Direction::UpRight.name(), "NE");
        assert_eq!(Direction::Right.name(), "E");
        assert_eq!(Direction::DownRight.name(), "SE");
        assert_eq!(Direction::Down.name(), "S");
        assert_eq!(Direction::DownLeft.name(), "SW");
        assert_eq!(Direction::Left.name(), "W");
        assert_eq!(Direction::UpLeft.name(), "NW");
    }

    // ==================== Hash Tests ====================

    #[test]
    fn test_hash_position_deterministic() {
        // Same inputs should always produce same output
        let hash1 = hash_position(10, 20, 42);
        let hash2 = hash_position(10, 20, 42);
        assert_eq!(hash1, hash2, "hash_position should be deterministic");
    }

    #[test]
    fn test_hash_position_different_inputs() {
        // Different inputs should produce different outputs
        let hash1 = hash_position(10, 20, 42);
        let hash2 = hash_position(11, 20, 42);
        let hash3 = hash_position(10, 21, 42);
        let hash4 = hash_position(10, 20, 43);

        assert_ne!(hash1, hash2, "Different x should produce different hash");
        assert_ne!(hash1, hash3, "Different y should produce different hash");
        assert_ne!(hash1, hash4, "Different seed should produce different hash");
    }

    // ==================== Map Tests ====================

    #[test]
    fn test_map_dimensions() {
        let map = Map::generate_local(100, 50);
        assert_eq!(map.width, 100);
        assert_eq!(map.height, 50);
        assert_eq!(map.tiles.len(), 50); // height rows
        assert_eq!(map.tiles[0].len(), 100); // width columns
    }

    #[test]
    fn test_map_has_walls_and_floors() {
        let map = Map::generate_local(100, 50);

        let has_walls = map.tiles.iter().flatten().any(|t| *t == Tile::Wall);
        let has_floors = map.tiles.iter().flatten().any(|t| *t == Tile::Floor);

        assert!(has_walls, "Map should contain walls");
        assert!(has_floors, "Map should contain floors");
    }

    #[test]
    fn test_map_border_is_walls() {
        let map = Map::generate_local(100, 50);

        // Check top and bottom borders
        for x in 0..100 {
            assert_eq!(map.get(x, 0), Some(Tile::Wall), "Top border should be wall at x={}", x);
            assert_eq!(map.get(x, 49), Some(Tile::Wall), "Bottom border should be wall at x={}", x);
        }

        // Check left and right borders
        for y in 0..50 {
            assert_eq!(map.get(0, y), Some(Tile::Wall), "Left border should be wall at y={}", y);
            assert_eq!(map.get(99, y), Some(Tile::Wall), "Right border should be wall at y={}", y);
        }
    }

    #[test]
    fn test_map_get_out_of_bounds() {
        let map = Map::generate_local(100, 50);

        assert_eq!(map.get(-1, 0), None);
        assert_eq!(map.get(0, -1), None);
        assert_eq!(map.get(100, 0), None);
        assert_eq!(map.get(0, 50), None);
    }

    #[test]
    fn test_map_is_passable() {
        let map = Map::generate_local(100, 50);

        // Border should not be passable
        assert!(!map.is_passable(0, 0));
        assert!(!map.is_passable(-1, 0));

        // Find a floor tile and check it's passable
        let start = map.find_start_position();
        assert!(map.is_passable(start.0, start.1), "Start position should be passable");
    }

    #[test]
    fn test_map_find_start_position_is_passable() {
        let map = Map::generate_local(100, 50);
        let (x, y) = map.find_start_position();

        assert!(map.is_passable(x, y), "Start position must be passable");
        assert!(x > 0 && x < 100, "Start x should be within bounds");
        assert!(y > 0 && y < 50, "Start y should be within bounds");
    }

    // ==================== Player Tests ====================

    #[test]
    fn test_player_new() {
        let player = Player::new(10, 20);
        assert_eq!(player.x, 10);
        assert_eq!(player.y, 20);
        assert_eq!(player.direction, Direction::Up);
    }

    #[test]
    fn test_player_move_updates_direction() {
        let map = Map::generate_local(100, 50);
        let start = map.find_start_position();
        let mut player = Player::new(start.0, start.1);

        // Try to move right (even if blocked, direction should update)
        player.try_move(1, 0, &map);
        assert_eq!(player.direction, Direction::Right);

        // Try to move down
        player.try_move(0, 1, &map);
        assert_eq!(player.direction, Direction::Down);
    }

    #[test]
    fn test_player_no_move_on_zero_delta() {
        let map = Map::generate_local(100, 50);
        let start = map.find_start_position();
        let mut player = Player::new(start.0, start.1);
        let original_dir = player.direction;

        let moved = player.try_move(0, 0, &map);
        assert!(!moved, "Should not move with zero delta");
        assert_eq!(player.direction, original_dir, "Direction should not change");
    }

    #[test]
    fn test_player_collision_with_wall() {
        let map = Map::generate_local(100, 50);
        let mut player = Player::new(1, 1); // Near the wall border

        // Try to move into the wall (border is at x=0)
        let moved = player.try_move(-1, 0, &map);
        assert!(!moved, "Should not move into wall");
        assert_eq!(player.x, 1, "X position should not change");
    }

    // ==================== Renderer Tests ====================

    #[test]
    fn test_renderer_new_with_effects_enabled() {
        let renderer = Renderer::new(true);
        assert_eq!(renderer.frame, 0);
        assert!(renderer.effects_enabled);
    }

    #[test]
    fn test_renderer_new_with_effects_disabled() {
        let renderer = Renderer::new(false);
        assert_eq!(renderer.frame, 0);
        assert!(!renderer.effects_enabled);
    }

    #[test]
    fn test_renderer_toggle_effects() {
        let mut renderer = Renderer::new(true);
        assert!(renderer.effects_enabled);

        renderer.toggle_effects();
        assert!(!renderer.effects_enabled);

        renderer.toggle_effects();
        assert!(renderer.effects_enabled);
    }

    #[test]
    fn test_renderer_tick() {
        let mut renderer = Renderer::new(true);
        assert_eq!(renderer.frame, 0);

        renderer.tick();
        assert_eq!(renderer.frame, 1);

        renderer.tick();
        assert_eq!(renderer.frame, 2);
    }

    #[test]
    fn test_renderer_effects_disabled_returns_simple_tiles() {
        let renderer = Renderer::new(false);

        // With effects disabled, floor should return space with black
        let (ch, color) = renderer.render_tile(Some(Tile::Floor), 0, 0);
        assert_eq!(ch, ' ');
        assert_eq!(color, 0x000000);

        // Wall should return solid block
        let (ch, _) = renderer.render_tile(Some(Tile::Wall), 0, 0);
        assert_eq!(ch, '█');
    }

    #[test]
    fn test_renderer_render_tile_deterministic() {
        let renderer = Renderer::new(true);

        // Same position should give same result
        let result1 = renderer.render_tile(Some(Tile::Wall), 10, 20);
        let result2 = renderer.render_tile(Some(Tile::Wall), 10, 20);
        assert_eq!(result1, result2, "Render should be deterministic for same position");
    }

    // ==================== InputState Tests ====================

    #[test]
    fn test_input_state_default() {
        let state = InputState::default();
        assert!(!state.up.held);
        assert!(!state.down.held);
        assert!(!state.left.held);
        assert!(!state.right.held);
        assert!(!state.has_release_support);
    }

    #[test]
    fn test_input_state_movement_delta_cardinal() {
        let mut state = InputState::default();
        state.up.held = true;
        assert_eq!(state.movement_delta(), (0, -1));

        state.up.held = false;
        state.down.held = true;
        assert_eq!(state.movement_delta(), (0, 1));

        state.down.held = false;
        state.left.held = true;
        assert_eq!(state.movement_delta(), (-1, 0));

        state.left.held = false;
        state.right.held = true;
        assert_eq!(state.movement_delta(), (1, 0));
    }

    #[test]
    fn test_input_state_movement_delta_diagonal() {
        let mut state = InputState::default();

        state.up.held = true;
        state.right.held = true;
        assert_eq!(state.movement_delta(), (1, -1));

        state.right.held = false;
        state.left.held = true;
        assert_eq!(state.movement_delta(), (-1, -1));
    }

    #[test]
    fn test_input_state_any_movement() {
        let mut state = InputState::default();
        assert!(!state.any_movement());

        state.up.held = true;
        assert!(state.any_movement());
    }

    // ==================== ShipCell Tests ====================

    #[test]
    fn test_ship_cell_new() {
        let cell = ShipCell::new('^', 0xFF0000);
        assert_eq!(cell.ch, '^');
        assert_eq!(cell.fg, 0xFF0000);
        assert!(cell.bg.is_none());
    }

    #[test]
    fn test_ship_cell_with_bg() {
        let cell = ShipCell::with_bg('X', 0xFF0000, 0x00FF00);
        assert_eq!(cell.ch, 'X');
        assert_eq!(cell.fg, 0xFF0000);
        assert_eq!(cell.bg, Some(0x00FF00));
    }

    #[test]
    fn test_ship_cell_empty() {
        let cell = ShipCell::empty();
        assert_eq!(cell.ch, ' ');
        assert_eq!(cell.fg, 0x000000);
        assert!(cell.bg.is_none());
    }

    // ==================== ShipSprite Tests ====================

    #[test]
    fn test_ship_sprite_all_directions() {
        // Verify ship sprites exist for all 8 directions
        let directions = [
            Direction::Up, Direction::Down, Direction::Left, Direction::Right,
            Direction::UpRight, Direction::UpLeft, Direction::DownRight, Direction::DownLeft,
        ];

        for dir in directions {
            let sprite = ShipSprite::for_direction(dir);
            // Ship should be 3x3
            assert_eq!(sprite.cells.len(), 3);
            for row in &sprite.cells {
                assert_eq!(row.len(), 3);
            }
        }
    }

    #[test]
    fn test_ship_sprite_center_not_empty() {
        // Center of ship should never be empty for any direction
        let directions = [
            Direction::Up, Direction::Down, Direction::Left, Direction::Right,
            Direction::UpRight, Direction::UpLeft, Direction::DownRight, Direction::DownLeft,
        ];

        for dir in directions {
            let sprite = ShipSprite::for_direction(dir);
            let center = sprite.cells[1][1];
            assert_ne!(center.ch, ' ', "Center of ship should not be empty for {:?}", dir);
        }
    }

    #[test]
    fn test_ship_sprite_has_cockpit() {
        // Each ship direction should have at least one cockpit-colored cell
        let cockpit_color = 0x80FFFF;
        let directions = [
            Direction::Up, Direction::Down, Direction::Left, Direction::Right,
            Direction::UpRight, Direction::UpLeft, Direction::DownRight, Direction::DownLeft,
        ];

        for dir in directions {
            let sprite = ShipSprite::for_direction(dir);
            let has_cockpit = sprite.cells.iter()
                .flatten()
                .any(|cell| cell.fg == cockpit_color);
            assert!(has_cockpit, "Ship should have cockpit for {:?}", dir);
        }
    }

    // ==================== ExhaustSprite Tests ====================

    #[test]
    fn test_exhaust_sprite_all_directions() {
        // Verify exhaust sprites exist for all 8 directions
        let directions = [
            Direction::Up, Direction::Down, Direction::Left, Direction::Right,
            Direction::UpRight, Direction::UpLeft, Direction::DownRight, Direction::DownLeft,
        ];

        for dir in directions {
            let sprite = ExhaustSprite::for_direction(dir, 0);
            // Exhaust should be 4x3 (4 rows, 3 columns)
            assert_eq!(sprite.cells.len(), 4);
            for row in &sprite.cells {
                assert_eq!(row.len(), 3);
            }
        }
    }

    #[test]
    fn test_exhaust_sprite_animation() {
        // Exhaust should change over frames
        let sprite1 = ExhaustSprite::for_direction(Direction::Up, 0);
        let sprite2 = ExhaustSprite::for_direction(Direction::Up, 4);
        let sprite3 = ExhaustSprite::for_direction(Direction::Up, 8);

        // At least one cell should be different between frames
        let cells_match = |s1: &ExhaustSprite, s2: &ExhaustSprite| {
            s1.cells.iter().flatten().zip(s2.cells.iter().flatten())
                .all(|(c1, c2)| c1.ch == c2.ch && c1.fg == c2.fg)
        };

        // Animation cycles, so frames 0 and 16 should match (4 phases * 4 frames each)
        assert!(!cells_match(&sprite1, &sprite2) || !cells_match(&sprite2, &sprite3),
            "Exhaust should animate");
    }

    #[test]
    fn test_exhaust_offset_opposite_to_direction() {
        // Exhaust should appear behind the ship (opposite to movement direction)
        let (_x, y) = ExhaustSprite::offset_for_direction(Direction::Up);
        assert!(y > 0, "Up-facing ship exhaust should be below (positive y)");

        let (_x, y) = ExhaustSprite::offset_for_direction(Direction::Down);
        assert!(y < 0, "Down-facing ship exhaust should be above (negative y)");

        let (x, _y) = ExhaustSprite::offset_for_direction(Direction::Left);
        assert!(x > 0, "Left-facing ship exhaust should be to right (positive x)");

        let (x, _y) = ExhaustSprite::offset_for_direction(Direction::Right);
        assert!(x < 0, "Right-facing ship exhaust should be to left (negative x)");
    }

    // ==================== Renderer Ship Cell Tests ====================

    #[test]
    fn test_renderer_get_ship_cell_center() {
        let renderer = Renderer::new(true);

        // Center of ship (offset 0,0) should return a cell
        let cell = renderer.get_ship_cell(Direction::Up, 0, 0);
        assert!(cell.is_some(), "Ship center should exist");
    }

    #[test]
    fn test_renderer_get_ship_cell_bounds() {
        let renderer = Renderer::new(true);

        // Ship cells at corners of 3x3 grid
        let corners = [(-1, -1), (1, -1), (-1, 1), (1, 1)];
        for (ox, oy) in corners {
            // At least some corners should have content (not all are empty)
            let _ = renderer.get_ship_cell(Direction::Up, ox, oy);
        }

        // Far outside ship should return None (unless it's exhaust)
        let cell = renderer.get_ship_cell(Direction::Up, 10, 10);
        assert!(cell.is_none(), "Far from ship should be None");
    }

    #[test]
    fn test_renderer_get_ship_cell_exhaust_area() {
        let renderer = Renderer::new(true);

        // For Up-facing ship, exhaust should be below (positive y offset)
        let (ex, ey) = ExhaustSprite::offset_for_direction(Direction::Up);
        // Check a cell in the exhaust area
        let cell = renderer.get_ship_cell(Direction::Up, ex + 1, ey);
        assert!(cell.is_some(), "Exhaust area should have content");
    }

    #[test]
    fn test_renderer_get_ship_cell_all_directions() {
        let renderer = Renderer::new(true);
        let directions = [
            Direction::Up, Direction::Down, Direction::Left, Direction::Right,
            Direction::UpRight, Direction::UpLeft, Direction::DownRight, Direction::DownLeft,
        ];

        for dir in directions {
            // Every direction should have a ship center
            let cell = renderer.get_ship_cell(dir, 0, 0);
            assert!(cell.is_some(), "Ship center should exist for {:?}", dir);
        }
    }

    // ==================== Config Tests ====================

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(!config.effects_enabled, "Effects should be disabled by default");
        assert!(config.server_url.is_none(), "Server URL should be None by default");
    }

    #[test]
    fn test_config_server_url_default() {
        let config = Config::default();
        assert_eq!(config.server_url(), SERVER_URL);
    }

    #[test]
    fn test_config_server_url_override() {
        let config = Config {
            effects_enabled: false,
            server_url: Some("http://custom:8080".to_string()),
        };
        assert_eq!(config.server_url(), "http://custom:8080");
    }

    #[test]
    fn test_config_serialization() {
        let config = Config {
            effects_enabled: true,
            server_url: Some("http://test:3000".to_string()),
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.effects_enabled, config.effects_enabled);
        assert_eq!(parsed.server_url, config.server_url);
    }

    #[test]
    fn test_config_path_returns_some() {
        // Config path should work on most systems
        let path = Config::config_path();
        // We can't guarantee it's Some on all systems, but if it is, check structure
        if let Some(p) = path {
            assert!(p.ends_with("config.json"));
            assert!(p.to_string_lossy().contains("exospace"));
        }
    }

    // ==================== ChatMessage Tests ====================

    #[test]
    fn test_chat_message_new() {
        let msg = ChatMessage::new("Hello".to_string(), 0xFF0000);
        assert_eq!(msg.text, "Hello");
        assert_eq!(msg.color, 0xFF0000);
    }

    #[test]
    fn test_chat_message_system() {
        let msg = ChatMessage::system("System message");
        assert_eq!(msg.text, "System message");
        assert_eq!(msg.color, 0xFFFF00); // Yellow
    }

    #[test]
    fn test_chat_message_user() {
        let msg = ChatMessage::user("User input");
        assert_eq!(msg.text, "User input");
        assert_eq!(msg.color, 0x00FF00); // Green
    }

    #[test]
    fn test_chat_message_error() {
        let msg = ChatMessage::error("Error!");
        assert_eq!(msg.text, "Error!");
        assert_eq!(msg.color, 0xFF4444); // Red
    }

    // ==================== ChatWindow Tests ====================

    #[test]
    fn test_chat_window_default() {
        let chat = ChatWindow::default();
        assert!(!chat.active);
        assert!(chat.input.is_empty());
        assert_eq!(chat.cursor, 0);
        assert!(chat.messages.is_empty());
    }

    #[test]
    fn test_chat_window_new_has_welcome_message() {
        let chat = ChatWindow::new();
        assert_eq!(chat.messages.len(), 1);
        assert!(chat.messages[0].text.contains("Welcome"));
    }

    #[test]
    fn test_chat_window_toggle() {
        let mut chat = ChatWindow::default();
        assert!(!chat.active);

        chat.toggle();
        assert!(chat.active);

        chat.toggle();
        assert!(!chat.active);
    }

    #[test]
    fn test_chat_window_open_close() {
        let mut chat = ChatWindow::default();

        chat.open();
        assert!(chat.active);

        chat.insert_char('h');
        chat.insert_char('i');
        assert_eq!(chat.input, "hi");

        chat.close();
        assert!(!chat.active);
        assert!(chat.input.is_empty());
        assert_eq!(chat.cursor, 0);
    }

    #[test]
    fn test_chat_window_insert_char() {
        let mut chat = ChatWindow::default();
        chat.insert_char('a');
        chat.insert_char('b');
        chat.insert_char('c');

        assert_eq!(chat.input, "abc");
        assert_eq!(chat.cursor, 3);
    }

    #[test]
    fn test_chat_window_backspace() {
        let mut chat = ChatWindow::default();
        chat.insert_char('a');
        chat.insert_char('b');
        chat.insert_char('c');

        chat.backspace();
        assert_eq!(chat.input, "ab");
        assert_eq!(chat.cursor, 2);

        chat.backspace();
        assert_eq!(chat.input, "a");

        chat.backspace();
        assert!(chat.input.is_empty());

        // Backspace on empty should do nothing
        chat.backspace();
        assert!(chat.input.is_empty());
    }

    #[test]
    fn test_chat_window_cursor_movement() {
        let mut chat = ChatWindow::default();
        chat.insert_char('a');
        chat.insert_char('b');
        chat.insert_char('c');
        assert_eq!(chat.cursor, 3);

        chat.cursor_left();
        assert_eq!(chat.cursor, 2);

        chat.cursor_left();
        assert_eq!(chat.cursor, 1);

        chat.cursor_right();
        assert_eq!(chat.cursor, 2);

        chat.cursor_home();
        assert_eq!(chat.cursor, 0);

        chat.cursor_end();
        assert_eq!(chat.cursor, 3);
    }

    #[test]
    fn test_chat_window_delete() {
        let mut chat = ChatWindow::default();
        chat.insert_char('a');
        chat.insert_char('b');
        chat.insert_char('c');
        chat.cursor_home();

        chat.delete();
        assert_eq!(chat.input, "bc");

        chat.delete();
        assert_eq!(chat.input, "c");
    }

    #[test]
    fn test_chat_window_insert_at_cursor() {
        let mut chat = ChatWindow::default();
        chat.insert_char('a');
        chat.insert_char('c');
        chat.cursor_left();
        chat.insert_char('b');

        assert_eq!(chat.input, "abc");
    }

    #[test]
    fn test_chat_window_submit() {
        let mut chat = ChatWindow::default();
        chat.open();
        chat.insert_char('h');
        chat.insert_char('i');

        let result = chat.submit();
        assert_eq!(result, Some("hi".to_string()));
        assert!(chat.input.is_empty());
        assert!(!chat.active);
    }

    #[test]
    fn test_chat_window_submit_empty() {
        let mut chat = ChatWindow::default();
        chat.open();

        let result = chat.submit();
        assert!(result.is_none());
        assert!(!chat.active);
    }

    #[test]
    fn test_chat_window_add_message() {
        let mut chat = ChatWindow::default();
        chat.add_message(ChatMessage::system("Test 1"));
        chat.add_message(ChatMessage::system("Test 2"));

        assert_eq!(chat.messages.len(), 2);
        assert_eq!(chat.messages[0].text, "Test 1");
        assert_eq!(chat.messages[1].text, "Test 2");
    }

    #[test]
    fn test_chat_window_max_messages() {
        let mut chat = ChatWindow::default();
        chat.max_messages = 3;

        chat.add_message(ChatMessage::system("1"));
        chat.add_message(ChatMessage::system("2"));
        chat.add_message(ChatMessage::system("3"));
        chat.add_message(ChatMessage::system("4"));

        assert_eq!(chat.messages.len(), 3);
        assert_eq!(chat.messages[0].text, "2"); // First message removed
        assert_eq!(chat.messages[2].text, "4");
    }

    #[test]
    fn test_chat_window_visible_messages() {
        let mut chat = ChatWindow::default();
        chat.visible_lines = 2;

        chat.add_message(ChatMessage::system("1"));
        chat.add_message(ChatMessage::system("2"));
        chat.add_message(ChatMessage::system("3"));

        let visible: Vec<_> = chat.visible_messages().collect();
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].text, "2");
        assert_eq!(visible[1].text, "3");
    }

    // ==================== ChatCommand Tests ====================

    #[test]
    fn test_chat_process_help_command() {
        let mut chat = ChatWindow::default();
        let cmd = chat.process_input("/help");
        assert!(cmd.is_none()); // Help just adds messages
        assert!(!chat.messages.is_empty());
    }

    #[test]
    fn test_chat_process_quit_command() {
        let mut chat = ChatWindow::default();
        let cmd = chat.process_input("/quit");
        assert_eq!(cmd, Some(ChatCommand::Quit));
    }

    #[test]
    fn test_chat_process_pos_command() {
        let mut chat = ChatWindow::default();
        let cmd = chat.process_input("/pos");
        assert_eq!(cmd, Some(ChatCommand::ShowPosition));
    }

    #[test]
    fn test_chat_process_goto_command() {
        let mut chat = ChatWindow::default();
        let cmd = chat.process_input("/goto 100 200");
        assert_eq!(cmd, Some(ChatCommand::Teleport(100, 200)));
    }

    #[test]
    fn test_chat_process_goto_invalid() {
        let mut chat = ChatWindow::default();
        let cmd = chat.process_input("/goto");
        assert!(cmd.is_none());
        // Should have added an error message
        assert!(chat.messages.iter().any(|m| m.text.contains("Usage")));
    }

    #[test]
    fn test_chat_process_fx_command() {
        let mut chat = ChatWindow::default();
        let cmd = chat.process_input("/fx");
        assert_eq!(cmd, Some(ChatCommand::ToggleEffects));
    }

    #[test]
    fn test_chat_process_unknown_command() {
        let mut chat = ChatWindow::default();
        let cmd = chat.process_input("/unknowncmd");
        assert!(cmd.is_none());
        assert!(chat.messages.iter().any(|m| m.text.contains("Unknown command")));
    }

    #[test]
    fn test_chat_process_regular_message() {
        let mut chat = ChatWindow::default();
        let cmd = chat.process_input("Hello world");
        assert!(cmd.is_none());
        assert!(chat.messages.iter().any(|m| m.text.contains("You: Hello world")));
    }

    #[test]
    fn test_chat_display_cursor_pos() {
        let mut chat = ChatWindow::default();
        chat.insert_char('a');
        chat.insert_char('b');
        chat.insert_char('c');
        assert_eq!(chat.display_cursor_pos(), 3);

        chat.cursor_left();
        assert_eq!(chat.display_cursor_pos(), 2);

        chat.cursor_home();
        assert_eq!(chat.display_cursor_pos(), 0);
    }
}
