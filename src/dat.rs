use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::anyhow;
use bevy::prelude::*;
use rw_rs::{bsf::parse_bsf_chunk, img::Img};

use crate::load_meshes;

#[derive(Resource)]
pub struct GameData {
    pub data_dir: PathBuf,
    pub img: Img<'static>,
    pub ide: Ide,
}

impl GameData {
    pub fn load_dat(
        &mut self,
        commands: &mut Commands,
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &mut ResMut<Assets<StandardMaterial>>,
    ) -> anyhow::Result<()> {
        let dat = std::fs::read_to_string(self.data_dir.join("data/gta3.dat"))?;
        let lines = dat.split('\n').map(|e| e.trim()).collect::<Vec<_>>();
        for line in lines {
            let line = line;
            let words = line
                .split_whitespace()
                .take_while(|s| !s.contains('#'))
                .collect::<Vec<_>>();
            if words.is_empty() {
                continue;
            }

            match words[0].to_lowercase().as_str() {
                "ide" | "mapzone" | "ipl" => {
                    self.load_ide(words[1], commands, meshes, materials)?
                }
                "splash" => {}
                "colfile" => {}
                _ => todo!(),
            }
        }
        Ok(())
    }

    pub fn load_ide(
        &mut self,
        path: &str,
        commands: &mut Commands,
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &mut ResMut<Assets<StandardMaterial>>,
    ) -> anyhow::Result<()> {
        let path = self.get_path(path).ok_or(anyhow!("{} not found!", path))?;
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

                "tobj" => {}

                "hier" => {}

                "cars" => {}

                "peds" => {}

                "path" => {}

                "2dfx" => {}

                "weap" => {}

                "anim" => {}

                "txdp" => {}

                // MAPZONE
                "zone" => {}

                // IPL
                "inst" => {
                    debug!("loading {}", words[1]);
                    let file = self
                        .img
                        .get_file(&format!("{}.dff", words[1]))
                        .unwrap_or_else(|| panic!("DFF {} not found in img", words[1]));
                    let (_, bsf) = parse_bsf_chunk(&file).unwrap();
                    let meshes_vec = load_meshes(&bsf)
                        .into_iter()
                        .map(|m| meshes.add(m))
                        .collect::<Vec<_>>();

                    if meshes_vec.is_empty() {
                        warn!("{} contained zero meshes", words[1]);
                        continue;
                    }

                    commands.spawn((PbrBundle {
                        mesh: meshes_vec[0].clone(),
                        material: materials.add(StandardMaterial { ..default() }),
                        transform: Transform {
                            translation: [
                                -(words[2].parse::<f32>().unwrap()),
                                words[4].parse().unwrap(),
                                words[3].parse::<f32>().unwrap(),
                            ]
                            .into(),
                            scale: [
                                words[5].parse().unwrap(),
                                words[6].parse().unwrap(),
                                words[7].parse().unwrap(),
                            ]
                            .into(),
                            rotation: Quat::from_array([
                                words[8].parse::<f32>().unwrap(),
                                -words[10].parse::<f32>().unwrap(),
                                -words[9].parse::<f32>().unwrap(),
                                words[11].parse::<f32>().unwrap(),
                            ])
                            .normalize(),
                        },
                        ..default()
                    },));
                }

                "zone" => {}

                "cull" => {}

                "pick" => {}

                "path" => {}

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

    // Case-insensitive path search from data_dir
    pub fn get_path(&self, path: &str) -> Option<PathBuf> {
        let mut matched = self.data_dir.to_owned();
        let path = path.replace('\\', "/");
        for elem in path.split('/') {
            let Ok(iter) = fs::read_dir(&matched) else {
                return None;
            };
            let mut found = String::new();
            for file in iter {
                let Ok(file) = file else { continue };
                let file_name = file.file_name();
                let file_name = file_name.to_string_lossy();
                if file_name.to_ascii_lowercase() == elem.to_ascii_lowercase() {
                    found = file_name.to_string();
                    break;
                }
            }
            if found.is_empty() {
                return None;
            }
            matched = matched.join(found);
        }
        Some(matched)
    }
}

#[derive(Default)]
pub struct Ide {
    objs: HashMap<u32, IdeObj>,
}

pub struct IdeObj {
    pub id: u32,
    pub model_name: String,
    pub txd_name: String,
    pub mesh_count: u32,
    pub draw_distance: [f32; 3],
    pub flags: u32,
}
