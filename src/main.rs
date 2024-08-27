use clap::Parser;
use influxdb2::Client;
use influxdb2_derive::WriteDataPoint;
use std::env;
use sysinfo::System;
use tokio::time::{self, Duration};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// InfluxDB的地址，格式为address:port
    #[clap(short, long, default_value = "localhost:8086")]
    db_host: String,

    /// InfluxDB的组织
    #[clap(short, long, default_value = "jsptb")]
    org: String,

    /// 目标bucket
    #[clap(short, long, default_value = "rust-test")]
    bucket: String,

    /// 采样时间间隔
    #[clap(short, long, default_value_t = 10)]
    interval: u64,
}

type Result<T> = core::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let token = env::var("INFLUXDB_TOKEN").expect("缺少数据库token环境变量：INFLUXDB_TOKEN");
    let client = Client::new(args.db_host, args.org, token);
    let interval = args.interval;
    tokio::spawn(async move {
        let mut interval_timer = time::interval(Duration::from_secs(interval));

        loop {
            interval_timer.tick().await;
            todo!("插入查询数据");
        }
    });

    // 让主线程保持运行
    tokio::signal::ctrl_c()
        .await
        .expect("监听ctrl-c退出信号失败");
    println!("程序终端，退出");
}

async fn get_process_info(sys: &System, pid: sysinfo::Pid) -> Result<ProcessInfo> {
    // 这里强制使用i64是因为数据库存储的是i64
    let process = sys.process(pid).ok_or("错误的pid".to_string())?;
    Ok(ProcessInfo {
        time: chrono::Local::now()
            .timestamp_nanos_opt()
            .expect("时间戳已经超过了64位"),
        name: process.name().to_str().unwrap().to_string(),
        cpu_usage: process.cpu_usage() as f64,
        total_written_bytes: process.disk_usage().total_written_bytes as i64,
        total_read_bytes: process.disk_usage().total_read_bytes as i64,
        virtual_memory: process.virtual_memory() as i64,
        status: process.status().to_string(),
    })
}

#[derive(Debug, WriteDataPoint)]
#[measurement = "process_info"]
struct ProcessInfo {
    #[influxdb(timestamp)]
    time: i64,
    /// 程序的名称
    #[influxdb(tag)]
    name: String,
    /// cpu使用率
    cpu_usage: f64,
    /// 内存写入总量
    total_written_bytes: i64,
    /// 内存读取总量
    total_read_bytes: i64,
    /// 虚拟内存使用量
    virtual_memory: i64,
    /// 状态
    status: String,
}
