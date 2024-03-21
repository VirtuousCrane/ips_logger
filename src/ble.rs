use btleplug::{api::{Central, Manager as _, Peripheral}, platform::Adapter};
use serde::{Serialize, Deserialize};
use std::{error::Error, sync::Arc};

#[derive(Debug, Serialize)]
pub struct Beacon {
    pub mac_address: String,
    pub rssi: i16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BeaconCalibrationData {
    pub device_identifier: String,
    pub mac_address: String,
    pub rssi: i16,
    pub diff: i16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MqttBeaconData {
    pub device_identifier: String,
    pub mac_address: i16,
    pub rssi: i16,
}

pub async fn get_scan_result(ble_central: &Adapter) -> Result<Arc<Vec<Beacon>>, Box<dyn Error>> {
    let peripherals = ble_central.peripherals().await?;
    let mut beacons = Vec::new();

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
        beacons.push(beacon);
    }
    
    Ok(Arc::new(beacons))
}