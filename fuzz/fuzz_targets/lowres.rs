#![no_main]

use bmr_fetch::lowres::LowresImage;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(img) = LowresImage::decode(data) else { return };
    for (x, z) in img.visible_pixels() {
        img.block_height(x as usize, z as usize);
    }
});
