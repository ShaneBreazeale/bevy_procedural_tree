#import bevy_pbr::{
    mesh_bindings::mesh,
    mesh_functions,
    skinning,
    morph::morph,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
}

struct TreeWindExtension {
    params: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> tree_wind: TreeWindExtension;

#ifdef MORPH_TARGETS
fn morph_vertex(vertex_in: Vertex) -> Vertex {
    var vertex = vertex_in;
    let first_vertex = mesh[vertex.instance_index].first_vertex_index;
    let vertex_index = vertex.index - first_vertex;

    let weight_count = bevy_pbr::morph::layer_count();
    for (var i: u32 = 0u; i < weight_count; i ++) {
        let weight = bevy_pbr::morph::weight_at(i);
        if weight == 0.0 {
            continue;
        }
        vertex.position += weight * morph(vertex_index, bevy_pbr::morph::position_offset, i);
#ifdef VERTEX_NORMALS
        vertex.normal += weight * morph(vertex_index, bevy_pbr::morph::normal_offset, i);
#endif
#ifdef VERTEX_TANGENTS
        vertex.tangent += vec4(weight * morph(vertex_index, bevy_pbr::morph::tangent_offset, i), 0.0);
#endif
    }
    return vertex;
}
#endif

@vertex
fn vertex(vertex_no_morph: Vertex) -> VertexOutput {
    var out: VertexOutput;

#ifdef MORPH_TARGETS
    var vertex = morph_vertex(vertex_no_morph);
#else
    var vertex = vertex_no_morph;
#endif

    let mesh_world_from_local = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);

#ifdef SKINNED
    var world_from_local = skinning::skin_model(
        vertex.joint_indices,
        vertex.joint_weights,
        vertex_no_morph.instance_index
    );
#else
    var world_from_local = mesh_world_from_local;
#endif

#ifdef VERTEX_NORMALS
#ifdef SKINNED
    out.world_normal = skinning::skin_normals(world_from_local, vertex.normal);
#else
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex_no_morph.instance_index
    );
#endif
#endif

#ifdef VERTEX_POSITIONS
    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    let time = tree_wind.params.x;
    let speed = tree_wind.params.y;
    let strength = tree_wind.params.z;
    let turbulence = tree_wind.params.w;
    let height_mask = smoothstep(0.15, 3.5, max(vertex.position.y, 0.0));
    let phase = time * speed + out.world_position.x * 0.31 + out.world_position.z * 0.19;
    let sway = vec3<f32>(
        sin(phase) * strength,
        0.0,
        cos(phase * 0.73) * strength * turbulence,
    ) * height_mask;
    out.world_position = out.world_position + vec4<f32>(sway, 0.0);
    out.position = position_world_to_clip(out.world_position.xyz);
#endif

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif

#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex.tangent,
        vertex_no_morph.instance_index
    );
#endif

#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif

#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex_no_morph.instance_index;
#endif

#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_no_morph.instance_index, mesh_world_from_local[3]);
#endif

    return out;
}
