//! Simple build-time cache support for generated archetype pools.
//!
//! The cache stores CPU mesh data for branch/leaf meshes so games can generate
//! archetype pools during import/build steps and load them later without running
//! procedural generation at startup.

use std::{
    fs::File,
    io::{self, BufReader, BufWriter, Read, Write},
    path::Path,
};

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, MeshVertexAttribute, PrimitiveTopology, VertexAttributeValues},
    prelude::*,
};

use crate::lod::TreeArchetype;

const MAGIC: &[u8; 8] = b"BPTCCH1\0";

#[derive(Clone, Debug)]
pub struct CachedArchetypePool {
    pub archetypes: Vec<CachedArchetype>,
}

#[derive(Clone, Debug)]
pub struct CachedArchetype {
    pub seed: u64,
    pub lods: Vec<CachedTreeMeshes>,
}

#[derive(Clone, Debug)]
pub struct CachedTreeMeshes {
    pub branches: CachedMesh,
    pub leaves: CachedMesh,
}

#[derive(Clone, Debug, Default)]
pub struct CachedMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub tangents: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl CachedArchetypePool {
    pub fn from_archetypes(archetypes: &[TreeArchetype]) -> io::Result<Self> {
        Ok(Self {
            archetypes: archetypes
                .iter()
                .map(|archetype| {
                    Ok(CachedArchetype {
                        seed: archetype.seed,
                        lods: archetype
                            .lods
                            .iter()
                            .map(|(branches, leaves)| {
                                Ok(CachedTreeMeshes {
                                    branches: CachedMesh::from_mesh(branches)?,
                                    leaves: CachedMesh::from_mesh(leaves)?,
                                })
                            })
                            .collect::<io::Result<Vec<_>>>()?,
                    })
                })
                .collect::<io::Result<Vec<_>>>()?,
        })
    }

    pub fn into_archetypes(self) -> Vec<TreeArchetype> {
        self.archetypes
            .into_iter()
            .map(|archetype| TreeArchetype {
                seed: archetype.seed,
                lods: archetype
                    .lods
                    .into_iter()
                    .map(|lod| (lod.branches.into_mesh(), lod.leaves.into_mesh()))
                    .collect(),
            })
            .collect()
    }
}

impl CachedMesh {
    pub fn from_mesh(mesh: &Mesh) -> io::Result<Self> {
        let positions = float3_attribute(mesh, Mesh::ATTRIBUTE_POSITION, "positions")?.to_vec();
        let normals = float3_attribute(mesh, Mesh::ATTRIBUTE_NORMAL, "normals")?.to_vec();
        let uvs = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
            Some(VertexAttributeValues::Float32x2(uvs)) => uvs.clone(),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "mesh is missing Float32x2 uv0",
                ));
            }
        };
        let tangents = match mesh.attribute(Mesh::ATTRIBUTE_TANGENT) {
            Some(VertexAttributeValues::Float32x4(tangents)) => tangents.clone(),
            None => Vec::new(),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "mesh tangent attribute must be Float32x4",
                ));
            }
        };
        let indices = match mesh.indices() {
            Some(Indices::U16(indices)) => indices.iter().map(|i| *i as u32).collect(),
            Some(Indices::U32(indices)) => indices.clone(),
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "mesh is missing indices",
                ));
            }
        };

        if normals.len() != positions.len()
            || uvs.len() != positions.len()
            || (!tangents.is_empty() && tangents.len() != positions.len())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "mesh attribute lengths do not match positions",
            ));
        }

        Ok(Self {
            positions,
            normals,
            uvs,
            tangents,
            indices,
        })
    }

    pub fn into_mesh(self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        if !self.tangents.is_empty() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, self.tangents);
        }
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

pub fn write_archetype_cache(
    path: impl AsRef<Path>,
    cache: &CachedArchetypePool,
) -> io::Result<()> {
    let mut writer = BufWriter::new(File::create(path)?);
    writer.write_all(MAGIC)?;
    write_u32(&mut writer, cache.archetypes.len())?;
    for archetype in &cache.archetypes {
        write_u64(&mut writer, archetype.seed)?;
        write_u32(&mut writer, archetype.lods.len())?;
        for lod in &archetype.lods {
            write_mesh(&mut writer, &lod.branches)?;
            write_mesh(&mut writer, &lod.leaves)?;
        }
    }
    writer.flush()
}

pub fn read_archetype_cache(path: impl AsRef<Path>) -> io::Result<CachedArchetypePool> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut magic = [0u8; 8];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid tree archetype cache header",
        ));
    }

    let archetype_count = read_u32(&mut reader)? as usize;
    let mut archetypes = Vec::with_capacity(archetype_count);
    for _ in 0..archetype_count {
        let seed = read_u64(&mut reader)?;
        let lod_count = read_u32(&mut reader)? as usize;
        let mut lods = Vec::with_capacity(lod_count);
        for _ in 0..lod_count {
            lods.push(CachedTreeMeshes {
                branches: read_mesh(&mut reader)?,
                leaves: read_mesh(&mut reader)?,
            });
        }
        archetypes.push(CachedArchetype { seed, lods });
    }

    Ok(CachedArchetypePool { archetypes })
}

fn float3_attribute<'a>(
    mesh: &'a Mesh,
    attribute: MeshVertexAttribute,
    name: &str,
) -> io::Result<&'a Vec<[f32; 3]>> {
    match mesh.attribute(attribute) {
        Some(VertexAttributeValues::Float32x3(values)) => Ok(values),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("mesh is missing Float32x3 {name}"),
        )),
    }
}

