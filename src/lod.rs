//! Helpers for generating lower-detail trees and seed-stable archetype pools.
//!
//! The functions in this module sit on top of [`crate::meshgen::generate_tree_meshes`].
//! They do not spawn entities, store resources, or choose materials. This keeps
//! them useful for games that want to generate a small pool of meshes once, then
//! handle their own caching, instancing, batching, and runtime LOD selection.

use bevy::prelude::*;
use fastrand::Rng;

use crate::enums::LeafBillboard;
use crate::meshgen::generate_tree_meshes;
use crate::settings::TreeMeshSettings;

/// One generated tree: branch mesh and leaf mesh.
pub type TreeMeshes = (Mesh, Mesh);

/// Controls how aggressively [`reduce_settings_for_lod`] lowers mesh detail.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodReduction {
    /// Added to the LOD denominator per level.
    ///
    /// The radial segment factor is:
    ///
    /// `1.0 / (1.0 + detail_loss_per_level * level)`
    ///
    /// For example, `2.0` gives factors `1.0`, `0.333`, `0.2` for levels
    /// 0, 1, and 2.
    pub detail_loss_per_level: f32,
    /// Minimum radial segments per branch ring.
    pub min_segments: u8,
    /// Minimum sections along each branch.
    pub min_sections: u8,
    /// Fraction of branch sections to preserve even at high LOD levels.
    ///
    /// A value of `0.4` means section counts use `0.4 + 0.6 * radial_factor`.
    pub section_preserve: f32,
    /// Added to the leaf-count denominator per level.
    ///
    /// The leaf-count factor is:
    ///
    /// `1.0 / (1.0 + leaf_loss_per_level * level)`
    pub leaf_loss_per_level: f32,
    /// Minimum leaves per terminal branch.
    pub min_leaf_count: u32,
    /// First LOD level that switches double leaf billboards to single billboards.
    ///
    /// This halves leaf-card vertices at far LODs while preserving approximate
    /// coverage through [`Self::leaf_size_compensation`].
    pub single_leaf_billboard_from_level: u32,
    /// How much to grow leaf cards as leaf count drops.
    ///
    /// `0.25` means a leaf-count factor of `0.5` increases leaf size by 12.5%.
    pub leaf_size_compensation: f32,
}

impl LodReduction {
    /// Creates a custom LOD reduction profile.
    pub const fn new(
        detail_loss_per_level: f32,
        min_segments: u8,
        min_sections: u8,
        section_preserve: f32,
    ) -> Self {
        Self {
            detail_loss_per_level,
            min_segments,
            min_sections,
            section_preserve,
            leaf_loss_per_level: 0.0,
            min_leaf_count: 1,
            single_leaf_billboard_from_level: u32::MAX,
            leaf_size_compensation: 0.0,
        }
    }

    /// Aggressive profile intended for stylized or distant background trees.
    pub const fn aggressive() -> Self {
        Self {
            detail_loss_per_level: 2.0,
            min_segments: 3,
            min_sections: 1,
            section_preserve: 0.4,
            leaf_loss_per_level: 0.75,
            min_leaf_count: 1,
            single_leaf_billboard_from_level: 2,
            leaf_size_compensation: 0.35,
        }
    }

    /// Gentler profile intended for visible hero trees where LOD popping matters more.
    pub const fn conservative() -> Self {
        Self {
            detail_loss_per_level: 1.0,
            min_segments: 4,
            min_sections: 1,
            section_preserve: 0.5,
            leaf_loss_per_level: 0.35,
            min_leaf_count: 1,
            single_leaf_billboard_from_level: 3,
            leaf_size_compensation: 0.2,
        }
    }

    fn radial_factor(self, level: u32) -> f32 {
        let loss = self.detail_loss_per_level.max(0.0);
        1.0 / (1.0 + loss * level as f32)
    }

    fn section_factor(self, level: u32) -> f32 {
        let preserve = self.section_preserve.clamp(0.0, 1.0);
        preserve + (1.0 - preserve) * self.radial_factor(level)
    }

    fn leaf_factor(self, level: u32) -> f32 {
        let loss = self.leaf_loss_per_level.max(0.0);
        1.0 / (1.0 + loss * level as f32)
    }
}

impl Default for LodReduction {
    fn default() -> Self {
        Self::aggressive()
    }
}

