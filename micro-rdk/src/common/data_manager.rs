use crate::common::data_collector::{DataCollectionError, DataCollector};
use crate::common::data_store::DataStore;
use crate::proto::app::data_sync::v1::{DataCaptureUploadRequest, DataType, UploadMetadata};

pub struct DataManager<StoreType> {
    collectors: Vec<DataCollector>,
    store: StoreType,
    sync_interval_ms: u64,
    part_id: String,
}

impl<StoreType> DataManager<StoreType>
where
    StoreType: DataStore,
{
    pub fn new(
        collectors: Vec<DataCollector>,
        store: StoreType,
        sync_interval_ms: u64,
        part_id: String,
    ) -> Self {
        Self {
            collectors,
            store,
            sync_interval_ms,
            part_id,
        }
    }

    pub fn sync_interval_ms(&self) -> u64 {
        self.sync_interval_ms
    }

    pub(crate) fn collection_intervals(&self) -> Vec<u64> {
        let mut intervals: Vec<u64> = self.collectors.iter().map(|x| x.time_interval()).collect();
        intervals.sort();
        intervals.dedup();
        intervals
    }

    fn readings_for_interval(
        &mut self,
        time_interval_ms: u64,
    ) -> Result<Vec<DataCaptureUploadRequest>, DataCollectionError> {
        self.collectors
            .iter_mut()
            .filter(|coll| coll.time_interval() == time_interval_ms)
            .map(|coll| {
                Ok(DataCaptureUploadRequest {
                    metadata: Some(UploadMetadata {
                        part_id: self.part_id.to_string(),
                        component_type: coll.component_type(),
                        component_name: coll.name(),
                        method_name: coll.method_str(),
                        r#type: DataType::TabularSensor.into(),
                        ..Default::default()
                    }),
                    sensor_contents: vec![coll.call_method()?],
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use anyhow::anyhow;

    use super::DataManager;
    use crate::common::status::Status;
    use crate::common::{
        data_collector::{CollectionMethod, DataCollector},
        data_store::{DataStore, DataStoreError},
        robot::ResourceType,
        sensor::{
            GenericReadingsResult, Readings, Sensor, SensorError, SensorResult, SensorT,
            TypedReadingsResult,
        },
    };

    #[derive(DoCommand)]
    struct TestSensorFailure {}

    impl Sensor for TestSensorFailure {}

    impl Readings for TestSensorFailure {
        fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
            Err(SensorError::SensorMethodUnimplemented(
                "test sensor failure",
            ))
        }
    }

    impl Status for TestSensorFailure {
        fn get_status(&self) -> anyhow::Result<Option<crate::google::protobuf::Struct>> {
            anyhow::bail!("failure")
        }
    }

    #[derive(DoCommand)]
    struct TestSensor {}

    impl Sensor for TestSensor {}

    impl Readings for TestSensor {
        fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
            Ok(self
                .get_readings()?
                .into_iter()
                .map(|v| (v.0, SensorResult::<f64> { value: v.1 }.into()))
                .collect())
        }
    }

    impl SensorT<f64> for TestSensor {
        fn get_readings(&self) -> Result<TypedReadingsResult<f64>, SensorError> {
            let mut x = HashMap::new();
            x.insert("thing".to_string(), 42.42);
            Ok(x)
        }
    }

    impl Status for TestSensor {
        fn get_status(&self) -> anyhow::Result<Option<crate::google::protobuf::Struct>> {
            anyhow::bail!("unimplemented")
        }
    }

    struct NoOpStore {}

    impl DataStore for NoOpStore {
        fn store_upload_requests(
            &mut self,
            requests: Vec<crate::proto::app::data_sync::v1::DataCaptureUploadRequest>,
        ) -> Result<Vec<crate::proto::app::data_sync::v1::DataCaptureUploadRequest>, DataStoreError>
        {
            Err(DataStoreError::Unimplemented)
        }
        fn read_messages(
            &mut self,
            number_of_messages: usize,
        ) -> Result<Vec<bytes::BytesMut>, DataStoreError> {
            Err(DataStoreError::Unimplemented)
        }
        fn clear(&mut self) {}
    }

    #[test_log::test]
    fn test_collection_intervals() {
        let resource_1 = ResourceType::Sensor(Arc::new(Mutex::new(TestSensor {})));
        let data_coll_1 = DataCollector::new(
            "r1".to_string(),
            resource_1,
            CollectionMethod::Readings,
            10.0,
        );
        assert!(data_coll_1.is_ok());
        let data_coll_1 = data_coll_1.unwrap();

        let resource_2 = ResourceType::Sensor(Arc::new(Mutex::new(TestSensor {})));
        let data_coll_2 = DataCollector::new(
            "r2".to_string(),
            resource_2,
            CollectionMethod::Readings,
            50.0,
        );
        assert!(data_coll_2.is_ok());
        let data_coll_2 = data_coll_2.unwrap();

        let resource_3 = ResourceType::Sensor(Arc::new(Mutex::new(TestSensor {})));
        let data_coll_3 = DataCollector::new(
            "r2".to_string(),
            resource_3,
            CollectionMethod::Readings,
            10.0,
        );
        assert!(data_coll_3.is_ok());
        let data_coll_3 = data_coll_3.unwrap();

        let data_colls = vec![data_coll_1, data_coll_2, data_coll_3];
        let store = NoOpStore {};

        let data_manager = DataManager::new(data_colls, store, 30, "1".to_string());
        let expected_collection_intervals: Vec<u64> = vec![20, 100];
        assert_eq!(
            data_manager.collection_intervals(),
            expected_collection_intervals
        );
    }
}
