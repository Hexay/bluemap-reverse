//! The packs shipped in `packs/` must load with this build and match `packs/index.json`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use bmr_pack::{FORMAT, Index, IndexEntry, Pack};

fn packs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packs")
}

fn committed_packs() -> Vec<PathBuf> {
    let mut packs: Vec<PathBuf> = fs::read_dir(packs_dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "pack"))
        .collect();
    packs.sort();
    assert!(!packs.is_empty(), "no packs committed");
    packs
}

fn committed_index() -> Index {
    Index::parse(&fs::read(packs_dir().join("index.json")).unwrap()).unwrap()
}

fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

#[test]
fn every_pack_loads_with_the_current_format() {
    for path in committed_packs() {
        let header = Pack::read_header(&path).unwrap();
        assert_eq!(header.format, FORMAT, "{}: rebuild it", path.display());
        Pack::load(&path).unwrap_or_else(|e| panic!("{}: {e:#}", path.display()));
    }
}

#[test]
fn index_describes_exactly_the_committed_files() {
    let index = committed_index();
    let mut entries: BTreeMap<&str, &IndexEntry> = index.packs.iter().map(|e| (e.file.as_str(), e)).collect();
    for path in committed_packs() {
        let file = path.file_name().unwrap().to_str().unwrap();
        let entry = entries.remove(file).unwrap_or_else(|| panic!("{file} is not in index.json"));
        entry.verify(&fs::read(&path).unwrap()).unwrap();
        let header = Pack::read_header(&path).unwrap();
        assert_eq!(entry.pack_format, FORMAT, "{file}");
        assert_eq!(entry.mc_version, header.meta.mc_version, "{file}");
        assert_eq!(entry.bluemap_version, header.meta.bluemap_version, "{file}");
        assert_eq!(sorted(index.textures_of(entry).unwrap()), sorted(header.site_textures), "{file}: fingerprint");
    }
    assert!(entries.is_empty(), "index.json lists missing files: {:?}", entries.keys());
}
