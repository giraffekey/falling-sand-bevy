use crate::GameState;
use bevy::asset::RenderAssetUsages;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, VertexAttributeValues};
use bevy::render::render_resource::PrimitiveTopology;
use bevy::window::PrimaryWindow;
use line_drawing::Bresenham;
use rand::prelude::*;
use std::cmp::max;
use std::time::Duration;

const TILE_SIZE: f32 = 4.0;

const GRID_WIDTH: usize = 320;

const GRID_HEIGHT: usize = 180;

const TICK_RATE: f32 = 0.01;

const BRUSH_SIZES: [isize; 4] = [0, 2, 4, 8];

#[derive(Debug, Clone, Copy)]
pub enum Material {
    Powder,
    Solid,
    Liquid(u8),
    Gas,
    Fire,
    Wind,
}

#[derive(Debug, Clone, Copy)]
pub struct Tile {
    pub material: Material,
    pub flammable: bool,
    pub lifespan: Option<u8>,
    pub color: [u8; 3],
}

impl Tile {
    pub fn falls(&self) -> bool {
        match self.material {
            Material::Powder | Material::Solid | Material::Liquid(_) => true,
            Material::Gas | Material::Fire | Material::Wind => false,
        }
    }

    pub fn sinks_under(&self, other: Tile) -> bool {
        match (self.material, other.material) {
            (Material::Powder, Material::Liquid(_)) => true,
            (Material::Solid, Material::Liquid(_)) => true,
            (Material::Liquid(a), Material::Liquid(b)) => a > b,
            (Material::Powder, Material::Gas) => true,
            (Material::Solid, Material::Gas) => true,
            (Material::Liquid(_), Material::Gas) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TileId {
    Sand,
    Stone,
    Wood,
    Water,
    Petroleum,
    Oxygen,
    Fire,
    Wind,
}

impl TileId {
    pub fn tile(&self) -> Tile {
        const TILE_SAND: Tile = Tile {
            material: Material::Powder,
            flammable: false,
            lifespan: None,
            color: [194, 178, 128],
        };

        const TILE_STONE: Tile = Tile {
            material: Material::Solid,
            flammable: false,
            lifespan: None,
            color: [83, 86, 91],
        };

        const TILE_WOOD: Tile = Tile {
            material: Material::Solid,
            flammable: true,
            lifespan: None,
            color: [164, 116, 73],
        };

        const TILE_WATER: Tile = Tile {
            material: Material::Liquid(1),
            flammable: false,
            lifespan: None,
            color: [30, 144, 255],
        };

        const TILE_PETROLEUM: Tile = Tile {
            material: Material::Liquid(0),
            flammable: true,
            lifespan: None,
            color: [59, 49, 49],
        };

        const TILE_OXYGEN: Tile = Tile {
            material: Material::Gas,
            flammable: true,
            lifespan: None,
            color: [187, 198, 213],
        };

        const TILE_FIRE: Tile = Tile {
            material: Material::Fire,
            flammable: false,
            lifespan: Some(5),
            color: [226, 88, 34],
        };

        const TILE_WIND: Tile = Tile {
            material: Material::Wind,
            flammable: false,
            lifespan: Some(15),
            color: [255, 255, 255],
        };

        match self {
            TileId::Sand => TILE_SAND,
            TileId::Stone => TILE_STONE,
            TileId::Wood => TILE_WOOD,
            TileId::Water => TILE_WATER,
            TileId::Petroleum => TILE_PETROLEUM,
            TileId::Oxygen => TILE_OXYGEN,
            TileId::Fire => TILE_FIRE,
            TileId::Wind => TILE_WIND,
        }
    }
}

#[derive(Resource)]
pub struct Grid {
    pub tiles: [[Option<TileId>; GRID_HEIGHT]; GRID_WIDTH],
    pub timer: Timer,
    pub brush_size: usize,
    pub selected: TileId,
}

#[derive(Resource)]
pub struct LastCursorPosition(Option<(usize, usize)>);

#[derive(Component)]
pub struct GridMesh;

pub struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), setup)
            .add_systems(Update, tick_grid.run_if(in_state(GameState::Playing)))
            .add_systems(Update, spawn_sand.run_if(in_state(GameState::Playing)))
            .add_systems(Update, draw_grid.run_if(in_state(GameState::Playing)))
            .add_systems(
                Update,
                update_brush_size.run_if(in_state(GameState::Playing)),
            )
            .add_systems(Update, select_tile.run_if(in_state(GameState::Playing)));
    }
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    commands.insert_resource(Grid {
        tiles: [[None; GRID_HEIGHT]; GRID_WIDTH],
        timer: Timer::new(Duration::from_secs_f32(TICK_RATE), TimerMode::Repeating),
        brush_size: 1,
        selected: TileId::Sand,
    });
    commands.insert_resource(LastCursorPosition(None));

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        VertexAttributeValues::from(vec![
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ]),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        VertexAttributeValues::from(vec![[0.0, 0.0, 0.0, 0.0]; 4]),
    );
    mesh.insert_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));

    commands
        .spawn(GridMesh)
        .insert(Mesh2d(meshes.add(mesh)))
        .insert(MeshMaterial2d(materials.add(Color::WHITE)))
        .insert(Transform::default());
}

