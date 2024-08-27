use clap::Parser;
use influxdb::Client;
use std::env;
use tokio::time::{self, Duration};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// InfluxDB的地址
    #[clap(short, long, default_value = "http://localhost:8086")]
    influxdb_url: String,

    /// 目标bucket
    #[clap(short, long, default_value = "mydb")]
    bucket: String,

    /// 探测时间间隔
    #[clap(short, long, default_value_t = 10)]
    interval: u64,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let token = env::var("INFLUXDB_TOKEN").expect("缺少数据库token环境变量：INFLUXDB_TOKEN");

    // 初始化 InfluxDB 客户端
    let client = Client::new(args.influxdb_url, args.bucket).with_token(token);

    // 启动子线程，每隔固定时间插入数据
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
        .expect("Failed to listen for ctrl-c");
    println!("Program interrupted, shutting down...");
}
