use super::{
    errors::ServerError,
    mdns::Mdns,
    utils::{NoHttp2, WebRtcNoOp},
};
#[cfg(feature = "esp32")]
use crate::esp32::exec::Esp32Executor;
#[cfg(feature = "native")]
use crate::native::exec::NativeExecutor;

#[cfg(feature = "esp32")]
use esp_idf_hal::{
    sys::uxTaskGetStackHighWaterMark,
    cpu::Core,
    task::thread::ThreadSpawnConfiguration,
};

use crate::{
    common::{
        app_client::{AppClient, AppClientBuilder, AppClientConfig, AppClientError, AppSignaling},
        grpc::{GrpcBody, GrpcServer},
        grpc_client::{GrpcClient, GrpcClientError},
        robot::LocalRobot,
        webrtc::{
            api::{WebRtcApi, WebRtcSdp},
            certificate::Certificate,
            dtls::{DtlsBuilder, DtlsConnector},
            exec::WebRtcExecutor,
            grpc::{WebRtcGrpcBody, WebRtcGrpcServer},
        },
    },
    proto::{self, app::v1::ConfigResponse},
};

use futures_lite::{
    future::{block_on, Boxed},
    ready, Future,
};
use hyper::server::conn::Http;

use smol::Task;
use smol_timeout::TimeoutExt;
use std::{
    fmt::Debug,
    marker::PhantomData,
    net::Ipv4Addr,
    pin::Pin,
    rc::Rc,
    sync::{Arc, Mutex},
    task::Poll,
};
use tokio::io::{AsyncRead, AsyncWrite};

#[cfg(feature = "native")]
type Executor<'a> = NativeExecutor<'a>;
#[cfg(feature = "esp32")]
type Executor<'a> = Esp32Executor<'a>;



pub trait TlsClientConnector {
    type Stream: AsyncRead + AsyncWrite + Unpin + 'static;

    fn connect(&mut self) -> Result<Self::Stream, ServerError>;
}

impl<A> TlsClientConnector for Arc<Mutex<A>> 
where
    A: ?Sized + TlsClientConnector
{
    type Stream = A::Stream;
    fn connect(&mut self) -> Result<Self::Stream, ServerError> {
        self.lock().unwrap().connect()
    }
} 

pub struct RobotCloudConfig {
    local_fqdn: String,
    name: String,
    fqdn: String,
}

impl RobotCloudConfig {
    pub fn new(local_fqdn: String, name: String, fqdn: String) -> Self {
        Self {
            local_fqdn,
            name,
            fqdn,
        }
    }
}

impl From<proto::app::v1::CloudConfig> for RobotCloudConfig {
    fn from(c: proto::app::v1::CloudConfig) -> Self {
        Self {
            local_fqdn: c.local_fqdn.clone(),
            name: c.local_fqdn.split('.').next().unwrap_or("").to_owned(),
            fqdn: c.fqdn.clone(),
        }
    }
}

impl From<&proto::app::v1::CloudConfig> for RobotCloudConfig {
    fn from(c: &proto::app::v1::CloudConfig) -> Self {
        Self {
            local_fqdn: c.local_fqdn.clone(),
            name: c.local_fqdn.split('.').next().unwrap_or("").to_owned(),
            fqdn: c.fqdn.clone(),
        }
    }
}

pub struct ViamServerBuilder<'a, M, C, T, CC = WebRtcNoOp, D = WebRtcNoOp, L = NoHttp2> {
    mdns: M,
    webrtc: Option<Box<WebRtcConfiguration<'a, D, CC>>>,
    port: u16, // gRPC/HTTP2 port
    http2_listener: L,
    _marker: PhantomData<T>,
    exec: Executor<'a>,
    app_connector: C,
    app_config: AppClientConfig,
}

impl<'a, M, C, T> ViamServerBuilder<'a, M, C, T>
where
    M: Mdns,
    C: TlsClientConnector,
    T: AsyncRead + AsyncWrite + Unpin + 'static,
{
    pub fn new(mdns: M, exec: Executor<'a>, app_connector: C, app_config: AppClientConfig) -> Self {
        Self {
            mdns,
            http2_listener: NoHttp2 {},
            port: 0,
            webrtc: None,
            _marker: PhantomData,
            exec,
            app_connector,
            app_config,
        }
    }
}

