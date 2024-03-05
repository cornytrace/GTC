use std::{collections::HashSet, sync::Mutex};

use bevy::{
    prelude::*,
    render::{
        mesh::PrimitiveTopology,
        render_asset::RenderAssetUsages,
        texture::{ImageAddressMode, ImageSamplerDescriptor},
    },
};
use rw_rs::bsf::{tex::TextureAddressingMode, Chunk, ChunkContent};

use crate::{assets::Txd, material::GTAMaterial, utils::to_xzy};

//TEMP: try to work around issue bevy#10820
static IMG_VEC: Mutex<Vec<Handle<Image>>> = Mutex::new(Vec::new());
static TXD_VEC: Mutex<Vec<Handle<Txd>>> = Mutex::new(Vec::new());

pub fn load_dff(
    bsf: &Chunk,
    txd_name: &str,
    server: &Res<AssetServer>,
    //images: &ResMut<Assets<Image>>,
) -> Vec<Vec<(Mesh, GTAMaterial)>> {
    TXD_VEC
        .lock()
        .unwrap()
        .push(server.load(format!("{txd_name}.txd")));

    let mut res = Vec::new();
    for geometry_chunk in &bsf
        .get_children()
        .iter()
        .find(|e| matches!(e.content, ChunkContent::GeometryList))
        .unwrap()
        .get_children()[1..]
    {
        let mut mesh_mat_vec = Vec::new();
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

            let mut normals = geo
                .normals
                .iter()
                .map(|t| Vec3::from(to_xzy(t.as_arr())))
                .collect::<Vec<_>>();

            /*if normals.is_empty() {
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
            }*/

            let tex_coords = geo
                .tex_coords
                .get(0)
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
                // Because Bevy only allows one Material per Mesh, we need to split the mesh
                for (mat_num, mat_chunk) in mat_list.get_children().iter().enumerate() {
                    let ChunkContent::Material(mat) = mat_chunk.content else {
                        continue;
                    };

                    // Mesh
                    let mut mesh = Mesh::new(topo, RenderAssetUsages::default());

                    let mut used_triangles = geo
                        .triangles
                        .iter()
                        .filter(|t| list.get_index(t.material_id.into()) as usize == mat_num)
                        .flat_map(|t| t.as_arr())
                        .collect::<Vec<_>>();

                    let mut used_vertices = vertices.clone();
                    let mut used_normals = normals.clone();
                    let mut used_tex_coords = tex_coords.clone();
                    let mut used_prelit = prelit.clone();

                    {
                        let mut i = 0;
                        while i < used_vertices.len() {
                            if used_triangles.contains(&(i as u16)) {
                                i += 1;
                            } else {
                                used_vertices.remove(i);
                                if !normals.is_empty() {
                                    used_normals.remove(i);
                                }
                                if !tex_coords.is_empty() {
                                    used_tex_coords.remove(i);
                                }
                                if !prelit.is_empty() {
                                    used_prelit.remove(i);
                                }
                                for triangle in &mut used_triangles {
                                    if (*triangle as usize) > i {
                                        *triangle -= 1;
                                    }
                                }
                            }
                        }
                    }

                    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, used_vertices);

                    if !normals.is_empty() {
                        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, used_normals);
                    }

                    if geo.tex_coords.len() == 1 {
                        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, used_tex_coords);
                    }

                    if !prelit.is_empty() {
                        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, used_prelit);
                    }

                    mesh.insert_indices(bevy::render::mesh::Indices::U16(used_triangles));

                    // Material
                    let mut tex_handle: Option<Handle<Image>> = None;
                    let mut sampler: ImageSamplerDescriptor = Default::default();
                    if let Some(tex_chunk) = mat_chunk.get_children().get(0) {
                        if let ChunkContent::Texture(tex) = &tex_chunk.content {
                            if let ChunkContent::String(tex_name) =
                                &tex_chunk.get_children()[0].content
                            {
                                let tex_path = format!("{txd_name}.txd#{tex_name}");
                                debug!("Loading {}", tex_path);

                                let tex_img: Handle<Image> = server.load(tex_path);
                                IMG_VEC.lock().unwrap().push(tex_img.clone());
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
                                    tex::TextureFilteringMode::FILTERNAFILTERMODE => todo!(),
                                    tex::TextureFilteringMode::FILTERNEAREST => ImageFilterMode::Nearest,
                                    tex::TextureFilteringMode::FILTERLINEAR => ImageFilterMode::Linear,
                                    tex::TextureFilteringMode::FILTERMIPNEAREST => todo!(),
                                    tex::TextureFilteringMode::FILTERMIPLINEAR => todo!(),
                                    tex::TextureFilteringMode::FILTERLINEARMIPNEAREST => todo!(),
                                    tex::TextureFilteringMode::FILTERLINEARMIPLINEAR => todo!(),
                                };*/
                            }
                        }
                    }

                    // TODO: VC and above have the surface properties in the material
                    let surf_prop = geo.surface_prop.unwrap();

                    let mat = GTAMaterial {
                        color: Color::rgba_from_array(mat.color.as_rgba_arr()),
                        texture: tex_handle,
                        sampler,
                        ambient_fac: surf_prop.ambient,
                        diffuse_fac: surf_prop.diffuse,
                        ambient_light: default(),
                    };

                    mesh_mat_vec.push((mesh, mat))
                }
            }
        }
        res.push(mesh_mat_vec);
    }
    res
}