fn write_mesh(writer: &mut impl Write, mesh: &CachedMesh) -> io::Result<()> {
    write_u32(writer, mesh.positions.len())?;
    for value in &mesh.positions {
        write_f32x3(writer, *value)?;
    }
    for value in &mesh.normals {
        write_f32x3(writer, *value)?;
    }
    for value in &mesh.uvs {
        write_f32x2(writer, *value)?;
    }
    write_u32(writer, mesh.tangents.len())?;
    for value in &mesh.tangents {
        write_f32x4(writer, *value)?;
    }
    write_u32(writer, mesh.indices.len())?;
    for index in &mesh.indices {
        writer.write_all(&index.to_le_bytes())?;
    }
    Ok(())
}

fn read_mesh(reader: &mut impl Read) -> io::Result<CachedMesh> {
    let vertex_count = read_u32(reader)? as usize;
    let mut positions = Vec::with_capacity(vertex_count);
    let mut normals = Vec::with_capacity(vertex_count);
    let mut uvs = Vec::with_capacity(vertex_count);
    for _ in 0..vertex_count {
        positions.push(read_f32x3(reader)?);
    }
    for _ in 0..vertex_count {
        normals.push(read_f32x3(reader)?);
    }
    for _ in 0..vertex_count {
        uvs.push(read_f32x2(reader)?);
    }
    let tangent_count = read_u32(reader)? as usize;
    let mut tangents = Vec::with_capacity(tangent_count);
    for _ in 0..tangent_count {
        tangents.push(read_f32x4(reader)?);
    }
    let index_count = read_u32(reader)? as usize;
    let mut indices = Vec::with_capacity(index_count);
    for _ in 0..index_count {
        indices.push(read_u32(reader)?);
    }

    Ok(CachedMesh {
        positions,
        normals,
        uvs,
        tangents,
        indices,
    })
}

fn write_u32(writer: &mut impl Write, value: usize) -> io::Result<()> {
    let value = u32::try_from(value)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "value exceeds u32"))?;
    writer.write_all(&value.to_le_bytes())
}

fn write_u64(writer: &mut impl Write, value: u64) -> io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

fn read_u32(reader: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(reader: &mut impl Read) -> io::Result<u64> {
    let mut bytes = [0u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn write_f32x2(writer: &mut impl Write, value: [f32; 2]) -> io::Result<()> {
    for n in value {
        writer.write_all(&n.to_le_bytes())?;
    }
    Ok(())
}

fn write_f32x3(writer: &mut impl Write, value: [f32; 3]) -> io::Result<()> {
    for n in value {
        writer.write_all(&n.to_le_bytes())?;
    }
    Ok(())
}

fn write_f32x4(writer: &mut impl Write, value: [f32; 4]) -> io::Result<()> {
    for n in value {
        writer.write_all(&n.to_le_bytes())?;
    }
    Ok(())
}

fn read_f32x2(reader: &mut impl Read) -> io::Result<[f32; 2]> {
    Ok([read_f32(reader)?, read_f32(reader)?])
}

fn read_f32x3(reader: &mut impl Read) -> io::Result<[f32; 3]> {
    Ok([read_f32(reader)?, read_f32(reader)?, read_f32(reader)?])
}

fn read_f32x4(reader: &mut impl Read) -> io::Result<[f32; 4]> {
    Ok([
        read_f32(reader)?,
        read_f32(reader)?,
        read_f32(reader)?,
        read_f32(reader)?,
    ])
}

fn read_f32(reader: &mut impl Read) -> io::Result<f32> {
    let mut bytes = [0u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(f32::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lod::generate_archetypes_from_settings;
    use crate::presets::tree_preset_settings;

    #[test]
    fn archetype_cache_round_trips_mesh_data() {
        let settings = tree_preset_settings(2);
        let archetypes = generate_archetypes_from_settings(&settings, 1234, 2)
            .expect("archetype generation should succeed");
        let cache = CachedArchetypePool::from_archetypes(&archetypes)
            .expect("generated meshes should be cacheable");

        let path = std::env::temp_dir().join(format!(
            "bevy_procedural_tree_cache_test_{}.bptc",
            std::process::id()
        ));
        write_archetype_cache(&path, &cache).expect("cache write should succeed");
        let loaded = read_archetype_cache(&path).expect("cache read should succeed");
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.archetypes.len(), archetypes.len());
        for (loaded, original) in loaded.archetypes.iter().zip(&archetypes) {
            assert_eq!(loaded.seed, original.seed);
            assert_eq!(loaded.lods.len(), original.lods.len());
            for (loaded_lod, (original_branches, original_leaves)) in
                loaded.lods.iter().zip(&original.lods)
            {
                assert_eq!(
                    loaded_lod.branches.positions.len(),
                    vertex_count(original_branches)
                );
                assert_eq!(
                    loaded_lod.leaves.positions.len(),
                    vertex_count(original_leaves)
                );
                assert!(!loaded_lod.branches.indices.is_empty());
                assert!(!loaded_lod.leaves.indices.is_empty());
            }
        }
    }

    fn vertex_count(mesh: &Mesh) -> usize {
        match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(positions)) => positions.len(),
            _ => 0,
        }
    }
}
