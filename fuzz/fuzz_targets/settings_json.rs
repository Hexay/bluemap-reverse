#![no_main]

use std::path::{Component, Path};

use bmr_fetch::settings::{MapSettings, SiteSettings};
use libfuzzer_sys::fuzz_target;

/// Mirror paths are `<out>/<map_data_root>/<id>/...`: neither may leave `<out>`.
fn stays_inside(rel: &str) -> bool {
    !rel.contains('\\') && Path::new(rel).components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

fuzz_target!(|data: &[u8]| {
    if let Ok(site) = SiteSettings::parse(data) {
        assert!(stays_inside(&site.map_data_root), "{:?}", site.map_data_root);
        for id in &site.maps {
            assert!(stays_inside(id), "{id:?}");
        }
    }
    if let Ok(map) = MapSettings::parse(data) {
        let (x, z) = map.start_block();
        let hires = map.hires_grid();
        hires.tile_min(hires.tile_of(x, z));
        for lod in 1..=map.lowres.lod_count {
            let grid = map.lowres_grid(lod);
            let scale = map.lod_scale(lod);
            grid.tiles_in(x, z, scale, scale).count();
        }
    }
});
