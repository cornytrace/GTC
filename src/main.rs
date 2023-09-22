mod dat;
mod objects;

mod flycam;

use std::{
    f32::consts::PI,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use anyhow::bail;
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
            data_dir: DATA_DIR.to_path_buf(),
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
            let mut data: Vec<u8> = Vec::new();
            if (raster.raster_format & (RasterFormat::FormatExtPal8 as u32)) != 0 {
                let (indices, palette) = RpRasterPalette::<256>::parse(&raster.data).unwrap();
                let indices = &indices[3..];
                for h in 0..(raster.height as usize) {
                    for w in 0..(raster.width as usize) {
                        let index = indices[w + (h * (raster.width as usize))];
                        let color = palette.0[index as usize];
                        data.push(color.r);
                        data.push(color.g);
                        data.push(color.b);
                        data.push(color.a);
                    }
                }
            } else if (raster.raster_format & (RasterFormat::FormatExtPal4 as u32)) != 0 {
                let (indices, palette) = RpRasterPalette::<32>::parse(&raster.data).unwrap();
                let indices = &indices[3..];
                for h in 0..(raster.height as usize) {
                    for w in 0..(raster.width as usize) {
                        let index = indices[w + (h * (raster.width as usize))];
                        let color = palette.0[index as usize];
                        data.push(color.r);
                        data.push(color.g);
                        data.push(color.b);
                        data.push(color.a);
                    }
                }
            } else {
                data = raster.data.clone();
            }

            let raster_format = raster.raster_format
                & !(RasterFormat::FormatExtAutoMipmap as u32)
                & !(RasterFormat::FormatExtPal4 as u32)
                & !(RasterFormat::FormatExtPal8 as u32)
                & !(RasterFormat::FormatExtMipmap as u32);
            let raster_format = RasterFormat::from_u32(raster_format).unwrap();
            let format = match raster_format {
                RasterFormat::Format8888 => TextureFormat::Rgba8Unorm,
                RasterFormat::Format888 => TextureFormat::Rgba8Unorm,
                _ => unimplemented!(),
            };
            let image = Image::new(
                Extent3d {
                    width: raster.width.into(),
                    height: raster.height.into(),
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                data,
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
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let splash = fs::read(file_data.data_dir.join("txd/SPLASH1.TXD")).unwrap();
    let (_, tex_bsf) = BsfChunk::parse(&splash).unwrap();
    let images_vec = load_textures(&tex_bsf);
    let images_vec = images_vec
        .into_iter()
        .map(|i| images.add(i))
        .collect::<Vec<_>>();

    let camera_and_light_transform =
        Transform::from_xyz(0.0, 300.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y);

    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(Quad::new([2.0, 1.0].into()))),
        material: materials.add(StandardMaterial {
            base_color_texture: Some(images_vec[0].clone()),
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

struct GTAAssetIo(Box<dyn AssetIo>);

impl AssetIo for GTAAssetIo {
    fn load_path<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> bevy::utils::BoxedFuture<'a, anyhow::Result<Vec<u8>, bevy::asset::AssetIoError>> {
        if path.components().count() == 1
            && path
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("dff") || x.eq_ignore_ascii_case("txd"))
        {
            let path_ext = path.extension();
            if let Some(path_ext) = path_ext {
                if let Some(file) = IMG
                    .lock()
                    .unwrap()
                    .get_file(path.to_string_lossy().as_ref())
                {
                    return Box::pin(async move { Ok(file.clone()) });
                } else if path_ext.eq_ignore_ascii_case("dff") {
                    return Box::pin(async move {
                        self.0.load_path(&Path::new("models").join(path)).await
                    });
                } else if path_ext.eq_ignore_ascii_case("txd") {
                    return Box::pin(async move {
                        self.0.load_path(&Path::new("txd").join(path)).await
                    });
                } else {
                    return self.0.load_path(path);
                }
            }
        }
        self.0.load_path(path)
    }

    fn read_directory(
        &self,
        path: &std::path::Path,
    ) -> anyhow::Result<Box<dyn Iterator<Item = PathBuf>>, bevy::asset::AssetIoError> {
        self.0.read_directory(path)
    }

    fn get_metadata(
        &self,
        path: &std::path::Path,
    ) -> anyhow::Result<bevy::asset::Metadata, bevy::asset::AssetIoError> {
        self.0.get_metadata(path)
    }

    fn watch_path_for_changes(
        &self,
        to_watch: &std::path::Path,
        to_reload: Option<PathBuf>,
    ) -> anyhow::Result<(), bevy::asset::AssetIoError> {
        self.0.watch_path_for_changes(to_watch, to_reload)
    }

    fn watch_for_changes(
        &self,
        configuration: &bevy::asset::ChangeWatcher,
    ) -> anyhow::Result<(), bevy::asset::AssetIoError> {
        self.0.watch_for_changes(configuration)
    }
}

struct CustomAssetIoPlugin;

impl Plugin for CustomAssetIoPlugin {
    fn build(&self, app: &mut App) {
        let default_io = AssetPlugin {
            asset_folder: DATA_DIR.to_string_lossy().to_string(),
            ..Default::default()
        }
        .create_platform_default_asset_io();

        // create the custom asset io instance
        let asset_io = GTAAssetIo(default_io);

        // the asset server is constructed and added the resource manager
        app.insert_resource(AssetServer::new(asset_io));
    }
}
