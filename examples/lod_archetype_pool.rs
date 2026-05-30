use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;

use bevy_procedural_tree::enums::TreeType;
use bevy_procedural_tree::lod::{generate_archetypes_from_settings_with_reduction, LodReduction};
use bevy_procedural_tree::settings::TreeMeshSettings;

const ARCHETYPES: u32 = 6;
const LOD_LEVELS: u32 = 3;
const BASE_SEED: u64 = 42;
const CELL_X: f32 = 8.0;
const CELL_Z: f32 = 9.0;
const DISPLAY_SCALE: f32 = 0.58;
const FLY_SPEED_MPS: f32 = 9.0;
const FAST_FLY_SPEED_MPS: f32 = 24.0;
const MOUSE_SENSITIVITY: f32 = 0.002;
const TARGET_CAMERA_FOCUS: Vec3 = Vec3 {
    x: (ARCHETYPES as f32 - 1.0) * CELL_X * 0.5,
    y: 1.8,
    z: (LOD_LEVELS as f32 - 1.0) * CELL_Z * 0.5,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            mode: AssetMode::Unprocessed,
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.56, 0.60, 0.64)))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 900.0,
            ..default()
        })
        .add_systems(Startup, setup)
        .add_systems(Update, free_look)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(
            ARCHETYPES as f32 * CELL_X + 2.0,
            LOD_LEVELS as f32 * CELL_Z + 2.0,
        ))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.78, 0.80, 0.70),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_xyz(TARGET_CAMERA_FOCUS.x, -0.01, TARGET_CAMERA_FOCUS.z)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.45, -0.25)),
    ));

    commands.spawn((
        PointLight {
            shadows_enabled: true,
            intensity: 450_000.0,
            range: 45.0,
            ..default()
        },
        Transform::from_xyz(
            TARGET_CAMERA_FOCUS.x - 6.0,
            18.0,
            TARGET_CAMERA_FOCUS.z + 10.0,
        ),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(
            TARGET_CAMERA_FOCUS.x - 8.0,
            18.0,
            TARGET_CAMERA_FOCUS.z + 28.0,
        )
        .looking_at(TARGET_CAMERA_FOCUS, Vec3::Y),
        Tonemapping::None,
    ));

    let bark_texture_color: Handle<Image> = asset_server.load("textures/bark_willow/color.dds");
    let bark_texture_normal: Handle<Image> =
        asset_server.load("textures/bark_willow/normal_gl.dds");
    let bark_texture_roughness: Handle<Image> =
        asset_server.load("textures/bark_willow/roughness.dds");
    let bark_material = materials.add(StandardMaterial {
        base_color_texture: Some(bark_texture_color),
        normal_map_texture: Some(bark_texture_normal),
        metallic_roughness_texture: Some(bark_texture_roughness),
        perceptual_roughness: 1.0,
        reflectance: 0.1,
        ..default()
    });

    let leaf_texture_color: Handle<Image> =
        asset_server.load("textures/deciduous_leaves/color.dds");
    let leaf_texture_normal: Handle<Image> =
        asset_server.load("textures/deciduous_leaves/normal_gl.dds");
    let leaf_texture_roughness: Handle<Image> =
        asset_server.load("textures/deciduous_leaves/roughness.dds");
    let leaf_materials = [
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.82, 0.96, 0.78),
            base_color_texture: Some(leaf_texture_color.clone()),
            normal_map_texture: Some(leaf_texture_normal.clone()),
            metallic_roughness_texture: Some(leaf_texture_roughness.clone()),
            perceptual_roughness: 1.0,
            reflectance: 0.1,
            cull_mode: None,
            double_sided: true,
            alpha_mode: AlphaMode::Mask(0.5),
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.62, 0.86, 0.58),
            base_color_texture: Some(leaf_texture_color.clone()),
            normal_map_texture: Some(leaf_texture_normal.clone()),
            metallic_roughness_texture: Some(leaf_texture_roughness.clone()),
            perceptual_roughness: 1.0,
            reflectance: 0.1,
            cull_mode: None,
            double_sided: true,
            alpha_mode: AlphaMode::Mask(0.5),
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 1.0, 0.82),
            base_color_texture: Some(leaf_texture_color),
            normal_map_texture: Some(leaf_texture_normal),
            metallic_roughness_texture: Some(leaf_texture_roughness),
            perceptual_roughness: 1.0,
            reflectance: 0.1,
            cull_mode: None,
            double_sided: true,
            alpha_mode: AlphaMode::Mask(0.5),
            ..default()
        }),
    ];
    let lod_marker_materials = [
        materials.add(Color::srgb(0.42, 0.62, 0.28)),
        materials.add(Color::srgb(0.86, 0.68, 0.28)),
        materials.add(Color::srgb(0.73, 0.36, 0.30)),
    ];

    let settings = varied_archetype_settings(ARCHETYPES);
    let archetypes = generate_archetypes_from_settings_with_reduction(
        &settings,
        BASE_SEED,
        LOD_LEVELS,
        LodReduction::aggressive(),
    )
    .expect("tree archetype generation should succeed");

    for (column, archetype) in archetypes.into_iter().enumerate() {
        for (row, (branches, leaves)) in archetype.lods.into_iter().enumerate() {
            let x = column as f32 * CELL_X;
            let z = row as f32 * CELL_Z;
            let transform = Transform::from_xyz(x, 0.0, z).with_scale(Vec3::splat(DISPLAY_SCALE));

            commands.spawn((
                Mesh3d(meshes.add(branches)),
                MeshMaterial3d(bark_material.clone()),
                transform,
            ));
            commands.spawn((
                Mesh3d(meshes.add(leaves)),
                MeshMaterial3d(leaf_materials[column % leaf_materials.len()].clone()),
                transform,
            ));

            commands.spawn((
                Mesh3d(meshes.add(Circle::new(0.18))),
                MeshMaterial3d(
                    lod_marker_materials[row.min(lod_marker_materials.len() - 1)].clone(),
                ),
                Transform::from_xyz(x - 1.25, 0.02, z - 1.25)
                    .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            ));
        }
    }
}

