use bevy_asset::{io::Reader, Asset, AssetLoader, LoadContext};
use bevy_reflect::TypePath;
use std::{io, sync::Arc};

/// UTF-8 source loaded from a `.verse` file.
#[derive(Asset, TypePath)]
pub struct VerseAsset(pub(crate) Arc<str>);

impl VerseAsset {
    pub fn new(source: impl Into<Arc<str>>) -> Self {
        Self(source.into())
    }

    pub fn source(&self) -> &str {
        &self.0
    }
}

#[derive(Default, TypePath)]
pub(crate) struct VerseLoader;

impl AssetLoader for VerseLoader {
    type Asset = VerseAsset;
    type Settings = ();
    type Error = io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &(),
        _: &mut LoadContext<'_>,
    ) -> Result<VerseAsset, io::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let source = String::from_utf8(bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        Ok(VerseAsset::new(source))
    }

    fn extensions(&self) -> &[&str] {
        &["verse"]
    }
}
