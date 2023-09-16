mod dat;
mod objects;

mod flycam;

use std::{f32::consts::PI, fs, path::PathBuf};

use anyhow::bail;
use bevy::{
    prelude::{shape::Quad, *},
    render::render_resource::{Extent3d, PrimitiveTopology, TextureDimension, TextureFormat},
};

use dat::{GameData, Ide};
use flycam::*;
use rw_rs::{
    bsf::{tex::RasterFormat, *},
    img::Img,
};

use num_traits::cast::FromPrimitive;

fn main() -> anyhow::Result<()> {
    let data_dir: PathBuf = PathBuf::from(std::env::var("GTA_DIR").unwrap_or(".".into()));
    if !data_dir.join("data/gta3.dat").exists() {
        bail!(
            "GTA files not found, set working directory or set the GTA_DIR environment variable."
        );
    }

    let img_path = data_dir.join("models/gta3.img");
    let img = Img::new(&img_path).expect("gta3.img not found");

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Grand Theft Crab".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(NoCameraPlayerPlugin)
        .add_systems(Startup, setup)
        .insert_resource(GameData {
            data_dir,
            img,
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

fn load_textures(bsf: &BsfChunk) -> Vec<Image> {
    let mut texture_vec = Vec::new();

    if bsf.header.ty != ChunkType::TextureDictionary {
        error!("File is not a TXD file!");
        return texture_vec;
    }

    for raster in &bsf.children[1..] {
        if let BsfChunkContent::RpRaster(raster) = &raster.content {
            /*if (raster.raster_format & (RasterFormat::FormatExtPal8 as u32)) != 0
                || (raster.raster_format & (RasterFormat::FormatExtPal4 as u32) != 0)
            {
                todo!("Palette textures not supported yet");
            }*/

            let raster_format = raster.raster_format
                & !(RasterFormat::FormatExtAutoMipmap as u32)
                & !(RasterFormat::FormatExtPal4 as u32)
                & !(RasterFormat::FormatExtPal8 as u32)
                & !(RasterFormat::FormatExtMipmap as u32);
            let raster_format = RasterFormat::from_u32(raster_format).unwrap();
            let format = match raster_format {
                RasterFormat::Format8888 => TextureFormat::Rgba8Uint,
                RasterFormat::Format888 => TextureFormat::Rgba8Uint,
                _ => unimplemented!(),
            };
            let image = Image::new(
                Extent3d {
                    width: raster.width.into(),
                    height: raster.height.into(),
                    depth_or_array_layers: raster.depth.into(),
                },
                TextureDimension::D2,
                raster.data.clone(),
                format,
            );
            texture_vec.push(image);
        } else {
            error!("Unexpected type {:?} found in TXD file", raster.header.ty);
            continue;
        }
    }

    texture_vec
}

fn setup(
    mut commands: Commands,
    mut file_data: ResMut<GameData>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let splash = fs::read(file_data.data_dir.join("txd/SPLASH1.TXD")).unwrap();
    let (_, tex_bsf) = BsfChunk::parse(&splash).unwrap();
    let images = load_textures(&tex_bsf);

    let camera_and_light_transform =
        Transform::from_xyz(0.0, 300.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y);

    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(Quad::new([2.0, 1.0].into()))),
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
