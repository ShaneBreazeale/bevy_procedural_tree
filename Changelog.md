### Unreleased
* Add LOD generation helpers and deterministic archetype-pool generation, including branch tessellation and leaf-card density reduction.
* Make the benchmark-selected balanced LOD profile the default, with conservative and aggressive presets available explicitly.
* Add settings-pool archetype generation for stronger silhouette variation.
* Add reusable prototype tree preset families for examples and archetype-pool experiments.
* Add an archetype/LOD grid example.
* Add profile and LOD hotkeys plus visible mesh statistics to the archetype/LOD viewer.
* Add a course-scale LOD benchmark example.
* Add mesh attribute contract tests for branch/leaf positions, normals, UVs, indices, bounds, and leaf-card UVs.
* Document LOD/archetype-pool usage and benchmark workflow in the README.

### v0.3
* migrate to bevy 0.18

### v0.2
* migrate to bevy 0.17
* made feature "perf_ui" optional in showroom example
* removed iyes_perf_ui dependency (and use the bevy FpsOverlayPlugin instead)

### v0.1.2
* Made `generate_tree_meshes()` public, to be used without the `TreeProceduralGenerationPlugin`

### v0.1.1
* Nicer Readme.md with pictures

### v0.1
* Initial release
