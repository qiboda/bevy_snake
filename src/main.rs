#![feature(iter_map_while)]

use std::time::Duration;
use bevy::{prelude::*, render::pass::ClearColor};

use rand::prelude::random;

const ARENA_WIDTH: u32 = 10;
const ARENA_HEIGHT: u32 = 10;

#[derive(Default, Debug, Eq, PartialEq, Copy, Clone)]
struct Position {
    x: i32,
    y: i32,
}

struct Size {
    width: f32,
    height: f32,
}

impl Size {
    pub fn square(x: f32) -> Self {
        Self {
            width: x,
            height: x,
        }
    }
}

fn main() {
    App::build()
        .insert_resource(WindowDescriptor {
            title: "Snake!".to_string(),
            width: 800.0,
            height: 800.0,
            ..Default::default()
        })
        .add_plugins(DefaultPlugins)
        .init_resource::<SnakeMoveTimer>()
        .init_resource::<SnakeEntities>()
        .init_resource::<LastTailPosition>()
        .insert_resource(ClearColor(Color::rgb(0.04, 0.04, 0.04)))
        .add_event::<GrowthEvent>()
        .add_event::<GameOverEvent>()
        .add_startup_system(setup.system())
        .add_startup_system_to_stage(StartupStage::PostStartup, snake_setup.system())
        .add_system(snake_timer.system())
        .add_system(food_spawner.system())
        .add_system(snake_movement.system().label("snake_movement"))
        .add_system(size_scaling.system())
        .add_system(position_translation.system())
        .add_system(
            snake_eating
                .system()
                .label("eating")
                .after("snake_movement"),
        )
        .add_system(snake_growth.system())
        .add_system(game_over.system())
        .run();
}

fn setup(mut commands: Commands, mut materials: ResMut<Assets<ColorMaterial>>) {
    commands.spawn_bundle(OrthographicCameraBundle::new_2d());

    commands.insert_resource(SnakeMaterials {
        head_material: materials.add(Color::rgb(0.7, 0.7, 0.7).into()),
        segment_material: materials.add(Color::rgb(0.3, 0.3, 0.3).into()),
        food_material: materials.add(Color::rgb(1.0, 1.0, 1.0).into()),
    });
}

fn snake_setup(
    mut commands: Commands,
    mut snake_entities: ResMut<SnakeEntities>,
    snake_material: Res<SnakeMaterials>,
) {
    snake_entities.0 = vec![
        commands
            .spawn_bundle(SpriteBundle {
                material: snake_material.head_material.clone(),
                sprite: Sprite::new(Vec2::new(10.0, 10.0)),
                ..Default::default()
            })
            .insert(Position { x: 3, y: 3 })
            .insert(Size::square(0.8))
            .insert(SnakeHead {
                direction: SnakeMoveDirection::Up,
            })
            .id(),
        spawn_segment(
            &mut commands,
            &snake_material.segment_material,
            Position { x: 3, y: 2 },
        ),
    ];
}

fn size_scaling(windows: Res<Windows>, mut q: Query<(&Size, &mut Sprite)>) {
    let window = windows.get_primary().unwrap();
    for (sprite_size, mut sprite) in q.iter_mut() {
        sprite.size = Vec2::new(
            sprite_size.width / ARENA_WIDTH as f32 * window.width() as f32,
            sprite_size.height / ARENA_HEIGHT as f32 * window.height() as f32,
        )
    }
}

struct SnakeHead {
    direction: SnakeMoveDirection,
}

struct SnakeMaterials {
    head_material: Handle<ColorMaterial>,
    segment_material: Handle<ColorMaterial>,
    food_material: Handle<ColorMaterial>,
}

struct NextHeadDirection(SnakeMoveDirection);

impl Default for NextHeadDirection {
    fn default() -> Self {
        NextHeadDirection(SnakeMoveDirection::Up)
    }
}

fn snake_movement(
    keyboard_input: Res<Input<KeyCode>>,
    snake_timer: Res<SnakeMoveTimer>,
    snake_entities: Res<SnakeEntities>,
    mut last_tail_position: ResMut<LastTailPosition>,
    mut game_over_events: EventWriter<GameOverEvent>,
    mut next_direction: Local<NextHeadDirection>,
    mut heads: Query<(Entity, &mut SnakeHead)>,
    mut positions: Query<&mut Position, Or<(With<SnakeSegment>, With<SnakeHead>)>>,
) {
    if let Some((head_entity, mut head)) = heads.iter_mut().next() {
        // set direction
        let dir = if keyboard_input.pressed(KeyCode::Left) {
            SnakeMoveDirection::Left
        } else if keyboard_input.pressed(KeyCode::Down) {
            SnakeMoveDirection::Down
        } else if keyboard_input.pressed(KeyCode::Up) {
            SnakeMoveDirection::Up
        } else if keyboard_input.pressed(KeyCode::Right) {
            SnakeMoveDirection::Right
        } else {
            next_direction.0
        };

        if dir != head.direction.opposite() {
            next_direction.0 = dir;
        }

        // timer finished
        if !snake_timer.0.finished() {
            return;
        }

        head.direction = next_direction.0;

        // update positions
        let snake_positions = snake_entities
            .0
            .iter()
            .map_while(|e| match positions.get_mut(*e) {
                Ok(pos) => Some(*pos),
                Err(_) => None,
            })
            .collect::<Vec<Position>>();

        snake_positions
            .iter()
            .zip(snake_entities.0.iter().skip(1))
            .for_each(|(pos, ent)| match positions.get_mut(*ent) {
                Ok(mut x) => *x = *pos,
                Err(_) => {}
            });

        last_tail_position.0 = Some(*snake_positions.last().unwrap());

        // change head positions
        let mut head_pos = positions.get_mut(head_entity).unwrap();
        match &head.direction {
            SnakeMoveDirection::Left => {
                head_pos.x -= 1;
            }
            SnakeMoveDirection::Right => {
                head_pos.x += 1;
            }
            SnakeMoveDirection::Up => {
                head_pos.y += 1;
            }
            SnakeMoveDirection::Down => {
                head_pos.y -= 1;
            }
        }

        // game over?
        if head_pos.x < 0
            || head_pos.y < 0
            || head_pos.x as u32 >= ARENA_WIDTH
            || head_pos.y as u32 >= ARENA_HEIGHT
        {
            game_over_events.send(GameOverEvent);
        }

        if snake_positions.contains(&head_pos) {
            game_over_events.send(GameOverEvent);
        }
    }
}

