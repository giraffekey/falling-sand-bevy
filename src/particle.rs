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

const PALETTE: [[u8; 3]; 7] = [
    [219, 209, 180],
    [211, 199, 162],
    [202, 188, 145],
    [194, 178, 128],
    [186, 168, 111],
    [177, 157, 94],
    [166, 145, 80],
];

#[derive(Resource)]
pub struct Grid {
    tiles: [[bool; GRID_HEIGHT]; GRID_WIDTH],
    timer: Timer,
    brush_size: usize,
}

#[derive(Resource)]
pub struct LastCursorPosition(Option<(usize, usize)>);

#[derive(Component)]
pub struct GridMesh;

#[derive(Component)]
pub struct Particle {
    x: usize,
    y: usize,
    color: [u8; 3],
}

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
            );
    }
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    commands.insert_resource(Grid {
        tiles: [[false; GRID_HEIGHT]; GRID_WIDTH],
        timer: Timer::new(Duration::from_secs_f32(TICK_RATE), TimerMode::Repeating),
        brush_size: 1,
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

fn tick_grid(
    time: Res<Time>,
    mut grid: ResMut<Grid>,
    mut q_particles: Query<&mut Particle>,
) -> Result {
    grid.timer.tick(time.delta());

    if grid.timer.just_finished() {
        let mut new_tiles = grid.tiles;
        for mut p in &mut q_particles {
            if p.y < GRID_HEIGHT - 1 {
                // Fall
                if !grid.tiles[p.x][p.y + 1] {
                    new_tiles[p.x][p.y] = false;
                    new_tiles[p.x][p.y + 1] = true;
                    p.y += 1;
                } else {
                    // Slide down left slopes
                    if p.x > 0
                        && !grid.tiles[p.x - 1][p.y + 1]
                        && (p.x == GRID_WIDTH - 1 || grid.tiles[p.x + 1][p.y])
                    {
                        new_tiles[p.x][p.y] = false;
                        new_tiles[p.x - 1][p.y + 1] = true;
                        p.x -= 1;
                        p.y += 1;
                    // Slide down right slopes
                    } else if p.x < GRID_WIDTH - 1
                        && !grid.tiles[p.x + 1][p.y + 1]
                        && (p.x == 0 || grid.tiles[p.x - 1][p.y])
                    {
                        new_tiles[p.x][p.y] = false;
                        new_tiles[p.x + 1][p.y + 1] = true;
                        p.x += 1;
                        p.y += 1;
                    // Topple pillars
                    } else if p.x > 0
                        && p.x < GRID_WIDTH - 1
                        && !grid.tiles[p.x - 1][p.y + 1]
                        && !grid.tiles[p.x + 1][p.y + 1]
                        && (p.y..GRID_HEIGHT - 1).all(|y| grid.tiles[p.x][y])
                    {
                        new_tiles[p.x][p.y] = false;
                        if thread_rng().gen() {
                            new_tiles[p.x - 1][p.y + 1] = true;
                            p.x -= 1;
                        } else {
                            new_tiles[p.x + 1][p.y + 1] = true;
                            p.x += 1;
                        }
                        p.y += 1;
                    }
                }
            }
        }
        grid.tiles = new_tiles;
    }

    Ok(())
}

fn spawn_sand(
    mut commands: Commands,
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
                    if !grid.tiles[x][y] {
                        let color = *PALETTE.choose(&mut rng).unwrap();
                        commands.spawn(Particle { x, y, color });
                        grid.tiles[x][y] = true;
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
    q_particles: Query<&Particle>,
    mut grid_mesh: Single<&mut Mesh2d, With<GridMesh>>,
) {
    let mut vertices = Vec::new();
    let mut vertex_colors = Vec::new();
    let mut indices = Vec::new();
    for (i, p) in q_particles.iter().enumerate() {
        let position = tiles_to_world(p.x, p.y);
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

        let c = [
            p.color[0] as f32 / 255.0,
            p.color[1] as f32 / 255.0,
            p.color[2] as f32 / 255.0,
            1.0,
        ];
        vertex_colors.extend([c, c, c, c]);

        let index = i as u32 * 4;
        indices.extend([
            index, index + 1, index + 2,
            index, index + 2, index + 3,
        ]);
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
