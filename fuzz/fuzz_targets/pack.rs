#![no_main]

use std::io::Write;
use std::sync::LazyLock;

use bmr_pack::{FORMAT, Header, MAGIC, Meta, Pack};
use libfuzzer_sys::fuzz_target;
use lzma_rust2::{XzOptions, XzWriter};

fn xz(raw: &[u8]) -> Vec<u8> {
    let mut w = XzWriter::new(Vec::new(), XzOptions::with_preset(0)).unwrap();
    w.write_all(raw).unwrap();
    w.finish().unwrap()
}

static HEADER: LazyLock<Vec<u8>> = LazyLock::new(|| {
    let meta = Meta {
        mc_version: "1.21.4".into(),
        data_version: 4189,
        bluemap_version: "5.27".into(),
        compact_palette: false,
        created_unix: 0,
    };
    let header = Header { format: FORMAT, meta, site_textures: vec!["minecraft:block/stone".into()] };
    xz(&postcard::to_stdvec(&header).unwrap())
});

// a whole pack file, or else a raw postcard body: xz's checksums would hide the body decoder otherwise
fuzz_target!(|data: &[u8]| {
    let file = if data.starts_with(MAGIC) {
        data.to_vec()
    } else {
        let mut f = MAGIC.to_vec();
        f.extend((HEADER.len() as u32).to_le_bytes());
        f.extend(HEADER.iter());
        f.extend(xz(data));
        f
    };
    if Pack::header_from_bytes(&file).is_ok()
        && let Ok(pack) = Pack::from_bytes(&file)
    {
        bmr_pack::check(&pack, Some("5.27"), &pack.site_textures).explain();
    }
});
