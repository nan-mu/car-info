use clap::Parser;
use influxdb2_derive::WriteDataPoint;
use priority_queue::PriorityQueue;
use std::cmp::Reverse;
use sysinfo::{CpuRefreshKind, ProcessRefreshKind, RefreshKind, System};
use tokio::time::{self, Duration};

type Result<T> = core::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() {
    // 得到一些硬件相关的参数
    let mut system =
        System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::everything()));
    // 文档里说要开始获得准确的 CPU 使用率，进程需要刷新两次。
    // 这里的MINIMUM_CPU_UPDATE_INTERVAL是刷新的最小间隔时间
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    // Refresh CPUs again to get actual value.
    system.refresh_cpu_usage();
    let cpu_num = system.cpus().len();

    // TODO: 之后从配置文件中读取以何种方式得到关注程序的pid
    // 现在先抽前20个
    let target_pids = get_top_20_cpu_process().await;
    let chunks_size = (target_pids.len() + cpu_num - 1) / cpu_num;
    assert!(chunks_size * cpu_num >= target_pids.len());
    // REVIEW: 这里确实有点抽象，我想要能move到spawn里面，有更高效的方法吗
    let target_pid_vecs = target_pids
        .chunks(chunks_size)
        .map(|pids| pids.iter().map(|&pid| pid).collect::<Vec<sysinfo::Pid>>())
        .collect::<Vec<Vec<sysinfo::Pid>>>();
    for pids in target_pid_vecs {
        // debug!("pids: {:?}", pids);
        // 之后这一层会抽象到函数里
        tokio::spawn(async move {
            let sys = System::new();
            let mut interval_timer = time::interval(Duration::from_secs(5));
            loop {
                interval_timer.tick().await;
                for pid in &pids {
                    match get_process_info(&sys, *pid).await {
                        Ok(info) => {
                            println!("{:?}", info);
                        }
                        Err(e) => {
                            eprintln!("获取进程信息失败: {}", e);
                        }
                    }
                }
            }
        });
    }

    // 让主线程保持运行
    tokio::signal::ctrl_c()
        .await
        .expect("监听ctrl-c退出信号失败");
    println!("程序终端，退出");
}

/// 根据Pid获取信息
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

/// 得到cpu占用前20进程的pid
async fn get_top_20_cpu_process() -> Vec<sysinfo::Pid> {
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        ProcessRefreshKind::new().with_cpu(),
    );
    // 文档里说要开始获得准确的 CPU 使用率，进程需要刷新两次。
    // 这里的MINIMUM_CPU_UPDATE_INTERVAL是刷新的最小间隔时间
    tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
    let mut heap = PriorityQueue::with_capacity(20);
    sys.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        ProcessRefreshKind::new().with_cpu(),
    );

    // 最小堆排序
    sys.processes().iter().for_each(|(pid, process)| {
        let cpu_usage = process.cpu_usage();
        // 相信它不会给出一个逆天cpu使用率。当然多核的情况之后再考虑
        assert!(cpu_usage >= 0.0 && cpu_usage <= 1.0);
        let cpu_usage = (cpu_usage * 100.0) as u8;
        if heap.len() < 20 {
            heap.push(pid, Reverse(cpu_usage));
        } else if let Some((_, min_usage)) = heap.peek() {
            if cpu_usage > min_usage.0 {
                heap.pop();
                heap.push(pid, Reverse(cpu_usage));
            }
        }
    });

    heap.into_iter().map(|(&pid, _)| pid).collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_top_20_cpu_process() {
        let process = get_top_20_cpu_process().await;
        println!("{:?}", process);
    }

    #[test]
    fn test_priority_queue_resize() {
        let mut queue = PriorityQueue::with_capacity(5);

        queue.push(1, Reverse(10));
        queue.push(2, Reverse(20));
        queue.push(3, Reverse(30));
        queue.push(4, Reverse(40));
        queue.push(5, Reverse(50));

        assert_eq!(queue.capacity(), 5);
        queue.push(6, Reverse(70));

        // 超过初始化的容量会自动扩容
        assert_eq!(queue.into_iter().count(), 6);
    }
}
