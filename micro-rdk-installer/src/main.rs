use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use clap::{arg, command, Args, Parser, Subcommand};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Password};
use micro_rdk_installer::flash::viam_flash;
use micro_rdk_installer::nvs::data::{ViamFlashStorageData, WifiCredentials};
use micro_rdk_installer::nvs::partition::{NVSPartition, NVSPartitionData};
use micro_rdk_installer::nvs::request::populate_nvs_storage_from_app;
use micro_rdk_installer::partition_table::create_partition_table;
use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct AppCloudConfig {
    r#id: String,
    app_address: String,
    secret: Secret<String>,
}

#[derive(Deserialize, Debug)]
struct AppConfig {
    cloud: AppCloudConfig,
}

#[derive(Subcommand)]
enum Commands {
    WriteBinary(WriteBinary),
    WriteFlash(WriteFlash),
    CreateNvsPartition(CreateNVSPartition),
}

#[derive(Args)]
struct WriteBinary {}

#[derive(Args)]
struct WriteFlash {
    #[arg(long = "app-config")]
    config: String,
    #[arg(long = "bootloader")]
    bootloader_path: String,
    #[arg(long = "app")]
    app_path: String,
    #[arg(long = "size", default_value = "32768")]
    nvs_size: usize,
    #[arg(long = "monitor")]
    should_monitor: bool
}

#[derive(Args)]
struct CreateNVSPartition {
    #[arg(long = "app-config")]
    config: String,
    #[arg(long = "output")]
    file_name: String,
    #[arg(long = "size", default_value = "32768")]
    size: usize,
}

#[derive(Parser)]
#[command(
    about = "A CLI that can compile a micro-RDK binary or flash a compilation of micro-RDK directly to an ESP32 provided configuration information"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

fn request_wifi() -> anyhow::Result<WifiCredentials> {
    let ssid: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Please enter WiFi SSID")
        .interact_text()?;
    let password: String = Password::with_theme(&ColorfulTheme::default())
        .with_prompt("Please enter WiFi Password")
        .validate_with(|input: &String| -> anyhow::Result<()> {
            if input.len() > 64 {
                anyhow::bail!("password length limited to 64 characters or less")
            }
            Ok(())
        })
        .interact()?;

    Ok(WifiCredentials { ssid, password })
}

fn create_nvs_partition_binary(config_path: String, size: usize) -> anyhow::Result<Vec<u8>> {
    let mut storage_data = ViamFlashStorageData::default();
    let config_str = fs::read_to_string(config_path)?;
    let app_config: AppConfig = serde_json::from_str(&config_str)?;
    storage_data.robot_credentials.robot_id = Some(app_config.cloud.r#id.to_string());
    storage_data.robot_credentials.app_address = Some(app_config.cloud.app_address.to_string());
    storage_data.robot_credentials.robot_secret =
        Some(app_config.cloud.secret.expose_secret().to_string());
    let wifi_cred = request_wifi()?;
    storage_data.wifi = Some(wifi_cred);
    populate_nvs_storage_from_app(&mut storage_data)?;
    let part = &mut NVSPartition::from_storage_data(storage_data, size)?;
    Ok(NVSPartitionData::try_from(part)?.to_bytes())
}

fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();
    match &cli.command {
        Some(Commands::WriteBinary(_)) => {
            anyhow::bail!("binary write not yet supported")
        }
        Some(Commands::WriteFlash(args)) => {
            let bootloader_path = Path::new(&args.bootloader_path);
            let binary_path = Path::new(&args.app_path);
            let partition_table = create_partition_table(args.nvs_size as u32, false);
            partition_table.validate().or_else(|err| {
                let mut file = fs::File::create("table.csv")?;
                file.write_all(partition_table.to_csv()?.as_bytes())?;
                Err(err)
            })?;
            let nvs_data = create_nvs_partition_binary(args.config.to_string(), args.nvs_size)?;
            viam_flash(bootloader_path, binary_path, partition_table, nvs_data, args.should_monitor)?;
        }
        Some(Commands::CreateNvsPartition(args)) => {
            let mut file = File::create(args.file_name.to_string())?;
            file.write_all(&create_nvs_partition_binary(
                args.config.to_string(),
                args.size,
            )?)?;
        }
        None => {
            anyhow::bail!("command required")
        }
    };
    Ok(())
}
