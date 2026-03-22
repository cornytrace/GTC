use std::{num::NonZero, result::Result};

use bevy::{
    asset::embedded_asset,
    ecs::system::{lifetimeless::SRes, SystemParamItem},
    image::ImageSamplerDescriptor,
    prelude::*,
    render::{
        mesh::{MeshVertexAttribute, VertexFormat},
        render_asset::RenderAssets,
        render_resource::{
            binding_types::{self, *},
            AsBindGroup, AsBindGroupError, BindGroupEntries, BindGroupLayout,
            BindGroupLayoutEntries, BindGroupLayoutEntry, BindingResource, BindingResources,
            BufferUsages, PreparedBindGroup, SamplerBindingType, ShaderRef, ShaderStages,
            ShaderType, TextureSampleType, UnpreparedBindGroup,
        },
        renderer::RenderDevice,
        texture::{FallbackImage, GpuImage},
        MainWorld, RenderApp,
    },
};

const MAX_TEXTURE_COUNT: usize = 16;

/// Bevy enforces one material per mesh, while a renderware mesh can have multiple materials.
/// The material is selected per triangle with a custom vertex attribute
#[derive(Debug, Clone, Asset, TypePath)]
pub struct GTAMaterial {
    pub mats: Vec<RwMaterial>,
}

#[derive(Debug, Clone)]
pub struct RwMaterial {
    pub color: LinearRgba,
    pub texture: Option<Handle<Image>>,
    pub sampler: ImageSamplerDescriptor,
    pub ambient_fac: f32,
    pub diffuse_fac: f32,
}

impl GTAMaterial {
    pub fn new_single(mat: RwMaterial) -> Self {
        Self { mats: vec![mat] }
    }
}

impl From<RwMaterial> for GTAMaterial {
    fn from(value: RwMaterial) -> Self {
        GTAMaterial::new_single(value)
    }
}

#[derive(Debug, Clone, Copy, ShaderType, Default)]
struct MaterialProperties {
    ambient_fac: f32,
    diffuse_fac: f32,
    _padding: u32,
    _padding2: u32,
}
const _: () = assert!(size_of::<MaterialProperties>().is_multiple_of(16));

pub const ATTRIBUTE_MATERIAL_ID: MeshVertexAttribute =
    MeshVertexAttribute::new("MaterialId", 79461375, VertexFormat::Uint32);

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
        layout: &bevy::render::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;

        let mut vertex_attributes = vec![
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
        ];
        if layout.0.contains(Mesh::ATTRIBUTE_UV_0) {
            // Make sure this matches the shader location
            vertex_attributes.push(Mesh::ATTRIBUTE_UV_0.at_shader_location(2));
        }
        if layout.0.contains(Mesh::ATTRIBUTE_COLOR) {
            // Make sure this matches the shader location
            vertex_attributes.push(Mesh::ATTRIBUTE_COLOR.at_shader_location(5));
        }
        //vertex_attributes.push(ATTRIBUTE_MATERIAL_ID.at_shader_location(8));

        let vertex_layout = layout.0.get_layout(&vertex_attributes)?;
        descriptor.vertex.buffers = vec![vertex_layout];

        Ok(())
    }
}

impl AsBindGroup for GTAMaterial {
    type Data = ();
    type Param = (
        SRes<RenderAssets<GpuImage>>,
        SRes<FallbackImage>,
        SRes<AmbientLight>,
    );

