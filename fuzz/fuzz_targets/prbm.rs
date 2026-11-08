#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(tile) = bmr_prbm::parse(data) {
        assert_eq!(tile.faces().count(), tile.face_count());
    }
});
