use std::{
    fs,
    ops::Index,
    path::{Path, PathBuf},
};

use crate::{utils::get_path, IMG};
use anyhow::Result;
use bevy::{
    asset::{AssetIo, AssetIoError, AssetLoader, LoadedAsset},
    prelude::*,
    reflect::TypeUuid,
    render::{
        render_resource::{Extent3d, TextureDimension, TextureFormat},
        texture::TextureFormatPixelInfo,
    },
};
use nom_derive::Parse;
use num_traits::FromPrimitive;
use rw_rs::bsf::{
    tex::{RasterFormat, RpRasterPalette},
    Chunk, ChunkContent,
};
use thiserror::Error;

pub struct GTAAssetIo;

impl AssetIo for GTAAssetIo {
    fn load_path<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> bevy::utils::BoxedFuture<'a, Result<Vec<u8>, bevy::asset::AssetIoError>> {
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
                        let Some(path) = get_path(&Path::new("models").join(path)) else {
                            return Err(AssetIoError::NotFound(path.to_path_buf()));
                        };
                        fs::read(path).map_err(AssetIoError::Io)
                    });
                } else if path_ext.eq_ignore_ascii_case("txd") {
                    return Box::pin(async move {
                        let Some(path) = get_path(&Path::new("txd").join(path))
                            .or_else(|| get_path(&Path::new("models").join(path)))
                        else {
                            return Err(AssetIoError::NotFound(path.to_path_buf()));
                        };
                        fs::read(path).map_err(AssetIoError::Io)
                    });
                } else {
                    return Box::pin(async move {
                        let Some(path) = get_path(path) else {
                            return Err(AssetIoError::NotFound(path.to_path_buf()));
                        };
                        fs::read(path).map_err(AssetIoError::Io)
                    });
                }
            }
        }
        if let Some(path) = get_path(path) {
            return Box::pin(async move { fs::read(path).map_err(AssetIoError::Io) });
        }
        Box::pin(async move { Err(AssetIoError::NotFound(path.to_path_buf())) })
    }

    fn read_directory(
        &self,
        _path: &std::path::Path,
    ) -> Result<Box<dyn Iterator<Item = PathBuf>>, bevy::asset::AssetIoError> {
        todo!()
    }

    fn get_metadata(
        &self,
        _path: &std::path::Path,
    ) -> Result<bevy::asset::Metadata, bevy::asset::AssetIoError> {
        todo!()
    }

    fn watch_path_for_changes(
        &self,
        _to_watch: &std::path::Path,
        _to_reload: Option<PathBuf>,
    ) -> Result<(), bevy::asset::AssetIoError> {
        warn!("watch_path_for_changes not yet implemented");
        Ok(())
    }

    fn watch_for_changes(
        &self,
        _configuration: &bevy::asset::ChangeWatcher,
    ) -> Result<(), bevy::asset::AssetIoError> {
        unimplemented!()
    }
}

pub struct CustomAssetIoPlugin;

impl Plugin for CustomAssetIoPlugin {
    fn build(&self, app: &mut App) {
        // create the custom asset io instance
        let asset_io = GTAAssetIo;

        // the asset server is constructed and added the resource manager
        app.insert_resource(AssetServer::new(asset_io));
        app.add_asset::<Txd>().init_asset_loader::<TxdLoader>();
    }
}

#[derive(Default)]
pub struct TxdLoader;

impl AssetLoader for TxdLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut bevy::asset::LoadContext,
    ) -> bevy::utils::BoxedFuture<'a, Result<()>> {
        Box::pin(async move { Ok(load_textures(bytes, load_context).await?) })
    }

    fn extensions(&self) -> &[&str] {
        &["txd"]
    }
}

#[derive(Reflect, Clone, TypeUuid, Debug, Default)]
#[uuid = "e23c54a5-4a4a-490c-97d8-5f31fdd79a1a"]
pub struct Txd(pub Vec<Handle<Image>>);

impl Index<usize> for Txd {
    type Output = Handle<Image>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

#[derive(Error, Debug)]
pub enum TxdError {
    #[error("invalid TXD file")]
    InvalidTxd,
}

async fn load_textures<'a, 'b>(
    bytes: &'a [u8],
    load_context: &'a mut bevy::asset::LoadContext<'b>,
) -> Result<(), TxdError> {
    let Ok((_, bsf)) = Chunk::parse(bytes) else {
        return Err(TxdError::InvalidTxd);
    };
    if !matches!(bsf.content, ChunkContent::TextureDictionary) {
        return Err(TxdError::InvalidTxd);
    }

    let mut texture_vec = Vec::new();

    for raster in &bsf.get_children()[1..] {
        if let ChunkContent::Raster(raster) = &raster.content {
            let mut data: Vec<u8> = Vec::new();
            if (raster.raster_format & (RasterFormat::FormatExtPal8 as u32)) != 0 {
                let (indices, palette) = RpRasterPalette::<256>::parse(&raster.data).unwrap();
                let indices = &indices[5..];
                for h in 0..(raster.height as usize) {
                    for w in 0..(raster.width as usize) {
                        let index = indices[w + (h * (raster.width as usize))];
                        let color = palette.0[index as usize];
                        data.push(color.b);
                        data.push(color.g);
                        data.push(color.r);
                        data.push(color.a);
                    }
                }
            } else if (raster.raster_format & (RasterFormat::FormatExtPal4 as u32)) != 0 {
                let (indices, palette) = RpRasterPalette::<32>::parse(&raster.data).unwrap();
                let indices = &indices[5..];
                for h in 0..(raster.height as usize) {
                    for w in 0..(raster.width as usize) {
                        let index = indices[w + (h * (raster.width as usize))];
                        let color = palette.0[index as usize];
                        data.push(color.b);
                        data.push(color.g);
                        data.push(color.r);
                        data.push(color.a);
                    }
                }
            } else {
                data = raster.data[4..].to_vec();
            }

            let raster_format = raster.raster_format
                & !(RasterFormat::FormatExtAutoMipmap as u32)
                & !(RasterFormat::FormatExtPal4 as u32)
                & !(RasterFormat::FormatExtPal8 as u32)
                & !(RasterFormat::FormatExtMipmap as u32);
            let raster_format = RasterFormat::from_u32(raster_format).unwrap();
            let format = match raster_format {
                RasterFormat::Format8888 => TextureFormat::Bgra8Unorm,
                RasterFormat::Format888 => TextureFormat::Bgra8Unorm,
                _ => unimplemented!(),
            };
            let image = Image::new(
                Extent3d {
                    width: raster.width.into(),
                    height: raster.height.into(),
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                data[0..raster.width as usize * raster.height as usize * format.pixel_size()]
                    .to_vec(),
                format,
            );

            let asset = LoadedAsset::new(image);
            texture_vec.push(load_context.set_labeled_asset(&raster.name, asset));
        } else if matches!(raster.content, ChunkContent::Extension) {
            error!("Unexpected type {:?} found in TXD file", raster.content);
            continue;
        }
    }

    load_context.set_default_asset(LoadedAsset::new(Txd(texture_vec)));
    Ok(())
}
