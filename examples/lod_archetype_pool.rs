use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::*;

use bevy_procedural_tree::lod::{generate_archetypes_from_settings_with_reduction, LodReduction};
use bevy_procedural_tree::presets::tree_preset_settings;

const ARCHETYPES: u32 = 6;
const LOD_LEVELS: u32 = 3;
const PROFILE_SPACING_Z: f32 = 36.0;
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
        .insert_resource(ViewerState::default())
        .add_systems(
            Update,
            (free_look, update_viewer_state, apply_viewer_state).chain(),
        )
        .run();
}

#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
enum ProfileKind {
    Balanced,
    Aggressive,
    Conservative,
}

impl ProfileKind {
    const ALL: [Self; 3] = [Self::Balanced, Self::Aggressive, Self::Conservative];

    fn label(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::Aggressive => "aggressive",
            Self::Conservative => "conservative",
        }
    }

    fn reduction(self) -> LodReduction {
        match self {
            Self::Balanced => LodReduction::balanced(),
            Self::Aggressive => LodReduction::aggressive(),
            Self::Conservative => LodReduction::conservative(),
        }
    }

    fn origin_z(self) -> f32 {
        match self {
            Self::Balanced => 0.0,
            Self::Aggressive => PROFILE_SPACING_Z,
            Self::Conservative => PROFILE_SPACING_Z * 2.0,
        }
    }
}

#[derive(Component)]
struct ViewerTreeMesh {
    profile: ProfileKind,
    lod: usize,
}

#[derive(Component)]
struct ViewerOverlay;

#[derive(Clone, Copy, Default)]
struct MeshStats {
    vertices: usize,
    triangles: usize,
}

impl MeshStats {
    fn add(self, other: Self) -> Self {
        Self {
            vertices: self.vertices + other.vertices,
            triangles: self.triangles + other.triangles,
        }
    }
}

#[derive(Clone, Copy)]
struct ProfileStats {
    profile: ProfileKind,
    lods: [MeshStats; LOD_LEVELS as usize],
}

#[derive(Resource)]
struct ViewerState {
    profile: ProfileKind,
    isolated_lod: Option<usize>,
    archetypes: usize,
    stats: Vec<ProfileStats>,
    changed: bool,
}

impl Default for ViewerState {
    fn default() -> Self {
        Self {
            profile: ProfileKind::Balanced,
            isolated_lod: None,
            archetypes: ARCHETYPES as usize,
            stats: Vec::new(),
            changed: true,
        }
    }
}

impl ViewerState {
    fn active_stats(&self) -> MeshStats {
        let Some(profile_stats) = self.stats.iter().find(|s| s.profile == self.profile) else {
            return MeshStats::default();
        };
        match self.isolated_lod {
            Some(lod) => profile_stats.lods[lod],
            None => profile_stats
                .lods
                .iter()
                .copied()
                .fold(MeshStats::default(), MeshStats::add),
        }
    }

