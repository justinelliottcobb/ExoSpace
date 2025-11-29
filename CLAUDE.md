# ExoSpace - Project Context for Claude

## Project Summary

ExoSpace is a Subspace/Continuum-inspired terminal space game. Rust workspace with server and multiple client implementations planned.

## Architecture

### Workspace Members
- `exospace-server` - Axum REST API serving map data
- `exospace-client-terminal` - Main client using libnotcurses-sys
- `exospace-client-pixel` - Planned pixel-based client (empty)
- `exospace-client-neural` - Planned AI client (empty)

### Key Dependencies
```toml
libnotcurses-sys = "3.11"  # Terminal graphics
axum = "0.8"               # Web server
tokio = "1"                # Async runtime
serde/serde_json = "1"     # Serialization
reqwest = "0.12"           # HTTP client (blocking)
dirs = "6"                 # Config directories
```

**Important**: Uses Rust 2024 edition - `gen` is a reserved keyword.

## File Locations

### Server (`exospace-server/src/main.rs`)
- `Tile` enum: Wall, Floor, Asteroid, Nebula
- `MapData` struct: tiles, width, height, start_x, start_y
- `MapGenerator`: Deterministic PRNG-based map generation
- `hash_position()`: Position-based hashing for procedural content
- Endpoints: `GET /map`, `GET /health`

### Terminal Client (`exospace-client-terminal/src/main.rs`)
Major structs in order of appearance:

1. **Tile, Direction** - Basic enums (Direction has 8 values with `to_char()`, `name()`, `from_delta()`)
2. **Config** - User settings (effects_enabled, server_url), saves to ~/.config/exospace/config.json
3. **Map** - Tile grid with `fetch_from_server()` and `generate_local()` fallback
4. **ShipCell** - Single cell: char, fg color, optional bg color
5. **ShipSprite** - 3x3 grid of ShipCells for each direction
6. **ExhaustSprite** - 3x4 animated exhaust trail behind ship
7. **Renderer** - Animation state, tile rendering, ship cell lookup
8. **KeyState, InputState** - Keyboard handling with release detection fallback
9. **Player** - Position and direction, collision-aware movement
10. **ChatMessage** - Text + color (system=yellow, user=green, error=red)
11. **ChatWindow** - Input buffer, cursor, message history, command processing
12. **ChatCommand** - Quit, ShowPosition, Teleport(x,y), ToggleEffects

### Rendering Details
- `putstr_yx()` must be used instead of `putchar_yx()` for colors to work
- `set_bg_default()` works better than `set_bg_rgb(0x000000)` for black backgrounds
- Ship is rendered via `renderer.get_ship_cell(direction, offset_x, offset_y)`
- Game area height = term_height - 5 (chat takes bottom 5 lines)

### Color Palette
```
Ship hull:     0x40C080 (cyan-green)
Ship cockpit:  0x80FFFF (bright cyan)
Ship wing:     0x3090A0 (dark cyan)
Ship accent:   0x60A0C0

Exhaust bright: 0xFF6600-0xFFFF00 (orange-yellow cycle)
Exhaust mid:    0xCC5500-0xCCCC00
Exhaust dim:    0x803300-0x808000
Exhaust faint:  0x401800-0x404000

Chat system:   0xFFFF00 (yellow)
Chat user:     0x00FF00 (green)
Chat error:    0xFF4444 (red)
Chat message:  0xAAAAAA (gray)
```

## Test Coverage (111 tests total)

### Server (38 tests)
- Tile passability and serialization
- Hash function determinism and distribution
- MapGenerator RNG and determinism
- Map dimensions, borders, content
- Start position validity
- HTTP endpoint integration tests

### Terminal Client (73 tests)
- Tile, Direction enums
- Map generation and bounds
- Player movement and collision
- Renderer state and effects toggle
- ShipCell, ShipSprite for all 8 directions
- ExhaustSprite animation and positioning
- InputState keyboard handling
- Config loading/saving
- ChatMessage types
- ChatWindow input, cursor, history
- ChatCommand parsing

## Known Issues / Quirks

1. Terminal background may appear gray instead of black on some terminals (notably Terminus on iPad). `set_bg_default()` is the current workaround.

2. Visual effects were toned down ~2/3 from original implementation to prevent terminal lockup on slower connections.

3. Effects are OFF by default in config for performance reasons.

4. **Windows Terminal compatibility**: Renders fine (with some issues) in Git Bash, but does not render at all in Nushell. This is likely a libnotcurses-sys compatibility issue with how different shells handle terminal capabilities.

## Common Patterns

### Adding a new chat command
1. Add variant to `ChatCommand` enum
2. Add match arm in `ChatWindow::process_input()`
3. Handle command in main loop's `ChatCommand` match
4. Add test in `test_chat_process_*` section

### Adding a new tile type
1. Add to `Tile` enum in both server and client
2. Update `is_passable()` if needed
3. Add rendering in `Renderer::render_tile()`
4. Update server's MapGenerator if it should be generated

### Testing
Run `cargo test --workspace` before committing. All 111 tests should pass.

## Future Considerations

- Multiplayer networking (WebSocket?)
- Player projectiles/weapons
- Other players rendering
- Sound effects
- Pixel client implementation
- AI/bot client for testing
