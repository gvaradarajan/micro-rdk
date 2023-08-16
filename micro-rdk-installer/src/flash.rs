use esp_idf_part::PartitionTable;
use espflash::{
    cli::{
        config::Config, connect, monitor::monitor, print_board_info, ConnectArgs, EspflashProgress,
    },
    flasher::{FlashFrequency, FlashMode},
    image_format::ImageFormatKind, error::Error,
};
use std::{fs, path::Path};

pub fn viam_flash(
    bootloader: &Path,
    elf_binary: &Path,
    partition_table: PartitionTable,
    nvs_data: Vec<u8>,
    should_monitor: bool,
) -> Result<(), Error> {
    let path = fs::canonicalize(bootloader)?;
    let bootloader_data = fs::read(path)?;

    let connect_args = ConnectArgs::default();
    let conf = Config::default();
    let mut flasher = connect(&connect_args, &conf).map_err(|_| Error::FlashConnect )?;

    flasher.disable_watchdog()?;

    let elf_data = fs::read(elf_binary)?;

    print_board_info(&mut flasher).map_err(|_| Error::FlashConnect )?;

    flasher.load_elf_to_flash_with_format_with_nvs(
        &elf_data,
        Some(bootloader_data),
        Some(partition_table),
        nvs_data,
        Some(ImageFormatKind::EspBootloader),
        Some(FlashMode::Dio),
        None,
        Some(FlashFrequency::_40Mhz),
        Some(&mut EspflashProgress::default()),
    )?;

    if should_monitor {
        let pid = flasher.get_usb_pid()?;

        // The 26MHz ESP32-C2's need to be treated as a special case.
        // let default_baud =
        //     if chip == Chip::Esp32c2 && args.connect_args.no_stub && target_xtal_freq == 26 {
        //         74_880
        //     } else {
        //         115_200
        //     };

        monitor(flasher.into_interface(), Some(&elf_data), pid, 115_200)?;
    }

    Ok(())
}
