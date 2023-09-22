use bevy::prelude::*;
use rw_rs::bsf::BsfChunk;

use crate::{dat::GameData, load_meshes, IMG};

pub fn spawn_obj(
    name: &str,
    pos: [f32; 3],
    scale: [f32; 3],
    rot: Quat,
    data: &mut GameData,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    commands: &mut Commands,
) {
    debug!("loading {}", name);
    let file = IMG
        .lock()
        .unwrap()
        .get_file(name)
        .unwrap_or_else(|| panic!("{} not found in img", name));
    let (_, bsf) = BsfChunk::parse(&file).unwrap();
    let meshes_vec = load_meshes(&bsf)
        .into_iter()
        .map(|m| meshes.add(m))
        .collect::<Vec<_>>();

    if meshes_vec.is_empty() {
        warn!("{} contained zero meshes", name);
        return;
    }

    commands.spawn((PbrBundle {
        mesh: meshes_vec.last().unwrap().clone(),
        material: materials.add(StandardMaterial { ..default() }),
        transform: Transform {
            translation: pos.into(),
            scale: scale.into(),
            rotation: rot,
        },
        ..default()
    },));
}
