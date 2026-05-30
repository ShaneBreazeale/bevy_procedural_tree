use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use bevy_procedural_tree::meshgen::generate_tree_meshes;
use bevy_procedural_tree::presets::{settings_for_preset, TreePreset};

const SHADER_ASSET_PATH: &str = "shaders/tree_wind.wgsl";
const TREE_COUNT: usize = 6;

type TreeWindMaterial = ExtendedMaterial<StandardMaterial, TreeWindExtension>;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            mode: AssetMode::Unprocessed,
            ..default()
        }))
        .add_plugins(MaterialPlugin::<TreeWindMaterial>::default())
        .insert_resource(ClearColor(Color::srgb(0.56, 0.60, 0.64)))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 850.0,
            ..default()
        })
        .add_systems(Startup, setup)
        .add_systems(Update, animate_wind_material)
        .run();
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct TreeWindExtension {
    /// x=time, y=speed, z=strength, w=turbulence.
    #[uniform(100)]
    params: Vec4,
}

impl MaterialExtension for TreeWindExtension {
    fn vertex_shader() -> ShaderRef {
        SHADER_ASSET_PATH.into()
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut bark_materials: ResMut<Assets<StandardMaterial>>,
    mut wind_materials: ResMut<Assets<TreeWindMaterial>>,
) {
    let bark_material = bark_materials.add(StandardMaterial {
        base_color: Color::srgb(0.34, 0.23, 0.14),
        perceptual_roughness: 0.95,
        ..default()
    });
    let leaf_material = wind_materials.add(ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::srgb(0.31, 0.68, 0.27),
            perceptual_roughness: 0.85,
            cull_mode: None,
            double_sided: true,
            ..default()
        },
        extension: TreeWindExtension {
            params: Vec4::new(0.0, 1.8, 0.16, 0.65),
        },
    });
    let ground_material = bark_materials.add(StandardMaterial {
        base_color: Color::srgb(0.76, 0.78, 0.67),
        perceptual_roughness: 1.0,
        ..default()
    });

    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(80.0, 44.0))),
        MeshMaterial3d(ground_material),
        Transform::from_xyz(0.0, -0.01, 0.0)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));

    let presets = [
        TreePreset::CompactRoundDeciduous,
        TreePreset::WideUmbrellaDeciduous,
        TreePreset::TallSparseDeciduous,
        TreePreset::LeaningEdgeDeciduous,
        TreePreset::ScrubOrnamental,
        TreePreset::TallNarrowConifer,
    ];
    for i in 0..TREE_COUNT {
        let settings = settings_for_preset(presets[i % presets.len()], i as u32, 0.45);
        let (branches, leaves) =
            generate_tree_meshes(&settings, &mut fastrand::Rng::with_seed(100 + i as u64))
                .expect("wind example tree generation should succeed");
        let min_y = mesh_min_y(&branches).min(mesh_min_y(&leaves));
        let y_offset = if min_y.is_finite() { -min_y } else { 0.0 };
        let x = (i as f32 - (TREE_COUNT - 1) as f32 * 0.5) * 10.0;
        let z = if i % 2 == 0 { -5.0 } else { 5.0 };
        let scale = 0.55 + (i % 3) as f32 * 0.06;
        let transform = Transform::from_xyz(x, y_offset * scale, z).with_scale(Vec3::splat(scale));

        commands.spawn((
            Mesh3d(meshes.add(branches)),
            MeshMaterial3d(bark_material.clone()),
            transform,
        ));
        commands.spawn((
            Mesh3d(meshes.add(leaves)),
            MeshMaterial3d(leaf_material.clone()),
            transform,
        ));
    }

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
        Transform::from_xyz(-24.0, 15.0, 32.0).looking_at(Vec3::new(0.0, 4.0, 0.0), Vec3::Y),
        Tonemapping::None,
    ));
}

fn animate_wind_material(time: Res<Time>, mut materials: ResMut<Assets<TreeWindMaterial>>) {
    for material in materials.iter_mut().map(|(_, material)| material) {
        material.extension.params.x = time.elapsed_secs();
    }
}

fn mesh_min_y(mesh: &Mesh) -> f32 {
    match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) => {
            positions.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min)
        }
        _ => 0.0,
    }
}
