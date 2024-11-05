use std::{ffi::c_void, mem::ManuallyDrop};

use crate::{
    common::grpc::{GrpcError, RpcAllocation},
    esp32::esp_idf_svc::sys::{
        heap_caps_free, heap_caps_malloc, MALLOC_CAP_8BIT, MALLOC_CAP_SPIRAM,
    },
};
use bytes::{Bytes, BytesMut, BufMut};

#[derive(Clone)]
pub struct Esp32RpcHeapAllocation {
    inner: ManuallyDrop<Vec<u8>>,
    ptr: *mut u8,
}

impl RpcAllocation for Esp32RpcHeapAllocation {
    fn get_allocation(size: usize) -> Result<Self, GrpcError> {
        let ptr = unsafe { heap_caps_malloc(size, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT) } as *mut u8;
        if ptr.is_null() {
            Err(GrpcError::RpcResourceExhausted)
        } else {
            let inner = ManuallyDrop::new(unsafe { Vec::from_raw_parts(ptr, size, size) });
            Ok(Self { inner, ptr })
        }
    }
    fn to_encoded_message<M: prost::Message>(self, m: M) -> Result<Bytes, GrpcError> {
        let mut buffer = BytesMut::from(self.inner.as_slice());
        if 5 + m.encoded_len() > buffer.capacity() {
            return Err(GrpcError::RpcResourceExhausted);
        }
        buffer.put_u8(0);
        buffer.put_u32(m.encoded_len().try_into().unwrap());
        let mut msg = buffer.split_off(5);
        m.encode(&mut msg).map_err(|_| GrpcError::RpcInternal)?;
        buffer.unsplit(msg);
        Ok(buffer.freeze())
    }
}

impl Drop for Esp32RpcHeapAllocation {
    fn drop(&mut self) {
        unsafe { heap_caps_free(self.ptr as *mut c_void) };
    }
}
