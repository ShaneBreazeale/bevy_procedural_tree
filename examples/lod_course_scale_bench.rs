#![cfg_attr(not(feature = "u32_indices"), allow(dead_code, unused_imports))]

use std::collections::BTreeSet;
use std::time::Instant;

use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::*;

use bevy_procedural_tree::enums::TreeType;
use bevy_procedural_tree::lod::{
    generate_archetypes_from_settings_with_reduction, LodReduction, TreeArchetype,
};
use bevy_procedural_tree::settings::TreeMeshSettings;

const ARCHETYPES: u32 = 24;
const LOD_LEVELS: u32 = 3;
const PLACEMENTS: usize = 3_096;
const COURSE_W: f32 = 1_800.0;
const COURSE_D: f32 = 1_200.0;
const BASE_SEED: u64 = 0xB06E_590F;

#[derive(Clone, Copy)]
struct Profile {
    name: &'static str,
    reduction: LodReduction,
}

#[derive(Clone, Copy)]
struct Placement {
    pos: Vec3,
    archetype: usize,
}

#[derive(Clone, Copy)]
struct CameraSample {
    name: &'static str,
    pos: Vec3,
}

#[derive(Clone, Copy, Default)]
struct MeshStats {
    branch_vertices: usize,
    branch_indices: usize,
    branch_tris: usize,
    leaf_vertices: usize,
    leaf_indices: usize,
    leaf_tris: usize,
}

#[derive(Default)]
struct ProfileSummary {
    samples: usize,
    expanded_vertices: usize,
    expanded_tris: usize,
    unique_pool_vertices: usize,
    unique_pool_tris: usize,
    max_unique_batches: usize,
}

impl MeshStats {
    fn total_vertices(self) -> usize {
        self.branch_vertices + self.leaf_vertices
    }

    fn total_tris(self) -> usize {
        self.branch_tris + self.leaf_tris
    }

    fn estimated_bytes(self) -> usize {
        // position + normal + uv per vertex, plus u32-equivalent index storage.
        self.total_vertices() * (3 * 4 + 3 * 4 + 2 * 4)
            + (self.branch_indices + self.leaf_indices) * 4
    }
}

#[cfg(not(feature = "u32_indices"))]
fn main() {
    eprintln!(
        "lod_course_scale_bench needs large meshes; run with `cargo run --example lod_course_scale_bench --features u32_indices`"
    );
}

