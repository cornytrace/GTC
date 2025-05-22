use avian3d::prelude::*;
use bevy::prelude::*;
use rw_rs::{bsf::Chunk, col::CollV1};

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
    assert!(data.name == ide.model_name);

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
        .next_back()
        .unwrap_or_default()
        .into_iter()
        .map(|(m, mat)| (meshes.add(m), materials.add(mat)))
        .collect::<Vec<_>>();

    if meshes_vec.is_empty() {
        warn!("{} contained zero meshes", data.name);
        return;
    }

    let mut ent = commands.spawn((
        Transform {
            translation: data.pos.into(),
            scale: data.scale.into(),
            rotation: data.rot,
        },
        Visibility::Visible,
    ));
    ent.with_children(|parent| {
        for (mesh, material) in meshes_vec {
            parent.spawn((Mesh3d(mesh), MeshMaterial3d(material)));
        }
    });

    if let Some(col) = game_data.col.get(&data.name) {
        spawn_collision(col, ent.id(), commands);
    }
}

pub fn spawn_collision(col: &CollV1, parent: Entity, mut commands: Commands) {
    let mut parent = commands.get_entity(parent).unwrap();
    parent.insert_if_new(RigidBody::Static);

    for sphere in &col.spheres {
        parent.with_child((
            Collider::sphere(sphere.radius),
            Transform::from_xyz(-sphere.center.x, sphere.center.z, sphere.center.y),
        ));
    }
    for tbox in &col.boxes {
        parent.with_child((
            Collider::cuboid(
                (tbox.max.x - tbox.min.x).abs(),
                (tbox.max.z - tbox.min.z).abs(),
                (tbox.max.y - tbox.min.y).abs(),
            ),
            Transform::from_xyz(
                -((tbox.max.x + tbox.min.x) / 2.0),
                (tbox.max.z + tbox.min.z) / 2.0,
                (tbox.max.y + tbox.min.y) / 2.0,
            ),
        ));
    }
    if !&col.vertices.is_empty() {
        parent.with_child(Collider::trimesh(
            col.vertices
                .iter()
                .map(|v| Vec3 {
                    x: -v.0[0],
                    y: v.0[2],
                    z: v.0[1],
                })
                .collect(),
            col.faces.iter().map(|f| [f.a, f.b, f.c]).collect(),
        ));
    }
}
