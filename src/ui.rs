use std::collections::HashMap;

use bevy::prelude::*;

use crate::assets::GTAFont;

#[derive(Component, Reflect, Debug)]
#[require(Node)]
pub struct GTAText {
    pub text: String,
    pub font: Handle<GTAFont>,
}

#[derive(Resource, Reflect, Debug)]
pub struct GTAFonts(HashMap<&'static str, Handle<GTAFont>>);

impl Default for GTAFonts {
    fn default() -> Self {
        GTAFonts(HashMap::new())
    }
}

pub fn gtatext_changed(
    q: Query<(Entity, &GTAText), Changed<GTAText>>,
    fonts: Res<Assets<GTAFont>>,
    mut commands: Commands,
) {
    for (e, comp) in q.iter() {
        let mut ent = commands.entity(e);
        let Some(font) = fonts.get(&comp.font) else {
            error!("GTAText {e} not updated due to missing font!");
            return;
        };

        ent.despawn_related::<Children>();
        for c in comp.text.chars() {
            let index = *font.index_table.get(&c).unwrap_or(&0) as usize;
            ent.with_child((
                ImageNode::from_atlas_image(
                    font.image.clone(),
                    TextureAtlas {
                        layout: font.atlas_layout.clone(),
                        index,
                    },
                ),
                Node {
                    height: Val::Px(32.0),
                    aspect_ratio: Some(1.0),
                    ..Default::default()
                },
            ));
        }
    }
}

pub fn init_fonts(
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut fonts: ResMut<Assets<GTAFont>>,
    mut font_list: ResMut<GTAFonts>,
    mut images: ResMut<Assets<Image>>,
) {
    let img = asset_server.load("fonts.txd#font1");
    let layout = get_font_char_bounds(images.get(&img).unwrap(), UVec2 { x: 32, y: 40 }, 16, 10);
    let layout = texture_atlas_layouts.add(layout);

    let mut map = HashMap::from([(' ', 0), ('!', 1)]);

    for (i, c) in ('0'..='9').enumerate() {
        map.insert(c, 16 + (i as u8));
    }

    for (i, c) in ('a'..='z').enumerate() {
        map.insert(c, 33 + (i as u8));
    }

    let font = fonts.add(GTAFont {
        image: img,
        atlas_layout: layout,
        index_table: map,
    });

    font_list.0.insert("font1", font);
}

fn get_font_char_bounds(
    img: &Image,
    tile_size: UVec2,
    columns: u32,
    rows: u32,
) -> TextureAtlasLayout {
    let mut atlas = TextureAtlasLayout::new_empty(UVec2 {
        x: tile_size.x * columns,
        y: tile_size.y * rows,
    });

    for tile_y in (0..tile_size.y * rows).step_by(tile_size.y as usize) {
        for tile_x in (0..tile_size.x * columns).step_by(tile_size.x as usize) {
            let mut x_min = tile_x + tile_size.x;
            let mut x_max = tile_x;
            let mut y_min = tile_y + tile_size.y;
            let mut y_max = tile_y;

            for y in tile_y..tile_y + tile_size.y {
                for x in tile_x..tile_x + tile_size.x {
                    if img.get_color_at(x, y).unwrap() == Color::BLACK {
                        continue;
                    } else {
                        x_min = x_min.min(x);
                        x_max = x_max.max(x);
                        y_min = y_min.min(y);
                        y_max = y_max.max(y);
                    }
                }
            }

            atlas.add_texture(URect {
                min: UVec2 { x: x_min, y: y_min },
                max: UVec2 { x: x_max, y: y_max },
            });
        }
    }

    atlas
}

pub fn test_font(mut commands: Commands, font_list: Res<GTAFonts>) {
    commands.spawn(GTAText {
        text: "0123456789".into(),
        font: font_list.0.get("font1").unwrap().clone(),
    });
}

pub struct GTAUIPlugin;

impl Plugin for GTAUIPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GTAFonts>()
            .init_asset::<GTAFont>()
            .add_systems(Startup, init_fonts)
            .add_systems(FixedUpdate, gtatext_changed);
    }
}
