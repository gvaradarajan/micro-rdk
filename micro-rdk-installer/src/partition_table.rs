use esp_idf_part::{AppType, DataType, Partition, PartitionTable, SubType, Type};

const NVS_OFFSET: u32 = 0x9000;
const PHY_INIT_SIZE: u32 = 0x1000;
const FACTORY_OFFSET: u32 = 0x40000;
const APP_SIZE: u32 = 0x3C0000;

fn create_nvs_partition_row(size: u32, encrypted: bool) -> Partition {
    Partition::new(
        "nvs",
        Type::Data,
        SubType::Data(DataType::Nvs),
        NVS_OFFSET,
        size,
        encrypted,
    )
}

pub fn create_partition_table(nvs_size: u32, encrypted: bool) -> PartitionTable {
    let mut partitions = vec![];
    partitions.push(create_nvs_partition_row(nvs_size, encrypted));
    let phy_init_offset = NVS_OFFSET + nvs_size;
    partitions.push(Partition::new(
        "phy_init",
        Type::Data,
        SubType::Data(DataType::Phy),
        phy_init_offset,
        PHY_INIT_SIZE,
        encrypted,
    ));
    // let factory_offset = phy_init_offset + PHY_INIT_SIZE;
    partitions.push(Partition::new(
        "factory",
        Type::App,
        SubType::App(AppType::Factory),
        FACTORY_OFFSET,
        APP_SIZE,
        encrypted,
    ));
    PartitionTable::new(partitions)
}
