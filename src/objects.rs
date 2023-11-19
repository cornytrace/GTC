use bevy::prelude::*;
use rw_rs::bsf::Chunk;

use crate::{dat::GameData, load_meshes, IMG};

pub fn spawn_obj(
    id: u32,
    name: &str,
    pos: [f32; 3],
    scale: [f32; 3],
    rot: Quat,
    data: &mut GameData,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    server: &Res<AssetServer>,
    commands: &mut Commands,
) {
    debug!("loading {}", name);
    let ide = data
        .ide
        .get_by_id(id)
        .expect("INST is not registered as IDE");

    let file = IMG
        .lock()
        .unwrap()
        .get_file(name)
        .unwrap_or_else(|| panic!("{} not found in img", name));
    let (_, bsf) = Chunk::parse(&file).unwrap();
    let meshes_vec = load_meshes(&bsf, &ide.txd_name, server)
        .into_iter()
        .last()
        .unwrap_or_default()
        .into_iter()
        .map(|(m, mat)| (meshes.add(m), materials.add(mat)))
        .collect::<Vec<_>>();

    if meshes_vec.is_empty() {
        warn!("{} contained zero meshes", name);
        return;
    }

    let mut ent = commands.spawn(SpatialBundle {
        transform: Transform {
            translation: pos.into(),
            scale: scale.into(),
            rotation: rot,
        },
        ..Default::default()
    });
    ent.with_children(|parent| {
        for (mesh, material) in meshes_vec {
            parent.spawn((PbrBundle { mesh, ..default() },));
        }
    });
}
