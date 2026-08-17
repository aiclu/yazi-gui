use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() {
    let client_id = format!(
        "{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let cwd = "D:\\Projects\\gui_for_yazi";
    let tmp = "D:\\Projects\\gui_for_yazi\\_probe_delete_test.txt";
    std::fs::write(tmp, "delete me").unwrap();
    eprintln!("[ops] created temp file");

    let mut child = Command::new("yazi")
        .args([
            "--client-id",
            &client_id,
            "--local-events",
            "cd,hover,gui-files",
        ])
        .env("YAZI_CONFIG_HOME", "D:\\Projects\\gui_for_yazi\\assets\\yazi")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let stdout = child.stdout.take().unwrap();
    let h = std::thread::spawn(move || {
        let mut last = String::new();
        for line in BufReader::new(stdout).lines() {
            if let Ok(l) = line {
                if l.contains("gui-files") {
                    last = l;
                }
            }
        }
        println!("[LAST gui-files] {}", last);
    });

    std::thread::sleep(Duration::from_secs(3));

    // 1) Rust 直接删除文件
    std::fs::remove_file(tmp).unwrap();
    eprintln!("[ops] fs 删除完成");

    // 2) 发 cd 到同一目录，触发 yazi 刷新
    let out = Command::new("ya")
        .args(["emit-to", &client_id, "cd", cwd])
        .output();
    match out {
        Ok(o) => eprintln!(
            "[ops] cd refresh -> exit={:?} stderr={:?}",
            o.status.code(),
            String::from_utf8_lossy(&o.stderr)
        ),
        Err(e) => eprintln!("[ops] cd error: {}", e),
    }

    std::thread::sleep(Duration::from_secs(3));

    let _ = child.kill();
    let _ = h.join();
    eprintln!("[ops] done");
}
