use std::{ops::Index, path::Path};

use crate::{utils::get_path, IMG};
use anyhow::Result;
use async_fs::File;
use bevy::{
    asset::{
        io::{AssetReader, AssetReaderError, PathStream, Reader, VecReader},
        AssetLoader, AsyncReadExt, LoadContext,
    },
    prelude::*,
    reflect::TypeUuid,
    render::{
        render_resource::{Extent3d, TextureDimension, TextureFormat},
        texture::TextureFormatPixelInfo,
    },
    utils::BoxedFuture,
};
use nom_derive::Parse;
use num_traits::FromPrimitive;
use rw_rs::bsf::{
    tex::{RasterFormat, RpRasterPalette},
    Chunk, ChunkContent,
};
use thiserror::Error;

pub struct GTAAssetReader;

impl AssetReader for GTAAssetReader {
    fn read<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> BoxedFuture<'a, Result<Box<Reader<'a>>, AssetReaderError>> {
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
                    return Box::pin(async move {
                        let reader: Box<Reader> = Box::new(VecReader::new(file));
                        Ok(reader)
                    });
                } else if path_ext.eq_ignore_ascii_case("dff") {
                    return Box::pin(async move {
                        let Some(path) = get_path(&Path::new("models").join(path)) else {
                            return Err(AssetReaderError::NotFound(path.to_path_buf()));
                        };
                        let reader: Box<Reader> = Box::new(File::open(&path).await?);
                        Ok(reader)
                    });
                } else if path_ext.eq_ignore_ascii_case("txd") {
                    return Box::pin(async move {
                        let Some(path) = get_path(&Path::new("txd").join(path))
                            .or_else(|| get_path(&Path::new("models").join(path)))
                        else {
                            return Err(AssetReaderError::NotFound(path.to_path_buf()));
                        };
                        let reader: Box<Reader> = Box::new(File::open(&path).await?);
                        Ok(reader)
                    });
                } else {
                    return Box::pin(async move {
                        let Some(path) = get_path(path) else {
                            return Err(AssetReaderError::NotFound(path.to_path_buf()));
                        };
                        let reader: Box<Reader> = Box::new(File::open(&path).await?);
                        Ok(reader)
                    });
                }
            }
        }
        if let Some(path) = get_path(path) {
            return Box::pin(async move {
                let reader: Box<Reader> = Box::new(File::open(&path).await?);
                Ok(reader)
            });
        }
        Box::pin(async move { Err(AssetReaderError::NotFound(path.to_path_buf())) })
    }

    fn read_meta<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> BoxedFuture<'a, Result<Box<Reader<'a>>, AssetReaderError>> {
        Box::pin(async move { Err(AssetReaderError::NotFound(path.to_path_buf())) })
    }

    fn read_directory<'a>(
        &'a self,
        _path: &'a std::path::Path,
    ) -> BoxedFuture<'a, Result<Box<PathStream>, AssetReaderError>> {
        todo!("read_directory")
    }

    fn is_directory<'a>(
        &'a self,
        _path: &'a Path,
    ) -> BoxedFuture<'a, std::prelude::v1::Result<bool, AssetReaderError>> {
        todo!("is_directory")
    }
}

#[derive(Default)]
pub struct TxdLoader;

impl AssetLoader for TxdLoader {
    fn load<'a>(
        &'a self,
        reader: &'a mut Reader,
        _settings: &'a Self::Settings,
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let mut bytes = Vec::new();
            let _ = reader.read_to_end(&mut bytes).await;
            load_textures(&bytes, load_context).await
        })
    }

    fn extensions(&self) -> &[&str] {
        &["txd"]
    }

    type Asset = Txd;

    type Settings = ();

    type Error = TxdError;
}

#[derive(Asset, Reflect, Clone, TypeUuid, Debug, Default)]
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
) -> Result<Txd, TxdError> {
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
                let indices = &indices[4..];
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
                let indices = &indices[4..];
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
                RasterFormat::Format8888 => TextureFormat::Bgra8UnormSrgb,
                RasterFormat::Format888 => TextureFormat::Bgra8UnormSrgb,
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
            texture_vec.push(load_context.labeled_asset_scope(raster.name.clone(), |_lc| image));
        } else if !matches!(raster.content, ChunkContent::Extension) {
            error!("Unexpected type {:?} found in TXD file", raster.content);
            continue;
        }
    }
    Ok(Txd(texture_vec))
}
