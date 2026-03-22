use bevy::{
    image::{ImageAddressMode, ImageSamplerDescriptor},
    prelude::*,
    render::{
        mesh::{Indices, PrimitiveTopology},
        render_asset::RenderAssetUsages,
    },
};
use rw_rs::bsf::{tex::TextureAddressingMode, Chunk, ChunkContent};

use crate::{
    material::{GTAMaterial, RwMaterial, ATTRIBUTE_MATERIAL_ID},
    utils::to_xzy,
};

pub fn load_dff(
    bsf: &Chunk,
    txd_name: &str,
    server: &Res<AssetServer>,
    //images: &ResMut<Assets<Image>>,
) -> Vec<(Mesh, GTAMaterial)> {
    let mut res = Vec::new();
    for geometry_chunk in &bsf
        .get_children()
        .iter()
        .find(|e| matches!(e.content, ChunkContent::GeometryList))
        .unwrap()
        .get_children()[1..]
    {
        if let ChunkContent::Geometry(geo) = &geometry_chunk.content {
            let topo = if geo.is_tristrip() {
                PrimitiveTopology::TriangleStrip
            } else {
                PrimitiveTopology::TriangleList
            };

            let vertices = geo
                .vertices
                .iter()
                .map(|t| to_xzy(t.as_arr()))
                .collect::<Vec<_>>();

            let triangles = geo
                .triangles
                .iter()
                .flat_map(|t| t.as_arr())
                .collect::<Vec<_>>();

            let material_ids = geo
                .triangles
                .iter()
                .flat_map(|t| {
                    [
                        t.material_id as u32,
                        t.material_id as u32,
                        t.material_id as u32,
                    ]
                })
                .collect::<Vec<_>>();

            let mut normals = geo
                .normals
                .iter()
                .map(|t| Vec3::from(to_xzy(t.as_arr())))
                .collect::<Vec<_>>();

            if normals.is_empty() {
                normals = vec![Vec3::ZERO; vertices.len()];
                for t in &geo.triangles {
                    let v1: Vec3 = vertices[t.vertex1 as usize].into();
                    let v2: Vec3 = vertices[t.vertex2 as usize].into();
                    let v3: Vec3 = vertices[t.vertex3 as usize].into();
                    let edge12 = v2 - v1;
                    let edge13 = v3 - v1;
                    let normal = edge12.cross(edge13);
                    normals[t.vertex1 as usize] += normal;
                    normals[t.vertex2 as usize] += normal;
                    normals[t.vertex3 as usize] += normal;
                }
                normals = normals.into_iter().map(|n| n.normalize()).collect();
            }

            let tex_coords = geo
                .tex_coords
                .first()
                .unwrap_or(&Vec::new())
                .iter()
                .map(|t| t.as_arr())
                .collect::<Vec<_>>();

            let prelit = geo
                .prelit
                .iter()
                .map(|c| c.as_rgba_arr())
                .collect::<Vec<_>>();

            let mat_list = geometry_chunk
                .get_children()
                .iter()
                .find(|c| matches!(c.content, ChunkContent::MaterialList(_)))
                .expect("geometry needs material list");
            if let ChunkContent::MaterialList(list) = &mat_list.content {
                let mut mat_vec = Vec::new();
                for mat_chunk in mat_list.get_children() {
                    let ChunkContent::Material(mat) = mat_chunk.content else {
                        panic!("No valid material chunk in material list");
                    };

                    // Material
                    let mut tex_handle: Option<Handle<Image>> = None;
                    let mut sampler: ImageSamplerDescriptor = Default::default();
                    if let Some(tex_chunk) = mat_chunk.get_children().first() {
                        if let ChunkContent::Texture(tex) = &tex_chunk.content {
                            if let ChunkContent::String(tex_name) =
                                &tex_chunk.get_children()[0].content
                            {
                                let tex_name = tex_name.to_ascii_lowercase();
                                let tex_path = format!("{txd_name}.txd#{tex_name}");
                                debug!("Loading {}", tex_path);

                                let tex_img: Handle<Image> = server.load(tex_path);
                                tex_handle = Some(tex_img);

                                sampler.address_mode_u = match tex.addressing[0] {
                                    TextureAddressingMode::TEXTUREADDRESSNATEXTUREADDRESS => {
                                        todo!()
                                    }
                                    TextureAddressingMode::TEXTUREADDRESSWRAP => {
                                        ImageAddressMode::Repeat
                                    }
                                    TextureAddressingMode::TEXTUREADDRESSMIRROR => {
                                        ImageAddressMode::MirrorRepeat
                                    }
                                    TextureAddressingMode::TEXTUREADDRESSCLAMP => {
                                        ImageAddressMode::ClampToEdge
                                    }
                                    TextureAddressingMode::TEXTUREADDRESSBORDER => {
                                        ImageAddressMode::ClampToBorder
                                    }
                                };
                                sampler.address_mode_v = match tex.addressing[1] {
                                    TextureAddressingMode::TEXTUREADDRESSNATEXTUREADDRESS => {
                                        todo!()
                                    }
                                    TextureAddressingMode::TEXTUREADDRESSWRAP => {
                                        ImageAddressMode::Repeat
                                    }
                                    TextureAddressingMode::TEXTUREADDRESSMIRROR => {
                                        ImageAddressMode::MirrorRepeat
                                    }
                                    TextureAddressingMode::TEXTUREADDRESSCLAMP => {
                                        ImageAddressMode::ClampToEdge
                                    }
                                    TextureAddressingMode::TEXTUREADDRESSBORDER => {
                                        ImageAddressMode::ClampToBorder
                                    }
                                };

                                /*let filter = match tex.filtering {
                                    TextureFilteringMode::FILTERNAFILTERMODE => todo!(),
                                    TextureFilteringMode::FILTERNEAREST => ImageFilterMode::Nearest,
                                    TextureFilteringMode::FILTERLINEAR => ImageFilterMode::Linear,
                                    TextureFilteringMode::FILTERMIPNEAREST => todo!(),
                                    TextureFilteringMode::FILTERMIPLINEAR => todo!(),
                                    TextureFilteringMode::FILTERLINEARMIPNEAREST => todo!(),
                                    TextureFilteringMode::FILTERLINEARMIPLINEAR => todo!(),
                                };*/
                            }
                        }
                    }

                    // TODO: VC and above have the surface properties in the material
                    let surf_prop = geo.surface_prop.unwrap();

                    let mat = RwMaterial {
                        color: LinearRgba::from_f32_array(mat.color.as_rgba_arr()),
                        texture: tex_handle,
                        sampler,
                        ambient_fac: surf_prop.ambient,
                        diffuse_fac: surf_prop.diffuse,
                    };

                    mat_vec.push(mat);
                }

                let mat_vec = {
                    let mut vec = Vec::new();
                    let mut count = 0;
                    for mat_id in &list.0 {
                        if *mat_id == -1 {
                            vec.push(mat_vec[count].clone());
                            count += 1;
                        } else {
                            vec.push(vec[*mat_id as usize].clone());
                        }
                    }
                    vec
                };

                let mat = GTAMaterial { mats: mat_vec };

                // Mesh
                let mut mesh = Mesh::new(topo, RenderAssetUsages::default());

                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);

                if !normals.is_empty() {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
                }

                if geo.tex_coords.len() == 1 {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, tex_coords);
                }

                if !prelit.is_empty() {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, prelit);
                }

                mesh.insert_attribute(ATTRIBUTE_MATERIAL_ID, material_ids);

                mesh.insert_indices(Indices::U16(triangles));

                res.push((mesh, mat));
            }
        }
    }
    res
}
