use shared::Position;

const WORLD_SCALE_FACTOR: f64 = 25.0;

pub fn world_space_to_screen_space(
    player_position: &Position,
    other: &Position,
    width: f64,
    height: f64,
) -> (f64, f64) {
    // difference in world space
    let dx = other.x - player_position.x;
    let dy = other.y - player_position.y;

    // scale and add screen center offset (current player is always at center of the screen)
    (
        width / 2.0 + dx * WORLD_SCALE_FACTOR,
        height / 2.0 + dy * WORLD_SCALE_FACTOR,
    )
}

pub fn screen_space_to_world_space(
    player_position: &Position,
    screen_x: f64,
    screen_y: f64,
    width: f64,
    height: f64,
) -> Position {
    // reverse of world_space_to_screen_space
    let world_x = player_position.x + (screen_x - width / 2.0) / WORLD_SCALE_FACTOR;
    let world_y = player_position.y + (screen_y - height / 2.0) / WORLD_SCALE_FACTOR;

    Position {
        x: world_x,
        y: world_y,
    }
}