fn tick_grid(time: Res<Time>, mut grid: ResMut<Grid>) {
    grid.timer.tick(time.delta());

    if grid.timer.just_finished() {
        let mut new_tiles = grid.tiles.clone();

        let mut rng = thread_rng();
        let mut coords: Vec<_> = (0..GRID_WIDTH)
            .map(|x| (0..GRID_HEIGHT).map(move |y| (x, y)))
            .flatten()
            .collect();
        coords.shuffle(&mut rng);

        for (x, y) in coords {
            if let Some(tile_id) = grid.tiles[x][y] {
                let tile = tile_id.tile();

                if y > 0 {
                    let above = grid.tiles[x][y - 1];

                    // Float
                    if above.is_some() && above.unwrap().tile().sinks_under(tile) {
                        new_tiles[x][y] = grid.tiles[x][y - 1];
                        new_tiles[x][y - 1] = grid.tiles[x][y];
                        continue;
                    }
                }

                if y < GRID_HEIGHT - 1 {
                    let below = grid.tiles[x][y + 1];

                    // Fall
                    if below.is_none() && tile.falls() {
                        new_tiles[x][y] = None;
                        new_tiles[x][y + 1] = grid.tiles[x][y];
                        continue;
                    }

                    // Sink
                    if below.is_some() && tile.sinks_under(below.unwrap().tile()) {
                        new_tiles[x][y] = grid.tiles[x][y + 1];
                        new_tiles[x][y + 1] = grid.tiles[x][y];
                        continue;
                    }

                    match tile.material {
                        Material::Powder => {
                            // Slide down slopes

                            let below_left = x > 0
                                && grid.tiles[x - 1][y + 1].is_none()
                                && grid.tiles[x - 1][y].is_none()
                                && new_tiles[x - 1][y + 1].is_none();
                            let below_right = x < GRID_WIDTH - 1
                                && grid.tiles[x + 1][y + 1].is_none()
                                && grid.tiles[x + 1][y].is_none()
                                && new_tiles[x + 1][y + 1].is_none();

                            let (below_left, below_right) = if below_left && below_right {
                                if rng.gen() {
                                    (true, false)
                                } else {
                                    (false, true)
                                }
                            } else {
                                (below_left, below_right)
                            };

                            if below_left {
                                new_tiles[x][y] = None;
                                new_tiles[x - 1][y + 1] = grid.tiles[x][y];
                                continue;
                            }

                            if below_right {
                                new_tiles[x][y] = None;
                                new_tiles[x + 1][y + 1] = grid.tiles[x][y];
                                continue;
                            }
                        }
                        Material::Solid => (),
                        Material::Liquid(_) => {
                            // Slide down slopes

                            let below_left = x > 0
                                && grid.tiles[x - 1][y + 1].is_none()
                                && grid.tiles[x - 1][y].is_none()
                                && new_tiles[x - 1][y + 1].is_none();
                            let below_right = x < GRID_WIDTH - 1
                                && grid.tiles[x + 1][y + 1].is_none()
                                && grid.tiles[x + 1][y].is_none()
                                && new_tiles[x + 1][y + 1].is_none();

                            let (below_left, below_right) = if below_left && below_right {
                                if rng.gen() {
                                    (true, false)
                                } else {
                                    (false, true)
                                }
                            } else {
                                (below_left, below_right)
                            };

                            if below_left {
                                new_tiles[x][y] = None;
                                new_tiles[x - 1][y + 1] = grid.tiles[x][y];
                                continue;
                            }

                            if below_right {
                                new_tiles[x][y] = None;
                                new_tiles[x + 1][y + 1] = grid.tiles[x][y];
                                continue;
                            }

                            // Fill gaps

                            let left = x > 0
                                && new_tiles[x - 1][y].is_none()
                                && (y == 0 || grid.tiles[x - 1][y - 1].is_none());
                            let right = x < GRID_WIDTH - 1
                                && new_tiles[x + 1][y].is_none()
                                && (y == 0 || grid.tiles[x + 1][y - 1].is_none());

                            let (left, right) = if left && right {
                                let open_left = (x as isize - 10..x as isize)
                                    .map(|x| {
                                        (y..GRID_HEIGHT)
                                            .map(|y| {
                                                grid.tiles
                                                    [x.clamp(0, GRID_WIDTH as isize - 1) as usize]
                                                    [y]
                                            })
                                            .collect::<Vec<_>>()
                                    })
                                    .flatten()
                                    .filter(|t| t.is_none())
                                    .count();
                                let open_right = (x + 1..x + 11)
                                    .map(|x| {
                                        (y..GRID_HEIGHT)
                                            .map(|y| grid.tiles[x.clamp(0, GRID_WIDTH - 1)][y])
                                            .collect::<Vec<_>>()
                                    })
                                    .flatten()
                                    .filter(|t| t.is_none())
                                    .count();

                                if open_left > open_right {
                                    (true, false)
                                } else if open_left < open_right {
                                    (false, true)
                                } else if rng.gen() {
                                    (true, false)
                                } else {
                                    (false, true)
                                }
                            } else {
                                (left, right)
                            };

                            if left {
                                new_tiles[x][y] = None;
                                new_tiles[x - 1][y] = grid.tiles[x][y];
                                continue;
                            }

                            if right {
                                new_tiles[x][y] = None;
                                new_tiles[x + 1][y] = grid.tiles[x][y];
                                continue;
                            }
                        }
                        Material::Gas => {
                            let dx = rng.gen_range(-1..=1);
                            let dy = rng.gen_range(-1..=1);

                            let new_x =
                                (x as isize + dx).clamp(0, GRID_WIDTH as isize - 1) as usize;
                            let new_y =
                                (y as isize + dy).clamp(0, GRID_HEIGHT as isize - 1) as usize;

                            if grid.tiles[new_x][new_y].is_none()
                                && new_tiles[new_x][new_y].is_none()
                            {
                                new_tiles[x][y] = None;
                                new_tiles[new_x][new_y] = grid.tiles[x][y];
                                continue;
                            }
                        }
                        Material::Fire => {}
                        Material::Wind => {}
                    }
                }
            }
        }

        grid.tiles = new_tiles;
    }
}

