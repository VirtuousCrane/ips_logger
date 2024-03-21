use btleplug::{api::{Central, CentralEvent, Manager as _, Peripheral, ScanFilter, ValueNotification}, platform::{Adapter, Manager}};
use csv::Writer;
use futures::{future::join_all, stream::StreamExt};
use clap::Parser;
use ips_logger::ble::{get_scan_result, Beacon, BeaconCalibrationData};
use rumqttc::{AsyncClient, Client, MqttOptions, Packet, QoS};
use tokio::{self, sync::RwLock, time};
use uuid::{uuid, Uuid};
use std::{collections::HashMap, error::Error, sync::{mpsc, Arc}, thread, time::Duration};

type BeaconList = Vec<Beacon>;
type BeaconCalibrationMap = RwLock<HashMap<String, HashMap<String, BeaconCalibrationData>>>;

const BLE_BEACON_UUID: Uuid = uuid!("422da7fb-7d15-425e-a65f-e0dbcc6f4c6a");

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// MQTT Host URL
    #[arg(long)]
    host: String,
    
    /// MQTT Port
    #[arg(short, long, default_value_t = 1883)]
    port: u16,

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
    let (ble_tx, ble_rx) = mpsc::channel::<Arc<BeaconList>>();
    let (mqtt_tx, mqtt_rx) = mpsc::channel::<Arc<BeaconCalibrationMap>>();
    
    // CSV
    let csv_writer = Writer::from_path(args.output.clone())?;
    let mqtt_csv_writer = Writer::from_path(String::from("mqtt_") + &args.output)?;

    // MQTT
    let beacon_calibration_map: Arc<BeaconCalibrationMap> = Arc::new(RwLock::new(HashMap::new()));
    let calibration_map_arc = beacon_calibration_map.clone();

    let mut mqttoptions = MqttOptions::new("beacon_logger", args.host, args.port);
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    
    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    client.subscribe(args.topic, QoS::AtMostOnce).await;

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
    
    // MQTT Thread
    let mqtt_process_handle = tokio::spawn(async move {
        let calibration_map= calibration_map_arc.clone();

        loop {
            let event = match eventloop.poll().await {
                Ok(notification) => {
                    notification
                },
                Err(_) => continue,
            };
            
            let packet = match event {
                rumqttc::Event::Incoming(pck) => pck,
                rumqttc::Event::Outgoing(_) => continue,
            };
    
            if let Packet::Publish(msg) = packet {
                let payload = msg.payload;
                if !payload.is_ascii() {
                    continue;
                }
    
                let data = String::from_utf8_lossy(&payload);
                let data_struct: BeaconCalibrationData = match serde_json::from_str(&data) {
                    Ok(b) => b,
                    Err(e) => {
                        println!("Invalid Format: {}", e.to_string());
                        continue;
                    }
                };
                
                let mut writer = calibration_map.write().await;
                match writer.get_mut(&data_struct.mac_address.clone()) {
                    Some(map) => {
                        map.insert(data_struct.device_identifier.clone(), data_struct);
                    }
                    None => {
                        let mac_address = data_struct.mac_address.clone();
                        let mut temp_map = HashMap::new();

                        temp_map.insert(data_struct.device_identifier.clone(), data_struct);
                        writer.insert(mac_address.to_ascii_uppercase(), temp_map);
                    }
                }
            }
        }
    });
    
    // CSV Thread
    let csv_handle = tokio::task::spawn(async move {
        let rx = ble_rx;
        let mut writer = csv_writer;
        let mut mqtt_writer = mqtt_csv_writer;
        let calibration_map_arc = beacon_calibration_map.clone();

        for beacons in rx {
            for beacon in beacons.iter() {
                // Writing to MQTT CSV
                let calibration_map = calibration_map_arc.read().await;

                println!("Querying at: {}", beacon.mac_address.clone());
                let calibrator_map = match calibration_map.get(&beacon.mac_address) {
                    Some(map) => map,
                    None => {
                        println!("Nothing in Calibration Map!");
                        continue;
                    },
                };
                
                for v in calibrator_map.values() {
                    if let Err(e) = mqtt_writer.serialize(v) {
                        println!("Failed to write beacon: {}", e.to_string());
                        continue;
                    }
                }
                
                // Writing to normal output
                if let Err(e) = writer.serialize(beacon) {
                    println!("Failed to write beacon: {}", e.to_string());
                    continue;
                }
                
            }

            if let Err(e) = writer.flush() {
                println!("Failed to write to file: {}", e.to_string());
            }
            
            if let Err(e) = mqtt_writer.flush() {
                println!("Failed to write to file: {}", e.to_string());
            }

        }
    });
    
    
    join_all(vec![ble_scan_handle, mqtt_process_handle, csv_handle]).await;
    
    Ok(())
}
