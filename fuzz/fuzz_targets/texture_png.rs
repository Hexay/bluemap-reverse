#![no_main]

use base64::Engine;
use bmr_prbm::Texture;
use libfuzzer_sys::fuzz_target;

// the embedded PNGs of textures.json, without making the fuzzer find valid base64 inside JSON
fuzz_target!(|data: &[u8]| {
    let texture = Texture {
        resource_path: "minecraft:block/fuzz".into(),
        color: [1.0; 4],
        half_transparent: false,
        texture: format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(data)),
        animation: Some(serde_json::Value::Null),
    };
    let _ = texture.frame_png();
});
