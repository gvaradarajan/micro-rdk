//! Contains the DataStore trait and a usable StaticMemoryDataStore.
//! Implementers of the trait are meant to be written to by DataCollectors (RSDK-6992, RSDK-6994)
//! and read from by a task that uploads the data to app (RSDK-6995)

use crate::proto::app::data_sync::v1::DataCaptureUploadRequest;
use bytes::{BufMut, BytesMut};
use prost::{EncodeError, Message};
use ringbuf::{ring_buffer::RbBase, LocalRb, Rb};
use std::{
    io::Cursor,
    mem::MaybeUninit,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use thiserror::Error;

static mut DATA_STORE: [MaybeUninit<u8>; 1024000] = [MaybeUninit::uninit(); 1024000];

#[derive(Error, Debug)]
pub enum DataStoreError {
    #[error(transparent)]
    EncodingError(#[from] EncodeError),
    #[error("DataCaptureUploadRequestTooLarge")]
    DataTooLarge,
    #[error("Store already initialized")]
    DataStoreInitialized,
    #[error("Data write failure")]
    DataWriteFailure,
    #[error("Current message is malformed")]
    DataIntegrityError,
    #[error("unimplemented")]
    Unimplemented,
}

lazy_static::lazy_static! {
    static ref DATA_STORE_INITIALIZED: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
}

pub trait DataStore {
    /// Attempts to store all of requests in the input vector. Any requests unable to be written
    /// due to exceeding capacity are returned in the result.
    fn store_upload_requests(
        &mut self,
        requests: Vec<DataCaptureUploadRequest>,
    ) -> Result<Vec<DataCaptureUploadRequest>, DataStoreError>;
    /// Attempts to read a number of byte-encoded DataCaptureUploadRequests. May return less than
    /// the requested number of messages if there are less messages remaining than requested
    fn read_messages(&mut self, number_of_messages: usize)
        -> Result<Vec<BytesMut>, DataStoreError>;
    /// WARNING: implementations of clear are meant to reset the entire data store. Must
    /// only be called when it is guaranteed that no other process has access to the data store.
    fn clear(&mut self);
}

/// StaticMemoryDataStore is an entity that governs the static bytes memory
/// reserved in DATA_STORE and treats it like a ring buffer of DataCaptureUploadRequests.
/// It should be treated as a global struct that should only be initialized once and is not
/// thread-safe (all interactions should be blocking).
pub struct StaticMemoryDataStore {
    buffer: LocalRb<u8, &'static mut [MaybeUninit<u8>]>,
}

impl StaticMemoryDataStore {
    pub fn new() -> Result<Self, DataStoreError> {
        unsafe {
            if !DATA_STORE_INITIALIZED.fetch_or(true, Ordering::SeqCst) {
                return Ok(Self {
                    buffer: LocalRb::from_raw_parts(&mut DATA_STORE, 0, 0),
                });
            }
        }
        Err(DataStoreError::DataStoreInitialized)
    }
}

impl DataStore for StaticMemoryDataStore {
    fn store_upload_requests(
        &mut self,
        requests: Vec<DataCaptureUploadRequest>,
    ) -> Result<Vec<DataCaptureUploadRequest>, DataStoreError> {
        let mut res = Vec::new();
        let mut return_remaining = false;
        for req in requests {
            if return_remaining {
                res.push(req);
                continue;
            }
            let encode_len = req.encoded_len();
            if encode_len > unsafe { DATA_STORE.len() / 2 } {
                return Err(DataStoreError::DataTooLarge);
            }
            if encode_len + 5 > self.buffer.vacant_len() {
                return_remaining = true;
                res.push(req);
                continue;
            }
            self.buffer
                .push(0)
                .map_err(|_| DataStoreError::DataWriteFailure)?;
            let len_bytes = (encode_len as u32).to_be_bytes();
            self.buffer.push_slice(&len_bytes);

            let mut buf = BytesMut::with_capacity(req.encoded_len());
            req.encode(&mut buf)?;
            let mut buf_iter = buf.into_iter();
            self.buffer.push_iter(&mut buf_iter);
        }
        Ok(res)
    }
    fn read_messages(
        &mut self,
        number_of_messages: usize,
    ) -> Result<Vec<BytesMut>, DataStoreError> {
        let mut res = Vec::new();
        for _ in 0..number_of_messages {
            if let Some(&&zero_byte) = self.buffer.iter().peekable().peek() {
                if zero_byte != 0 {
                    return Err(DataStoreError::DataIntegrityError);
                }
                let _ = self.buffer.pop();
            } else {
                break;
            }
            let mut encoded_len: [u8; 4] = [0; 4];
            self.buffer.pop_slice(&mut encoded_len);
            let encoded_len = u32::from_be_bytes(encoded_len) as usize;
            let mut msg_vec: Vec<u8> = vec![0; encoded_len];
            self.buffer.pop_slice(msg_vec.as_mut_slice());
            let mut msg_bytes = BytesMut::with_capacity(encoded_len);
            msg_bytes.put(Cursor::new(msg_vec));
            res.push(msg_bytes);
        }
        Ok(res)
    }
    fn clear(&mut self) {
        self.buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::common::data_store::DataStore;
    use crate::google::protobuf::{value::Kind, Struct, Value};
    use crate::proto::app::data_sync::v1::{sensor_data::Data, DataType, UploadMetadata};
    use crate::proto::app::data_sync::v1::{DataCaptureUploadRequest, SensorData};
    use prost::Message;

    #[test_log::test]
    fn test_data_store() {
        let store = super::StaticMemoryDataStore::new();
        assert!(store.is_ok());
        let mut store = store.unwrap();

        let mut requests = vec![];
        let msg_1 = DataCaptureUploadRequest {
            metadata: None,
            sensor_contents: vec![],
        };
        requests.push(msg_1);
        let msg_2 = DataCaptureUploadRequest {
            metadata: Some(UploadMetadata {
                part_id: "part_id".to_string(),
                component_type: "component_a".to_string(),
                component_name: "test_comp".to_string(),
                method_name: "do_it".to_string(),
                r#type: DataType::TabularSensor.into(),
                ..Default::default()
            }),
            sensor_contents: vec![],
        };
        requests.push(msg_2);
        let msg_3 = DataCaptureUploadRequest {
            metadata: None,
            sensor_contents: vec![SensorData {
                metadata: None,
                data: Some(Data::Struct(Struct {
                    fields: HashMap::from([
                        (
                            "thing_1".to_string(),
                            Value {
                                kind: Some(Kind::NumberValue(245.01)),
                            },
                        ),
                        (
                            "thing_2".to_string(),
                            Value {
                                kind: Some(Kind::BoolValue(true)),
                            },
                        ),
                    ]),
                })),
            }],
        };
        requests.push(msg_3);

        let res = store.store_upload_requests(requests);
        assert!(res.is_ok());
        assert_eq!(res.unwrap().len(), 0);

        let read_messages = store.read_messages(3);
        assert!(read_messages.is_ok());
        let mut read_messages = read_messages.unwrap();

        let msg = DataCaptureUploadRequest::decode(&mut read_messages[0]);
        assert!(msg.is_ok());
        let msg = msg.unwrap();

        assert_eq!(
            msg,
            DataCaptureUploadRequest {
                metadata: None,
                sensor_contents: vec![],
            }
        );

        let msg = DataCaptureUploadRequest::decode(&mut read_messages[1]);
        assert!(msg.is_ok());
        let msg = msg.unwrap();

        assert_eq!(
            msg,
            DataCaptureUploadRequest {
                metadata: Some(UploadMetadata {
                    part_id: "part_id".to_string(),
                    component_type: "component_a".to_string(),
                    component_name: "test_comp".to_string(),
                    method_name: "do_it".to_string(),
                    r#type: DataType::TabularSensor.into(),
                    ..Default::default()
                }),
                sensor_contents: vec![],
            }
        );

        let msg = DataCaptureUploadRequest::decode(&mut read_messages[2]);
        assert!(msg.is_ok());
        let msg = msg.unwrap();

        assert_eq!(
            msg,
            DataCaptureUploadRequest {
                metadata: None,
                sensor_contents: vec![SensorData {
                    metadata: None,
                    data: Some(Data::Struct(Struct {
                        fields: HashMap::from([
                            (
                                "thing_1".to_string(),
                                Value {
                                    kind: Some(Kind::NumberValue(245.01)),
                                },
                            ),
                            (
                                "thing_2".to_string(),
                                Value {
                                    kind: Some(Kind::BoolValue(true)),
                                },
                            ),
                        ]),
                    })),
                }],
            }
        );

        store.clear();

        // test ring buffer wrap

        let num_of_initial_messages: usize = 22262;
        let mut requests = vec![];
        for _ in 0..num_of_initial_messages {
            requests.push(DataCaptureUploadRequest {
                metadata: None,
                sensor_contents: vec![SensorData {
                    metadata: None,
                    data: Some(Data::Struct(Struct {
                        fields: HashMap::from([
                            (
                                "thing_1".to_string(),
                                Value {
                                    kind: Some(Kind::NumberValue(245.01)),
                                },
                            ),
                            (
                                "thing_2".to_string(),
                                Value {
                                    kind: Some(Kind::BoolValue(true)),
                                },
                            ),
                        ]),
                    })),
                }],
            });
        }
        let res = store.store_upload_requests(requests);
        assert!(res.is_ok());
        let res = res.unwrap();
        assert_eq!(res.len(), 2);

        let read_messages = store.read_messages(2);
        assert!(read_messages.is_ok());
        let read_messages = read_messages.unwrap();
        assert_eq!(read_messages.len(), 2);

        let res = store.store_upload_requests(res);
        assert!(res.is_ok());
        assert_eq!(res.unwrap().len(), 0);
    }
}
