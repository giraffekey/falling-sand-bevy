use crate::cell::{Material, *};
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

const DATA_SIZE: f32 = 4.0;

const GRID_WIDTH: usize = 320;

const GRID_HEIGHT: usize = 180;

const TICK_RATE: f32 = 0.01;

const BRUSH_SIZES: [isize; 4] = [0, 2, 4, 8];

#[derive(Resource)]
pub struct Grid {
    pub cells: Vec<Vec<Option<Cell>>>,
    pub timer: Timer,
    pub brush_size: usize,
    pub selected: CellId,
}

#[derive(Resource)]
pub struct LastCursorPosition(Option<(usize, usize)>);

#[derive(Component)]
pub struct GridMesh;

pub struct GridPlugin;

impl Plugin for GridPlugin {
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
    commands.spawn((Camera2d, Msaa::Off));
    commands.insert_resource(Grid {
        cells: vec![vec![None; GRID_HEIGHT]; GRID_WIDTH],
        timer: Timer::new(Duration::from_secs_f32(TICK_RATE), TimerMode::Repeating),
        brush_size: 1,
        selected: CellId::Sand,
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
        let mut new_cells = grid.cells.clone();

        let mut rng = thread_rng();
        let mut coords: Vec<_> = (0..GRID_WIDTH)
            .map(|x| (0..GRID_HEIGHT).map(move |y| (x, y)))
            .flatten()
            .collect();
        coords.shuffle(&mut rng);

        for (x, y) in coords {
            if let Some(mut cell) = grid.cells[x][y] {
                if let Some(life) = &mut cell.life {
                    *life -= 1;
                    if *life == 0 {
                        new_cells[x][y] = None;
                        continue;
                    }
                }

                if y > 0 {
                    let above = grid.cells[x][y - 1];

                    // Float
                    if above.is_some() && above.unwrap().sinks_under(Some(cell)) {
                        new_cells[x][y] = above;
                        new_cells[x][y - 1] = Some(cell);
                        continue;
                    }
                }

                if y < GRID_HEIGHT - 1 {
                    if cell.falls() {
                        // Fall
                        if cell.sinks_under(grid.cells[x][y + 1])
                            || cell.dissolves(grid.cells[x][y + 1])
                        {
                            if cell.dissolves(grid.cells[x][y + 1]) {
                                new_cells[x][y] = None;
                                new_cells[x][y + 1] = None;
                            } else {
                                new_cells[x][y] = grid.cells[x][y + 1];
                                new_cells[x][y + 1] = Some(cell);
                            }
                            continue;
                        } else {
                            match grid.cells[x][y + 1] {
                                // Extinguish fire
                                Some(c) if c.material() == Material::Fire => {
                                    new_cells[x][y] = None;
                                    if !cell.flammable() {
                                        new_cells[x][y + 1] = Some(cell);
                                    }
                                    continue;
                                }
                                // Dissolve in acid
                                Some(c) if c.dissolves(Some(cell)) => {
                                    new_cells[x][y] = None;
                                    new_cells[x][y + 1] = None;
                                }
                                _ => (),
                            }
                        }
                    }

                    // Slide down slopes
                    if cell.slides() {
                        let below_left = x > 0
                            && (cell.sinks_under(grid.cells[x - 1][y + 1])
                                || cell.dissolves(grid.cells[x - 1][y + 1]))
                            && cell.sinks_under(grid.cells[x - 1][y])
                            && grid.cells[x - 1][y + 1] == new_cells[x - 1][y + 1];
                        let below_right = x < GRID_WIDTH - 1
                            && (cell.sinks_under(grid.cells[x + 1][y + 1])
                                || cell.dissolves(grid.cells[x + 1][y + 1]))
                            && cell.sinks_under(grid.cells[x + 1][y])
                            && grid.cells[x + 1][y + 1] == new_cells[x + 1][y + 1];

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
                            if cell.dissolves(grid.cells[x - 1][y + 1]) {
                                new_cells[x][y] = None;
                                new_cells[x - 1][y + 1] = None;
                            } else {
                                new_cells[x][y] = grid.cells[x - 1][y + 1];
                                new_cells[x - 1][y + 1] = Some(cell);
                            }
                            continue;
                        }

                        if below_right {
                            if cell.dissolves(grid.cells[x + 1][y + 1]) {
                                new_cells[x][y] = None;
                                new_cells[x + 1][y + 1] = None;
                            } else {
                                new_cells[x][y] = grid.cells[x + 1][y + 1];
                                new_cells[x + 1][y + 1] = Some(cell);
                            }
                            continue;
                        }
                    }

                    match cell.material() {
                        Material::Powder | Material::Solid => (),
                        Material::Liquid(_) | Material::Acid => {
                            // Fill gaps

                            let left = x > 0
                                && (cell.sinks_under(new_cells[x - 1][y])
                                    || cell.dissolves(new_cells[x - 1][y]))
                                && (y == 0 || cell.sinks_under(grid.cells[x - 1][y - 1]));
                            let right = x < GRID_WIDTH - 1
                                && (cell.sinks_under(new_cells[x + 1][y])
                                    || cell.dissolves(new_cells[x + 1][y]))
                                && (y == 0 || cell.sinks_under(grid.cells[x + 1][y - 1]));

                            let (left, right) = if left && right {
                                if rng.gen() {
                                    (true, false)
                                } else {
                                    (false, true)
                                }
                            } else {
                                (left, right)
                            };

                            if left {
                                if cell.dissolves(new_cells[x - 1][y]) {
                                    new_cells[x][y] = None;
                                    new_cells[x - 1][y] = None;
                                } else {
                                    new_cells[x][y] = new_cells[x - 1][y];
                                    new_cells[x - 1][y] = Some(cell);
                                }
                                continue;
                            }

                            if right {
                                if cell.dissolves(new_cells[x + 1][y]) {
                                    new_cells[x][y] = None;
                                    new_cells[x + 1][y] = None;
                                } else {
                                    new_cells[x][y] = new_cells[x + 1][y];
                                    new_cells[x + 1][y] = Some(cell);
                                }
                                continue;
                            }
                        }
                        Material::Gas => {
                            // Disperse

                            let dx = rng.gen_range(-1..=1);
                            let dy = rng.gen_range(-1..=1);

                            let new_x =
                                (x as isize + dx).clamp(0, GRID_WIDTH as isize - 1) as usize;
                            let new_y =
                                (y as isize + dy).clamp(0, GRID_HEIGHT as isize - 1) as usize;

                            if grid.cells[new_x][new_y].is_none()
                                && new_cells[new_x][new_y].is_none()
                            {
                                new_cells[x][y] = None;
                                new_cells[new_x][new_y] = Some(cell);
                                continue;
                            }
                        }
                        Material::Fire => {
                            // Spread flames

                            let flammables: Vec<_> = adjacent(x, y)
                                .into_iter()
                                .filter(|&(nx, ny)| {
                                    grid.cells[nx][ny].is_some()
                                        && grid.cells[nx][ny].unwrap().flammable()
                                })
                                .collect();

                            for (nx, ny) in flammables {
                                let open: Vec<_> = adjacent(nx, ny)
                                    .into_iter()
                                    .filter(|&(ax, ay)| {
                                        grid.cells[ax][ay].is_none() && new_cells[ax][ay].is_none()
                                    })
                                    .collect();

                                if let Some(&(ax, ay)) = open.choose(&mut rng) {
                                    new_cells[ax][ay] = Some(Cell {
                                        id: cell.id,
                                        life: cell.lifespan(),
                                    });
                                }

                                let chance = match grid.cells[nx][ny].unwrap().material() {
                                    Material::Liquid(_) => 0.55,
                                    _ => 0.1,
                                };

                                if rng.gen::<f32>() < chance {
                                    new_cells[nx][ny] = Some(Cell {
                                        id: cell.id,
                                        life: cell.lifespan(),
                                    });
                                }
                            }

                            // Rise

                            let dx = rng.gen_range(-1..=1);
                            let dy = rng.gen_range(-2..=0);

                            let new_x =
                                (x as isize + dx).clamp(0, GRID_WIDTH as isize - 1) as usize;
                            let new_y =
                                (y as isize + dy).clamp(0, GRID_HEIGHT as isize - 1) as usize;

                            new_cells[x][y] = None;

                            match grid.cells[new_x][new_y] {
                                Some(c) => {
                                    if c.flammable() {
                                        new_cells[new_x][new_y] = Some(cell);
                                    }
                                }
                                None => new_cells[new_x][new_y] = Some(cell),
                            }

                            continue;
                        }
                        Material::Wind => todo!(),
                    }
                }
            }
        }

        grid.cells = new_cells;
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

                for (x, y) in tiles[..max(tiles.len() / 2, 1)].iter().copied() {
                    if grid.cells[x][y].is_none() {
                        grid.cells[x][y] = Some(Cell {
                            id: grid.selected,
                            life: grid.selected.data().lifespan,
                        });
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
            if let Some(cell) = grid.cells[x][y] {
                let position = tiles_to_world(x, y);
                vertices.extend([
                    [
                        position.x - DATA_SIZE / 2.0,
                        position.y - DATA_SIZE / 2.0,
                        0.0,
                    ],
                    [
                        position.x + DATA_SIZE / 2.0,
                        position.y - DATA_SIZE / 2.0,
                        0.0,
                    ],
                    [
                        position.x + DATA_SIZE / 2.0,
                        position.y + DATA_SIZE / 2.0,
                        0.0,
                    ],
                    [
                        position.x - DATA_SIZE / 2.0,
                        position.y + DATA_SIZE / 2.0,
                        0.0,
                    ],
                ]);

                let [r, g, b] = cell.color();
                let c = [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0];
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
        grid.selected = CellId::Sand;
    }
    if keyboard_input.just_pressed(KeyCode::Digit2) {
        grid.selected = CellId::Stone;
    }
    if keyboard_input.just_pressed(KeyCode::Digit3) {
        grid.selected = CellId::Wood;
    }
    if keyboard_input.just_pressed(KeyCode::Digit4) {
        grid.selected = CellId::Water;
    }
    if keyboard_input.just_pressed(KeyCode::Digit5) {
        grid.selected = CellId::Oil;
    }
    if keyboard_input.just_pressed(KeyCode::Digit6) {
        grid.selected = CellId::Acid;
    }
    if keyboard_input.just_pressed(KeyCode::Digit7) {
        grid.selected = CellId::Oxygen;
    }
    if keyboard_input.just_pressed(KeyCode::Digit8) {
        grid.selected = CellId::Fire;
    }
}

fn world_to_tiles(position: Vec2) -> Option<(usize, usize)> {
    let x = (position.x + GRID_WIDTH as f32 * DATA_SIZE / 2.0) / DATA_SIZE;
    let y = (-position.y + GRID_HEIGHT as f32 * DATA_SIZE / 2.0) / DATA_SIZE;
    if x >= 0.0 && (x as usize) < GRID_WIDTH && y >= 0.0 && (y as usize) < GRID_HEIGHT {
        Some((x as usize, y as usize))
    } else {
        None
    }
}

fn tiles_to_world(x: usize, y: usize) -> Vec2 {
    Vec2::new(
        x as f32 * DATA_SIZE - GRID_WIDTH as f32 * DATA_SIZE / 2.0 + DATA_SIZE / 2.0,
        -(y as f32 * DATA_SIZE - GRID_HEIGHT as f32 * DATA_SIZE / 2.0 + DATA_SIZE / 2.0),
    )
}

fn adjacent(x: usize, y: usize) -> Vec<(usize, usize)> {
    let mut ids = Vec::new();
    if x > 0 {
        ids.push((x - 1, y));
    }
    if x < GRID_WIDTH - 1 {
        ids.push((x + 1, y));
    }
    if y > 0 {
        ids.push((x, y - 1));
    }
    if y < GRID_HEIGHT - 1 {
        ids.push((x, y + 1));
    }
    ids
}

fn neighbors(x: usize, y: usize) -> Vec<(usize, usize)> {
    neighbors_within(x, y, 1)
}

fn neighbors_within(x: usize, y: usize, n: usize) -> Vec<(usize, usize)> {
    let mut ids = Vec::new();
    for dx in -(n as isize)..=n as isize {
        for dy in -(n as isize)..=n as isize {
            let nx = x as isize + dx;
            let ny = y as isize + dy;

            if nx < 0 || nx >= GRID_WIDTH as isize || ny < 0 || ny >= GRID_HEIGHT as isize {
                continue;
            }

            let nx = nx as usize;
            let ny = ny as usize;

            if !(nx == 0 && ny == 0) {
                ids.push((nx, ny));
            }
        }
    }
    ids
}
