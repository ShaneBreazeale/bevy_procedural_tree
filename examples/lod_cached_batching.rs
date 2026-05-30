use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::mesh::{Indices, MeshVertexAttribute, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use bevy_procedural_tree::cache::{
    archetype_cache_key, archetype_cache_key_matches, read_archetype_cache, write_archetype_cache,
    write_archetype_cache_key, CachedArchetypePool,
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
const CACHE_KEY_PATH: &str = "target/tree_archetype_pool.bptc.key";
const DISPLAY_SCALE: f32 = 0.42;
const CHUNK_M: f32 = 300.0;
const LOD_CAMERA_POS: Vec3 = Vec3::new(650.0, 2.0, 515.0);

#[derive(Clone, Copy)]
struct Placement {
    pos: Vec3,
    yaw: f32,
    scale: f32,
    archetype: usize,
    lod: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct BatchKey {
    chunk_x: i32,
    chunk_z: i32,
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
    let cache_key_path = PathBuf::from(CACHE_KEY_PATH);
    let archetypes = load_or_build_cache(&cache_path, &cache_key_path)
        .expect("tree archetype cache should load");
    let placements = course_scale_placements(PLACEMENTS, ARCHETYPES);
    let grouped = group_placements(&placements);

    println!(
        "cached chunk batching: {} placements, {} archetypes, {} LODs, {} chunk batches",
        placements.len(),
        archetypes.len(),
        LOD_LEVELS,
        grouped.len(),
    );
    for (key, group) in grouped.iter().take(24) {
        println!(
            "batch chunk=({},{}), archetype={}, lod={}, placements={}",
            key.chunk_x,
            key.chunk_z,
            key.archetype,
            key.lod,
            group.len()
        );
    }
    if grouped.len() > 24 {
        println!("... {} more chunk batches", grouped.len() - 24);
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

    let mut branch_chunk_meshes = 0usize;
    let mut leaf_chunk_meshes = 0usize;
    for (key, group) in grouped {
        let (branches, leaves) = &archetypes[key.archetype].lods[key.lod];
        commands.spawn((
            Mesh3d(meshes.add(combine_mesh_instances(branches, &group))),
            MeshMaterial3d(bark_material.clone()),
            Transform::default(),
        ));
        commands.spawn((
            Mesh3d(meshes.add(combine_mesh_instances(leaves, &group))),
            MeshMaterial3d(leaf_material.clone()),
            Transform::default(),
        ));
        branch_chunk_meshes += 1;
        leaf_chunk_meshes += 1;
    }
    println!(
        "spawned {} combined chunk meshes ({} branch + {} leaf)",
        branch_chunk_meshes + leaf_chunk_meshes,
        branch_chunk_meshes,
        leaf_chunk_meshes
    );

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

fn load_or_build_cache(path: &Path, key_path: &Path) -> std::io::Result<Vec<TreeArchetype>> {
    let settings = tree_preset_settings(ARCHETYPES);
    let reduction = LodReduction::balanced();
    let expected_key = archetype_cache_key(&settings, BASE_SEED, LOD_LEVELS, reduction);

    if archetype_cache_key_matches(key_path, expected_key)? {
        match read_archetype_cache(path) {
            Ok(cache) => {
                println!("loaded fresh tree archetype cache from {}", path.display());
                return Ok(cache.into_archetypes());
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                println!("cache key matched, but cache file is missing; rebuilding");
            }
            Err(err) => return Err(err),
        }
    } else {
        println!("tree archetype cache key is missing or stale; rebuilding");
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let archetypes = generate_archetypes_from_settings_with_reduction(
        &settings, BASE_SEED, LOD_LEVELS, reduction,
    )
    .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err.to_string()))?;
    let cache = CachedArchetypePool::from_archetypes(&archetypes)?;
    write_archetype_cache(path, &cache)?;
    write_archetype_cache_key(key_path, expected_key)?;
    println!(
        "wrote tree archetype cache {} and key {}",
        path.display(),
        key_path.display()
    );
    Ok(archetypes)
}

fn group_placements(placements: &[Placement]) -> BTreeMap<BatchKey, Vec<Placement>> {
    let mut grouped: BTreeMap<BatchKey, Vec<Placement>> = BTreeMap::new();
    for placement in placements {
        let key = BatchKey {
            chunk_x: chunk_index(placement.pos.x, COURSE_W),
            chunk_z: chunk_index(placement.pos.z, COURSE_D),
            archetype: placement.archetype,
            lod: placement.lod,
        };
        grouped.entry(key).or_default().push(*placement);
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
            let hash_f = hash as f32 / u32::MAX as f32;
            let hash2_f = hash.rotate_left(11) as f32 / u32::MAX as f32;
            Placement {
                pos,
                yaw: hash_f * std::f32::consts::TAU,
                scale: DISPLAY_SCALE * (0.85 + hash2_f * 0.3),
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

fn chunk_index(value: f32, span: f32) -> i32 {
    let max_chunk = (span / CHUNK_M).ceil() as i32 - 1;
    ((value / CHUNK_M).floor() as i32).clamp(0, max_chunk.max(0))
}

fn hash2(mut x: u32, mut y: u32) -> u32 {
    x = x.wrapping_mul(0x85eb_ca6b);
    y ^= x.rotate_left(13);
    y = y.wrapping_mul(0xc2b2_ae35);
    y ^ (y >> 16)
}

fn combine_mesh_instances(source: &Mesh, placements: &[Placement]) -> Mesh {
    let positions = float3_attribute(source, Mesh::ATTRIBUTE_POSITION);
    let normals = float3_attribute(source, Mesh::ATTRIBUTE_NORMAL);
    let uvs = float2_attribute(source, Mesh::ATTRIBUTE_UV_0);
    let tangents = float4_attribute(source, Mesh::ATTRIBUTE_TANGENT);
    let source_indices = mesh_indices(source);

    let vertex_count = positions.len() * placements.len();
    let mut out_positions = Vec::with_capacity(vertex_count);
    let mut out_normals = Vec::with_capacity(vertex_count);
    let mut out_uvs = Vec::with_capacity(vertex_count);
    let mut out_tangents = tangents.as_ref().map(|_| Vec::with_capacity(vertex_count));
    let mut out_indices = Vec::with_capacity(source_indices.len() * placements.len());

    for placement in placements {
        let rotation = Quat::from_rotation_y(placement.yaw);
        let base = out_positions.len() as u32;
        for (i, position) in positions.iter().enumerate() {
            let transformed =
                rotation.mul_vec3(Vec3::from_array(*position)) * placement.scale + placement.pos;
            out_positions.push(transformed.to_array());
            out_normals.push(rotation.mul_vec3(Vec3::from_array(normals[i])).to_array());
            out_uvs.push(uvs[i]);
            if let (Some(source_tangents), Some(out_tangents)) = (&tangents, &mut out_tangents) {
                let tangent = source_tangents[i];
                let rotated = rotation.mul_vec3(Vec3::new(tangent[0], tangent[1], tangent[2]));
                out_tangents.push([rotated.x, rotated.y, rotated.z, tangent[3]]);
            }
        }
        out_indices.extend(source_indices.iter().map(|index| base + *index));
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, out_positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, out_normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, out_uvs);
    if let Some(out_tangents) = out_tangents {
        mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, out_tangents);
    }
    mesh.insert_indices(Indices::U32(out_indices));
    mesh
}

fn float3_attribute(mesh: &Mesh, attribute: MeshVertexAttribute) -> &Vec<[f32; 3]> {
    match mesh.attribute(attribute) {
        Some(VertexAttributeValues::Float32x3(values)) => values,
        _ => panic!("source mesh is missing Float32x3 attribute"),
    }
}

fn float2_attribute(mesh: &Mesh, attribute: MeshVertexAttribute) -> &Vec<[f32; 2]> {
    match mesh.attribute(attribute) {
        Some(VertexAttributeValues::Float32x2(values)) => values,
        _ => panic!("source mesh is missing Float32x2 attribute"),
    }
}

fn float4_attribute(mesh: &Mesh, attribute: MeshVertexAttribute) -> Option<Vec<[f32; 4]>> {
    match mesh.attribute(attribute) {
        Some(VertexAttributeValues::Float32x4(values)) => Some(values.clone()),
        None => None,
        _ => panic!("source mesh has non-Float32x4 tangent attribute"),
    }
}

fn mesh_indices(mesh: &Mesh) -> Vec<u32> {
    match mesh.indices() {
        Some(Indices::U16(indices)) => indices.iter().map(|index| *index as u32).collect(),
        Some(Indices::U32(indices)) => indices.clone(),
        None => (0..float3_attribute(mesh, Mesh::ATTRIBUTE_POSITION).len() as u32).collect(),
    }
}
