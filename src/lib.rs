pub mod enums;
pub mod errors;
pub mod settings;

pub mod cache;
pub mod lod;
pub mod meshgen;
pub mod presets;

use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};
use fastrand::Rng;

use crate::{meshgen::generate_tree_meshes, settings::TreeMeshSettings};

pub struct TreeProceduralGenerationPlugin;

impl Plugin for TreeProceduralGenerationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TreeMeshSettings>();
        app.register_type::<TreeMeshSettings>();
        app.init_resource::<TreeDefaultMaterials>();
        app.register_type::<TreeDefaultMaterials>();
        app.register_type::<TreeMaterialPolicy>();
        app.register_type::<Tree>();
        app.register_type::<Leaves>();
        app.register_type::<TreeLeafChild>();

        app.add_systems(
            PostUpdate,
            update_all_tree_meshes_with_global_settings
                .run_if(resource_changed::<TreeMeshSettings>),
        );
        app.add_systems(PostUpdate, update_all_tree_meshes_with_local_settings);
    }
}

#[derive(Reflect, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreeMaterialPolicy {
    /// Insert default/override [`StandardMaterial`] components for branches and leaves.
    ///
    /// This keeps the original convenience path: add [`Tree`] to an entity and
    /// it renders with [`TreeDefaultMaterials`] unless overrides are supplied.
    #[default]
    StandardDefaults,
    /// Insert only generated [`Mesh3d`] components. Callers own branch and leaf materials.
    ///
    /// Bevy material component types are static (`MeshMaterial3d<M>`), so a
    /// crate cannot insert a "generic material" dynamically. Use this policy
    /// when spawning trees with custom material plugins.
    MeshOnly,
}

#[derive(Component, Reflect, Clone, Debug)]
#[component(on_add = new_tree_component_added)]
pub struct Tree {
    /// the seed for the rng (same seed and TreeMeshSettings = same tree mesh)
    /// the seed is always local to each tree instance (regardless if the tree is using global TreeMeshSettings)
    pub seed: u64,
    /// the settings to use for this tree; if set to none the settings from the global TreeMeshSettings resource are used
    pub tree_mesh_settings_override: Option<TreeMeshSettings>,
    /// Controls whether the plugin inserts StandardMaterial components or only generated meshes.
    pub material_policy: TreeMaterialPolicy,
    /// defaults to Color::WHITE
    pub bark_material_override: Option<MeshMaterial3d<StandardMaterial>>,
    /// defaults to green -> Color::LinearRgba(LinearRgba { red: 0.0, green: 1.0, blue: 0.0, alpha: 1.0 }
    /// recommendation: AlphaMode::Mask(0.x) is recommend to be set for the leaves (depending on the texture used)
    pub leaf_material_override: Option<MeshMaterial3d<StandardMaterial>>,
}

impl Default for Tree {
    fn default() -> Self {
        Self {
            seed: 0,
            tree_mesh_settings_override: None,
            material_policy: TreeMaterialPolicy::StandardDefaults,
            bark_material_override: None,
            leaf_material_override: None,
        }
    }
}

#[derive(Resource, Reflect)]
pub struct TreeDefaultMaterials {
    /// defaults to Color::WHITE
    pub bark_material: MeshMaterial3d<StandardMaterial>,
    /// defaults to green -> Color::LinearRgba(LinearRgba { red: 0.0, green: 1.0, blue: 0.0, alpha: 1.0 }
    /// recommendation: AlphaMode::Mask(0.x) is recommend to be set for the leaves (depending on the texture used)
    pub leaf_material: MeshMaterial3d<StandardMaterial>,
}

impl FromWorld for TreeDefaultMaterials {
    fn from_world(world: &mut World) -> Self {
        let mut materials = world
            .get_resource_mut::<Assets<StandardMaterial>>()
            .unwrap();
        Self {
            bark_material: MeshMaterial3d(materials.add(Color::WHITE)),
            leaf_material: MeshMaterial3d(materials.add(Color::LinearRgba(LinearRgba {
                red: 0.0,
                green: 1.0,
                blue: 0.0,
                alpha: 1.0,
            }))),
        }
    }
}

/// Marker inserted on the generated leaf child entity.
///
/// Custom-material users can query `Added<Leaves>` and attach their own leaf
/// [`MeshMaterial3d<M>`] after spawning a [`Tree`] with [`TreeMaterialPolicy::MeshOnly`].
#[derive(Component, Reflect, Clone, Copy, Debug, Default)]
pub struct Leaves;

#[derive(Component, Reflect, Clone, Copy, Debug)]
struct TreeLeafChild(Entity);

