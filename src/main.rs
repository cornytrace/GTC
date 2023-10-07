mod assets;
mod dat;
mod objects;
mod utils;

mod flycam;

use std::{
    f32::consts::PI,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use anyhow::bail;
use assets::{CustomAssetIoPlugin, Txd};
use bevy::{
    asset::AssetIo,
    prelude::{shape::Quad, *},
    render::render_resource::{Extent3d, PrimitiveTopology, TextureDimension, TextureFormat},
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;

use dat::{GameData, Ide};
use flycam::*;
use rw_rs::{
    bsf::{
        tex::{RasterFormat, RpMaterial, RpRasterPalette},
        *,
    },
    img::Img,
};

use num_traits::cast::FromPrimitive;

use nom_derive::Parse;

use lazy_static::lazy_static;
use utils::to_xzy;
lazy_static! {
    static ref DATA_DIR: PathBuf = PathBuf::from(std::env::var("GTA_DIR").unwrap_or(".".into()));
    static ref IMG: Mutex<Img<'static>> =
        Mutex::new(Img::new(&DATA_DIR.join("models/gta3.img")).expect("gta3.img not found"));
}

fn main() -> anyhow::Result<()> {
    if !DATA_DIR.join("data/gta3.dat").exists() {
        bail!(
            "GTA files not found, set working directory or set the GTA_DIR environment variable."
        );
    }
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Grand Theft Crab".into(),
                        ..default()
                    }),
                    ..default()
                })
                .add_before::<bevy::asset::AssetPlugin, _>(CustomAssetIoPlugin),
        )
        .add_plugins((NoCameraPlayerPlugin, WorldInspectorPlugin::new()))
        .add_systems(Startup, setup)
        .insert_resource(GameData {
            ide: Ide::default(),
        })
        .run();

    Ok(())
}

fn load_meshes(
    bsf: &BsfChunk,
    txd_name: &str,
    server: &Res<AssetServer>,
) -> Vec<Vec<(Mesh, StandardMaterial)>> {
    let mut res = Vec::new();
    for geometry_chunk in &bsf
        .children
        .iter()
        .find(|e| e.header.ty == ChunkType::GeometryList)
        .unwrap()
        .children[1..]
    {
        let mut mesh_mat_vec = Vec::new();
        if let BsfChunkContent::RpGeometry(geo) = &geometry_chunk.content {
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

            let normals = geo
                .normals
                .iter()
                .map(|t| to_xzy(t.as_arr()))
                .collect::<Vec<_>>();

            let tex_coords = geo
                .tex_coords
                .get(0)
                .unwrap_or(&Vec::new())
                .iter()
                .map(|t| t.as_arr())
                .collect::<Vec<_>>();

            let _prelit = geo.prelit.iter().map(|c| c.as_arr()).collect::<Vec<_>>();

            let mat_list = geometry_chunk
                .children
                .iter()
                .find(|c| c.header.ty == ChunkType::MaterialList)
                .expect("geometry needs material list");
            if let BsfChunkContent::RpMaterialList(list) = &mat_list.content {
                for (i, mat_chunk) in mat_list.children.iter().enumerate() {
                    let BsfChunkContent::RpMaterial(mat) = mat_chunk.content else {
                        continue;
                    };

                    // Mesh
                    let mut mesh = Mesh::new(topo);

                    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices.clone());
                    if !geo.normals.is_empty() {
                        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals.clone());
                    }
                    if geo.tex_coords.len() == 1 {
                        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, tex_coords.clone());
                    }

                    if geo.normals.is_empty() {
                        mesh.duplicate_vertices();
                        mesh.compute_flat_normals();
                    }

                    let triangles = geo
                        .triangles
                        .iter()
                        .filter(|t| list.get_index(t.material_id.into()) as usize == i)
                        .flat_map(|t| t.as_arr())
                        .collect::<Vec<_>>();

                    mesh.set_indices(Some(bevy::render::mesh::Indices::U16(triangles)));

                    // Material
                    let mut tex_handle: Option<Handle<Image>> = None;
                    if let Some(tex_chunk) = mat_chunk.children.get(1) {
                        if tex_chunk.header.ty == ChunkType::Texture {
                            if let BsfChunkContent::String(tex_name) =
                                &tex_chunk.children[1].content
                            {
                                let tex_path = format!("{txd_name}.txd#{tex_name}");
                                warn!("Loading {}", tex_path);
                                tex_handle = Some(server.load(tex_path));
                                //TODO set sampler properties
                            }
                        }
                    }

                    let std_mat = StandardMaterial {
                        base_color: Color::WHITE,
                        base_color_texture: tex_handle,
                        double_sided: true,
                        cull_mode: None,
                        unlit: true,
                        ..Default::default()
                    };

                    mesh_mat_vec.push((mesh, std_mat))
                }
            }
        }
        res.push(mesh_mat_vec);
    }
    res
}

fn setup(
    mut commands: Commands,
    mut file_data: ResMut<GameData>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    txds: Res<Assets<Txd>>,
    asset_server: Res<AssetServer>,
) {
    let splash: Handle<Image> = asset_server.load("dyntraffic.txd#towaway");

    let tl = IMG.lock().unwrap().get_file("trafficlight1.dff").unwrap();
    let (_, tl) = BsfChunk::parse(&tl).unwrap();
    let meshes_vec = load_meshes(&tl, "dyntraffic", &asset_server)
        .into_iter()
        .last()
        .unwrap()
        .into_iter()
        .map(|(m, mat)| (meshes.add(m), materials.add(mat)))
        .collect::<Vec<_>>();

    let camera_and_light_transform =
        Transform::from_xyz(0.0, 300.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y);

    let mut ent = commands.spawn(SpatialBundle {
        transform: camera_and_light_transform,
        ..Default::default()
    });
    ent.with_children(|parent| {
        for (mesh, material) in meshes_vec {
            parent.spawn((PbrBundle {
                mesh,
                material,
                ..default()
            },));
        }
    });

    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(Quad::new([2.0, 1.0].into()))),
        material: materials.add(StandardMaterial {
            base_color_texture: Some(splash),
            ..Default::default()
        }),
        transform: camera_and_light_transform,
        ..default()
    });

    //file_data
    //    .load_dat(&mut commands, &mut meshes, &mut materials, asset_server)
    //    .expect("Error loading gta3.dat");

    // Camera in 3D space.
    commands.spawn((
        Camera3dBundle {
            transform: camera_and_light_transform,
            ..default()
        },
        FlyCam,
    ));

    // ambient light
    commands.insert_resource(AmbientLight {
        color: Color::ORANGE_RED,
        brightness: 0.02,
    });

    // directional 'sun' light
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        transform: Transform {
            translation: Vec3::new(0.0, 1000.0, 0.0),
            rotation: Quat::from_rotation_x(-PI / 4.),
            ..default()
        },
        ..default()
    });
}
