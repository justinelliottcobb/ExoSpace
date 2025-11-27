use libnotcurses_sys::*;
use std::time::{Duration, Instant};

/// Tile types in the map
#[derive(Clone, Copy, PartialEq)]
enum Tile {
    Wall,
    Floor,
}

impl Tile {
    fn is_passable(&self) -> bool {
        matches!(self, Tile::Floor)
    }
}

/// 8-directional orientation
#[derive(Clone, Copy, PartialEq, Default)]
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
    /// Get direction from movement delta
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

    /// Get the character to display for this direction
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

    /// Get direction name for status display
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

/// The game map
struct Map {
    tiles: Vec<Vec<Tile>>,
    width: usize,
    height: usize,
}

impl Map {
    /// Generate a large maze with variable passage widths
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

        // Create vertical corridors connecting horizontal ones
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

        // Add some internal walls/pillars for interest
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

/// State for a single key with timeout-based release fallback
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

/// Tracks which movement keys are currently held with proper press/release handling
struct InputState {
    up: KeyState,
    down: KeyState,
    left: KeyState,
    right: KeyState,
    /// Whether we've detected release events (terminal supports kitty protocol)
    has_release_support: bool,
    /// Timeout for key release fallback (for terminals without release events)
    /// Longer timeout allows for easier key combinations (hold one, tap another)
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
            // 300ms gives a comfortable window to combine keys
            // (hold left, then press up within 300ms for diagonal)
            key_timeout: Duration::from_millis(300),
        }
    }
}

impl InputState {
    /// Update key state based on input event
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

    /// For terminals without release events, timeout keys that haven't been seen recently
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

    /// Get the movement delta from current key states
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

/// Player state
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

    /// Try to move the player with diagonal support and wall sliding
    /// Updates direction based on attempted movement (even if blocked)
    fn try_move(&mut self, dx: i32, dy: i32, map: &Map) -> bool {
        if dx == 0 && dy == 0 {
            return false;
        }

        // Always update direction based on input, even if movement is blocked
        if let Some(dir) = Direction::from_delta(dx, dy) {
            self.direction = dir;
        }

        let new_x = self.x + dx;
        let new_y = self.y + dy;

        // Try full movement first
        if map.is_passable(new_x, new_y) {
            self.x = new_x;
            self.y = new_y;
            return true;
        }

        // If diagonal is blocked, try sliding along walls
        if dx != 0 && dy != 0 {
            // Try horizontal only
            if map.is_passable(self.x + dx, self.y) {
                self.x += dx;
                return true;
            }
            // Try vertical only
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

    // Generate a large maze
    let map = Map::generate(500, 200);

    // Create player at starting position
    let start = map.find_start_position();
    let mut player = Player::new(start.0, start.1);

    // Get the standard plane and terminal dimensions
    let stdplane = unsafe { nc.stdplane() };
    let (mut term_height, mut term_width) = stdplane.dim_yx();

    // Input and timing state
    let mut input_state = InputState::default();
    let mut last_move_time = Instant::now();
    let move_delay = Duration::from_millis(33); // ~30 moves per second when holding keys

    // Main loop
    loop {
        // Process all pending input events (non-blocking)
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

        // Timeout stale keys for terminals without release events
        input_state.timeout_stale_keys();

        // Process movement if keys are held and enough time has passed
        if input_state.any_movement() && last_move_time.elapsed() >= move_delay {
            let (dx, dy) = input_state.movement_delta();
            player.try_move(dx, dy, &map);
            last_move_time = Instant::now();
        }

        // Render
        stdplane.erase();

        let center_screen_x = term_width / 2;
        let center_screen_y = (term_height.saturating_sub(1)) / 2;

        for screen_y in 0..term_height.saturating_sub(1) {
            for screen_x in 0..term_width {
                let map_x = player.x + (screen_x as i32 - center_screen_x as i32);
                let map_y = player.y + (screen_y as i32 - center_screen_y as i32);

                if screen_x == center_screen_x && screen_y == center_screen_y {
                    // Draw player with directional indicator
                    stdplane.set_fg_rgb(0x00FF00);
                    stdplane.putchar_yx(screen_y, screen_x, player.direction.to_char())?;
                } else {
                    let (ch, fg) = match map.get(map_x, map_y) {
                        Some(Tile::Wall) => ('█', 0x606060u32),
                        Some(Tile::Floor) => (' ', 0x000000u32),
                        None => ('░', 0x303030u32),
                    };

                    stdplane.set_fg_rgb(fg);
                    stdplane.putchar_yx(screen_y, screen_x, ch)?;
                }
            }
        }

        // Status bar
        let release_indicator = if input_state.has_release_support {
            "Kitty"
        } else {
            "Fallback"
        };

        stdplane.set_fg_rgb(0xFFFFFF);
        stdplane.set_bg_rgb(0x000080);

        let status = format!(
            " ({}, {}) {} | {}x{} | [{}] | Arrows to move, Q to quit ",
            player.x,
            player.y,
            player.direction.name(),
            map.width,
            map.height,
            release_indicator
        );
        let padded_status = format!("{:<width$}", status, width = term_width as usize);
        stdplane.putstr_yx(Some(term_height - 1), Some(0), &padded_status)?;
        stdplane.set_bg_rgb(0x000000);

        nc.render()?;

        // Small sleep to prevent busy-waiting (~60 FPS)
        std::thread::sleep(Duration::from_millis(16));
    }

    unsafe { nc.stop()? };
    Ok(())
}
