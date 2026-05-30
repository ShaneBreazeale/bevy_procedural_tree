use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;

use bevy_procedural_tree::cache::{
    read_archetype_cache, write_archetype_cache, CachedArchetypePool,
};
use bevy_procedural_tree::lod::{
    generate_archetypes_from_settings_with_reduction, LodReduction, TreeArchetype,
};
use bevy_procedural_tree::presets::tree_preset_settings;

const ARCHETYPES: u32 = 24;
const LOD_LEVELS: u32 = 3;
const PLACEMENTS: usize = 3_096;
const COURSE_W: f32 = 1_800.0;
const COURSE_D: f32 = 1_200.0;
const BASE_SEED: u64 = 0xCACE_BA7C;
const CACHE_PATH: &str = "target/tree_archetype_pool.bptc";
const DISPLAY_SCALE: f32 = 0.42;
const LOD_CAMERA_POS: Vec3 = Vec3::new(650.0, 2.0, 515.0);

#[derive(Clone, Copy)]
struct Placement {
    pos: Vec3,
    archetype: usize,
    lod: usize,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(ClearColor(Color::srgb(0.55, 0.60, 0.64)))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 700.0,
            ..default()
        })
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cache_path = PathBuf::from(CACHE_PATH);
    let archetypes = load_or_build_cache(&cache_path).expect("tree archetype cache should load");
    let mesh_handles = register_mesh_handles(archetypes, &mut meshes);
    let placements = course_scale_placements(PLACEMENTS, ARCHETYPES);
    let grouped = group_placements(&placements);

    println!(
        "cached batching: {} placements, {} archetypes, {} LODs, {} batches",
        placements.len(),
        mesh_handles.len(),
        LOD_LEVELS,
        grouped.len(),
    );
    for ((archetype, lod), group) in &grouped {
        println!(
            "batch archetype={}, lod={}, placements={}",
            archetype,
            lod,
            group.len()
        );
    }

    let bark_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.34, 0.23, 0.14),
        perceptual_roughness: 0.95,
        ..default()
    });
    let leaf_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.34, 0.72, 0.30),
        perceptual_roughness: 0.85,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    let ground_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.76, 0.62),
        perceptual_roughness: 1.0,
        ..default()
    });

    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(COURSE_W, COURSE_D))),
        MeshMaterial3d(ground_material),
        Transform::from_xyz(COURSE_W * 0.5, -0.01, COURSE_D * 0.5)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));

    for ((archetype, lod), group) in grouped {
        let handles = &mesh_handles[archetype][lod];
        for placement in group {
            let transform =
                Transform::from_translation(placement.pos).with_scale(Vec3::splat(DISPLAY_SCALE));
            commands.spawn((
                Mesh3d(handles.branches.clone()),
                MeshMaterial3d(bark_material.clone()),
                transform,
            ));
            commands.spawn((
                Mesh3d(handles.leaves.clone()),
                MeshMaterial3d(leaf_material.clone()),
                transform,
            ));
        }
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
        Transform::from_xyz(640.0, 240.0, 980.0).looking_at(Vec3::new(780.0, 0.0, 520.0), Vec3::Y),
        Tonemapping::None,
    ));
}

fn load_or_build_cache(path: &Path) -> std::io::Result<Vec<TreeArchetype>> {
    match read_archetype_cache(path) {
        Ok(cache) => {
            println!("loaded tree archetype cache from {}", path.display());
            Ok(cache.into_archetypes())
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            println!(
                "cache missing; generating tree archetypes at {}",
                path.display()
            );
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let settings = tree_preset_settings(ARCHETYPES);
            let archetypes = generate_archetypes_from_settings_with_reduction(
                &settings,
                BASE_SEED,
                LOD_LEVELS,
                LodReduction::balanced(),
            )
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err.to_string()))?;
            let cache = CachedArchetypePool::from_archetypes(&archetypes)?;
            write_archetype_cache(path, &cache)?;
            Ok(archetypes)
        }
        Err(err) => Err(err),
    }
}

#[derive(Clone)]
struct TreeMeshHandles {
    branches: Handle<Mesh>,
    leaves: Handle<Mesh>,
}

fn register_mesh_handles(
    archetypes: Vec<TreeArchetype>,
    meshes: &mut Assets<Mesh>,
) -> Vec<Vec<TreeMeshHandles>> {
    archetypes
        .into_iter()
        .map(|archetype| {
            archetype
                .lods
                .into_iter()
                .map(|(branches, leaves)| TreeMeshHandles {
                    branches: meshes.add(branches),
                    leaves: meshes.add(leaves),
                })
                .collect()
        })
        .collect()
}

fn group_placements(placements: &[Placement]) -> BTreeMap<(usize, usize), Vec<Placement>> {
    let mut grouped: BTreeMap<(usize, usize), Vec<Placement>> = BTreeMap::new();
    for placement in placements {
        grouped
            .entry((placement.archetype, placement.lod))
            .or_default()
            .push(*placement);
    }
    grouped
}

fn course_scale_placements(count: usize, archetypes: u32) -> Vec<Placement> {
    let mut rng = fastrand::Rng::with_seed(0xC011_DA7A);
    (0..count)
        .map(|i| {
            let corridor_t = i as f32 / count.max(1) as f32;
            let along_x = corridor_t * COURSE_W;
            let along_z =
                COURSE_D * (0.35 + 0.18 * (corridor_t * std::f32::consts::TAU * 3.0).sin());
            let background = rng.f32() < 0.38;
            let (x, z) = if background {
                (rng.f32() * COURSE_W, rng.f32() * COURSE_D)
            } else {
                let side = if rng.bool() { 1.0 } else { -1.0 };
                let offset = side * (35.0 + rng.f32() * 130.0);
                (
                    (along_x + (rng.f32() - 0.5) * 30.0).clamp(0.0, COURSE_W),
                    (along_z + offset).clamp(0.0, COURSE_D),
                )
            };
            let pos = Vec3::new(x, 0.0, z);
            let hash = hash2((x * 8.0).round() as u32, (z * 8.0).round() as u32);
            Placement {
                pos,
                archetype: (hash as usize) % archetypes as usize,
                lod: lod_for_distance(LOD_CAMERA_POS.distance(pos)),
            }
        })
        .collect()
}

fn lod_for_distance(distance: f32) -> usize {
    if distance <= 50.0 {
        0
    } else if distance <= 200.0 {
        1
    } else {
        2
    }
}

fn hash2(mut x: u32, mut y: u32) -> u32 {
    x = x.wrapping_mul(0x85eb_ca6b);
    y ^= x.rotate_left(13);
    y = y.wrapping_mul(0xc2b2_ae35);
    y ^ (y >> 16)
}
