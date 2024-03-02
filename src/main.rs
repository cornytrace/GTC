mod assets;
mod dat;
mod objects;
mod utils;

mod flycam;

use std::{f32::consts::PI, path::PathBuf, sync::Mutex};

use anyhow::bail;
use assets::{GTAAssetReader, Txd, TxdLoader};
use bevy::{
    asset::io::{AssetSource, AssetSourceId},
    audio::AudioPlugin,
    log::LogPlugin,
    prelude::*,
    render::{render_asset::RenderAssetUsages, render_resource::PrimitiveTopology},
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;

use dat::{GameData, Ide};
use flycam::*;
use rw_rs::{bsf::*, img::Img};

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
        .register_asset_source(
            AssetSourceId::default(),
            AssetSource::build().with_reader(|| Box::new(GTAAssetReader)),
        )
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Grand Theft Crab".into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(LogPlugin {
                    filter: "info,wgpu_core=warn,wgpu_hal=warn,gtc=info".into(),
                    level: bevy::log::Level::DEBUG,
                    ..Default::default()
                })
                .disable::<AudioPlugin>(),
        )
        .register_asset_loader(TxdLoader)
        .init_asset::<Txd>()
        .add_plugins((NoCameraPlayerPlugin, WorldInspectorPlugin::new()))
        .add_systems(Startup, setup)
        .insert_resource(GameData {
            ide: Ide::default(),
        })
        .run();

    Ok(())
}

fn load_meshes(
    bsf: &Chunk,
    txd_name: &str,
    server: &Res<AssetServer>,
    //images: &ResMut<Assets<Image>>,
) -> Vec<Vec<(Mesh, StandardMaterial)>> {
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
                .get(0)
                .unwrap_or(&Vec::new())
                .iter()
                .map(|t| t.as_arr())
                .collect::<Vec<_>>();

            let _prelit = geo.prelit.iter().map(|c| c.as_arr()).collect::<Vec<_>>();

            let mat_list = geometry_chunk
                .get_children()
                .iter()
                .find(|c| matches!(c.content, ChunkContent::MaterialList(_)))
                .expect("geometry needs material list");
            if let ChunkContent::MaterialList(list) = &mat_list.content {
                for (i, mat_chunk) in mat_list.get_children().iter().enumerate() {
                    let ChunkContent::Material(mat) = mat_chunk.content else {
                        continue;
                    };

                    // Mesh
                    let mut mesh = Mesh::new(topo, RenderAssetUsages::default());

                    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices.clone());

                    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals.clone());

                    if geo.tex_coords.len() == 1 {
                        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, tex_coords.clone());
                    }

                    let triangles = geo
                        .triangles
                        .iter()
                        .filter(|t| list.get_index(t.material_id.into()) as usize == i)
                        .flat_map(|t| t.as_arr())
                        .collect::<Vec<_>>();

                    mesh.insert_indices(bevy::render::mesh::Indices::U16(triangles));

                    // Material
                    let mut tex_handle: Option<Handle<Image>> = None;
                    if let Some(tex_chunk) = mat_chunk.get_children().get(0) {
                        if matches!(tex_chunk.content, ChunkContent::Texture(_)) {
                            if let ChunkContent::String(tex_name) =
                                &tex_chunk.get_children()[0].content
                            {
                                let tex_path = format!("{txd_name}.txd#{tex_name}");
                                debug!("Loading {}", tex_path);
                                let tex: Handle<Image> = server.load(tex_path);
                                tex_handle = Some(tex);
                            }
                        }
                    }

                    let std_mat = StandardMaterial {
                        base_color: Color::WHITE,
                        base_color_texture: tex_handle,
                        cull_mode: None,
                        double_sided: true,
                        unlit: false,
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
    let camera_and_light_transform =
        Transform::from_xyz(0.0, 300.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y);

    // Compile-time  switch between loading single object and entire city
    if true {
        file_data
            .load_dat(&mut commands, &mut meshes, &mut materials, asset_server)
            .expect("Error loading gta3.dat");
    } else {
        let tl = IMG.lock().unwrap().get_file("trafficlight1.dff").unwrap();
        let (_, tl) = Chunk::parse(&tl).unwrap();
        let meshes_vec = load_meshes(&tl, "dyntraffic", &asset_server)
            .into_iter()
            .last()
            .unwrap()
            .into_iter()
            .map(|(m, mat)| (meshes.add(m), materials.add(mat)))
            .collect::<Vec<_>>();

        let mut ent = commands.spawn(SpatialBundle {
            transform: Transform::from_xyz(0.0, 290.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
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
    }

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