    fn as_bind_group(
        &self,
        layout: &BindGroupLayout,
        render_device: &RenderDevice,
        (image_assets, fallback_image, ambient_res): &mut SystemParamItem<'_, '_, Self::Param>,
    ) -> Result<PreparedBindGroup<Self::Data>, AsBindGroupError> {
        // retrieve the render resources from handles
        let mut images = vec![];
        for handle in self
            .mats
            .iter()
            .take(MAX_TEXTURE_COUNT)
            .map(|m| m.texture.clone())
        {
            match handle {
                Some(handle) => match image_assets.get(&handle) {
                    Some(image) => images.push(image),
                    None => return Err(AsBindGroupError::RetryNextUpdate),
                },
                None => images.push(&fallback_image.d2),
            }
        }

        let fallback_image = &fallback_image.d2;

        let textures = vec![&fallback_image.texture_view; MAX_TEXTURE_COUNT];

        // convert bevy's resource types to WGPU's references
        let mut textures: Vec<_> = textures.into_iter().map(|texture| &**texture).collect();

        // fill in up to the first `MAX_TEXTURE_COUNT` textures and samplers to the arrays
        for (id, image) in images.into_iter().enumerate() {
            textures[id] = &*image.texture_view;
        }

        let samplers: Vec<_> = self
            .mats
            .iter()
            .map(|m| render_device.create_sampler(&m.sampler.as_wgpu()))
            .collect();

        let mut ref_samplers = vec![&*fallback_image.sampler; MAX_TEXTURE_COUNT];

        for (id, sampler) in samplers.iter().enumerate() {
            ref_samplers[id] = sampler;
        }
        let samplers = BindingResource::SamplerArray(&ref_samplers);

        let color = {
            let mut array = [default(); MAX_TEXTURE_COUNT];
            for (id, e) in self.mats.iter().map(|m| m.color).enumerate() {
                array[id] = e;
            }
            let mut buffer = bevy::render::render_resource::encase::UniformBuffer::new(Vec::new());
            buffer.write(&array).unwrap();

            bevy::render::render_resource::OwnedBindingResource::Buffer(
                render_device.create_buffer_with_data(
                    &bevy::render::render_resource::BufferInitDescriptor {
                        label: None,
                        usage: BufferUsages::UNIFORM,
                        contents: buffer.as_ref(),
                    },
                ),
            )
        };
        let color = color.get_binding();

        let mat_props = {
            let mut array = [default(); MAX_TEXTURE_COUNT];
            for (id, (a, d)) in self
                .mats
                .iter()
                .map(|m| m.ambient_fac)
                .zip(self.mats.iter().map(|m| m.diffuse_fac))
                .enumerate()
            {
                array[id] = MaterialProperties {
                    ambient_fac: a,
                    diffuse_fac: d,
                    _padding: 0,
                    _padding2: 0,
                };
            }
            let mut buffer = bevy::render::render_resource::encase::UniformBuffer::new(Vec::new());
            buffer.write(&array).unwrap();

            bevy::render::render_resource::OwnedBindingResource::Buffer(
                render_device.create_buffer_with_data(
                    &bevy::render::render_resource::BufferInitDescriptor {
                        label: None,
                        usage: BufferUsages::UNIFORM,
                        contents: buffer.as_ref(),
                    },
                ),
            )
        };
        let mat_props = mat_props.get_binding();

        let ambient_light = {
            let mut buffer = bevy::render::render_resource::encase::UniformBuffer::new(Vec::new());
            buffer.write(&ambient_res.0.to_vec4()).unwrap();

            bevy::render::render_resource::OwnedBindingResource::Buffer(
                render_device.create_buffer_with_data(
                    &bevy::render::render_resource::BufferInitDescriptor {
                        label: None,
                        usage: BufferUsages::UNIFORM,
                        contents: buffer.as_ref(),
                    },
                ),
            )
        };
        let ambient_light = ambient_light.get_binding();

        let bind_group = render_device.create_bind_group(
            "gta_material_bind_group",
            layout,
            &BindGroupEntries::sequential((
                color,
                &textures[..],
                samplers,
                mat_props,
                ambient_light,
            )),
        );

        Ok(PreparedBindGroup {
            bindings: BindingResources(vec![]),
            bind_group,
            data: (),
        })
    }

    fn unprepared_bind_group(
        &self,
        _layout: &BindGroupLayout,
        _render_device: &RenderDevice,
        _param: &mut SystemParamItem<'_, '_, Self::Param>,
        _force_no_bindless: bool,
    ) -> Result<UnpreparedBindGroup<Self::Data>, AsBindGroupError> {
        // We implement `as_bind_group`` directly because bindless texture
        // arrays can't be owned.
        // Or rather, they can be owned, but then you can't make a `&'a [&'a
        // TextureView]` from a vec of them in `get_binding()`.
        Err(AsBindGroupError::CreateBindGroupDirectly)
    }

    fn bind_group_layout_entries(_: &RenderDevice, _: bool) -> Vec<BindGroupLayoutEntry>
    where
        Self: Sized,
    {
        let count = NonZero::<u32>::new(MAX_TEXTURE_COUNT as u32).unwrap();
        BindGroupLayoutEntries::with_indices(
            // The layout entries will only be visible in the fragment stage
            ShaderStages::VERTEX_FRAGMENT,
            (
                (0, binding_types::uniform_buffer::<Vec4>(false).count(count)),
                // Screen texture
                //
                // @group(2) @binding(1) var textures: binding_array<texture_2d<f32>>;
                (
                    1,
                    texture_2d(TextureSampleType::Float { filterable: true }).count(count),
                ),
                // Sampler
                //
                // @group(2) @binding(2) var nearest_sampler: sampler;
                //
                // Note: as with textures, multiple samplers can also be bound
                // onto one binding slot:
                //
                // ```
                // sampler(SamplerBindingType::Filtering)
                //     .count(NonZero::<u32>::new(MAX_TEXTURE_COUNT as u32).unwrap()),
                // ```
                //
                // One may need to pay attention to the limit of sampler binding
                // amount on some platforms.
                (2, sampler(SamplerBindingType::Filtering).count(count)),
                (
                    3,
                    binding_types::uniform_buffer::<MaterialProperties>(false).count(count),
                ),
                (4, binding_types::uniform_buffer::<Vec4>(false)),
            ),
        )
        .to_vec()
    }
}

#[derive(Debug, Clone, Copy, Resource)]
pub struct AmbientLight(LinearRgba);

pub struct GTAMaterialPlugin;

impl Plugin for GTAMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/gta_material.wgsl");

        app.insert_resource(AmbientLight(LinearRgba {
            red: 0.3,
            green: 0.3,
            blue: 0.3,
            alpha: 1.,
        }))
        .add_plugins(MaterialPlugin::<GTAMaterial>::default());

        let render_app = app.sub_app_mut(RenderApp);
        render_app.add_systems(ExtractSchedule, insert_ambient_light);
    }
}

fn insert_ambient_light(mut commands: Commands, world: ResMut<MainWorld>) {
    if let Some(res) = world.get_resource::<AmbientLight>() {
        commands.insert_resource(*res);
    }
}
