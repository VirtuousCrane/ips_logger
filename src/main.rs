use btleplug::{api::{Central, CentralEvent, Manager as _, Peripheral, ScanFilter}, platform::{Adapter, Manager}};
use futures::stream::StreamExt;
use clap::Parser;
use tokio::{self, time};
use uuid::{uuid, Uuid};
use std::{error::Error, time::Duration};

const BLE_BEACON_UUID: Uuid = uuid!("422da7fb-7d15-425e-a65f-e0dbcc6f4c6a");

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// MQTT Host URL
    #[arg(long)]
    host: String,
    
    /// MQTT Port
    #[arg(short, long, default_value_t = 1883)]
    port: u32,

    /// MQTT Subscribe Topic
    #[arg(short, long)]
    topic: String,

    #[arg(short, long)]
    verbose: bool,

    /// Output file name
    #[arg(default_value_t = String::from("output.csv"))]
    output: String,
}

struct Beacon {
    mac_address: String,
    rssi: i16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
/* 
    let args = Args::parse();
    
    println!("Host: {:?}", args.host);
    println!("Port: {:?}", args.port);
    println!("Topic: {:?}", args.topic);
    println!("Verbose: {:?}", args.verbose);
    println!("Output: {:?}", args.output)
*/
    let ble_manager = Manager::new().await?;
    let ble_adapters = ble_manager.adapters().await?;
    let ble_central = ble_adapters.into_iter().nth(0).unwrap();
    
    ble_central.start_scan(ScanFilter { services: vec![BLE_BEACON_UUID] }).await?;

    loop {
        time::sleep(Duration::from_secs(2)).await;

        let peripherals = ble_central.peripherals().await?;

        for peripheral in peripherals.iter() {
            let mac_address = peripheral.address();

            let properties_result = peripheral.properties().await;
            let properties = match properties_result {
                Ok(property) => property,
                Err(_) => {
                    println!("Failed to extract property");
                    continue;
                },
            };

            let rssi_option = match properties {
                Some(property) => property.rssi,
                None => continue,
            };

            let rssi = match rssi_option {
                Some(r) => r,
                None => {
                    println!("Failed to get RSSI");
                    continue;
                }
            };
            
            let beacon = Beacon { mac_address: mac_address.to_string(), rssi };
            
            println!("Discovered: {} rssi: {}", beacon.mac_address, beacon.rssi);
        }
    }
    
    Ok(())
}