impl<'a, M, C, T, CC, D, L> ViamServerBuilder<'a, M, C, T, CC, D, L>
where
    M: Mdns,
    C: TlsClientConnector,
    T: AsyncRead + AsyncWrite + Unpin + 'static,
    CC: Certificate + 'a,
    D: DtlsBuilder,
    D::Output: 'a,
    L: AsyncableTcpListener<T>,
    L::Output: Http2Connector<Stream = T>,
{
    pub fn with_http2<L2, T2>(
        self,
        http2_listener: L2,
        port: u16,
    ) -> ViamServerBuilder<'a, M, C, T2, CC, D, L2> {
        ViamServerBuilder {
            mdns: self.mdns,
            port,
            _marker: PhantomData,
            http2_listener,
            exec: self.exec,
            webrtc: self.webrtc,
            app_connector: self.app_connector,
            app_config: self.app_config,
        }
    }
    pub fn with_webrtc<D2, CC2>(
        self,
        webrtc: Box<WebRtcConfiguration<'a, D2, CC2>>,
    ) -> ViamServerBuilder<'a, M, C, T, CC2, D2, L> {
        ViamServerBuilder {
            mdns: self.mdns,
            webrtc: Some(webrtc),
            port: self.port,
            http2_listener: self.http2_listener,
            _marker: self._marker,
            exec: self.exec,
            app_connector: self.app_connector,
            app_config: self.app_config,
        }
    }
    pub fn build(
        mut self,
        config: &ConfigResponse,
    ) -> Result<ViamServer<'a, C, T, CC, D, L>, ServerError> {
        let cfg: RobotCloudConfig = config
            .config
            .as_ref()
            .unwrap()
            .cloud
            .as_ref()
            .unwrap()
            .into();

        self.app_config.set_rpc_host(cfg.fqdn.clone());

        self.mdns
            .set_hostname(&cfg.name)
            .map_err(|e| ServerError::Other(e.into()))?;
        self.mdns
            .add_service(
                &cfg.local_fqdn.replace('.', "-"),
                "_rpc",
                "_tcp",
                self.port,
                &[("grpc", "")],
            )
            .map_err(|e| ServerError::Other(e.into()))?;
        self.mdns
            .add_service(
                &cfg.fqdn.replace('.', "-"),
                "_rpc",
                "_tcp",
                self.port,
                &[("grpc", "")],
            )
            .map_err(|e| ServerError::Other(e.into()))?;

        let cloned_exec = self.exec.clone();
        let http2_listener = HttpListener::new(self.http2_listener);

        let srv = ViamServer::new(
            http2_listener,
            self.webrtc,
            cloned_exec,
            self.app_connector,
            self.app_config,
        );

        Ok(srv)
    }
}

pub trait Http2Connector: std::fmt::Debug {
    type Stream;
    fn accept(&mut self) -> std::io::Result<Self::Stream>;
}

#[derive(Debug)]
pub struct HttpListener<L, T> {
    listener: L,
    marker: PhantomData<T>,
}
pin_project_lite::pin_project! {
    pub struct OwnedListener<T> {
        #[pin]
        pub inner: Boxed<std::io::Result<T>>,
    }
}

impl<T> Future for OwnedListener<T> {
    type Output = std::io::Result<T>;
    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let r = ready!(this.inner.poll(cx));
        Poll::Ready(r)
    }
}

pub trait AsyncableTcpListener<T> {
    type Output: Debug + Http2Connector<Stream = T>;
    fn as_async_listener(&self) -> OwnedListener<Self::Output>;
}

impl<L, T> HttpListener<L, T>
where
    L: AsyncableTcpListener<T>,
{
    pub fn new(asyncable: L) -> Self {
        HttpListener {
            listener: asyncable,
            marker: PhantomData,
        }
    }
    fn next_conn(&self) -> OwnedListener<L::Output> {
        self.listener.as_async_listener()
    }
}