#[cfg(feature = "u32_indices")]
fn main() {
    let settings = archetype_settings(ARCHETYPES);
    let placements = course_scale_placements(PLACEMENTS, ARCHETYPES);
    let cameras = camera_samples();

    println!("mesh_stats_csv");
    println!(
        "profile,archetype,lod,branch_vertices,leaf_vertices,total_vertices,branch_tris,leaf_tris,total_tris,estimated_bytes,gen_us"
    );

    let mut generated = Vec::new();
    for profile in profiles() {
        let started = Instant::now();
        let archetypes = generate_archetypes_from_settings_with_reduction(
            &settings,
            BASE_SEED,
            LOD_LEVELS,
            profile.reduction,
        )
        .expect("archetype generation should succeed");
        let total_gen_us = started.elapsed().as_micros() as u64;

        let stats = archetype_stats(&archetypes);
        let rows = ARCHETYPES as usize * LOD_LEVELS as usize;
        let avg_gen_us = total_gen_us / rows.max(1) as u64;

        for (archetype_i, lod_stats) in stats.iter().enumerate() {
            for (lod, stat) in lod_stats.iter().enumerate() {
                println!(
                    "{},{},{},{},{},{},{},{},{},{},{}",
                    profile.name,
                    archetype_i,
                    lod,
                    stat.branch_vertices,
                    stat.leaf_vertices,
                    stat.total_vertices(),
                    stat.branch_tris,
                    stat.leaf_tris,
                    stat.total_tris(),
                    stat.estimated_bytes(),
                    avg_gen_us,
                );
            }
        }

        generated.push((profile, stats));
    }

    println!();
    println!("course_camera_stats_csv");
    println!(
        "profile,camera,lod0_count,lod1_count,lod2_count,cull_count,expanded_vertices,expanded_tris,unique_archetype_lod_batches,unique_pool_vertices,unique_pool_tris"
    );

    let mut summaries = Vec::new();
    for (profile, stats) in &generated {
        let mut summary = ProfileSummary::default();
        for camera in &cameras {
            let mut lod_counts = [0usize; LOD_LEVELS as usize];
            let mut cull_count = 0usize;
            let mut expanded_vertices = 0usize;
            let mut expanded_tris = 0usize;
            let mut used_keys = BTreeSet::new();

            for placement in &placements {
                let Some(lod) = lod_for_distance(camera.pos.distance(placement.pos)) else {
                    cull_count += 1;
                    continue;
                };
                let stat = stats[placement.archetype][lod];
                lod_counts[lod] += 1;
                expanded_vertices += stat.total_vertices();
                expanded_tris += stat.total_tris();
                used_keys.insert((placement.archetype, lod));
            }

            let mut unique_pool_vertices = 0usize;
            let mut unique_pool_tris = 0usize;
            for (archetype, lod) in &used_keys {
                let stat = stats[*archetype][*lod];
                unique_pool_vertices += stat.total_vertices();
                unique_pool_tris += stat.total_tris();
            }

            println!(
                "{},{},{},{},{},{},{},{},{},{},{}",
                profile.name,
                camera.name,
                lod_counts[0],
                lod_counts[1],
                lod_counts[2],
                cull_count,
                expanded_vertices,
                expanded_tris,
                used_keys.len(),
                unique_pool_vertices,
                unique_pool_tris,
            );

            summary.samples += 1;
            summary.expanded_vertices += expanded_vertices;
            summary.expanded_tris += expanded_tris;
            summary.unique_pool_vertices += unique_pool_vertices;
            summary.unique_pool_tris += unique_pool_tris;
            summary.max_unique_batches = summary.max_unique_batches.max(used_keys.len());
        }
        summaries.push((profile.name, summary));
    }

    println!();
    println!("profile_summary_csv");
    println!(
        "profile,avg_expanded_vertices,avg_expanded_tris,avg_unique_pool_vertices,avg_unique_pool_tris,max_unique_archetype_lod_batches"
    );
    for (profile, summary) in summaries {
        let samples = summary.samples.max(1);
        println!(
            "{},{},{},{},{},{}",
            profile,
            summary.expanded_vertices / samples,
            summary.expanded_tris / samples,
            summary.unique_pool_vertices / samples,
            summary.unique_pool_tris / samples,
            summary.max_unique_batches,
        );
    }
}

fn profiles() -> [Profile; 4] {
    [
        Profile {
            name: "conservative",
            reduction: LodReduction::conservative(),
        },
        Profile {
            name: "balanced",
            reduction: LodReduction::balanced(),
        },
        Profile {
            name: "aggressive",
            reduction: LodReduction::aggressive(),
        },
        Profile {
            name: "far_only",
            reduction: LodReduction {
                detail_loss_per_level: 0.4,
                min_segments: 4,
                min_sections: 2,
                section_preserve: 0.75,
                leaf_loss_per_level: 0.25,
                min_leaf_count: 2,
                single_leaf_billboard_from_level: 2,
                leaf_size_compensation: 0.15,
            },
        },
    ]
}

fn lod_for_distance(distance: f32) -> Option<usize> {
    if distance <= 50.0 {
        Some(0)
    } else if distance <= 200.0 {
        Some(1)
    } else if distance <= 1_000.0 {
        Some(2)
    } else {
        None
    }
}

fn archetype_stats(archetypes: &[TreeArchetype]) -> Vec<Vec<MeshStats>> {
    archetypes
        .iter()
        .map(|archetype| {
            archetype
                .lods
                .iter()
                .map(|(branch, leaf)| MeshStats {
                    branch_vertices: vertex_count(branch),
                    branch_indices: index_count(branch),
                    branch_tris: triangle_count(branch),
                    leaf_vertices: vertex_count(leaf),
                    leaf_indices: index_count(leaf),
                    leaf_tris: triangle_count(leaf),
                })
                .collect()
        })
        .collect()
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

fn triangle_count(mesh: &Mesh) -> usize {
    index_count(mesh) / 3
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
            let hash = hash2((x * 8.0).round() as u32, (z * 8.0).round() as u32);
            Placement {
                pos: Vec3::new(x, 0.0, z),
                archetype: (hash as usize) % archetypes as usize,
            }
        })
        .collect()
}

