use std::{ops::Index, path::Path};

use crate::{utils::get_path, IMG};
use async_fs::File;
use bevy::{
    asset::{
        io::{AssetReader, AssetReaderError, PathStream, Reader, VecReader},
        AssetLoader, LoadContext,
    },
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
    },
};
use nom_derive::Parse;
use num_traits::FromPrimitive;
use rw_rs::bsf::{
    tex::{RasterFormat, RpRasterPalette},
    Chunk, ChunkContent,
};
use thiserror::Error;

pub struct GTAAssetReader;

// This exposes the files in gta3.img as files in a VFS
impl AssetReader for GTAAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        if path.components().count() == 1
            && path
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("dff") || x.eq_ignore_ascii_case("txd"))
        {
            let path_ext = path.extension();
            if let Some(path_ext) = path_ext {
                {
                    if let Some(file) = IMG
                        .lock()
                        .unwrap()
                        .get_file(path.to_string_lossy().as_ref())
                    {
                        return Ok(Box::new(VecReader::new(file)) as Box<dyn Reader>);
                    }
                }
                if path_ext.eq_ignore_ascii_case("dff") {
                    let Some(path) = get_path(&Path::new("models").join(path)) else {
                        return Err(AssetReaderError::NotFound(path.to_path_buf()));
                    };
                    return Ok(Box::new(File::open(&path).await?) as Box<dyn Reader>);
                } else if path_ext.eq_ignore_ascii_case("txd") {
                    let Some(path) = get_path(&Path::new("txd").join(path))
                        .or_else(|| get_path(&Path::new("models").join(path)))
                    else {
                        return Err(AssetReaderError::NotFound(path.to_path_buf()));
                    };
                    return Ok(Box::new(File::open(&path).await?) as Box<dyn Reader>);
                } else {
                    let Some(path) = get_path(path) else {
                        return Err(AssetReaderError::NotFound(path.to_path_buf()));
                    };
                    return Ok(Box::new(File::open(&path).await?) as Box<dyn Reader>);
                }
            }
        }
        if let Some(path) = get_path(path) {
            return Ok(Box::new(File::open(&path).await?) as Box<dyn Reader>);
        }
        Err(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_meta<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> Result<impl Reader + 'a, AssetReaderError> {
        Err::<Box<dyn Reader>, AssetReaderError>(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_directory<'a>(
        &'a self,
        _path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        todo!("read_directory")
    }

    async fn is_directory<'a>(&'a self, _path: &'a Path) -> Result<bool, AssetReaderError> {
        todo!("is_directory")
    }
}

#[derive(Default)]
pub struct TxdLoader;

impl AssetLoader for TxdLoader {
    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        let _ = reader.read_to_end(&mut bytes).await;
        let Ok((_, bsf)) = Chunk::parse(&bytes) else {
            return Err(TxdError::InvalidTxd);
        };
        if !matches!(bsf.content, ChunkContent::TextureDictionary) {
            return Err(TxdError::InvalidTxd);
        }

        let mut texture_vec = Vec::new();

        for raster in &bsf.get_children()[1..] {
            if let ChunkContent::Raster(raster) = &raster.content {
                let mut data: Vec<u8> = Vec::new();

                let raster_format = RasterFormat::from_u32(
                    raster.raster_format
                        & !(RasterFormat::FormatExtAutoMipmap as u32)
                        & !(RasterFormat::FormatExtPal4 as u32)
                        & !(RasterFormat::FormatExtPal8 as u32)
                        & !(RasterFormat::FormatExtMipmap as u32),
                )
                .unwrap();

                if (raster.raster_format & (RasterFormat::FormatExtPal8 as u32)) != 0 {
                    let (indices, palette) = RpRasterPalette::<256>::parse(&raster.data).unwrap();
                    let indices = &indices[4..];
                    for h in 0..(raster.height as usize) {
                        for w in 0..(raster.width as usize) {
                            let index = indices[w + (h * (raster.width as usize))];
                            let color = palette.0[index as usize];
                            data.push(color.r);
                            data.push(color.g);
                            data.push(color.b);
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
                            data.push(color.r);
                            data.push(color.g);
                            data.push(color.b);
                            data.push(color.a);
                        }
                    }
                } else if matches!(raster_format, RasterFormat::Format1555) {
                    // TODO: Support DXT
                    for p in raster.data[4..].chunks_exact(2) {
                        let p = u16::from_le_bytes([p[0], p[1]]);
                        let mut a = (p >> 15) as u8;
                        let r = ((p >> 10) & 0b11111) as u8;
                        let g = ((p >> 5) & 0b11111) as u8;
                        let b = (p & 0b11111) as u8;

                        if a != 0 {
                            a = 255
                        }

                        data.push(r);
                        data.push(g);
                        data.push(b);
                        data.push(a);
                    }
                } else {
                    data = raster.data[4..].to_vec();
                }

                let format = match raster_format {
                    RasterFormat::Format8888 => TextureFormat::Rgba8UnormSrgb,
                    RasterFormat::Format888 => TextureFormat::Rgba8UnormSrgb,
                    RasterFormat::Format1555 => TextureFormat::Rgba8UnormSrgb,
                    _ => unimplemented!(),
                };
                let image = Image::new(
                    Extent3d {
                        width: raster.width.into(),
                        height: raster.height.into(),
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    data,
                    format,
                    RenderAssetUsages::default(),
                );
                texture_vec.push(
                    load_context.labeled_asset_scope(raster.name.to_ascii_lowercase(), |_lc| image),
                );
            } else if !matches!(raster.content, ChunkContent::Extension) {
                error!("Unexpected type {:?} found in TXD file", raster.content);
                continue;
            }
        }
        Ok(Txd(texture_vec))
    }

    fn extensions(&self) -> &[&str] {
        &["txd"]
    }

    type Asset = Txd;

    type Settings = ();

    type Error = TxdError;
}

#[derive(Asset, Reflect, Clone, Debug, Default)]
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