fn spawn_sand(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Single<&Window, With<PrimaryWindow>>,
    q_camera: Single<(&Camera, &GlobalTransform)>,
    mut grid: ResMut<Grid>,
    mut last_cursor_position: ResMut<LastCursorPosition>,
) -> Result {
    if buttons.pressed(MouseButton::Left) {
        let (camera, camera_transform) = *q_camera;
        if let Some(position) = q_window
            .cursor_position()
            .and_then(|cursor| Some(camera.viewport_to_world(camera_transform, cursor)))
            .map(|ray| ray.map(|ray| ray.origin.truncate()))
        {
            if let Some((cx, cy)) = world_to_tiles(position?) {
                let mut tiles = Vec::new();
                let brush_size = BRUSH_SIZES[grid.brush_size];

                let cursor_positions = match last_cursor_position.0 {
                    Some(last) => Bresenham::new(
                        (last.0 as isize, last.1 as isize),
                        (cx as isize, cy as isize),
                    )
                    .map(|(cx, cy)| (cx as usize, cy as usize))
                    .collect(),
                    None => vec![(cx, cy)],
                };

                for (cx, cy) in cursor_positions {
                    for x in (cx as isize - brush_size)..=(cx as isize + brush_size) {
                        for y in (cy as isize - brush_size)..=(cy as isize + brush_size) {
                            if (x - cx as isize).pow(2) + (y - cy as isize).pow(2)
                                <= brush_size.pow(2)
                                && x >= 0
                                && (x as usize) < GRID_WIDTH
                                && y >= 0
                                && (y as usize) < GRID_HEIGHT
                            {
                                tiles.push((x as usize, y as usize));
                            }
                        }
                    }
                }

                let mut rng = thread_rng();
                tiles.shuffle(&mut rng);

                for (x, y) in &tiles[..max(tiles.len() / 2, 1)] {
                    let x = *x;
                    let y = *y;
                    if grid.tiles[x][y].is_none() {
                        grid.tiles[x][y] = Some(grid.selected);
                    }
                }

                last_cursor_position.0 = Some((cx, cy));
            }
        }
    } else {
        last_cursor_position.0 = None;
    }

    Ok(())
}

