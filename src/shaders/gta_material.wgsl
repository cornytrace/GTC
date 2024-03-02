#import bevy_pbr::forward_io::VertexOutput

@group(2) @binding(0) var<uniform> material_color: vec4<f32>;
@group(2) @binding(1) var material_color_texture: texture_2d<f32>;
@group(2) @binding(2) var material_color_sampler: sampler;
@group(2) @binding(3) var<uniform> material_ambient_factor: f32;
@group(2) @binding(4) var<uniform> material_diffuse_factor: f32;

@fragment
fn fragment(
    mesh: VertexOutput,
) -> @location(0) vec4<f32> {
    var diffuse = material_color;

    //diffuse.rgb += ambient.rgb* material_ambient_factor;

    #ifdef VERTEX_COLORS
    diffuse *= mesh.color;
    #endif

    diffuse *= textureSample(material_color_texture, material_color_sampler, mesh.uv);

    return diffuse;
}