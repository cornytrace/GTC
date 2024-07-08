mod assets;
mod dat;
mod material;
mod mesh;
mod objects;
mod utils;

mod flycam;

use std::{path::PathBuf, sync::Mutex};

use anyhow::bail;
use assets::{GTAAssetReader, Txd, TxdLoader};
use bevy::{
    asset::io::{AssetSource, AssetSourceId},
    audio::AudioPlugin,
    log::LogPlugin,
    prelude::*,
    render::texture::{ImageAddressMode, ImageSamplerDescriptor},
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;

use dat::{GameData, Ide};
use flycam::*;
use material::{GTAMaterial, GTAMaterialPlugin};
use mesh::load_dff;
use objects::spawn_obj;
use rw_rs::{bsf::*, img::Img};

use lazy_static::lazy_static;
use utils::to_xzy;
lazy_static! {
    static ref GTA_DIR: PathBuf = PathBuf::from(std::env::var("GTA_DIR").unwrap_or(".".into()));
    static ref IMG: Mutex<Img<'static>> =
        Mutex::new(Img::new(&GTA_DIR.join("models/gta3.img")).expect("gta3.img not found"));
}

fn main() -> AppExit {
    if !GTA_DIR.join("data/gta3.dat").exists() {
        error!(
            "GTA files not found, set working directory or set the GTA_DIR environment variable."
        );
        return AppExit::error();
    }
    let mut app = App::new();
    app.register_asset_source(
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
            .set(ImagePlugin {
                default_sampler: ImageSamplerDescriptor {
                    address_mode_u: ImageAddressMode::Repeat,
                    address_mode_v: ImageAddressMode::Repeat,
                    ..Default::default()
                },
            })
            .disable::<AudioPlugin>(),
    )
    .register_asset_loader(TxdLoader)
    .init_asset::<Txd>()
    .add_plugins(GTAMaterialPlugin)
    .add_plugins((NoCameraPlayerPlugin, WorldInspectorPlugin::new()))
    .add_systems(Startup, setup)
    .insert_resource(GameData {
        ide: Ide::default(),
    })
    .observe(spawn_obj);

    app.run()
}

fn setup(
    mut commands: Commands,
    mut file_data: ResMut<GameData>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<GTAMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    txds: Res<Assets<Txd>>,
    asset_server: Res<AssetServer>,
) {
    let camera_and_light_transform =
        Transform::from_xyz(0.0, 300.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y);

    // Camera in 3D space.
    commands.spawn((
        Camera3dBundle {
            transform: camera_and_light_transform,
            ..default()
        },
        FlyCam,
    ));

    // Compile-time  switch between loading single object and entire city
    if true {
        file_data
            .load_dat(&mut commands, &mut meshes, &mut materials, asset_server)
            .expect("Error loading gta3.dat");
    } else {
        let tl = IMG.lock().unwrap().get_file("trafficlight1.dff").unwrap();
        let (_, tl) = Chunk::parse(&tl).unwrap();
        let meshes_vec = load_dff(&tl, "dyntraffic", &asset_server)
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
                parent.spawn((MaterialMeshBundle::<GTAMaterial> {
                    mesh,
                    material,
                    ..default()
                },));
            }
        });
    }

    // ambient light
    /*commands.insert_resource(AmbientLight {
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
    });*/
}