pub struct ViamServer<'a, C, T, CC, D, L> {
    http_listener: HttpListener<L, T>,
    webrtc_config: Option<Box<WebRtcConfiguration<'a, D, CC>>>,
    exec: Executor<'a>,
    app_connector: C,
    app_config: AppClientConfig,
    app_client: Option<AppClient<'a>>,
    webtrc_conn: Option<Task<Result<(), ServerError>>>,
}
impl<'a, C, T, CC, D, L> ViamServer<'a, C, T, CC, D, L>
where
    C: TlsClientConnector,
    T: AsyncRead + AsyncWrite + Unpin + 'static,
    CC: Certificate + 'a,
    D: DtlsBuilder,
    D::Output: 'a,
    L: AsyncableTcpListener<T>,
    L::Output: Http2Connector<Stream = T>,
{
    fn new(
        http_listener: HttpListener<L, T>,
        webrtc_config: Option<Box<WebRtcConfiguration<'a, D, CC>>>,
        exec: Executor<'a>,
        app_connector: C,
        app_config: AppClientConfig,
    ) -> Self {
        Self {
            http_listener,
            webrtc_config,
            exec,
            app_connector,
            app_config,
            app_client: None,
            webtrc_conn: None,
        }
    }
    pub fn serve_forever(&mut self, robot: Arc<Mutex<LocalRobot>>) {
        let cloned_exec = self.exec.clone();
        block_on(cloned_exec.run(Box::pin(self.serve(robot))));
    }

    // #[cfg(feature = "esp32")]
    // fn spawn_data_task(&mut self, robot: Arc<Mutex<LocalRobot>>) -> std::sync::mpsc::Receiver<crate::proto::app::data_sync::v1::DataCaptureUploadRequest> {
    //     ThreadSpawnConfiguration {
    //         name: Some(b"data_task\0"),
    //         stack_size: 12288,
    //         priority: 20,
    //         pin_to_core: Some(Core::Core1),
    //         ..Default::default()
    //     }
    //     .set()
    //     .unwrap();

    //     let cloned_robot = robot.clone();
    //     let _ = std::thread::Builder::new().stack_size(12288).spawn(move || {
    //         println!("start stack size: {:?}", unsafe { uxTaskGetStackHighWaterMark(std::ptr::null_mut()) });
            
    //         let task_intervals = cloned_robot.lock().unwrap().get_collector_time_intervals_ms();
    //         if !task_intervals.is_empty() {
    //             if let Some(client) = self.app_client.as_mut() {
    //                 let part_id = client.robot_part_id();
    //                 let mut tasks = TaskIndicesToTimeIntervals::new(task_intervals).unwrap();
    //                 let original_intervals = tasks.original_intervals();
    //                 loop {
    //                     let (task_indices, wait_time) = tasks.next().unwrap();
    //                     std::thread::sleep(std::time::Duration::from_millis(wait_time));
    //                     for task_index in task_indices.iter() {
    //                         let time_interval_key = original_intervals.get(*task_index).unwrap();
    //                         // println!("remaining stack size before readings: {:?}", unsafe { uxTaskGetStackHighWaterMark(std::ptr::null_mut()) });
    //                         if let Ok(sensor_readings) = cloned_robot
    //                             .lock()
    //                             .as_mut()
    //                             .unwrap()
    //                             .collect_readings(
    //                                 &part_id,
    //                                 time_interval_key,
    //                             )
    //                         {
                            
        
    //                             if let Err(err) = client.push_sensor_data(sensor_readings)
    //                             {
    //                                 println!("failed to push: {:?}", err);
    //                                 log::error!("error while reporting sensor data: {}", err);
    //                             } else {
    //                                 println!("pushed maybe?")
    //                             }
    //                             // println!("readings collected");
    //                         }
    //                     }
    //                     // std::thread::sleep(std::time::Duration::from_millis(2000));
    //                     // println!("in core 1: part: {:?}", robot_part_id);
    //                 }
    //             }
    //         }
    //         // loop {
    //         //     std::thread::sleep(std::time::Duration::from_millis(2000));
    //         //     println!("in core 1");
    //         // }
    //     });
    //     ThreadSpawnConfiguration::default().set().unwrap();

    // }

    async fn serve(&mut self, robot: Arc<Mutex<LocalRobot>>) {

        // #[cfg(feature = "esp32")]
        // let data_channel = self.spawn_data_task(robot.clone());

        let cloned_robot = robot.clone();
        let mut current_prio = None;

        loop {
            let _ = smol::Timer::after(std::time::Duration::from_millis(300)).await;

            if self.app_client.is_none() {
                let conn = self.app_connector.connect().unwrap();
                let cloned_exec = self.exec.clone();
                let grpc_client = Box::new(
                    GrpcClient::new(conn, cloned_exec, "https://app.viam.com:443").unwrap(),
                );
                let app_client = AppClientBuilder::new(grpc_client, self.app_config.clone())
                    .build()
                    .unwrap();
                let _ = self.app_client.insert(app_client);
            }

            let sig = if let Some(webrtc_config) = self.webrtc_config.as_ref() {
                let ip = self.app_config.get_ip();
                let signaling = self.app_client.as_mut().unwrap().connect_signaling();
                futures::future::Either::Left(WebRTCSignalingAnswerer {
                    webrtc_config: Some(webrtc_config),
                    future: signaling,
                    ip,
                })
            } else {
                futures::future::Either::Right(WebRTCSignalingAnswerer::<
                    '_,
                    '_,
                    CC,
                    D,
                    futures_lite::future::Pending<Result<AppSignaling, AppClientError>>,
                >::default())
            };

            let listener = self.http_listener.next_conn();

            log::info!("waiting for connection");

            let connection = futures_lite::future::or(
                async move {
                    let p = listener.await;
                    p.map(IncomingConnection::Http2Connection)
                        .map_err(|e| ServerError::Other(e.into()))
                },
                async {
                    let mut api = sig.await?;

                    let prio = self
                        .webtrc_conn
                        .as_ref()
                        .and_then(|f| (!f.is_finished()).then_some(&current_prio))
                        .unwrap_or(&None);

                    let sdp = api
                        .answer(prio)
                        .await
                        .map_err(|e| ServerError::Other(Box::new(e)))?;

                    // When the current priority is lower than the priority of the incoming connection then
                    // we cancel and close the current webrtc connection (if any)
                    if let Some(task) = self.webtrc_conn.take() {
                        if !task.is_finished() {
                            let _ = task.cancel().await;
                        }
                    }

                    let _ = current_prio.insert(sdp.1);

                    Ok(IncomingConnection::WebRtcConnection(WebRTCConnection {
                        webrtc_api: api,
                        sdp: sdp.0,
                        server: None,
                        robot: cloned_robot.clone(),
                    }))
                },
            );
            let connection = connection.await;

            if let Err(err) = connection {
                if let ServerError::ServerAppClientError(AppClientError::AppGrpcClientError(
                    GrpcClientError::ProtoError(err),
                )) = err
                {
                    // Google load balancer may terminate a connection after some time
                    // it will do so by sending a GOAWAY frame
                    if err.is_go_away() || err.is_io() || err.is_library() {
                        let _ = self.app_client.take();
                    }
                }
                continue;
            }
            if let Err(e) = match connection.unwrap() {
                IncomingConnection::Http2Connection(c) => self.serve_http2(c, robot.clone()).await,

                IncomingConnection::WebRtcConnection(mut c) => match c.open_data_channel().await {
                    Err(e) => Err(e),
                    Ok(_) => {
                        let t = self.exec.spawn(async move { c.run().await });
                        let _task = self.webtrc_conn.insert(t);
                        Ok(())
                    }
                },
            } {
                log::error!("error while serving {}", e);
            }
        }
    }
    async fn serve_http2<U>(
        &self,
        mut c: U,
        robot: Arc<Mutex<LocalRobot>>,
    ) -> Result<(), ServerError>
    where
        U: Http2Connector<Stream = T>,
    {
        let srv = GrpcServer::new(robot.clone(), GrpcBody::new());
        let connection = c.accept().map_err(|e| ServerError::Other(e.into()))?;

        Box::new(
            Http::new()
                .with_executor(self.exec.clone())
                .http2_only(true)
                .http2_initial_stream_window_size(2048)
                .http2_initial_connection_window_size(2048)
                .http2_max_send_buf_size(4096)
                .http2_max_concurrent_streams(1)
                .serve_connection(connection, srv),
        )
        .await
        .map_err(|e| ServerError::Other(e.into()))
    }
}
#[derive(Debug)]
pub enum IncomingConnection<L, U> {
    Http2Connection(L),
    WebRtcConnection(U),
}

