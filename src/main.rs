use clap::Parser;
use influxdb2::Client;
use influxdb2_derive::WriteDataPoint;
use std::env;
use sysinfo::{ProcessRefreshKind, System};
use tokio::time::{self, Duration};

type Result<T> = core::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() {
    let args = ProcessInfo::parse();
    let token = env::var("INFLUXDB_TOKEN").expect("缺少数据库token环境变量：INFLUXDB_TOKEN");
    let client = Client::new(args.db_host, args.org, token);
    let interval = args.interval;

    tokio::spawn(async move {
        let mut system = System::new();

        // 刷新所有进程的信息，但只关注
        system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            ProcessRefreshKind::new()
                .with_cpu()
                .with_disk_usage()
                .with_user(sysinfo::UpdateKind::OnlyIfNotSet)
                .with_memory(),
        );
    });

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

/// 同时也是所有的配置项
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Config {
    /// InfluxDB的地址，格式为address:port
    #[clap(short, long)]
    db_host: Option<String>,

    /// InfluxDB的组织
    #[clap(short, long)]
    org: Option<String>,

    /// 目标bucket
    #[clap(short, long)]
    bucket: Option<String>,

    /// 采样时间间隔
    #[clap(short, long)]
    interval: Option<u64>,

    /// 待监控程序的pid，格式为pid1,pid2,pid3
    #[clap(short, long, last = true)]
    pids: Vec<u64>,
}