fn varied_archetype_settings(count: u32) -> Vec<TreeMeshSettings> {
    (0..count).map(archetype_settings).collect()
}

fn archetype_settings(index: u32) -> TreeMeshSettings {
    let mut settings = TreeMeshSettings::default();
    match index % 6 {
        // Compact round deciduous.
        0 => {
            settings.branch.length = [3.6, 2.2, 1.2, 0.32];
            settings.branch.angle = [0.0, 46.0, 54.0, 66.0];
            settings.branch.children = [6, 4, 9];
            settings.branch.trunk_base_radius = 0.17;
            settings.leaves.count = 4;
            settings.leaves.size = 0.22;
        }
        // Tall narrow deciduous.
        1 => {
            settings.branch.length = [6.2, 2.4, 1.0, 0.28];
            settings.branch.angle = [0.0, 23.0, 30.0, 38.0];
            settings.branch.children = [5, 4, 6];
            settings.branch.force.direction = Vec3::Y;
            settings.branch.force.strength = 0.16;
            settings.branch.trunk_base_radius = 0.20;
            settings.leaves.count = 3;
            settings.leaves.size = 0.20;
        }
        // Broad low canopy.
        2 => {
            settings.branch.length = [3.2, 3.8, 2.0, 0.55];
            settings.branch.angle = [0.0, 70.0, 62.0, 58.0];
            settings.branch.children = [7, 5, 12];
            settings.branch.force.direction = Vec3::new(0.0, 0.35, 0.0);
            settings.branch.force.strength = 0.03;
            settings.branch.trunk_base_radius = 0.24;
            settings.leaves.count = 5;
            settings.leaves.size = 0.28;
        }
        // Conical evergreen.
        3 => {
            settings.tree_type = TreeType::Evergreen;
            settings.branch.length = [7.4, 2.0, 0.9, 0.30];
            settings.branch.angle = [0.0, 66.0, 58.0, 48.0];
            settings.branch.children = [14, 6, 7];
            settings.branch.force.direction = Vec3::new(0.0, -0.35, 0.0);
            settings.branch.force.strength = 0.18;
            settings.branch.trunk_base_radius = 0.17;
            settings.leaves.count = 5;
            settings.leaves.size = 0.20;
        }
        // Open sparse deciduous.
        4 => {
            settings.branch.length = [5.2, 3.3, 1.8, 0.45];
            settings.branch.angle = [0.0, 40.0, 45.0, 52.0];
            settings.branch.children = [4, 3, 5];
            settings.branch.gnarliness = [0.02, 0.34, 0.28, 0.08];
            settings.branch.trunk_base_radius = 0.22;
            settings.leaves.count = 2;
            settings.leaves.size = 0.34;
            settings.leaves.size_variance = 0.45;
        }
        // Dense spreading deciduous.
        _ => {
            settings.branch.length = [4.6, 3.0, 1.7, 0.48];
            settings.branch.angle = [0.0, 56.0, 48.0, 62.0];
            settings.branch.children = [9, 5, 13];
            settings.branch.gnarliness = [-0.02, 0.18, 0.18, 0.06];
            settings.branch.trunk_base_radius = 0.21;
            settings.leaves.count = 7;
            settings.leaves.size = 0.25;
            settings.leaves.size_variance = 0.28;
        }
    }
    settings
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
        let yaw = yaw + delta_yaw;
        let pitch = (pitch + delta_pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);

        camera.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
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
