#![no_main]

use std::sync::LazyLock;

use bmr_world::BlockRegistry;
use libfuzzer_sys::fuzz_target;

static REGISTRY: LazyLock<BlockRegistry> = LazyLock::new(bmr_fuzz::registry);

fuzz_target!(|data: &[u8]| {
    let Ok(chunks) = bmr_world::parse_region_where(data, &|_| true) else { return };
    for (_, nbt) in chunks {
        bmr_fuzz::decode_chunk(&nbt, &REGISTRY);
    }
});
