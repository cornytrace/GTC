use bevy::{
    asset::embedded_asset,
    image::ImageSamplerDescriptor,
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderRef},
};

#[derive(AsBindGroup, Debug, Clone, Asset, TypePath)]
pub struct GTAMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[texture(1)]
    #[sampler(2)] // TODO
    pub texture: Option<Handle<Image>>,
    //#[sampler(2)]
    pub sampler: ImageSamplerDescriptor,
    #[uniform(3)]
    pub ambient_fac: f32,
    #[uniform(4)]
    pub diffuse_fac: f32,

    //TODO: should be global, not instance specific
    #[uniform(5)]
    pub ambient_light: LinearRgba,
}

impl Material for GTAMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://gtc/shaders/gta_material.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://gtc/shaders/gta_material.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::AlphaToCoverage
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline<Self>,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::render::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;

        Ok(())
    }
}

fn update_ambient(light: Res<AmbientLight>, mut materials: ResMut<Assets<GTAMaterial>>) {
    if !light.is_changed() {
        return;
    }

    for (_, material) in materials.iter_mut() {
        material.ambient_light = light.color.into();
    }
}

pub struct GTAMaterialPlugin;

impl Plugin for GTAMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/gta_material.wgsl");

        app.add_plugins(MaterialPlugin::<GTAMaterial>::default())
            .insert_resource(AmbientLight {
                color: Color::srgb_u8(85, 85, 85),
                brightness: 1.0,
                ..Default::default()
            })
            .add_systems(Update, update_ambient);
    }
}