fn camera_samples() -> [CameraSample; 5] {
    [
        CameraSample {
            name: "hole_1_tee",
            pos: Vec3::new(120.0, 2.0, 410.0),
        },
        CameraSample {
            name: "fairway_mid",
            pos: Vec3::new(650.0, 2.0, 515.0),
        },
        CameraSample {
            name: "green_approach",
            pos: Vec3::new(1_110.0, 2.0, 455.0),
        },
        CameraSample {
            name: "elevated_overview",
            pos: Vec3::new(900.0, 180.0, 600.0),
        },
        CameraSample {
            name: "back_nine_edge",
            pos: Vec3::new(1_550.0, 3.0, 760.0),
        },
    ]
}

fn archetype_settings(count: u32) -> Vec<TreeMeshSettings> {
    (0..count)
        .map(|i| {
            let mut settings = TreeMeshSettings::default();
            let t = i as f32 / count.saturating_sub(1).max(1) as f32;
            let wave = (i as f32 * 1.618_034).sin() * 0.5 + 0.5;
            match i % 6 {
                0 => {
                    settings.branch.length = [3.6 + wave * 0.4, 2.2, 1.2, 0.32];
                    settings.branch.angle = [0.0, 46.0, 54.0, 66.0];
                    settings.branch.children = [6, 4, 9];
                    settings.branch.trunk_base_radius = 0.17;
                    settings.leaves.count = 4;
                    settings.leaves.size = 0.22;
                }
                1 => {
                    settings.branch.length = [5.7 + wave, 2.4, 1.0, 0.28];
                    settings.branch.angle = [0.0, 23.0, 30.0, 38.0];
                    settings.branch.children = [5, 4, 6];
                    settings.branch.force.direction = Vec3::Y;
                    settings.branch.force.strength = 0.16;
                    settings.branch.trunk_base_radius = 0.20;
                    settings.leaves.count = 3;
                    settings.leaves.size = 0.20;
                }
                2 => {
                    settings.branch.length = [3.2, 3.4 + wave * 0.7, 2.0, 0.55];
                    settings.branch.angle = [0.0, 70.0, 62.0, 58.0];
                    settings.branch.children = [7, 5, 12];
                    settings.branch.force.direction = Vec3::new(0.0, 0.35, 0.0);
                    settings.branch.force.strength = 0.03;
                    settings.branch.trunk_base_radius = 0.24;
                    settings.leaves.count = 5;
                    settings.leaves.size = 0.28;
                }
                3 => {
                    settings.tree_type = TreeType::Evergreen;
                    settings.branch.length = [6.8 + wave * 1.0, 2.0, 0.9, 0.30];
                    settings.branch.angle = [0.0, 66.0, 58.0, 48.0];
                    settings.branch.children = [14, 6, 7];
                    settings.branch.force.direction = Vec3::new(0.0, -0.35, 0.0);
                    settings.branch.force.strength = 0.18;
                    settings.branch.trunk_base_radius = 0.17;
                    settings.leaves.count = 5;
                    settings.leaves.size = 0.20;
                }
                4 => {
                    settings.branch.length = [4.8 + t * 0.8, 3.3, 1.8, 0.45];
                    settings.branch.angle = [0.0, 40.0, 45.0, 52.0];
                    settings.branch.children = [4, 3, 5];
                    settings.branch.gnarliness = [0.02, 0.34, 0.28, 0.08];
                    settings.branch.trunk_base_radius = 0.22;
                    settings.leaves.count = 2;
                    settings.leaves.size = 0.34;
                    settings.leaves.size_variance = 0.45;
                }
                _ => {
                    settings.branch.length = [4.3 + wave * 0.6, 3.0, 1.7, 0.48];
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
        })
        .collect()
}

fn hash2(mut x: u32, mut y: u32) -> u32 {
    x = x.wrapping_mul(0x85eb_ca6b);
    y ^= x.rotate_left(13);
    y = y.wrapping_mul(0xc2b2_ae35);
    y ^ (y >> 16)
}
