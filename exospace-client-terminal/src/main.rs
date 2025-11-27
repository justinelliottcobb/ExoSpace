use libnotcurses_sys::*;
use std::time::{Duration, Instant};

/// Tile types in the map
#[derive(Clone, Copy, PartialEq, Debug)]
enum Tile {
    Wall,
    Floor,
    Asteroid,
    Nebula,
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
}

impl Map {
    fn generate(width: usize, height: usize) -> Self {
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

        Map { tiles, width, height }
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

/// Visual renderer with animation state
struct Renderer {
    frame: u64,
    star_chars: [char; 4],
    asteroid_chars: [char; 4],
    effects_enabled: bool,
}

impl Renderer {
    fn new() -> Self {
        Renderer {
            frame: 0,
            star_chars: ['.', '+', '*', 'o'],
            asteroid_chars: ['o', 'O', '0', '@'],
            effects_enabled: true,
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
                // Vibrant wall colors - electric blues, cyans, purples
                let wall_variant = pos_hash % 100;
                let base_color = if wall_variant < 40 {
                    // Electric blue walls
                    let intensity = 0x60 + ((pos_hash % 0x40) as u32);
                    (0x20 << 16) | (intensity << 8) | 0xFF
                } else if wall_variant < 60 {
                    // Cyan-tinted walls
                    0x40CCCC
                } else if wall_variant < 75 {
                    // Purple accent walls
                    0x8040C0
                } else if wall_variant < 90 {
                    // Teal metallic
                    0x309090
                } else {
                    // Bright highlights
                    0x60A0D0
                };

                // Different wall characters for texture - block chars are safe
                let ch = match pos_hash % 8 {
                    0..=5 => '█',
                    6 => '▓',
                    _ => '▒',
                };

                (ch, base_color)
            }

            Some(Tile::Floor) => {
                // Starfield with more colorful stars
                let star_chance = pos_hash % 25;  // More frequent stars

                if star_chance == 0 {
                    // Bright twinkling star
                    let twinkle = ((self.frame / 8) + (pos_hash as u64)) % 4;
                    let colors = [0xFFFFFF, 0xFFFF80, 0x80FFFF, 0xFFFFFF];
                    (self.star_chars[twinkle as usize], colors[twinkle as usize])
                } else if star_chance == 1 {
                    // Blue star
                    ('*', 0x6080FF)
                } else if star_chance == 2 {
                    // Red giant
                    ('*', 0xFF6040)
                } else if star_chance == 3 {
                    // Yellow star
                    ('*', 0xFFCC00)
                } else if star_chance == 4 {
                    // Cyan star
                    ('.', 0x00FFFF)
                } else if star_chance == 5 {
                    // Dim white star
                    ('.', 0x606060)
                } else if star_chance == 6 {
                    // Purple distant star
                    ('.', 0x8060A0)
                } else {
                    // Empty space
                    (' ', 0x000000)
                }
            }

            Some(Tile::Asteroid) => {
                // Rotating asteroid with more color variety
                let rotation = ((self.frame / 12) + (pos_hash as u64 / 3)) % 4;
                let ch = self.asteroid_chars[rotation as usize];

                // More varied asteroid colors - rusty reds, metallic blues
                let color_variant = pos_hash % 6;
                let color = match color_variant {
                    0 => 0xCC8844, // Bright rust
                    1 => 0x88AACC, // Metallic blue
                    2 => 0xAA6633, // Copper
                    3 => 0x999999, // Silver
                    4 => 0xBB7744, // Bronze
                    _ => 0x77AAAA, // Teal-grey
                };

                (ch, color)
            }

            Some(Tile::Nebula) => {
                // Vibrant, colorful nebula with flowing animation
                let flow = ((self.frame / 4) as i32 + x / 3 + y / 2) % 20;

                // Much brighter nebula colors by region
                let region = hash_position(x / 15, y / 15, 123);
                let base_hue = region % 8;

                let (r, g, b) = match base_hue {
                    0 => (0xFF, 0x40, 0xFF), // Hot magenta
                    1 => (0x40, 0xFF, 0xFF), // Bright cyan
                    2 => (0xFF, 0x80, 0x40), // Orange fire
                    3 => (0x80, 0x40, 0xFF), // Deep purple
                    4 => (0x40, 0xFF, 0x80), // Electric green
                    5 => (0xFF, 0x40, 0x80), // Pink
                    6 => (0x60, 0x80, 0xFF), // Soft blue
                    _ => (0xFF, 0xFF, 0x40), // Yellow
                };

                // Animated intensity - pulsing brighter
                let pulse = ((flow as u32 % 10) * 8) as i32;
                let dim = 40 + (pos_hash % 30) as i32;  // Base dimming for depth
                let color = ((((r as i32 - dim + pulse).max(0).min(255)) as u32) << 16)
                    | ((((g as i32 - dim + pulse).max(0).min(255)) as u32) << 8)
                    | (((b as i32 - dim + pulse).max(0).min(255)) as u32);

                // Simple single-width nebula characters only
                let ch = match (pos_hash + self.frame as u32 / 6) % 5 {
                    0 => '.',
                    1 => ':',
                    2 => '*',
                    3 => '+',
                    _ => '~',
                };

                (ch, color)
            }

            None => {
                // Out of bounds - deep void with rare distant stars
                if pos_hash % 60 == 0 {
                    ('.', 0x303040)
                } else {
                    (' ', 0x000000)
                }
            }
        }
    }

    /// Render the player with a glow effect
    fn render_player(&self, direction: Direction) -> (char, u32, Option<u32>) {
        // Pulsing glow - vary green channel from 0xAA to 0xFF
        let pulse = (self.frame % 30) as u32;
        let green = if pulse < 15 {
            0xAA + (pulse * 5)  // 0xAA (170) to ~0xF5 (245)
        } else {
            0xF5 - ((pulse - 15) * 5)  // back down to 0xAA
        };
        let glow_intensity = green << 8;  // Put in green channel

        // Engine glow behind player (background color)
        let engine_glow = match direction {
            Direction::Up | Direction::Down | Direction::Left | Direction::Right => Some(0x002200),
            _ => Some(0x001800), // Dimmer for diagonals
        };

        (direction.to_char(), glow_intensity, engine_glow)
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

fn main() -> NcResult<()> {
    let nc = unsafe { Nc::new()? };

    let map = Map::generate(500, 200);
    let start = map.find_start_position();
    let mut player = Player::new(start.0, start.1);
    let mut renderer = Renderer::new();

    let stdplane = unsafe { nc.stdplane() };
    let (mut term_height, mut term_width) = stdplane.dim_yx();

    let mut input_state = InputState::default();
    let mut last_move_time = Instant::now();
    let move_delay = Duration::from_millis(33);

    loop {
        let mut quit = false;
        let mut input = NcInput::new_empty();

        loop {
            match nc.get_nblock(Some(&mut input)) {
                Ok(received) => match received {
                    NcReceived::NoInput => break,
                    NcReceived::Char('q') | NcReceived::Char('Q') => {
                        quit = true;
                        break;
                    }
                    NcReceived::Char('b') | NcReceived::Char('B') => {
                        renderer.toggle_effects();
                    }
                    NcReceived::Key(key) => {
                        let evtype = NcInputType::from(input.evtype);
                        match key {
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
                },
                Err(_) => break,
            }
        }

        if quit {
            break;
        }

        input_state.timeout_stale_keys();

        if input_state.any_movement() && last_move_time.elapsed() >= move_delay {
            let (dx, dy) = input_state.movement_delta();
            player.try_move(dx, dy, &map);
            last_move_time = Instant::now();
        }

        // Update animation frame
        renderer.tick();

        // Render
        stdplane.erase();

        let center_screen_x = term_width / 2;
        let center_screen_y = (term_height.saturating_sub(1)) / 2;

        for screen_y in 0..term_height.saturating_sub(1) {
            for screen_x in 0..term_width {
                let map_x = player.x + (screen_x as i32 - center_screen_x as i32);
                let map_y = player.y + (screen_y as i32 - center_screen_y as i32);

                if screen_x == center_screen_x && screen_y == center_screen_y {
                    let (ch, fg, bg) = renderer.render_player(player.direction);
                    if let Some(bg_color) = bg {
                        stdplane.set_bg_rgb(bg_color);
                    }
                    stdplane.set_fg_rgb(fg);
                    let s: String = ch.into();
                    stdplane.putstr_yx(Some(screen_y), Some(screen_x), &s)?;
                    stdplane.set_bg_default();
                } else {
                    let tile = map.get(map_x, map_y);
                    let (ch, fg) = renderer.render_tile(tile, map_x, map_y);

                    stdplane.set_fg_rgb(fg);
                    stdplane.set_bg_default();
                    let s: String = ch.into();
                    stdplane.putstr_yx(Some(screen_y), Some(screen_x), &s)?;
                }
            }
        }

        // Status bar
        // Check what tile player is on
        let current_tile = map.get(player.x, player.y);
        let tile_name = match current_tile {
            Some(Tile::Floor) => "Space",
            Some(Tile::Nebula) => "Nebula",
            _ => "???",
        };

        stdplane.set_fg_rgb(0x00FF00);
        stdplane.set_bg_rgb(0x000020);

        let effects_indicator = if renderer.effects_enabled { "FX:ON" } else { "FX:OFF" };
        let status = format!(
            " ({:>4},{:>4}) {:>2} | {} | {} | B:Effects Q:Quit ",
            player.x,
            player.y,
            player.direction.name(),
            tile_name,
            effects_indicator
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
        let map = Map::generate(100, 50);
        assert_eq!(map.width, 100);
        assert_eq!(map.height, 50);
        assert_eq!(map.tiles.len(), 50); // height rows
        assert_eq!(map.tiles[0].len(), 100); // width columns
    }

    #[test]
    fn test_map_has_walls_and_floors() {
        let map = Map::generate(100, 50);

        let has_walls = map.tiles.iter().flatten().any(|t| *t == Tile::Wall);
        let has_floors = map.tiles.iter().flatten().any(|t| *t == Tile::Floor);

        assert!(has_walls, "Map should contain walls");
        assert!(has_floors, "Map should contain floors");
    }

    #[test]
    fn test_map_border_is_walls() {
        let map = Map::generate(100, 50);

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
        let map = Map::generate(100, 50);

        assert_eq!(map.get(-1, 0), None);
        assert_eq!(map.get(0, -1), None);
        assert_eq!(map.get(100, 0), None);
        assert_eq!(map.get(0, 50), None);
    }

    #[test]
    fn test_map_is_passable() {
        let map = Map::generate(100, 50);

        // Border should not be passable
        assert!(!map.is_passable(0, 0));
        assert!(!map.is_passable(-1, 0));

        // Find a floor tile and check it's passable
        let start = map.find_start_position();
        assert!(map.is_passable(start.0, start.1), "Start position should be passable");
    }

    #[test]
    fn test_map_find_start_position_is_passable() {
        let map = Map::generate(100, 50);
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
        let map = Map::generate(100, 50);
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
        let map = Map::generate(100, 50);
        let start = map.find_start_position();
        let mut player = Player::new(start.0, start.1);
        let original_dir = player.direction;

        let moved = player.try_move(0, 0, &map);
        assert!(!moved, "Should not move with zero delta");
        assert_eq!(player.direction, original_dir, "Direction should not change");
    }

    #[test]
    fn test_player_collision_with_wall() {
        let map = Map::generate(100, 50);
        let mut player = Player::new(1, 1); // Near the wall border

        // Try to move into the wall (border is at x=0)
        let moved = player.try_move(-1, 0, &map);
        assert!(!moved, "Should not move into wall");
        assert_eq!(player.x, 1, "X position should not change");
    }

    // ==================== Renderer Tests ====================

    #[test]
    fn test_renderer_new() {
        let renderer = Renderer::new();
        assert_eq!(renderer.frame, 0);
        assert!(renderer.effects_enabled);
    }

    #[test]
    fn test_renderer_toggle_effects() {
        let mut renderer = Renderer::new();
        assert!(renderer.effects_enabled);

        renderer.toggle_effects();
        assert!(!renderer.effects_enabled);

        renderer.toggle_effects();
        assert!(renderer.effects_enabled);
    }

    #[test]
    fn test_renderer_tick() {
        let mut renderer = Renderer::new();
        assert_eq!(renderer.frame, 0);

        renderer.tick();
        assert_eq!(renderer.frame, 1);

        renderer.tick();
        assert_eq!(renderer.frame, 2);
    }

    #[test]
    fn test_renderer_effects_disabled_returns_simple_tiles() {
        let mut renderer = Renderer::new();
        renderer.effects_enabled = false;

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
        let renderer = Renderer::new();

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
}
