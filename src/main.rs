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

use dat::{GameData, Ide};
use flycam::*;
use rw_rs::{
    bsf::{
        tex::{RasterFormat, RpRasterPalette},
        *,
    },
    img::Img,
};

use num_traits::cast::FromPrimitive;

use nom_derive::Parse;

use lazy_static::lazy_static;
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
        .add_plugins(NoCameraPlayerPlugin)
        .add_systems(Startup, setup)
        .insert_resource(GameData {
            ide: Ide::default(),
        })
        .run();

    Ok(())
}

fn load_meshes(bsf: &BsfChunk) -> Vec<Mesh> {
    let mut mesh_vec = Vec::new();

    for geometry_chunk in &bsf
        .children
        .iter()
        .find(|e| e.header.ty == ChunkType::GeometryList)
        .unwrap()
        .children[1..]
    {
        if let BsfChunkContent::RpGeometry(geo) = &geometry_chunk.content {
            let topo = if geo.is_tristrip() {
                PrimitiveTopology::TriangleStrip
            } else {
                PrimitiveTopology::TriangleList
            };
            let mut mesh = Mesh::new(topo);
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_POSITION,
                geo.vertices
                    .iter()
                    .map(|t| to_xzy(t.as_arr()))
                    .collect::<Vec<_>>(),
            );
            if !geo.normals.is_empty() {
                mesh.insert_attribute(
                    Mesh::ATTRIBUTE_NORMAL,
                    geo.normals
                        .iter()
                        .map(|t| to_xzy(t.as_arr()))
                        .collect::<Vec<_>>(),
                );
            }
            if geo.tex_coords.len() == 1 {
                mesh.insert_attribute(
                    Mesh::ATTRIBUTE_UV_0,
                    geo.tex_coords[0]
                        .iter()
                        .map(|t| t.as_arr())
                        .collect::<Vec<_>>(),
                );
            }

            /*if !geo.prelit.is_empty() {
                mesh.insert_attribute(
                    Mesh::ATTRIBUTE_COLOR,
                    geo.prelit.iter().map(|c| c.as_arr()).collect::<Vec<_>>(),
                );
            }*/

            mesh.set_indices(Some(bevy::render::mesh::Indices::U16(
                geo.triangles
                    .iter()
                    .flat_map(|t| t.as_arr())
                    .collect::<Vec<_>>(),
            )));

            if geo.normals.is_empty() {
                mesh.duplicate_vertices();
                mesh.compute_flat_normals();
            }

            mesh_vec.push(mesh);
        }
    }
    mesh_vec
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
    let splash: Handle<Image> = asset_server.load("txd/splash1.txd#splash1");

    let camera_and_light_transform =
        Transform::from_xyz(0.0, 300.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y);

    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(Quad::new([2.0, 1.0].into()))),
        material: materials.add(StandardMaterial {
            base_color_texture: Some(splash),
            ..Default::default()
        }),
        transform: camera_and_light_transform,
        ..default()
    });

    file_data
        .load_dat(&mut commands, &mut meshes, &mut materials)
        .expect("Error loading gta3.dat");

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

    /*commands.spawn(
        TextBundle::from_section(
            "Controls:\nX/Y/Z: Rotate\nR: Reset orientation\n+/-: Show different geometry in dff",
            TextStyle {
                font_size: 20.0,
                ..default()
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        }),
    );*/
}

// For converting GTA coords system to Bevy
fn to_xzy<T: Copy + std::ops::Neg<Output = T>>(coords: [T; 3]) -> [T; 3] {
    [-coords[0], coords[2], coords[1]]
}
