use axum::{
    extract::Query,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Tile types in the map
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum Tile {
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

/// Map data that can be serialized and sent to clients
#[derive(Serialize, Deserialize)]
pub struct MapData {
    pub tiles: Vec<Vec<Tile>>,
    pub width: usize,
    pub height: usize,
    pub start_x: i32,
    pub start_y: i32,
}

/// Query parameters for map generation
#[derive(Deserialize)]
pub struct MapQuery {
    #[serde(default = "default_width")]
    width: usize,
    #[serde(default = "default_height")]
    height: usize,
    #[serde(default)]
    seed: Option<u64>,
}

fn default_width() -> usize {
    500
}

fn default_height() -> usize {
    200
}

/// Simple deterministic hash for procedural generation
fn hash_position(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = seed;
    h ^= x as u32;
    h = h.wrapping_mul(2654435761);
    h ^= y as u32;
    h = h.wrapping_mul(2654435761);
    h ^= h >> 13;
    h = h.wrapping_mul(1274126177);
    h ^= h >> 16;
    h
}

/// Map generator
struct MapGenerator {
    rng_state: u64,
}

impl MapGenerator {
    fn new(seed: u64) -> Self {
        MapGenerator { rng_state: seed }
    }

    fn rand(&mut self) -> u64 {
        self.rng_state = self.rng_state.wrapping_mul(1103515245).wrapping_add(12345);
        (self.rng_state >> 16) & 0x7fff
    }

    fn generate(&mut self, width: usize, height: usize) -> MapData {
        let mut tiles = vec![vec![Tile::Wall; width]; height];

        // Create main corridors with varying widths
        let mut y = 2;
        while y < height - 2 {
            let corridor_height = (self.rand() % 15 + 3) as usize;
            let wall_height = (self.rand() % 4 + 1) as usize;

            for cy in y..(y + corridor_height).min(height - 1) {
                for x in 1..width - 1 {
                    tiles[cy][x] = Tile::Floor;
                }
            }
            y += corridor_height + wall_height;
        }

        // Add vertical passages
        let num_passages = width / 30;
        for i in 0..num_passages {
            let x = (i * 30) + 15 + (self.rand() % 10) as usize;
            if x < width - 1 {
                let passage_width = (self.rand() % 8 + 2) as usize;
                for px in x..(x + passage_width).min(width - 1) {
                    for y in 1..height - 1 {
                        tiles[y][px] = Tile::Floor;
                    }
                }
            }
        }

        // Add some rooms
        let num_rooms = (width * height) / 2000;
        for _ in 0..num_rooms {
            let room_w = (self.rand() % 20 + 5) as usize;
            let room_h = (self.rand() % 15 + 5) as usize;
            let room_x = (self.rand() as usize % (width - room_w - 2)) + 1;
            let room_y = (self.rand() as usize % (height - room_h - 2)) + 1;

            for ry in room_y..(room_y + room_h).min(height - 1) {
                for rx in room_x..(room_x + room_w).min(width - 1) {
                    tiles[ry][rx] = Tile::Floor;
                }
            }
        }

        // Add asteroid fields (clusters of impassable asteroids)
        let num_asteroid_fields = (width * height) / 5000;
        for _ in 0..num_asteroid_fields {
            let center_x = (self.rand() as usize % (width - 20)) + 10;
            let center_y = (self.rand() as usize % (height - 10)) + 5;
            let field_size = (self.rand() % 8 + 3) as i32;

            for dy in -field_size..=field_size {
                for dx in -field_size..=field_size {
                    let dist = (dx * dx + dy * dy) as f32;
                    if dist < (field_size * field_size) as f32 * 0.7 {
                        let ax = (center_x as i32 + dx) as usize;
                        let ay = (center_y as i32 + dy) as usize;
                        if ax > 0 && ax < width - 1 && ay > 0 && ay < height - 1 {
                            if tiles[ay][ax] == Tile::Floor && self.rand() % 3 != 0 {
                                tiles[ay][ax] = Tile::Asteroid;
                            }
                        }
                    }
                }
            }
        }

        // Add nebula zones (passable but visually distinct)
        let num_nebulae = (width * height) / 8000;
        for _ in 0..num_nebulae {
            let center_x = (self.rand() as usize % (width - 30)) + 15;
            let center_y = (self.rand() as usize % (height - 15)) + 7;
            let nebula_size = (self.rand() % 12 + 5) as i32;

            for dy in -nebula_size..=nebula_size {
                for dx in -nebula_size..=nebula_size {
                    let dist = (dx * dx + dy * dy) as f32;
                    if dist < (nebula_size * nebula_size) as f32 * 0.8 {
                        let nx = (center_x as i32 + dx) as usize;
                        let ny = (center_y as i32 + dy) as usize;
                        if nx > 0 && nx < width - 1 && ny > 0 && ny < height - 1 {
                            if tiles[ny][nx] == Tile::Floor {
                                tiles[ny][nx] = Tile::Nebula;
                            }
                        }
                    }
                }
            }
        }

        // Find start position
        let (start_x, start_y) = self.find_start_position(&tiles, width, height);

        MapData {
            tiles,
            width,
            height,
            start_x,
            start_y,
        }
    }

    fn find_start_position(&self, tiles: &[Vec<Tile>], width: usize, height: usize) -> (i32, i32) {
        // Find a passable tile near the center
        let center_x = width / 2;
        let center_y = height / 2;

        for radius in 0..50 {
            for dy in -(radius as i32)..=(radius as i32) {
                for dx in -(radius as i32)..=(radius as i32) {
                    let x = (center_x as i32 + dx) as usize;
                    let y = (center_y as i32 + dy) as usize;
                    if x < width && y < height {
                        if tiles[y][x].is_passable() {
                            return (x as i32, y as i32);
                        }
                    }
                }
            }
        }
        (1, 1)
    }
}

/// Handler for the map endpoint
async fn get_map(Query(params): Query<MapQuery>) -> Json<MapData> {
    let seed = params.seed.unwrap_or(12345);
    let mut generator = MapGenerator::new(seed);
    let map = generator.generate(params.width, params.height);
    Json(map)
}

/// Health check endpoint
async fn health() -> &'static str {
    "OK"
}

#[tokio::main]
async fn main() {
    // Build our application with routes
    let app = Router::new()
        .route("/", get(health))
        .route("/health", get(health))
        .route("/map", get(get_map));

    // Run it
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Exospace server listening on {}", addr);
    println!("  GET /map           - Generate a map (query params: width, height, seed)");
    println!("  GET /health        - Health check");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_passability() {
        assert!(Tile::Floor.is_passable());
        assert!(Tile::Nebula.is_passable());
        assert!(!Tile::Wall.is_passable());
        assert!(!Tile::Asteroid.is_passable());
    }

    #[test]
    fn test_map_generator_deterministic() {
        let mut generator1 = MapGenerator::new(12345);
        let mut generator2 = MapGenerator::new(12345);

        let map1 = generator1.generate(100, 50);
        let map2 = generator2.generate(100, 50);

        assert_eq!(map1.tiles, map2.tiles);
        assert_eq!(map1.start_x, map2.start_x);
        assert_eq!(map1.start_y, map2.start_y);
    }

    #[test]
    fn test_map_generator_different_seeds() {
        let mut generator1 = MapGenerator::new(12345);
        let mut generator2 = MapGenerator::new(54321);

        let map1 = generator1.generate(100, 50);
        let map2 = generator2.generate(100, 50);

        // Maps with different seeds should be different
        assert_ne!(map1.tiles, map2.tiles);
    }

    #[test]
    fn test_map_dimensions() {
        let mut generator = MapGenerator::new(12345);
        let map = generator.generate(100, 50);

        assert_eq!(map.width, 100);
        assert_eq!(map.height, 50);
        assert_eq!(map.tiles.len(), 50);
        assert_eq!(map.tiles[0].len(), 100);
    }

    #[test]
    fn test_map_has_all_tile_types() {
        let mut generator = MapGenerator::new(12345);
        let map = generator.generate(500, 200);

        let has_walls = map.tiles.iter().flatten().any(|t| *t == Tile::Wall);
        let has_floors = map.tiles.iter().flatten().any(|t| *t == Tile::Floor);
        let has_asteroids = map.tiles.iter().flatten().any(|t| *t == Tile::Asteroid);
        let has_nebulae = map.tiles.iter().flatten().any(|t| *t == Tile::Nebula);

        assert!(has_walls, "Map should contain walls");
        assert!(has_floors, "Map should contain floors");
        assert!(has_asteroids, "Map should contain asteroids");
        assert!(has_nebulae, "Map should contain nebulae");
    }

    #[test]
    fn test_start_position_is_passable() {
        let mut generator = MapGenerator::new(12345);
        let map = generator.generate(100, 50);

        let start_tile = map.tiles[map.start_y as usize][map.start_x as usize];
        assert!(start_tile.is_passable(), "Start position must be passable");
    }

    #[test]
    fn test_hash_position_deterministic() {
        let hash1 = hash_position(10, 20, 42);
        let hash2 = hash_position(10, 20, 42);
        assert_eq!(hash1, hash2);
    }
}
