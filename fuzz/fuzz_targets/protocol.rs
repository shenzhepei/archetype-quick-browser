#![no_main]

use archetype_protocol::{Codec, MAX_FRAME_BODY_BYTES};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let codec = Codec::default();
    let _ = codec.decode(data);

    if data.len() <= MAX_FRAME_BODY_BYTES {
        let mut framed = u32::try_from(data.len()).unwrap().to_be_bytes().to_vec();
        framed.extend_from_slice(data);
        let _ = codec.decode(framed.as_slice());
    }
});