    fn lod_label(&self) -> String {
        match self.isolated_lod {
            Some(lod) => format!("LOD {}", lod),
            None => "all LODs".to_string(),
        }
    }
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut viewer_state: ResMut<ViewerState>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(
            ARCHETYPES as f32 * CELL_X + 2.0,
            ProfileKind::ALL.len() as f32 * PROFILE_SPACING_Z,
        ))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.78, 0.80, 0.70),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_xyz(
            TARGET_CAMERA_FOCUS.x,
            -0.01,
            TARGET_CAMERA_FOCUS.z + PROFILE_SPACING_Z,
        )
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
        ViewerOverlay,
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

    let presets = tree_preset_settings(ARCHETYPES);
    viewer_state.stats.clear();

    for profile in ProfileKind::ALL {
        let archetypes = generate_archetypes_from_settings_with_reduction(
            &presets,
            BASE_SEED,
            LOD_LEVELS,
            profile.reduction(),
        )
        .expect("tree archetype generation should succeed");

        let mut profile_stats = ProfileStats {
            profile,
            lods: [MeshStats::default(); LOD_LEVELS as usize],
        };

        for (column, archetype) in archetypes.into_iter().enumerate() {
            for (row, (branches, leaves)) in archetype.lods.into_iter().enumerate() {
                let branch_stats = mesh_stats(&branches);
                let leaf_stats = mesh_stats(&leaves);
                profile_stats.lods[row] = profile_stats.lods[row].add(branch_stats).add(leaf_stats);

                let x = column as f32 * CELL_X;
                let z = profile.origin_z() + row as f32 * CELL_Z;
                let transform =
                    Transform::from_xyz(x, 0.0, z).with_scale(Vec3::splat(DISPLAY_SCALE));

                commands.spawn((
                    Mesh3d(meshes.add(branches)),
                    MeshMaterial3d(bark_material.clone()),
                    transform,
                    ViewerTreeMesh { profile, lod: row },
                ));
                commands.spawn((
                    Mesh3d(meshes.add(leaves)),
                    MeshMaterial3d(leaf_materials[column % leaf_materials.len()].clone()),
                    transform,
                    ViewerTreeMesh { profile, lod: row },
                ));

                commands.spawn((
                    Mesh3d(meshes.add(Circle::new(0.18))),
                    MeshMaterial3d(
                        lod_marker_materials[row.min(lod_marker_materials.len() - 1)].clone(),
                    ),
                    Transform::from_xyz(x - 1.25, 0.02, z - 1.25)
                        .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
                    ViewerTreeMesh { profile, lod: row },
                ));
            }
        }

        viewer_state.stats.push(profile_stats);
    }
}

fn mesh_stats(mesh: &Mesh) -> MeshStats {
    MeshStats {
        vertices: vertex_count(mesh),
        triangles: index_count(mesh) / 3,
    }
}

fn vertex_count(mesh: &Mesh) -> usize {
    match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(positions)) => positions.len(),
        _ => 0,
    }
}

fn index_count(mesh: &Mesh) -> usize {
    match mesh.indices() {
        Some(Indices::U16(indices)) => indices.len(),
        Some(Indices::U32(indices)) => indices.len(),
        None => vertex_count(mesh),
    }
}

fn update_viewer_state(keyboard: Res<ButtonInput<KeyCode>>, mut state: ResMut<ViewerState>) {
    if keyboard.just_pressed(KeyCode::Digit1) {
        state.isolated_lod = Some(0);
        state.changed = true;
    }
    if keyboard.just_pressed(KeyCode::Digit2) {
        state.isolated_lod = Some(1);
        state.changed = true;
    }
    if keyboard.just_pressed(KeyCode::Digit3) {
        state.isolated_lod = Some(2);
        state.changed = true;
    }
    if keyboard.just_pressed(KeyCode::Digit0) {
        state.isolated_lod = None;
        state.changed = true;
    }
    if keyboard.just_pressed(KeyCode::KeyB) {
        state.profile = ProfileKind::Balanced;
        state.changed = true;
    }
    if keyboard.just_pressed(KeyCode::KeyA) {
        state.profile = ProfileKind::Aggressive;
        state.changed = true;
    }
    if keyboard.just_pressed(KeyCode::KeyC) {
        state.profile = ProfileKind::Conservative;
        state.changed = true;
    }
}

fn apply_viewer_state(
    mut state: ResMut<ViewerState>,
    mut tree_query: Query<(&ViewerTreeMesh, &mut Visibility)>,
    mut overlay: Single<&mut Text, With<ViewerOverlay>>,
) {
    if !state.changed {
        return;
    }
    state.changed = false;

    for (tree, mut visibility) in &mut tree_query {
        let profile_visible = tree.profile == state.profile;
        let lod_visible = state.isolated_lod.is_none_or(|lod| lod == tree.lod);
        *visibility = if profile_visible && lod_visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    let stats = state.active_stats();
    **overlay = format!(
        "Profile: {}\nLOD: {}\nArchetypes: {}\nVertices: {}\nTriangles: {}",
        state.profile.label(),
        state.lod_label(),
        state.archetypes,
        stats.vertices,
        stats.triangles,
    )
    .into();
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
