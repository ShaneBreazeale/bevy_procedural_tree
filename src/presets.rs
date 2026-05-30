//! Opinionated [`TreeMeshSettings`] preset families for examples and prototypes.
//!
//! These are not required by the generator. They provide a small reusable pool
//! of distinct silhouettes for games that want to build an archetype set before
//! investing in their own species or art-direction pipeline.

use bevy::prelude::*;

use crate::enums::TreeType;
use crate::settings::TreeMeshSettings;

/// Reusable tree-shape families with intentionally different silhouettes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TreePreset {
    CompactRoundDeciduous,
    WideUmbrellaDeciduous,
    TallSparseDeciduous,
    LeaningEdgeDeciduous,
    TallNarrowConifer,
    ScrubOrnamental,
}

impl TreePreset {
    pub const ALL: [Self; 6] = [
        Self::CompactRoundDeciduous,
        Self::WideUmbrellaDeciduous,
        Self::TallSparseDeciduous,
        Self::LeaningEdgeDeciduous,
        Self::TallNarrowConifer,
        Self::ScrubOrnamental,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::CompactRoundDeciduous => "compact round deciduous",
            Self::WideUmbrellaDeciduous => "wide umbrella deciduous",
            Self::TallSparseDeciduous => "tall sparse deciduous",
            Self::LeaningEdgeDeciduous => "leaning edge deciduous",
            Self::TallNarrowConifer => "tall narrow conifer",
            Self::ScrubOrnamental => "scrub ornamental",
        }
    }
}

/// Returns `count` settings by cycling through [`TreePreset::ALL`] with mild
/// deterministic variation per repetition.
pub fn tree_preset_settings(count: u32) -> Vec<TreeMeshSettings> {
    (0..count)
        .map(|index| {
            let preset = TreePreset::ALL[index as usize % TreePreset::ALL.len()];
            let repetition = index / TreePreset::ALL.len() as u32;
            let variation = ((index as f32 * 1.618_034).sin() * 0.5 + 0.5) * 0.22;
            settings_for_preset(preset, repetition, variation)
        })
        .collect()
}

/// Builds settings for one named preset.
pub fn settings_for_preset(
    preset: TreePreset,
    repetition: u32,
    variation: f32,
) -> TreeMeshSettings {
    let mut settings = TreeMeshSettings::default();
    let repeat_scale = 1.0 + repetition as f32 * 0.035;

    match preset {
        TreePreset::CompactRoundDeciduous => {
            settings.branch.length = [3.6 * repeat_scale, 2.2 + variation, 1.2, 0.32];
            settings.branch.angle = [0.0, 46.0, 54.0, 66.0];
            settings.branch.children = [6, 4, 9];
            settings.branch.trunk_base_radius = 0.17;
            settings.leaves.count = 4;
            settings.leaves.size = 0.22 + variation * 0.12;
        }
        TreePreset::WideUmbrellaDeciduous => {
            settings.branch.length = [3.2, 4.2 * repeat_scale, 2.2, 0.62];
            settings.branch.angle = [0.0, 74.0, 64.0, 58.0];
            settings.branch.children = [8, 5, 11];
            settings.branch.force.direction = Vec3::new(0.0, 0.25, 0.0);
            settings.branch.force.strength = 0.03;
            settings.branch.trunk_base_radius = 0.24;
            settings.leaves.count = 5;
            settings.leaves.size = 0.29 + variation * 0.10;
            settings.leaves.size_variance = 0.35;
        }
        TreePreset::TallSparseDeciduous => {
            settings.branch.length = [6.8 * repeat_scale, 2.8, 1.2, 0.34];
            settings.branch.angle = [0.0, 22.0, 34.0, 42.0];
            settings.branch.children = [5, 3, 6];
            settings.branch.force.direction = Vec3::Y;
            settings.branch.force.strength = 0.18;
            settings.branch.trunk_base_radius = 0.20;
            settings.branch.start = [0.0, 0.46, 0.48, 0.0];
            settings.leaves.count = 2;
            settings.leaves.size = 0.24 + variation * 0.08;
            settings.leaves.start = 0.35;
        }
        TreePreset::LeaningEdgeDeciduous => {
            settings.branch.length = [5.3 * repeat_scale, 3.4, 1.7, 0.42];
            settings.branch.angle = [0.0, 42.0, 46.0, 54.0];
            settings.branch.children = [5, 4, 8];
            settings.branch.force.direction = Vec3::new(0.85, 0.35, -0.2);
            settings.branch.force.strength = 0.32;
            settings.branch.gnarliness = [0.04, 0.40, 0.32, 0.12];
            settings.branch.trunk_base_radius = 0.23;
            settings.leaves.count = 4;
            settings.leaves.size = 0.27 + variation * 0.10;
            settings.leaves.size_variance = 0.40;
        }
        TreePreset::TallNarrowConifer => {
            settings.tree_type = TreeType::Evergreen;
            settings.branch.length = [7.6 * repeat_scale, 2.0, 0.9, 0.30];
            settings.branch.angle = [0.0, 67.0, 58.0, 48.0];
            settings.branch.children = [14, 6, 7];
            settings.branch.force.direction = Vec3::new(0.0, -0.35, 0.0);
            settings.branch.force.strength = 0.18;
            settings.branch.trunk_base_radius = 0.17;
            settings.branch.radius_factor = [1.0, 0.42, 0.48, 0.52];
            settings.leaves.count = 5;
            settings.leaves.size = 0.20 + variation * 0.08;
        }
        TreePreset::ScrubOrnamental => {
            settings.branch.length = [2.8 * repeat_scale, 2.0, 1.1, 0.30];
            settings.branch.angle = [0.0, 58.0, 52.0, 62.0];
            settings.branch.children = [8, 4, 9];
            settings.branch.gnarliness = [0.02, 0.26, 0.22, 0.08];
            settings.branch.trunk_base_radius = 0.14;
            settings.branch.start = [0.0, 0.18, 0.34, 0.0];
            settings.leaves.count = 6;
            settings.leaves.size = 0.20 + variation * 0.12;
            settings.leaves.size_variance = 0.50;
        }
    }

    settings
}
