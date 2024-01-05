#![allow(dead_code)]

use std::{
    net::{Ipv4Addr, SocketAddr},
    rc::Rc,
    sync::{Arc, Mutex},
};

use crate::common::{
    app_client::{AppClientBuilder, AppClientConfig},
    conn::server::{ViamServerBuilder, WebRtcConfiguration},
    entry::RobotRepresentation,
    grpc_client::GrpcClient,
    log::config_log_entry,
    robot::LocalRobot,
};

use crate::common::conn::server::TlsClientConnector;

use super::{
    certificate::WebRtcCertificate,
    conn::mdns::Esp32Mdns,
    dtls::Esp32DtlsBuilder,
    exec::Esp32Executor,
    tcp::{Esp32Listener, Esp32Stream},
    tls::{Esp32Tls, Esp32TlsServerConfig},
    webhook::Webhook,
};

use embedded_svc::http::client::Client as HttpClient;
use esp_idf_svc::http::client::{Configuration as HttpConfiguration, EspHttpConnection};
use esp_idf_hal::sys::{settimeofday, timeval, uxTaskGetStackHighWaterMark};
use esp_idf_hal::cpu::Core;
use esp_idf_hal::task::thread::ThreadSpawnConfiguration;

struct TaskIndicesToTimeIntervals {
    original_intervals: Vec<u64>,
    remaining_times: Vec<u64>,
}

impl TaskIndicesToTimeIntervals {
    pub fn new(original_intervals: Vec<u64>) -> anyhow::Result<Self> {
        if original_intervals.is_empty() {
            anyhow::bail!("WaitTimeForIntervals must take at least one time interval")
        }
        let remaining_times = original_intervals.to_vec();
        Ok(Self {
            original_intervals,
            remaining_times,
        })
    }

    pub fn original_intervals(&self) -> Vec<u64> {
        self.original_intervals.to_vec()
    }

    // pub fn advance_wait_time(&mut self, milliseconds: u64) -> Vec<usize> {
    //     let mut task_indices = vec![];
    //     for (i, time_remaining) in self.remaining_times.iter_mut().enumerate() {
    //         if *time_remaining > milliseconds {
    //             *time_remaining -= milliseconds;
    //         } else {
    //             let missed_task_instances = (milliseconds / *time_remaining) as usize;
    //             *time_remaining = *self.original_intervals.get(i).unwrap();
    //             for _ in (0..missed_task_instances) {
    //                 task_indices.push(i)
    //             }
    //         }
    //     }
    //     task_indices
    // }
}

impl Iterator for TaskIndicesToTimeIntervals {
    type Item = (Vec<usize>, u64);
    fn next(&mut self) -> Option<Self::Item> {
        let min = self.remaining_times.iter().min().unwrap();
        let wait_time = *min;
        let mut min_indices = vec![];
        for (i, time_remaining) in self.remaining_times.iter_mut().enumerate() {
            if *time_remaining != wait_time {
                *time_remaining -= wait_time;
            } else {
                *time_remaining = *self.original_intervals.get(i).unwrap();
                min_indices.push(i)
            }
        }
        Some((min_indices, wait_time))
    }
}

