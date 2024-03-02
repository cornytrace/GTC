use bevy::{
    prelude::*,
    render::{
        render_resource::{AsBindGroup, ShaderRef},
        texture::{ImageSampler, ImageSamplerDescriptor},
    },
};

#[derive(AsBindGroup, Debug, Clone, Asset, TypePath)]
pub struct GTAMaterial {
    #[uniform(0)]
    pub color: Color,
    #[texture(1)]
    #[sampler(2)] // TODO
    pub texture: Option<Handle<Image>>,
    //#[sampler(2)]
    pub sampler: ImageSamplerDescriptor,
    #[uniform(3)]
    pub ambient_fac: f32,
    #[uniform(4)]
    pub diffuse_fac: f32,
}

impl Material for GTAMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://gtc/shaders/gta_material.wgsl".into()
    }
}
