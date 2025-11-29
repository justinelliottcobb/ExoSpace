# ExoSpace

A terminal-based space game inspired by Subspace/Continuum, built in Rust.

## Overview

ExoSpace is a multiplayer-capable space shooter rendered entirely in the terminal using ASCII/Unicode graphics. Navigate through procedurally generated maps filled with nebulae, asteroid fields, and open space.

## Project Structure

```
exospace/
├── exospace-server/          # Axum-based game server
├── exospace-client-terminal/ # Terminal client (libnotcurses)
├── exospace-client-pixel/    # Pixel-based client (planned)
├── exospace-client-neural/   # AI/neural client (planned)
└── Cargo.toml                # Workspace configuration
```

## Features

### Terminal Client
- **3x3 ASCII ship** with 8 directional sprites
- **Animated exhaust trail** (3x4) with color gradient
- **Procedurally generated maps** with walls, floors, asteroids, and nebulae
- **Visual effects** including twinkling stars and nebula animations (toggleable)
- **Chat/command system** with in-game commands
- **Player-centric scrolling** - the ship stays centered while the map scrolls
- **Diagonal movement** via simultaneous key presses
- **Collision detection** with wall sliding

### Server
- RESTful API using Axum
- Deterministic map generation with seed support
- JSON-serialized map data

## Controls

### Movement
- **Arrow keys** - Move ship (combines for diagonal movement)

### Commands
- **Q** - Quit game
- **B** - Toggle background effects
- **Enter** - Open chat
- **/** - Open command input

### Chat Commands
- `/help` - Show available commands
- `/pos` - Display current position
- `/goto X Y` - Teleport to coordinates
- `/fx` - Toggle visual effects
- `/quit` - Exit game

## Building

```bash
# Build all packages
cargo build --release

# Run the server
cargo run --package exospace-server

# Run the terminal client
cargo run --package exospace-client-terminal
```

## Testing

```bash
# Run all tests
cargo test --workspace

# Run specific package tests
cargo test --package exospace-server
cargo test --package exospace-client-terminal
```

## Configuration

User configuration is stored at `~/.config/exospace/config.json`:

```json
{
  "effects_enabled": false,
  "server_url": null
}
```

- `effects_enabled` - Whether visual effects are on (default: false)
- `server_url` - Custom server URL (default: http://localhost:3000)

## Requirements

- Rust 2024 edition
- A terminal with Unicode support
- libnotcurses for the terminal client

## Platform Notes

**Linux**: Works well in most terminal emulators.

**Windows Terminal**:
- Git Bash: Renders with some minor issues
- Nushell: Does not render (libnotcurses compatibility issue)

**iPad (Terminus)**: Works but background may appear gray instead of black.

## License

[Add license here]
