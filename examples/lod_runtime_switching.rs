use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::*;

use bevy_procedural_tree::lod::generate_tree_lods;
use bevy_procedural_tree::presets::{settings_for_preset, TreePreset};

const LOD_LEVELS: usize = 3;
const TREE_POS: Vec3 = Vec3::new(0.0, 0.0, 0.0);
const TREE_SCALE: f32 = 0.7;
const FLY_SPEED_MPS: f32 = 9.0;
const FAST_FLY_SPEED_MPS: f32 = 24.0;
const MOUSE_SENSITIVITY: f32 = 0.002;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            mode: AssetMode::Unprocessed,
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.56, 0.60, 0.64)))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 850.0,
            ..default()
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (free_look, update_runtime_lod, update_overlay))
        .run();
}

#[derive(Component)]
struct RuntimeLodPart {
    handles: [Handle<Mesh>; LOD_LEVELS],
    current_lod: usize,
}

#[derive(Component)]
struct RuntimeLodOverlay;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let settings = settings_for_preset(TreePreset::WideUmbrellaDeciduous, 0, 0.35);
    let lods = generate_tree_lods(&settings, 64, LOD_LEVELS as u32)
        .expect("runtime LOD tree generation should succeed");
    let min_y = lods
        .iter()
        .map(|(branches, leaves)| mesh_min_y(branches).min(mesh_min_y(leaves)))
        .fold(f32::INFINITY, f32::min);
    let y_offset = if min_y.is_finite() { -min_y } else { 0.0 };

    let mut branch_handles = Vec::with_capacity(LOD_LEVELS);
    let mut leaf_handles = Vec::with_capacity(LOD_LEVELS);
    for (branches, leaves) in lods {
        branch_handles.push(meshes.add(branches));
        leaf_handles.push(meshes.add(leaves));
    }
    let branch_handles: [Handle<Mesh>; LOD_LEVELS] = branch_handles.try_into().unwrap();
    let leaf_handles: [Handle<Mesh>; LOD_LEVELS] = leaf_handles.try_into().unwrap();

    let bark_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.25, 0.16),
        perceptual_roughness: 0.95,
        ..default()
    });
    let leaf_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.30, 0.64, 0.27),
        perceptual_roughness: 0.8,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    let ground_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.76, 0.78, 0.67),
        perceptual_roughness: 1.0,
        ..default()
    });

    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(180.0, 180.0))),
        MeshMaterial3d(ground_material),
        Transform::from_xyz(0.0, -0.01, 0.0)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));

    let tree_transform = Transform::from_translation(TREE_POS + Vec3::Y * y_offset * TREE_SCALE)
        .with_scale(Vec3::splat(TREE_SCALE));
    commands.spawn((
        Mesh3d(branch_handles[0].clone()),
        MeshMaterial3d(bark_material),
        tree_transform,
        RuntimeLodPart {
            handles: branch_handles,
            current_lod: 0,
        },
    ));
    commands.spawn((
        Mesh3d(leaf_handles[0].clone()),
        MeshMaterial3d(leaf_material),
        tree_transform,
        RuntimeLodPart {
            handles: leaf_handles,
            current_lod: 0,
        },
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.35, -0.2)),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-18.0, 8.0, 42.0).looking_at(Vec3::new(0.0, 5.0, 0.0), Vec3::Y),
        Tonemapping::None,
    ));

    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            padding: UiRect::all(px(10)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.03, 0.04, 0.035, 0.72)),
        RuntimeLodOverlay,
    ));
}

fn update_runtime_lod(
    camera: Single<&Transform, (With<Camera>, Without<RuntimeLodPart>)>,
    mut parts: Query<(&mut Mesh3d, &mut RuntimeLodPart, &GlobalTransform)>,
) {
    for (mut mesh, mut part, transform) in &mut parts {
        let distance = camera.translation.distance(transform.translation());
        let lod = lod_for_distance(distance, part.current_lod);
        if lod != part.current_lod {
            part.current_lod = lod;
            *mesh = Mesh3d(part.handles[lod].clone());
        }
    }
}

fn update_overlay(
    camera: Single<&Transform, With<Camera>>,
    parts: Query<(&RuntimeLodPart, &GlobalTransform)>,
    mut overlay: Single<&mut Text, With<RuntimeLodOverlay>>,
) {
    let Some((part, transform)) = parts.iter().next() else {
        return;
    };
    let distance = camera.translation.distance(transform.translation());
    **overlay = format!(
        "Runtime LOD switching\nDistance: {:.1} m\nActive LOD: {}\nBands: 0<=50, 1<=200, 2 far\nHold left mouse + WASD/QE",
        distance, part.current_lod
    )
    .into();
}

fn lod_for_distance(distance: f32, current_lod: usize) -> usize {
    match current_lod {
        0 if distance > 60.0 => 1,
        1 if distance < 45.0 => 0,
        1 if distance > 220.0 => 2,
        2 if distance < 180.0 => 1,
        _ => current_lod,
    }
}

fn free_look(
    mut camera: Single<&mut Transform, With<Camera>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    time: Res<Time>,
) {
    const PITCH_LIMIT: f32 = core::f32::consts::FRAC_PI_2 - 0.0001;

    if mouse_buttons.pressed(MouseButton::Left) {
        let delta = mouse_motion.delta;
        let delta_yaw = -delta.x * MOUSE_SENSITIVITY;
        let delta_pitch = -delta.y * MOUSE_SENSITIVITY;
        let (yaw, pitch, _) = camera.rotation.to_euler(EulerRot::YXZ);
        camera.rotation = Quat::from_euler(
            EulerRot::YXZ,
            yaw + delta_yaw,
            (pitch + delta_pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT),
            0.0,
        );
    }

    let mut direction = Vec3::ZERO;
    if keyboard.pressed(KeyCode::KeyW) {
        direction += *camera.forward();
    }
    if keyboard.pressed(KeyCode::KeyS) {
        direction -= *camera.forward();
    }
    if keyboard.pressed(KeyCode::KeyD) {
        direction += *camera.right();
    }
    if keyboard.pressed(KeyCode::KeyA) {
        direction -= *camera.right();
    }
    if keyboard.pressed(KeyCode::KeyE) {
        direction += Vec3::Y;
    }
    if keyboard.pressed(KeyCode::KeyQ) {
        direction -= Vec3::Y;
    }

    if direction.length_squared() > 0.0 {
        let fast = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
        let speed = if fast {
            FAST_FLY_SPEED_MPS
        } else {
            FLY_SPEED_MPS
        };
        camera.translation += direction.normalize() * speed * time.delta_secs();
    }
}

fn mesh_min_y(mesh: &Mesh) -> f32 {
    match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(positions)) => {
            positions.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min)
        }
        _ => 0.0,
    }
}

#[allow(dead_code)]
fn index_count(mesh: &Mesh) -> usize {
    match mesh.indices() {
        Some(Indices::U16(indices)) => indices.len(),
        Some(Indices::U32(indices)) => indices.len(),
        None => match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(positions)) => positions.len(),
            _ => 0,
        },
    }
}