pub fn serve_web(
    app_config: AppClientConfig,
    tls_server_config: Esp32TlsServerConfig,
    repr: RobotRepresentation,
    _ip: Ipv4Addr,
    webrtc_certificate: WebRtcCertificate,
) {
    let cloned_app_cfg = app_config.clone();
    let exec = Esp32Executor::new();
    // let cloned_exec = exec.clone();
    let client_connector = Arc::new(Mutex::new(Esp32Tls::new_client()));
    

    let (mut srv, robot, part_id) = {
        // let mut client_connector = Esp32Tls::new_client();
        let mdns = Esp32Mdns::new("".to_string()).unwrap();

        let (cfg_response, robot, part_id) = {
            let cloned_exec = exec.clone();
            // let conn = client_connector.open_ssl_context(None).unwrap();
            // let conn = Esp32Stream::TLSStream(Box::new(conn));
            // let grpc_client =
            //     Box::new(GrpcClient::new(conn, cloned_exec, "https://app.viam.com:443").unwrap());

            // let builder = AppClientBuilder::new(grpc_client, app_config.clone());

            // let mut client = builder.build().unwrap();
            let conn = client_connector.lock().unwrap().open_ssl_context(None).unwrap();
            let conn = Esp32Stream::TLSStream(Box::new(conn));
            let grpc_client =
                Box::new(GrpcClient::new(conn, cloned_exec, "https://app.viam.com:443").unwrap());
            let builder = AppClientBuilder::new(grpc_client, app_config.clone());
            let mut client = builder.build().unwrap();

            let (cfg_response, cfg_received_datetime) = client.get_config().unwrap();
            let part_id = client.robot_part_id();

            if let Some(current_dt) = cfg_received_datetime.as_ref() {
                let tz = chrono_tz::Tz::UTC;
                std::env::set_var("TZ", tz.name());
                let tv_sec = current_dt.timestamp() as i32;
                let tv_usec = current_dt.timestamp_subsec_micros() as i32;
                let current_timeval = timeval { tv_sec, tv_usec };
                let res = unsafe { settimeofday(&current_timeval as *const timeval, std::ptr::null()) };
                if res != 0 {
                    println!("could not set time of day for timezone {:?} and timestamp {:?}", tz.name(), current_dt);
                }
            }

            let robot = match repr {
                RobotRepresentation::WithRobot(robot) => Arc::new(Mutex::new(robot)),
                RobotRepresentation::WithRegistry(registry) => {
                    log::info!("building robot from config");
                    let r = match LocalRobot::from_cloud_config(
                        &cfg_response,
                        registry,
                        cfg_received_datetime,
                    ) {
                        Ok(robot) => {
                            if let Some(datetime) = cfg_received_datetime {
                                let logs = vec![config_log_entry(datetime, None)];
                                client.push_logs(logs).expect("could not push logs to app");
                            }
                            robot
                        }
                        Err(err) => {
                            if let Some(datetime) = cfg_received_datetime {
                                let logs = vec![config_log_entry(datetime, Some(&err))];
                                client.push_logs(logs).expect("could not push logs to app");
                            }
                            panic!("{}", err)
                        }
                    };
                    Arc::new(Mutex::new(r))
                }
            };

            (cfg_response, robot, part_id)
        };

        let address: SocketAddr = "0.0.0.0:12346".parse().unwrap();
        let tls = Box::new(Esp32Tls::new_server(&tls_server_config));
        let tls_listener = Esp32Listener::new(address.into(), Some(tls)).unwrap();

        let webrtc_certificate = Rc::new(webrtc_certificate);
        let dtls = Esp32DtlsBuilder::new(webrtc_certificate.clone());

        let cloned_exec = exec.clone();

        let webrtc = Box::new(WebRtcConfiguration::new(
            webrtc_certificate,
            dtls,
            exec.clone(),
        ));

        let robot_cfg = cfg_response.as_ref().config.as_ref().unwrap();

        if let Ok(webhook) = Webhook::from_robot_config(robot_cfg) {
            if webhook.has_endpoint() {
                // only make a client if a webhook url is present
                let mut client = HttpClient::wrap(
                    EspHttpConnection::new(&HttpConfiguration {
                        crt_bundle_attach: Some(esp_idf_sys::esp_crt_bundle_attach),
                        ..Default::default()
                    })
                    .unwrap(),
                );

                let _ = webhook.send(&mut client);
            }
        }

        (
            Box::new(
                ViamServerBuilder::new(mdns, cloned_exec, client_connector.clone(), app_config)
                    .with_webrtc(webrtc)
                    .with_http2(tls_listener, 12346)
                    .build(&cfg_response)
                    .unwrap(),
            ),
            robot,
            part_id
        )
    };

    ThreadSpawnConfiguration {
        name: Some(b"data_task\0"),
        stack_size: 12288,
        priority: 20,
        pin_to_core: Some(Core::Core1),
        ..Default::default()
    }
    .set()
    .unwrap();

    // std::thread::spawn( || {
    //     println!("start stack size: {:?}", unsafe { uxTaskGetStackHighWaterMark(std::ptr::null_mut()) });
    // });

    // let _thing = std::thread::Builder::new().stack_size(12288).spawn( || {
    //     println!("start stack size: {:?}", unsafe { uxTaskGetStackHighWaterMark(std::ptr::null_mut()) });
    // }).unwrap();

    // let _thing = std::thread::Builder::new().stack_size(12288).spawn( || {
    //     // 2292
    //     println!("start stack size: {:?}", unsafe { uxTaskGetStackHighWaterMark(std::ptr::null_mut()) });
    //     // match ThreadSpawnConfiguration::get() {
    //     //     Some(cfg) => {
    //     //         println!("thread stack size inside: {:?}", cfg.stack_size);
    //     //     }
    //     //     None => {
    //     //         println!("thread conf unavailable inside");
    //     //     }
    //     // }
    //     println!("thread name: {:?}", std::thread::current().name())
    // }).unwrap();

    // match ThreadSpawnConfiguration::get() {
    //     Some(cfg) => {
    //         println!("thread stack size outside: {:?}", cfg.stack_size);
    //     },
    //     None => {
    //         println!("thread conf unavailable outside");
    //     }
    // }
    let cloned_robot = robot.clone();
    // // let cloned_exec = exec.clone();
    let _ = std::thread::Builder::new().stack_size(12288).spawn(move || {
        println!("start stack size: {:?}", unsafe { uxTaskGetStackHighWaterMark(std::ptr::null_mut()) });
        let exec = Esp32Executor::new();

        // let conn = client_connector.lock().unwrap().open_ssl_context(None).unwrap();
        // let conn = Esp32Stream::TLSStream(Box::new(conn));
        // let conn = client_connector.lock().unwrap().connect().unwrap();
        // println!("remaining stack size before grpc: {:?}", unsafe { uxTaskGetStackHighWaterMark(std::ptr::null_mut()) });
        // let grpc_client =
        //     Arc::new(Box::new(GrpcClient::new(conn, exec, "https://app.viam.com:443").unwrap()));
        // let builder = AppClientBuilder::new(grpc_client, cloned_app_cfg);
        // let mut client = builder.build().unwrap();

        // let robot_part_id = client.robot_part_id();
        let task_intervals = cloned_robot.lock().unwrap().get_collector_time_intervals_ms();
        if !task_intervals.is_empty() {
            let mut tasks = TaskIndicesToTimeIntervals::new(task_intervals).unwrap();
            let original_intervals = tasks.original_intervals();
            loop {
                let (task_indices, wait_time) = tasks.next().unwrap();
                std::thread::sleep(std::time::Duration::from_millis(wait_time));
                for task_index in task_indices.iter() {
                    let time_interval_key = original_intervals.get(*task_index).unwrap();
                    println!("remaining stack size before readings: {:?}", unsafe { uxTaskGetStackHighWaterMark(std::ptr::null_mut()) });
                    if let Ok(sensor_readings) = cloned_robot
                        .lock()
                        .as_mut()
                        .unwrap()
                        .collect_readings(
                            &part_id,
                            time_interval_key,
                        )
                    {
                        let cloned_exec = exec.clone();
                        let cloned_app_cfg = cloned_app_cfg.clone();
                        let conn = client_connector.lock().unwrap().connect().unwrap();
                        // // println!("remaining stack size before grpc: {:?}", unsafe { uxTaskGetStackHighWaterMark(std::ptr::null_mut()) });
                        let grpc_client =
                            Box::new(GrpcClient::new(conn, cloned_exec, "https://app.viam.com:443").unwrap());
                        let builder = AppClientBuilder::new(grpc_client, cloned_app_cfg);
                        let mut client = builder.build().unwrap();

                        if let Err(err) = client.push_sensor_data(sensor_readings)
                        {
                            println!("failed to push: {:?}", err);
                            log::error!("error while reporting sensor data: {}", err);
                        } else {
                            println!("pushed maybe?")
                        }
                        // println!("readings collected");
                    }
                }
                // std::thread::sleep(std::time::Duration::from_millis(2000));
                // println!("in core 1: part: {:?}", robot_part_id);
            }
        }
        // loop {
        //     std::thread::sleep(std::time::Duration::from_millis(2000));
        //     println!("in core 1");
        // }
    });

    ThreadSpawnConfiguration::default().set().unwrap();

    srv.serve_forever(robot);
}
