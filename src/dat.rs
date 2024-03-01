use std::collections::HashMap;

use anyhow::anyhow;
use bevy::prelude::*;

use crate::{
    objects::spawn_obj,
    to_xzy,
    utils::{get_path, to_path},
    DATA_DIR,
};

#[derive(Resource)]
pub struct GameData {
    pub ide: Ide,
}

impl GameData {
    pub fn load_dat(
        &mut self,
        commands: &mut Commands,
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &mut ResMut<Assets<StandardMaterial>>,
        server: Res<AssetServer>,
    ) -> anyhow::Result<()> {
        let dat = std::fs::read_to_string(DATA_DIR.join("data/gta3.dat"))?;
        let lines = dat.split('\n').map(|e| e.trim()).collect::<Vec<_>>();
        for line in lines {
            let words = line
                .split_whitespace()
                .take_while(|s| !s.contains('#'))
                .collect::<Vec<_>>();
            if words.is_empty() {
                continue;
            }

            let ty = words[0].to_lowercase();
            match ty.as_str() {
                "ide" | "mapzone" | "ipl" => {
                    self.load_def(ty.as_str(), words[1], commands, meshes, materials, &server)?
                }
                "splash" => {}
                "colfile" => {}
                _ => todo!(),
            }
        }
        Ok(())
    }

    pub fn load_def(
        &mut self,
        ty: &str,
        path: &str,
        commands: &mut Commands,
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &mut ResMut<Assets<StandardMaterial>>,
        server: &Res<AssetServer>,
    ) -> anyhow::Result<()> {
        let path = get_path(&to_path(path)).ok_or(anyhow!("{} not found!", path))?;
        let dat = std::fs::read_to_string(&path)?;
        let lines = dat.split('\n').map(|e| e.trim()).collect::<Vec<_>>();

        let mut section = String::new();
        for (linecount, line) in lines.into_iter().enumerate() {
            let linecount = linecount + 1;
            let line = line.replace(',', "");
            let words = line
                .split_whitespace()
                .take_while(|s| !s.contains('#'))
                .collect::<Vec<_>>();

            if words.is_empty() {
                continue;
            }

            if words.len() == 1 {
                if words[0] == "end" {
                    section = "".to_string();
                } else {
                    section = words[0].to_owned();
                }
                continue;
            }
            match section.to_lowercase().as_str() {
                // IDE
                "objs" => {
                    let mut obj = IdeObj {
                        id: words[0].parse().unwrap(),
                        model_name: words[1].to_string(),
                        txd_name: words[2].to_string(),
                        mesh_count: 0,
                        draw_distance: [0.0; 3],
                        flags: 0,
                    };
                    match words.len() {
                        n @ 6..=8 => {
                            let n = n - 5;
                            obj.mesh_count = n as u32;
                            for i in 0..n {
                                obj.draw_distance[i] = words[4 + i].parse().unwrap();
                            }
                            obj.flags = words[4 + n].parse().unwrap();
                        }
                        5 => {
                            obj.mesh_count = 1;
                            obj.draw_distance[0] = words[3].parse().unwrap();
                            obj.flags = words[4].parse().unwrap();
                        }
                        _ => {
                            error!("Error parsing obj on line {} of file {}, invalid amount of arguments", linecount, &path.display());
                            continue;
                        }
                    }
                    self.ide.objs.insert(obj.id, obj);
                }

                "tobj" => {
                    // TODO: parse TimeOn & TimeOff
                    let mut obj = IdeObj {
                        id: words[0].parse().unwrap(),
                        model_name: words[1].to_string(),
                        txd_name: words[2].to_string(),
                        mesh_count: 0,
                        draw_distance: [0.0; 3],
                        flags: 0,
                    };
                    match words.len() {
                        n @ 8..=10 => {
                            let n = n - 7;
                            obj.mesh_count = n as u32;
                            for i in 0..n {
                                obj.draw_distance[i] = words[4 + i].parse().unwrap();
                            }
                            obj.flags = words[4 + n].parse().unwrap();
                        }
                        7 => {
                            obj.mesh_count = 1;
                            obj.draw_distance[0] = words[3].parse().unwrap();
                            obj.flags = words[4].parse().unwrap();
                        }
                        _ => {
                            error!("Error parsing obj on line {} of file {}, invalid amount of arguments", linecount, &path.display());
                            continue;
                        }
                    }
                    self.ide.objs.insert(obj.id, obj);
                }

                "hier" => {}

                "cars" => {}

                "peds" => {}

                "path" if ty == "ide" => {}

                "2dfx" => {}

                "weap" => {}

                "anim" => {}

                "txdp" => {}

                // IPL
                "inst" => {
                    if words[1].starts_with("LOD") {
                        continue;
                    }
                    let name = format!("{}.dff", words[1]);

                    let pos = to_xzy([
                        words[2].parse::<f32>().unwrap(),
                        words[3].parse::<f32>().unwrap(),
                        words[4].parse::<f32>().unwrap(),
                    ]);

                    let scale = [
                        words[5].parse().unwrap(),
                        words[6].parse().unwrap(),
                        words[7].parse().unwrap(),
                    ];

                    let rot = Quat::from_array([
                        words[8].parse::<f32>().unwrap(),
                        -words[10].parse::<f32>().unwrap(),
                        -words[9].parse::<f32>().unwrap(),
                        words[11].parse::<f32>().unwrap(),
                    ])
                    .normalize();

                    spawn_obj(
                        words[0].parse::<u32>().unwrap(),
                        &name,
                        pos,
                        scale,
                        rot,
                        self,
                        meshes,
                        materials,
                        server,
                        commands,
                    );
                }

                "zone" => {}

                "cull" => {}

                "pick" => {}

                "path" if ty == "ipl" => {}

                "occl" => {}

                "" => {
                    error!("Line {} found outside of a section", linecount)
                }

                s => {
                    error!(
                        "Unknown section {} found in file {} at line {}, ignoring",
                        s,
                        &path.display(),
                        linecount
                    );
                }
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct Ide {
    objs: HashMap<u32, IdeObj>,
}

impl Ide {
    pub fn get_by_id(&self, id: u32) -> Option<&IdeObj> {
        self.objs.get(&id)
    }
}

pub struct IdeObj {
    pub id: u32,
    pub model_name: String,
    pub txd_name: String,
    pub mesh_count: u32,
    pub draw_distance: [f32; 3],
    pub flags: u32,
}
