mod assets;
mod dat;
mod material;
mod mesh;
mod objects;
mod utils;

mod flycam;

use std::{path::PathBuf, sync::Mutex};

use assets::{GTAAssetReader, Txd, TxdLoader};
use avian3d::prelude::*;
use bevy::{
    asset::io::{AssetSource, AssetSourceId},
    audio::AudioPlugin,
    image::{ImageAddressMode, ImageSamplerDescriptor},
    log::LogPlugin,
    prelude::*,
};
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};

use clap::Parser;
use dat::GameData;
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

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long)]
    viewer: bool,
}

fn main() -> AppExit {
    let args = Args::parse();

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
    .add_plugins((
        PhysicsPlugins::default(), /*PhysicsDebugPlugin::default()*/
    ))
    .add_plugins((
        EguiPlugin {
            enable_multipass_for_primary_context: false,
        },
        WorldInspectorPlugin::new(),
    ))
    .insert_resource(GameData::default())
    .add_observer(spawn_obj);

    if args.viewer {
        app.add_systems(Startup, setup_viewer)
            .add_plugins(ViewerCameraPlugin);
    } else {
        app.add_systems(Startup, setup_game)
            .add_plugins(GameCameraPlugin);
    }

    app.run()
}

fn setup_game(
    mut commands: Commands,
    mut game_data: ResMut<GameData>,
    mut materials: ResMut<Assets<GTAMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    game_data
        .load_dat(&mut commands)
        .expect("Error loading gta3.dat");

    const WATER_TILE_SIZE: f32 = 32.0;

    #[derive(Component)]
    struct WaterParent;
    let mut water_parent = commands.spawn((
        Transform::from_xyz(2048.0 - 16.0, 0.0, -(2048.0 - 16.0)),
        Visibility::Visible,
        WaterParent,
    ));
    match game_data.load_water() {
        Ok(()) => {
            for i in 0..128 * 128 {
                let height = game_data.water_level[i];
                if height == f32::NEG_INFINITY {
                    continue;
                }

                water_parent.with_child((
                    Mesh3d(meshes.add(Plane3d::new(
                        Vec3::Y,
                        Vec2 {
                            x: WATER_TILE_SIZE / 2.0,
                            y: WATER_TILE_SIZE / 2.0,
                        },
                    ))),
                    MeshMaterial3d(materials.add(GTAMaterial {
                        color: LinearRgba {
                            red: 1.0,
                            green: 1.0,
                            blue: 1.0,
                            alpha: 1.0,
                        },
                        texture: Some(asset_server.load("particle.txd#water_old")),
                        sampler: ImageSamplerDescriptor::default(),
                        ambient_fac: 0.0,
                        diffuse_fac: 1.0,
                        ambient_light: LinearRgba {
                            red: 0.0,
                            green: 0.0,
                            blue: 0.0,
                            alpha: 1.0,
                        },
                    })),
                    Transform::from_xyz(
                        -(f32::floor((i as f32) / 128.0) * WATER_TILE_SIZE),
                        height,
                        (i % 128) as f32 * WATER_TILE_SIZE,
                    ),
                ));
            }
        }
        Err(e) => error!("Error loading water: {e}"),
    }
}

fn setup_viewer(
    mut commands: Commands,
    mut _game_data: ResMut<GameData>,
    mut materials: ResMut<Assets<GTAMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    let tl = IMG.lock().unwrap().get_file("trafficlight1.dff").unwrap();
    let (_, tl) = Chunk::parse(&tl).unwrap();
    let meshes_vec = load_dff(&tl, "dyntraffic", &asset_server)
        .into_iter()
        .next_back()
        .unwrap()
        .into_iter()
        .map(|(m, mat)| (meshes.add(m), materials.add(mat)))
        .collect::<Vec<_>>();

    let mut ent = commands.spawn((
        Transform::from_xyz(0.0, 290.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        Visibility::Visible,
    ));
    ent.with_children(|parent| {
        for (mesh, material) in meshes_vec {
            parent.spawn((Mesh3d(mesh), MeshMaterial3d(material)));
        }
    });

    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::X, Vec2 { x: 32., y: 32. }))),
        MeshMaterial3d(materials.add(GTAMaterial {
            color: LinearRgba {
                red: 1.0,
                green: 1.0,
                blue: 1.0,
                alpha: 255.0,
            },
            texture: Some(asset_server.load("particle.txd#water_old")),
            sampler: ImageSamplerDescriptor::default(),
            ambient_fac: 0.0,
            diffuse_fac: 1.0,
            ambient_light: LinearRgba {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            },
        })),
    ));
}