fn new_tree_component_added(mut world: DeferredWorld, context: HookContext) {
    let tree_entity = context.entity;

    let tree: Tree = (*world.entity(tree_entity).components::<&Tree>()).clone();
    let Some(tree_mesh_settings) = tree
        .tree_mesh_settings_override
        .clone()
        .or_else(|| world.get_resource::<TreeMeshSettings>().cloned())
    else {
        error!("TreeMeshSettings resource is missing");
        return;
    };

    let mut rng: Rng = Rng::with_seed(tree.seed);

    match generate_tree_meshes(&tree_mesh_settings, &mut rng) {
        Ok((branches_mesh, leaves_mesh)) => {
            // retrieve AssetServer
            let mut meshes = world.get_resource_mut::<Assets<Mesh>>().unwrap();

            // meshes
            let branches_mesh = Mesh3d(meshes.add(branches_mesh));
            let leaves_mesh = Mesh3d(meshes.add(leaves_mesh));

            let leaf_material = match tree.material_policy {
                TreeMaterialPolicy::StandardDefaults => {
                    let default_materials = world.get_resource::<TreeDefaultMaterials>().unwrap();
                    Some(
                        tree.leaf_material_override
                            .clone()
                            .unwrap_or_else(|| default_materials.leaf_material.clone()),
                    )
                }
                TreeMaterialPolicy::MeshOnly => None,
            };

            let existing_branch_material = world
                .entity(tree_entity)
                .get::<MeshMaterial3d<StandardMaterial>>()
                .cloned();
            let branch_material = match tree.material_policy {
                TreeMaterialPolicy::StandardDefaults => {
                    let default_materials = world.get_resource::<TreeDefaultMaterials>().unwrap();
                    Some(
                        tree.bark_material_override
                            .clone()
                            .or(existing_branch_material)
                            .unwrap_or_else(|| default_materials.bark_material.clone()),
                    )
                }
                TreeMaterialPolicy::MeshOnly => None,
            };

            let mut commands = world.commands();
            let mut leaves = commands.spawn((Name::new("ProcGenTreeLeaves"), Leaves, leaves_mesh));
            if let Some(leaf_material) = leaf_material {
                leaves.insert(leaf_material);
            }
            let leaves_id = leaves.id();

            let mut tree_commands = commands.entity(tree_entity);
            tree_commands.insert((
                Name::new("ProcGenTreeBranches"),
                TreeLeafChild(leaves_id),
                branches_mesh,
            ));
            if let Some(branch_material) = branch_material {
                tree_commands.insert(branch_material);
            }
            tree_commands.add_child(leaves_id);
        }
        Err(err) => error!("Error during tree mesh generation: {}", err),
    }
}

fn update_all_tree_meshes_with_local_settings(
    trees: Query<(Entity, &Tree, &TreeLeafChild), Changed<Tree>>,
    branch_materials: Query<&MeshMaterial3d<StandardMaterial>>,
    mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    global_tree_settings: Res<TreeMeshSettings>,
    default_materials: Res<TreeDefaultMaterials>,
    mut commands: Commands,
) {
    // For now we are regenerating the whole tree mesh each time
    // TODO: Try to modify in place (or at least only branch/leaf levels or textures that need modification)
    for (tree_entity, tree, leaves_entity) in trees.iter() {
        let tree_settings: &TreeMeshSettings = match tree.tree_mesh_settings_override {
            Some(ref tree_settings) => tree_settings,
            None => global_tree_settings.as_ref(),
        };

        let mut rng: Rng = Rng::with_seed(tree.seed);

        match generate_tree_meshes(tree_settings, &mut rng) {
            Ok((branches_mesh, leaves_mesh)) => {
                let branches_mesh = Mesh3d(meshes.add(branches_mesh));
                let leaves_mesh = Mesh3d(meshes.add(leaves_mesh));

                commands.entity(tree_entity).insert(branches_mesh);
                commands.entity(leaves_entity.0).insert(leaves_mesh);

                if tree.material_policy == TreeMaterialPolicy::StandardDefaults {
                    let current_bark_material = branch_materials.get(tree_entity).ok();
                    match tree.bark_material_override {
                        Some(ref bark_material_from_local_settings) => {
                            if current_bark_material != Some(bark_material_from_local_settings) {
                                commands
                                    .entity(tree_entity)
                                    .insert(bark_material_from_local_settings.clone());
                            }
                        }
                        None => {
                            if current_bark_material != Some(&default_materials.bark_material) {
                                commands
                                    .entity(tree_entity)
                                    .insert(default_materials.bark_material.clone());
                            }
                        }
                    }

                    match tree.leaf_material_override {
                        Some(ref leaf_material_from_local_settings) => {
                            if mesh_materials.get(leaves_entity.0).ok()
                                != Some(leaf_material_from_local_settings)
                            {
                                commands
                                    .entity(leaves_entity.0)
                                    .insert(leaf_material_from_local_settings.clone());
                            }
                        }
                        None => {
                            if mesh_materials.get(leaves_entity.0).ok()
                                != Some(&default_materials.leaf_material)
                            {
                                commands
                                    .entity(leaves_entity.0)
                                    .insert(default_materials.leaf_material.clone());
                            }
                        }
                    }
                }
            }
            Err(err) => error!("Error during tree mesh generation: {}", err),
        }
    }
}

fn update_all_tree_meshes_with_global_settings(
    trees: Query<(Entity, &Tree, &TreeLeafChild)>,
    tree_settings: Res<TreeMeshSettings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
) {
    // For now we are regenerating the whole tree mesh each time
    // TODO: Try to modify in place (or at least only branch/leaf levels or textures that need modification)

    for (tree_entity, tree, leaves_entity) in trees.iter() {
        if tree.tree_mesh_settings_override.is_none() {
            let mut rng: Rng = Rng::with_seed(tree.seed);

            match generate_tree_meshes(&tree_settings, &mut rng) {
                Ok((branches_mesh, leaves_mesh)) => {
                    let branches_mesh = Mesh3d(meshes.add(branches_mesh));
                    let leaves_mesh = Mesh3d(meshes.add(leaves_mesh));

                    commands.entity(tree_entity).insert(branches_mesh);
                    commands.entity(leaves_entity.0).insert(leaves_mesh);
                }
                Err(err) => error!("Error during tree mesh generation: {}", err),
            }
        }
    }
}
