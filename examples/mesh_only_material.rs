use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use bevy_procedural_tree::presets::{settings_for_preset, TreePreset};
use bevy_procedural_tree::{Leaves, Tree, TreeMaterialPolicy, TreeProceduralGenerationPlugin};

const SHADER_ASSET_PATH: &str = "shaders/tree_wind.wgsl";

type TreeWindMaterial = ExtendedMaterial<StandardMaterial, TreeWindExtension>;

#[derive(Resource, Clone)]
struct LeafMaterial(Handle<TreeWindMaterial>);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            mode: AssetMode::Unprocessed,
            ..default()
        }))
        .add_plugins(MaterialPlugin::<TreeWindMaterial>::default())
        .add_plugins(TreeProceduralGenerationPlugin)
        .insert_resource(ClearColor(Color::srgb(0.56, 0.60, 0.64)))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 850.0,
            ..default()
        })
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, attach_leaf_materials)
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
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut wind_materials: ResMut<Assets<TreeWindMaterial>>,
) {
    let bark_material = wind_materials.add(ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::srgb(0.34, 0.23, 0.14),
            perceptual_roughness: 0.95,
            ..default()
        },
        extension: TreeWindExtension { params: Vec4::ZERO },
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
    commands.insert_resource(LeafMaterial(leaf_material));

    let ground_material = standard_materials.add(StandardMaterial {
        base_color: Color::srgb(0.76, 0.78, 0.67),
        perceptual_roughness: 1.0,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(18.0, 12.0))),
        MeshMaterial3d(ground_material),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));

    commands.spawn((
        Tree {
            seed: 7,
            tree_mesh_settings_override: Some(settings_for_preset(
                TreePreset::WideUmbrellaDeciduous,
                0,
                0.65,
            )),
            material_policy: TreeMaterialPolicy::MeshOnly,
            bark_material_override: None,
            leaf_material_override: None,
        },
        MeshMaterial3d(bark_material),
        Transform::from_xyz(0.0, 0.0, 0.0),
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
        Transform::from_xyz(-5.0, 4.0, 8.0).looking_at(Vec3::new(0.0, 2.5, 0.0), Vec3::Y),
        Tonemapping::None,
    ));
}

fn attach_leaf_materials(
    leaves: Query<Entity, Added<Leaves>>,
    leaf_material: Res<LeafMaterial>,
    mut commands: Commands,
) {
    for entity in &leaves {
        commands
            .entity(entity)
            .insert(MeshMaterial3d(leaf_material.0.clone()));
    }
}

fn animate_wind_material(time: Res<Time>, mut materials: ResMut<Assets<TreeWindMaterial>>) {
    for material in materials.iter_mut().map(|(_, material)| material) {
        material.extension.params.x = time.elapsed_secs();
    }
}
