#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let names = bmr_prbm::parse_texture_names(data);
    let Ok(textures) = bmr_prbm::parse_textures(data) else { return };
    assert_eq!(names.map(|n| n.len()).ok(), Some(textures.len()));
    for t in &textures {
        let _ = t.frame_png();
        let stem = t.file_stem();
        assert!(stem.chars().all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c)), "{stem:?}");
    }
});
