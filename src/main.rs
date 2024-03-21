use btleplug::{api::{Central, CentralEvent, Manager as _, Peripheral, ScanFilter}, platform::{Adapter, Manager}};
use csv::Writer;
use futures::{future::join_all, stream::StreamExt};
use clap::Parser;
use ips_logger::ble::{get_scan_result, Beacon};
use tokio::{self, time};
use uuid::{uuid, Uuid};
use std::{error::Error, sync::{mpsc, Arc}, time::Duration};

type BeaconList = Arc<Vec<Beacon>>;

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

    /// BLE Scan Period (Scan every n Seconds)
    #[arg(long, default_value_t = 2)]
    period: u64,

    /// Output file name
    #[arg(default_value_t = String::from("output.csv"))]
    output: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    // Arg Parse
    let args = Args::parse();
    
    println!("Host: {:?}", args.host);
    println!("Port: {:?}", args.port);
    println!("Topic: {:?}", args.topic);
    println!("Verbose: {:?}", args.verbose);
    println!("Scan Period: {:?}", args.period);
    println!("Output: {:?}", args.output);
    
    // Common
    let (ble_tx, ble_rx) = mpsc::channel::<BeaconList>();
    
    // CSV
    let csv_writer = Writer::from_path(args.output)?;

    // BLE Scan
    let ble_manager = Manager::new().await?;
    let ble_adapters = ble_manager.adapters().await?;
    let ble_central = ble_adapters.into_iter().nth(0).unwrap();

    ble_central.start_scan(ScanFilter { services: vec![BLE_BEACON_UUID] }).await?;

    // BLE Process
    let ble_scan_handle = tokio::spawn(async move {
        let tx = ble_tx;

        loop {
            time::sleep(Duration::from_secs(args.period)).await;
            let beacons = get_scan_result(&ble_central).await;
            
            match beacons {
                Ok(beacons_arc) => {
                    match tx.send(beacons_arc) {
                        Ok(_) => (),
                        Err(e) => println!("Failed to pass message between BLE and Output Channel: {}", e.to_string()),
                    }
                },
                Err(e) => {
                    println!("Failed to fetch scan result: {}", e.to_string());
                    continue;
                }
            }
        }
    });
    
    // CSV Thread
    let csv_handle = tokio::task::spawn(async move {
        let rx = ble_rx;
        let mut writer = csv_writer;

        for beacons in rx {
            for beacon in beacons.iter() {
                if let Err(e) = writer.serialize(beacon) {
                    println!("Failed to write beacon: {}", e.to_string());
                    continue;
                }
            }

            if let Err(e) = writer.flush() {
                println!("Failed to write to file: {}", e.to_string());
            }
        }
    });
    
    
    join_all(vec![ble_scan_handle, csv_handle]).await;
    
    Ok(())
}
