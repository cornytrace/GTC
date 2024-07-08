use bevy::{prelude::*, render::view::VisibilityRange, utils::info};
use rw_rs::bsf::Chunk;

use crate::{dat::GameData, material::GTAMaterial, mesh::load_dff, IMG};

#[derive(Event)]
pub struct SpawnObject {
    pub id: u32,
    pub name: String,
    pub pos: [f32; 3],
    pub scale: [f32; 3],
    pub rot: Quat,
}

pub fn spawn_obj(
    trigger: Trigger<SpawnObject>,
    game_data: Res<GameData>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GTAMaterial>>,
    server: Res<AssetServer>,
    mut commands: Commands,
) {
    let data = trigger.event();
    debug!("loading {}", data.name);

    let Some(ide) = game_data.ide.get_by_id(data.id) else {
        error!("tried to spawn IPL with invalid IDE id {0}", data.id);
        return;
    };

    if ide.draw_distance[0] > 299.0 {
        if !data.name.contains("LOD") {
            warn!("skipping LOD with non-lod name {}", data.name);
        } else {
            info!("skipping LOD {}", data.name);
        }
        return;
    }

    let file = IMG
        .lock()
        .unwrap()
        .get_file(&format!("{}.dff", data.name))
        .unwrap_or_else(|| panic!("{} not found in img", data.name));
    let (_, bsf) = Chunk::parse(&file).unwrap();
    let meshes_vec = load_dff(&bsf, &ide.txd_name, &server)
        .into_iter()
        .last()
        .unwrap_or_default()
        .into_iter()
        .map(|(m, mat)| (meshes.add(m), materials.add(mat)))
        .collect::<Vec<_>>();

    if meshes_vec.is_empty() {
        warn!("{} contained zero meshes", data.name);
        return;
    }

    let mut ent = commands.spawn(SpatialBundle {
        transform: Transform {
            translation: data.pos.into(),
            scale: data.scale.into(),
            rotation: data.rot,
        },
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
