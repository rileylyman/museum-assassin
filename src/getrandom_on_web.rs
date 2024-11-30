use sapp_jsutils::JsObject;

extern "C" {
    fn macroquad_js_get_random_buffer(length: usize) -> JsObject;
}

/// Required by `getrandom` crate.
fn getrandom(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    let obj = unsafe { macroquad_js_get_random_buffer(buf.len()) };
    let mut bytes = Vec::with_capacity(buf.len());
    obj.to_byte_buffer(&mut bytes);

    for (target, data) in buf.iter_mut().zip(bytes) {
        *target = data;
    }
    Ok(())
}
getrandom::register_custom_getrandom!(getrandom);