fn position_translation(windows: Res<Windows>, mut q: Query<(&Position, &mut Transform)>) {
    fn convert(pos: f32, bound_window: f32, bound_game: f32) -> f32 {
        let tile_size = bound_window / bound_game;
        pos / bound_game * bound_window - (bound_window / 2.0) + (tile_size / 2.0)
    }
    let window = windows.get_primary().unwrap();
    for (pos, mut transform) in q.iter_mut() {
        transform.translation = Vec3::new(
            convert(pos.x as f32, window.width() as f32, ARENA_WIDTH as f32),
            convert(pos.y as f32, window.height() as f32, ARENA_HEIGHT as f32),
            0.0,
        );
    }
}

struct Food;

struct FoodSpawnTimer(Timer);

impl Default for FoodSpawnTimer {
    fn default() -> Self {
        Self(Timer::new(Duration::from_millis(1000), true))
    }
}

fn food_spawner(
    mut commands: Commands,
    snake_materials: Res<SnakeMaterials>,
    time: Res<Time>,
    mut timer: Local<FoodSpawnTimer>,
) {
    timer.0.tick(time.delta());
    if timer.0.finished() {
        commands
            .spawn_bundle(SpriteBundle {
                material: snake_materials.food_material.clone(),
                sprite: Sprite::new(Vec2::new(10.0, 10.0)),
                ..Default::default()
            })
            .insert(Food)
            .insert(Position {
                x: (random::<f32>() * (ARENA_WIDTH as f32)) as i32,
                y: (random::<f32>() * (ARENA_HEIGHT as f32)) as i32,
            })
            .insert(Size::square(0.8));
    }
}

#[derive(PartialEq, Copy, Clone)]
enum SnakeMoveDirection {
    Left,
    Up,
    Right,
    Down,
}

impl SnakeMoveDirection {
    fn opposite(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Up => Self::Down,
            Self::Down => Self::Up,
        }
    }
}

struct SnakeMoveTimer(Timer);

impl Default for SnakeMoveTimer {
    fn default() -> Self {
        Self(Timer::new(Duration::from_millis(1500), true))
    }
}

fn snake_timer(time: Res<Time>, mut snake_timer: ResMut<SnakeMoveTimer>) {
    snake_timer.0.tick(time.delta());
}

struct SnakeSegment;

#[derive(Default)]
struct SnakeEntities(Vec<Entity>);

fn spawn_segment(
    commands: &mut Commands,
    material: &Handle<ColorMaterial>,
    position: Position,
) -> Entity {
    commands
        .spawn_bundle(SpriteBundle {
            material: material.clone(),
            ..Default::default()
        })
        .insert(SnakeSegment)
        .insert(position)
        .insert(Size::square(0.65))
        .id()
}

struct GrowthEvent;

fn snake_eating(
    mut commands: Commands,
    snake_timer: ResMut<SnakeMoveTimer>,
    mut growth_events: EventWriter<GrowthEvent>,
    food_positions: Query<(Entity, &Position), With<Food>>,
    head_positions: Query<&Position, With<SnakeHead>>,
) {
    if !snake_timer.0.finished() {
        return;
    }

    for head_pos in head_positions.iter() {
        for (ent, food_pos) in food_positions.iter() {
            if food_pos == head_pos {
                commands.entity(ent).despawn();
                growth_events.send(GrowthEvent {});
            }
        }
    }
}

#[derive(Default)]
struct LastTailPosition(Option<Position>);

fn snake_growth(
    mut commands: Commands,
    last_tail_position: Res<LastTailPosition>,
    mut growth_events: EventReader<GrowthEvent>,
    mut snake_entities: ResMut<SnakeEntities>,
    materials: Res<SnakeMaterials>,
) {
    for _event in growth_events.iter() {
        snake_entities.0.push(spawn_segment(
            &mut commands,
            &materials.segment_material,
            last_tail_position.0.unwrap(),
        ));
    }
}

struct GameOverEvent;

fn game_over(
    mut commands: Commands,
    mut game_over_event: EventReader<GameOverEvent>,
    food: Query<Entity, With<Food>>,
    snake_entities: ResMut<SnakeEntities>,
    materials: Res<SnakeMaterials>,
) {
    if game_over_event.iter().next().is_some() {
        for ent in food.iter().chain(snake_entities.0.clone()) {
            commands.entity(ent).despawn();
        }
        snake_setup(commands, snake_entities, materials);
    }
}
