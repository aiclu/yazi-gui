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
    eprintln!("[pipe] client_id = {}", client_id);

    let mut child = Command::new("yazi")
        .args([
            "--client-id",
            &client_id,
            "--local-events",
            "cd,hover,gui-files",
        ])
        .env("YAZI_CONFIG_HOME", "D:\\Projects\\gui_for_yazi\\assets\\yazi")
        .current_dir("D:\\Projects\\gui_for_yazi")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let h_out = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(l) => println!("[STDOUT] {}", l),
                Err(_) => break,
            }
        }
        println!("[STDOUT EOF]");
    });

    let h_err = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            match line {
                Ok(l) => println!("[STDERR] {}", l),
                Err(_) => break,
            }
        }
        println!("[STDERR EOF]");
    });

    std::thread::sleep(Duration::from_secs(3));

    let out = Command::new("ya")
        .args(["emit-to", &client_id, "cd", "D:\\"])
        .output();
    match out {
        Ok(o) => eprintln!(
            "[pipe] ya emit-to -> exit={:?} stdout={:?} stderr={:?}",
            o.status.code(),
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
        Err(e) => eprintln!("[pipe] ya spawn error: {}", e),
    }

    std::thread::sleep(Duration::from_secs(3));

    let _ = child.kill();
    let _ = h_out.join();
    let _ = h_err.join();
    eprintln!("[pipe] done");
}
