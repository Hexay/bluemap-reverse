#![no_main]

use std::ffi::OsStr;
use std::path::Path;

use bmr_pack::Index;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(index) = Index::parse(data) else { return };
    for entry in &index.packs {
        // downloads are written to <data>/packs/<file>
        assert!(Path::new(&entry.file).file_name() == Some(OsStr::new(&entry.file)), "{:?}", entry.file);
    }
    for entry in index.usable() {
        let _ = index.textures_of(entry);
        let _ = entry.verify(data);
    }
});