#[derive(Clone)]
pub struct WebRtcConfiguration<'a, D, CC> {
    pub dtls: D,
    pub cert: Rc<CC>,
    pub exec: Executor<'a>,
}

impl<'a, D, CC> WebRtcConfiguration<'a, D, CC>
where
    D: DtlsBuilder,
    CC: Certificate,
{
    pub fn new(cert: Rc<CC>, dtls: D, exec: Executor<'a>) -> Self {
        Self { dtls, cert, exec }
    }
}
struct WebRTCConnection<C, D, E> {
    webrtc_api: WebRtcApi<C, D, E>,
    sdp: Box<WebRtcSdp>,
    server: Option<WebRtcGrpcServer<GrpcServer<WebRtcGrpcBody>>>,
    robot: Arc<Mutex<LocalRobot>>,
}
impl<C, D, E> WebRTCConnection<C, D, E>
where
    C: Certificate,
    D: DtlsConnector,
    E: WebRtcExecutor<Pin<Box<dyn Future<Output = ()>>>> + Clone,
{
    async fn open_data_channel(&mut self) -> Result<(), ServerError> {
        self.webrtc_api
            .run_ice_until_connected(&self.sdp)
            .timeout(std::time::Duration::from_secs(10))
            .await
            .ok_or(ServerError::ServerConnectionTimeout)?
            .map_err(|e| ServerError::Other(e.into()))?;

        let c = self
            .webrtc_api
            .open_data_channel()
            .timeout(std::time::Duration::from_secs(10))
            .await
            .ok_or(ServerError::ServerConnectionTimeout)?
            .map_err(|e| ServerError::Other(e.into()))?;
        let srv = WebRtcGrpcServer::new(
            c,
            GrpcServer::new(self.robot.clone(), WebRtcGrpcBody::default()),
        );
        let _ = self.server.insert(srv);
        Ok(())
    }
    async fn run(&mut self) -> Result<(), ServerError> {
        if self.server.is_none() {
            return Err(ServerError::ServerConnectionNotConfigured);
        }
        let srv = self.server.as_mut().unwrap();
        loop {
            if let Err(e) = srv.next_request().await {
                return Err(ServerError::Other(Box::new(e)));
            }
        }
    }
}