/// Returns a copy of `base` with lower branch tessellation for `level`.
///
/// Level 0 keeps the original settings. Higher levels reduce the per-branch
/// `segments`, `sections`, and leaf-card density while leaving lengths,
/// branching counts, force, and seed behavior unchanged. Regenerating with the
/// same seed therefore produces the same broad tree shape with fewer polygons.
pub fn reduce_settings_for_lod(
    base: &TreeMeshSettings,
    level: u32,
    reduction: LodReduction,
) -> TreeMeshSettings {
    let mut settings = base.clone();
    let radial_factor = reduction.radial_factor(level);
    let section_factor = reduction.section_factor(level);
    let leaf_factor = reduction.leaf_factor(level);
    let min_segments = reduction.min_segments.max(3);
    let min_sections = reduction.min_sections.max(1);
    let min_leaf_count = reduction.min_leaf_count.max(1);

    for branch_level in 0..4 {
        let segments = (base.branch.segments[branch_level] as f32 * radial_factor).round() as u8;
        let sections = (base.branch.sections[branch_level] as f32 * section_factor).round() as u8;

        settings.branch.segments[branch_level] = segments.max(min_segments);
        settings.branch.sections[branch_level] = sections.max(min_sections);
    }

    settings.leaves.count = ((base.leaves.count as f32 * leaf_factor).round() as u32)
        .max(min_leaf_count)
        .min(base.leaves.count.max(min_leaf_count));
    settings.leaves.size =
        base.leaves.size * (1.0 + (1.0 - leaf_factor) * reduction.leaf_size_compensation.max(0.0));
    if level >= reduction.single_leaf_billboard_from_level {
        settings.leaves.leaf_billboard = LeafBillboard::Single;
    }

    settings
}

/// Returns a lower-detail settings copy using [`LodReduction::default`].
pub fn lod_settings(base: &TreeMeshSettings, level: u32) -> TreeMeshSettings {
    reduce_settings_for_lod(base, level, LodReduction::default())
}

/// Generates `levels` LOD meshes for a single seed using a custom reduction.
pub fn generate_tree_lods_with_reduction(
    base: &TreeMeshSettings,
    seed: u64,
    levels: u32,
    reduction: LodReduction,
) -> Result<Vec<TreeMeshes>, BevyError> {
    (0..levels)
        .map(|level| {
            let settings = reduce_settings_for_lod(base, level, reduction);
            generate_tree_meshes(&settings, &mut Rng::with_seed(seed))
        })
        .collect()
}

/// Generates `levels` LOD meshes for a single seed using [`LodReduction::default`].
pub fn generate_tree_lods(
    base: &TreeMeshSettings,
    seed: u64,
    levels: u32,
) -> Result<Vec<TreeMeshes>, BevyError> {
    generate_tree_lods_with_reduction(base, seed, levels, LodReduction::default())
}

/// One seed-jittered tree variant with all generated LOD meshes.
pub struct TreeArchetype {
    /// Seed used to generate every LOD for this archetype.
    pub seed: u64,
    /// LOD meshes in near-to-far order. `lods[0]` is full detail.
    pub lods: Vec<TreeMeshes>,
}

/// Deterministically derives the seed for one archetype in a pool.
pub fn archetype_seed(base_seed: u64, archetype_index: u32) -> u64 {
    base_seed.wrapping_add(archetype_index as u64)
}

/// Generates a seed-stable archetype pool using a custom LOD reduction.
///
/// The returned vector length is `count`; each entry contains `lod_levels` LOD
/// meshes. Consumers can pick an archetype by hashing world position or gameplay
/// metadata, then batch or instance all placements that choose the same
/// archetype and LOD.
pub fn generate_archetypes_with_reduction(
    base: &TreeMeshSettings,
    base_seed: u64,
    count: u32,
    lod_levels: u32,
    reduction: LodReduction,
) -> Result<Vec<TreeArchetype>, BevyError> {
    (0..count)
        .map(|index| {
            let seed = archetype_seed(base_seed, index);
            generate_tree_lods_with_reduction(base, seed, lod_levels, reduction)
                .map(|lods| TreeArchetype { seed, lods })
        })
        .collect()
}

/// Generates one archetype per settings entry using a custom LOD reduction.
///
/// Use this when seed-only variation is not enough and each archetype should
/// have a distinct silhouette, species, height, branch spread, or leaf profile.
/// The seed for entry `i` is still derived from `base_seed`, so the pool remains
/// deterministic.
pub fn generate_archetypes_from_settings_with_reduction(
    settings: &[TreeMeshSettings],
    base_seed: u64,
    lod_levels: u32,
    reduction: LodReduction,
) -> Result<Vec<TreeArchetype>, BevyError> {
    settings
        .iter()
        .enumerate()
        .map(|(index, settings)| {
            let seed = archetype_seed(base_seed, index as u32);
            generate_tree_lods_with_reduction(settings, seed, lod_levels, reduction)
                .map(|lods| TreeArchetype { seed, lods })
        })
        .collect()
}