fn draw_grid(
    mut meshes: ResMut<Assets<Mesh>>,
    grid: Res<Grid>,
    mut grid_mesh: Single<&mut Mesh2d, With<GridMesh>>,
) {
    let mut vertices = Vec::new();
    let mut vertex_colors = Vec::new();
    let mut indices = Vec::new();

    for x in 0..GRID_WIDTH {
        for y in 0..GRID_HEIGHT {
            if let Some(tile_id) = grid.tiles[x][y] {
                let position = tiles_to_world(x, y);
                vertices.extend([
                    [
                        position.x - TILE_SIZE / 2.0,
                        position.y - TILE_SIZE / 2.0,
                        0.0,
                    ],
                    [
                        position.x + TILE_SIZE / 2.0,
                        position.y - TILE_SIZE / 2.0,
                        0.0,
                    ],
                    [
                        position.x + TILE_SIZE / 2.0,
                        position.y + TILE_SIZE / 2.0,
                        0.0,
                    ],
                    [
                        position.x - TILE_SIZE / 2.0,
                        position.y + TILE_SIZE / 2.0,
                        0.0,
                    ],
                ]);

                let color = tile_id.tile().color;
                let c = [
                    color[0] as f32 / 255.0,
                    color[1] as f32 / 255.0,
                    color[2] as f32 / 255.0,
                    1.0,
                ];
                vertex_colors.extend([c, c, c, c]);

                let index = vertices.len() as u32 - 4;
                indices.extend([index, index + 1, index + 2, index, index + 2, index + 3]);
            }
        }
    }

    if !vertices.is_empty() && !vertex_colors.is_empty() {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            VertexAttributeValues::from(vertices),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_COLOR,
            VertexAttributeValues::from(vertex_colors),
        );
        mesh.insert_indices(Indices::U32(indices));

        grid_mesh.0 = meshes.add(mesh);
    }
}

fn update_brush_size(mut evr_scroll: EventReader<MouseWheel>, mut grid: ResMut<Grid>) {
    for ev in evr_scroll.read() {
        if ev.y < 0.0 && grid.brush_size > 0 {
            grid.brush_size -= 1;
        } else if ev.y > 0.0 && grid.brush_size < BRUSH_SIZES.len() - 1 {
            grid.brush_size += 1;
        }
    }
}

fn select_tile(keyboard_input: Res<ButtonInput<KeyCode>>, mut grid: ResMut<Grid>) {
    if keyboard_input.just_pressed(KeyCode::Digit1) {
        grid.selected = TileId::Sand;
    }
    if keyboard_input.just_pressed(KeyCode::Digit2) {
        grid.selected = TileId::Stone;
    }
    if keyboard_input.just_pressed(KeyCode::Digit3) {
        grid.selected = TileId::Wood;
    }
    if keyboard_input.just_pressed(KeyCode::Digit4) {
        grid.selected = TileId::Water;
    }
    if keyboard_input.just_pressed(KeyCode::Digit5) {
        grid.selected = TileId::Petroleum;
    }
    if keyboard_input.just_pressed(KeyCode::Digit6) {
        grid.selected = TileId::Oxygen;
    }
    if keyboard_input.just_pressed(KeyCode::Digit7) {
        grid.selected = TileId::Fire;
    }
    if keyboard_input.just_pressed(KeyCode::Digit8) {
        grid.selected = TileId::Wind;
    }
}

fn world_to_tiles(position: Vec2) -> Option<(usize, usize)> {
    let x = (position.x + GRID_WIDTH as f32 * TILE_SIZE / 2.0) / TILE_SIZE;
    let y = (-position.y + GRID_HEIGHT as f32 * TILE_SIZE / 2.0) / TILE_SIZE;
    if x >= 0.0 && (x as usize) < GRID_WIDTH && y >= 0.0 && (y as usize) < GRID_HEIGHT {
        Some((x as usize, y as usize))
    } else {
        None
    }
}

fn tiles_to_world(x: usize, y: usize) -> Vec2 {
    Vec2::new(
        x as f32 * TILE_SIZE - GRID_WIDTH as f32 * TILE_SIZE / 2.0 + TILE_SIZE / 2.0,
        -(y as f32 * TILE_SIZE - GRID_HEIGHT as f32 * TILE_SIZE / 2.0 + TILE_SIZE / 2.0),
    )
}
