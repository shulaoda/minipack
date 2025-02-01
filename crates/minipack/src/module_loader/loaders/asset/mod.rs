use minipack_common::AssetView;

pub mod asset_generator;

pub fn create_asset_view(source: impl Into<Box<[u8]>>) -> AssetView {
  AssetView { source: source.into() }
}