/// Generates one archetype per settings entry using [`LodReduction::default`].
pub fn generate_archetypes_from_settings(
    settings: &[TreeMeshSettings],
    base_seed: u64,
    lod_levels: u32,
) -> Result<Vec<TreeArchetype>, BevyError> {
    generate_archetypes_from_settings_with_reduction(
        settings,
        base_seed,
        lod_levels,
        LodReduction::default(),
    )
}

/// Generates a seed-stable archetype pool using [`LodReduction::default`].
pub fn generate_archetypes(
    base: &TreeMeshSettings,
    base_seed: u64,
    count: u32,
    lod_levels: u32,
) -> Result<Vec<TreeArchetype>, BevyError> {
    generate_archetypes_with_reduction(base, base_seed, count, lod_levels, LodReduction::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    fn position_count(mesh: &Mesh) -> usize {
        match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(positions)) => positions.len(),
            _ => 0,
        }
    }

    #[test]
    fn lod_zero_keeps_base_settings() {
        let base = TreeMeshSettings::default();

        assert_eq!(lod_settings(&base, 0), base);
    }

    #[test]
    fn aggressive_lod_reduces_segments_and_sections() {
        let base = TreeMeshSettings::default();

        let lod1 = lod_settings(&base, 1);
        let lod2 = lod_settings(&base, 2);

        assert_eq!(lod1.branch.segments, [3, 3, 3, 3]);
        assert_eq!(lod1.branch.sections, [7, 5, 4, 2]);
        assert_eq!(lod2.branch.segments, [3, 3, 3, 3]);
        assert_eq!(lod2.branch.sections, [6, 4, 3, 2]);
        assert_eq!(lod1.leaves.count, 2);
        assert_eq!(lod2.leaves.count, 1);
        assert_eq!(lod2.leaves.leaf_billboard, LeafBillboard::Single);
    }

    #[test]
    fn generated_lods_reduce_branch_and_leaf_vertex_count() {
        let base = TreeMeshSettings::default();
        let lods = generate_tree_lods(&base, 123, 3).expect("LOD generation should succeed");

        assert_eq!(lods.len(), 3);

        let lod0_vertices = position_count(&lods[0].0);
        let lod1_vertices = position_count(&lods[1].0);
        let lod2_vertices = position_count(&lods[2].0);
        let lod0_leaf_vertices = position_count(&lods[0].1);
        let lod1_leaf_vertices = position_count(&lods[1].1);
        let lod2_leaf_vertices = position_count(&lods[2].1);

        assert!(lod0_vertices > lod1_vertices);
        assert!(lod1_vertices >= lod2_vertices);
        assert!(lod0_leaf_vertices > lod1_leaf_vertices);
        assert!(lod1_leaf_vertices > lod2_leaf_vertices);
    }

    #[test]
    fn archetype_pool_has_deterministic_seeds_and_lod_counts() {
        let base = TreeMeshSettings::default();
        let archetypes =
            generate_archetypes(&base, 9000, 4, 2).expect("archetype generation should succeed");

        assert_eq!(archetypes.len(), 4);
        for (index, archetype) in archetypes.iter().enumerate() {
            assert_eq!(archetype.seed, archetype_seed(9000, index as u32));
            assert_eq!(archetype.lods.len(), 2);
        }
    }

    #[test]
    fn settings_pool_generates_one_archetype_per_settings_entry() {
        let mut settings = vec![TreeMeshSettings::default(); 3];
        settings[1].branch.length[0] *= 0.75;
        settings[2].branch.length[0] *= 1.35;

        let archetypes = generate_archetypes_from_settings(&settings, 7000, 2)
            .expect("settings-pool archetype generation should succeed");

        assert_eq!(archetypes.len(), settings.len());
        for (index, archetype) in archetypes.iter().enumerate() {
            assert_eq!(archetype.seed, archetype_seed(7000, index as u32));
            assert_eq!(archetype.lods.len(), 2);
        }
    }

    #[test]
    fn custom_reduction_cannot_drop_below_valid_geometry_floors() {
        let base = TreeMeshSettings::default();
        let reduction = LodReduction::new(100.0, 0, 0, 0.0);
        let far_lod = reduce_settings_for_lod(&base, 9, reduction);

        assert_eq!(far_lod.branch.segments, [3, 3, 3, 3]);
        assert_eq!(far_lod.branch.sections, [1, 1, 1, 1]);
        assert_eq!(far_lod.leaves.count, 3);
    }
}
