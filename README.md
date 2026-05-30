# bevy_procedural_tree
Procedural 3D trees for bevy - ported from the javascript ez-tree repository with some adjustments to better fit bevy and some changes to the semantics of the parameters due to personal opinion.

![Showroom example](/images/showroom.jpg)

## Features
* Mesh generation based on given TreeMeshSettings (a standard Mesh3d)
* Generation by global TreeMeshSettings or per instance (chosen per entity)
* User can provide a material for the branches and leafs separately 
* Auto regeneration of the meshes when the settings change
* LOD and archetype-pool helpers for games that cache, batch, or instance trees themselves
* Optional use of u32_indices for the mesh (default is u16; see `u32_indices` feature in Cargo.toml)

## Usage
See the showroom example:

```bash
cargo run --example showroom --features "inspector"
```

To inspect generated LODs and seed-stable archetype pools, run:

```bash
cargo run --example lod_archetype_pool
```

The archetype-pool viewer includes profile and LOD toggles for visual tuning:

* `1`, `2`, `3` isolate LOD 0, 1, or 2
* `0` shows all LOD rows again
* `B`, `A`, `C` switch between balanced, aggressive, and conservative reduction profiles

The overlay reports the selected profile, active LOD view, archetype count, and visible vertex/triangle totals.

To print CSV-style LOD mesh stats and course-scale placement estimates, run:

```bash
cargo run --example lod_course_scale_bench --features u32_indices
```

In the showroom are two trees: The tree in the middle uses the global `TreeMeshSettings` resource. The tree to the side uses the `TreeMeshSettings` component, which can be modified on the entity itself via the inspector.

### Quick start (with TreeProceduralGenerationPlugin)
1. To enable auto generation: add the `TreeProceduralGenerationPlugin` to your app
2. (Optional) Modify the `TreeMeshSettings` and `TreeDefaultMaterials` to your liking
3. Spawn an entity and add the `Tree`component

Internally this will generate the Mesh3d for the entity and a child entity for the mesh of the leaves. It will apply the materials from the `TreeDefaultMaterials` resource, or from a provided override.

### Quick start (without TreeProceduralGenerationPlugin)
1. use `bevy_procedural_tree::meshgen::generate_tree_meshes()` to generate two meshes (branches/trunk mesh and leaves mesh)
2. use the meshes for anything you like

For repeated trees, use `bevy_procedural_tree::lod::generate_archetypes()` to build a deterministic pool of seed-jittered variants, each with multiple generated LOD levels. LOD generation reduces branch tessellation and leaf-card density while keeping the same broad tree shape. The default `LodReduction::balanced()` profile is tuned from the course-scale benchmark; use `aggressive()` for background-heavy scenes and `conservative()` for hero trees. If seed-only variation is too subtle, use `generate_archetypes_from_settings()` with a prepared set of varied `TreeMeshSettings`.

### LOD and archetype pools
The `lod` module is intended for games that already handle their own caching, instancing, batching, or chunk baking. It does not spawn entities and it does not choose materials; it only returns branch and leaf `Mesh` pairs.

```rust
use bevy_procedural_tree::lod::generate_archetypes;
use bevy_procedural_tree::settings::TreeMeshSettings;

let settings = TreeMeshSettings::default();
let archetypes = generate_archetypes(
    &settings,
    12_345, // base seed
    24,     // archetype count
    3,      // LOD levels
)?;

// Pick an archetype deterministically, for example from a world-position hash.
let tree = &archetypes[archetype_index];
let (branch_mesh, leaf_mesh) = &tree.lods[lod_level];
```

`LodReduction::default()` is currently `LodReduction::balanced()`. The included benchmark compares these profiles:

* `balanced()` - default; keeps most of the course-scale savings while preserving more visible detail
* `aggressive()` - lower geometry for dense background forests or stylized distant trees
* `conservative()` - higher detail for hero trees or close camera work

For stronger shape variation than seed changes alone provide, prepare several `TreeMeshSettings` presets and call `generate_archetypes_from_settings()` or `generate_archetypes_from_settings_with_reduction()`.

The `presets` module includes six reusable prototype families:

* compact round deciduous
* wide umbrella deciduous
* tall sparse deciduous
* leaning edge deciduous
* tall narrow conifer
* scrub ornamental

Use `bevy_procedural_tree::presets::tree_preset_settings(count)` to build a deterministic settings pool from these families.

### Course-scale benchmark
The benchmark example estimates mesh generation cost and course-scale placement cost for 24 archetypes, 3 LOD levels, and 3,096 deterministic tree placements:

```bash
cargo run --example lod_course_scale_bench --features u32_indices
```

It prints CSV-style sections for per-mesh stats, camera-distance LOD assignment, and profile summaries. Use it to tune `LodReduction` values before wiring generated trees into a larger renderer.

### Explanation of the most important structs
#### TreeMeshSettings resource
Defines the general structure of the generated 3d mesh. Every parameter is documented.

#### TreeDefaultTextures resource
Defines the default materials used by trees which do not use the override.

#### Tree component
Added to an entity to generate a new tree. It has 4 parameters:
* a seed to make this tree unique (using the same seed, with the same TreeMeshSettings produces the same tree mesh)
* an optional override for the `TreeMeshSettings` resource
* an optional override for the `TreeDefaultMaterials` bark material
* an optional override for the `TreeDefaultMaterials` leaf material

## Possible ToDos
* Do not regenerate the whole tree each time the settings change (but do partial updates)
* Provide an example vertex shader for wind
* Implement "growing"
* Caching of already generated trees (i.e. with the lru crate)
* Runtime LOD switching example
* Different "normal" modes (currently just orthogonal to the surface; i.e. inspiration: [Reddit: Fluffy trees](https://www.reddit.com/r/Unity3D/comments/jhwfkj/fluffy_trees_using_custom_shader_that_turns_quad/))

## Future research
* How to generalize materials to not force the user to provide a StandardMaterial

## Supported Bevy Versions

| Bevy    | bevy_procedural_tree |
| ------- | ----- |
| 0.18    | 0.3   |
| 0.17    | 0.2   |
| 0.16    | 0.1   |

## Acknowledgements
* https://github.com/dgreenheck/ez-tree
* https://ambientcg.com
* https://polyhaven.com
