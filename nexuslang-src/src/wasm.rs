use crate::playground::run_playground_json;
use std::slice;

#[no_mangle]
pub extern "C" fn nexus_alloc(len: usize) -> *mut u8 {
    let mut buffer = Vec::with_capacity(len);
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    ptr
}

#[no_mangle]
pub unsafe extern "C" fn nexus_dealloc(ptr: *mut u8, len: usize) {
    if !ptr.is_null() && len > 0 {
        drop(Vec::from_raw_parts(ptr, 0, len));
    }
}

#[no_mangle]
pub unsafe extern "C" fn nexus_playground_run(ptr: *const u8, len: usize) -> *mut u8 {
    let source_bytes = slice::from_raw_parts(ptr, len);
    let source = match std::str::from_utf8(source_bytes) {
        Ok(source) => source,
        Err(error) => {
            return encode_result(format!(
                "{{\"ok\":false,\"stage\":\"input\",\"message\":\"UTF-8 invalido: {}\",\"diagnostic\":{{\"message\":\"UTF-8 invalido\",\"line\":null,\"column\":null}},\"stats\":{{\"tokens\":0,\"decls\":0,\"warnings\":0}},\"tokens\":[],\"ast\":[],\"erp\":{{}},\"warnings\":[],\"output\":[]}}",
                error
            ));
        }
    };

    encode_result(run_playground_json(source))
}

#[no_mangle]
pub unsafe extern "C" fn nexus_free_result(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }

    let len_bytes = slice::from_raw_parts(ptr, 4);
    let payload_len =
        u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]) as usize;
    drop(Vec::from_raw_parts(ptr, payload_len + 4, payload_len + 4));
}

fn encode_result(payload: String) -> *mut u8 {
    let payload = payload.into_bytes();
    let payload_len = payload.len() as u32;
    let mut out = Vec::with_capacity(payload.len() + 4);
    out.extend_from_slice(&payload_len.to_le_bytes());
    out.extend_from_slice(&payload);
    let ptr = out.as_mut_ptr();
    std::mem::forget(out);
    ptr
}
