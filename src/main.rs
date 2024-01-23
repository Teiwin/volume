use log::{debug, info};
use std::time::Duration;
use std::io::BufReader;
use std::io::BufRead;
use pulsectl::controllers::SinkController;
use pulsectl::controllers::DeviceControl;
use pulsectl::controllers::AppControl;
use libpulse_binding::volume::Volume;

const START_BYTE: u8 = 0xFF;
const SENSOR_BYTE: u8 = 0xF0;
const BUTTONS_BYTE: u8 = 0xF1;
const END_BYTE: u8 = 0xFE;

fn main() {
    // arduino communication stuff
    let serial_port = match serialport::new("/dev/ttyUSB0", 9600)
        .timeout(Duration::from_secs(3600))
        .open() {
            Ok(serialport) => serialport,
            Err(_) => panic!("Failed to open serial port")
        };

    let mut reader = BufReader::new(serial_port);
    let mut buf = vec![];

    // pulseaudio stuff
    let mut handler = SinkController::create().unwrap();

    // button stuff
    let mut app_selected = false;
    let mut sink_selected = 0;

    loop{
        match reader.read_until(END_BYTE, &mut buf) {
                Ok(_) => {},
                Err(_) => panic!("Failed to read buffer")
            }

        // if the buffer is smaller than one packet, continue to read
        if buf.len() < 5 {
            continue;
        }
        debug!("{:?}",buf);

        // find the packet index (0xFF 0xFF 0x.. 0x.. 0xFE)
        let mut volume = None;
        let mut buttons = None;
        for i in 0..buf.len()-4 {
            if buf[i] == START_BYTE && buf[i+4] == END_BYTE{ // we identified a packet
                match buf[i+1] {
                    SENSOR_BYTE => {volume = Some((buf[i + 2] as u32) << 8 | buf[i + 3] as u32);},
                    BUTTONS_BYTE => {buttons = Some((buf[i + 2] as u32) << 8 | buf[i + 3] as u32);},
                    _ => {}
                }
            }
        }

        if volume.is_some() { // we didn't find a packet
            buf.clear();

            // create the volume element
            let final_volume: u32 = 65535 * volume.unwrap() / 1024;
            let volume = Volume(final_volume) ;

            if app_selected {
                // output APPLICATIONS
                let apps = match handler.list_applications() {
                        Ok(apps) => apps,
                        Err(_) => panic!("Could not get list of Application streams") 
                    };

                info!("App Selected: {:?}", apps[sink_selected].name);
                info!("Volume : {:?}", volume.0);

                let current_volume = apps[sink_selected].volume;

                // compute the percentage difference
                let percentage: f64 = (volume.0 as f64 - current_volume.avg().0 as f64) / 65535.0;
                if percentage.abs() > 0.1 {
                    info!("GRAND ECART current, {}, sensor, {}, percentage {}",
                        current_volume.avg().0,
                        volume.0,
                        percentage);
                    continue;
                }

                if percentage > 0.0 {
                    handler.increase_app_volume_by_percent(apps[sink_selected].index, percentage);
                } else {
                    handler.decrease_app_volume_by_percent(apps[sink_selected].index, -percentage);
                }

            } else {
                // output DEVICES
                let devices = match handler.list_devices() {
                    Ok(devices) => devices,
                    Err(_) => panic!("Could not get list of playback devices.")
                };

                info!("Device Selected: {}",
                    devices[sink_selected].description.as_ref().unwrap());
                info!("Volume : {:?}",volume.0);

                let mut current_volume = devices[sink_selected].volume;

                if (volume.0 as i32 - current_volume.avg().0 as i32).abs() > 10000 {
                    info!("GRAND ECART current, {}, sensor, {}",
                        current_volume.avg().0,
                        volume.0);
                    continue;
                }

                let channel_volumes = current_volume.set(
                    current_volume.len(),
                    volume);

                // set the volume of the default device
                handler.set_device_volume_by_index(devices[sink_selected].index, channel_volumes);
            }
        }

        if buttons.is_some() {
            buf.clear();
            debug!("Bouton : {}", buttons.unwrap());
            // list apps
            let apps = match handler.list_applications() {
                Ok(apps) => apps,
                Err(_) => panic!("Could not get list of Application streams")
            };
            // list devices
            let devices = match handler.list_devices() {
                Ok(devices) => devices,
                Err(_) => panic!("Could not get list of playback devices")
            };

            if buttons.unwrap() == 0b0010  {
                // button 1 pressed
                app_selected = !app_selected;
                info!("App : {}", app_selected);
                if app_selected {
                    // check that index is in bounds
                    if sink_selected > apps.len()-1 {
                        sink_selected = apps.len()-1
                    }
                } else {
                    // check that index is in bounds
                    if sink_selected > devices.len()-1 {
                        sink_selected = devices.len()-1
                    }
                }
            }

            if app_selected {
                if buttons.unwrap() == 0b0001 {
                    info!("Next sink");
                    if sink_selected == apps.len()-1 {
                        sink_selected = 0
                    } else {
                        sink_selected += 1;
                    }
                }
    
                if buttons.unwrap() == 0b1000 {
                    info!("Previous sink");
                    if sink_selected == 0 {
                        sink_selected = apps.len()-1;
                    } else {
                        sink_selected -= 1;
                    }
                }
            } else {
                if buttons.unwrap() == 0b0001 {
                    info!("Next sink");
                    if sink_selected == devices.len()-1 {
                        sink_selected = 0
                    } else {
                        sink_selected += 1;
                    }
                }
    
                if buttons.unwrap() == 0b1000 {
                    info!("Previous sink");
                    if sink_selected == 0 {
                        sink_selected = devices.len()-1;
                    } else {
                        sink_selected -= 1;
                    }
                }
            }
        }
    }
}