pin_project_lite::pin_project! {
    struct WebRTCSignalingAnswerer<'a,'b, C,D,F> {
        #[pin]
        future: F,
        webrtc_config: Option<&'b WebRtcConfiguration<'a,D,C>>,
        ip: Ipv4Addr,
    }
}

impl<'a, 'b, C, D, F> WebRTCSignalingAnswerer<'a, 'b, C, D, F> {
    fn default() -> WebRTCSignalingAnswerer<
        'a,
        'b,
        C,
        D,
        impl Future<Output = Result<AppSignaling, AppClientError>>,
    > {
        WebRTCSignalingAnswerer {
            future: futures_lite::future::pending::<Result<AppSignaling, AppClientError>>(),
            webrtc_config: None,
            ip: Ipv4Addr::new(0, 0, 0, 0),
        }
    }
}

impl<'a, 'b, C, D, F> Future for WebRTCSignalingAnswerer<'a, 'b, C, D, F>
where
    F: Future<Output = Result<AppSignaling, AppClientError>>,
    C: Certificate,
    D: DtlsBuilder,
{
    type Output = Result<WebRtcApi<C, D::Output, Executor<'a>>, ServerError>;
    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let r = ready!(this.future.poll(cx));
        let s = match r {
            Err(e) => return Poll::Ready(Err(ServerError::ServerAppClientError(e))),
            Ok(s) => s,
        };
        Poll::Ready(Ok(WebRtcApi::new(
            this.webrtc_config.as_ref().unwrap().exec.clone(),
            s.0,
            s.1,
            this.webrtc_config.as_ref().unwrap().cert.clone(),
            *this.ip,
            this.webrtc_config.as_ref().unwrap().dtls.make().unwrap(),
        )))
    }
}

// struct TaskIndicesToTimeIntervals {
//     original_intervals: Vec<u64>,
//     remaining_times: Vec<u64>,
// }

// impl TaskIndicesToTimeIntervals {
//     pub fn new(original_intervals: Vec<u64>) -> anyhow::Result<Self> {
//         if original_intervals.is_empty() {
//             anyhow::bail!("WaitTimeForIntervals must take at least one time interval")
//         }
//         let remaining_times = original_intervals.to_vec();
//         Ok(Self {
//             original_intervals,
//             remaining_times,
//         })
//     }

//     pub fn original_intervals(&self) -> Vec<u64> {
//         self.original_intervals.to_vec()
//     }

//     // pub fn advance_wait_time(&mut self, milliseconds: u64) -> Vec<usize> {
//     //     let mut task_indices = vec![];
//     //     for (i, time_remaining) in self.remaining_times.iter_mut().enumerate() {
//     //         if *time_remaining > milliseconds {
//     //             *time_remaining -= milliseconds;
//     //         } else {
//     //             let missed_task_instances = (milliseconds / *time_remaining) as usize;
//     //             *time_remaining = *self.original_intervals.get(i).unwrap();
//     //             for _ in (0..missed_task_instances) {
//     //                 task_indices.push(i)
//     //             }
//     //         }
//     //     }
//     //     task_indices
//     // }
// }

// impl Iterator for TaskIndicesToTimeIntervals {
//     type Item = (Vec<usize>, u64);
//     fn next(&mut self) -> Option<Self::Item> {
//         let min = self.remaining_times.iter().min().unwrap();
//         let wait_time = *min;
//         let mut min_indices = vec![];
//         for (i, time_remaining) in self.remaining_times.iter_mut().enumerate() {
//             if *time_remaining != wait_time {
//                 *time_remaining -= wait_time;
//             } else {
//                 *time_remaining = *self.original_intervals.get(i).unwrap();
//                 min_indices.push(i)
//             }
//         }
//         Some((min_indices, wait_time))
//     }
// }