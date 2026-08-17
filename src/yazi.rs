use anyhow::Result;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

/// 一个文件条目（来自 yazi 插件发布的文件列表）。
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub is_hidden: bool,
    pub size: u64,
    pub mtime: f64,
}

/// 一个从 yazi `--local-events` 流解析出来的事件。
#[derive(Debug, Clone)]
pub enum YaziEvent {
    Cd { tab: usize, url: Option<String> },
    Hover { tab: usize, url: Option<String> },
    /// 完整目录列表（由 gui-files 插件通过 ps.pub 发布）。
    GuiFiles {
        cwd: String,
        files: Vec<FileEntry>,
        hovered: Option<String>,
    },
    Other { kind: String, body: serde_json::Value },
}

impl YaziEvent {
    /// 解析一行事件。格式：`kind,receiver,sender,{json}`
    pub fn parse(line: &str) -> Option<Self> {
        let mut parts = line.splitn(4, ',');
        let kind = parts.next()?;
        let _receiver = parts.next()?;
        let _sender = parts.next()?;
        let body_str = parts.next()?;
        let body: serde_json::Value = serde_json::from_str(body_str).ok()?;

        let tab = body.get("tab").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let url = body.get("url").and_then(|v| v.as_str()).map(str::to_owned);

        match kind {
            "cd" => Some(YaziEvent::Cd { tab, url }),
            "hover" => Some(YaziEvent::Hover { tab, url }),
            "gui-files" => {
                let cwd = body
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let files = body
                    .get("files")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(parse_file_entry).collect())
                    .unwrap_or_default();
                let hovered = body.get("hovered").and_then(|v| v.as_str()).map(str::to_owned);
                Some(YaziEvent::GuiFiles { cwd, files, hovered })
            }
            _ => Some(YaziEvent::Other {
                kind: kind.to_string(),
                body,
            }),
        }
    }
}

fn parse_file_entry(v: &serde_json::Value) -> Option<FileEntry> {
    let name = v.get("name")?.as_str()?.to_string();
    let is_dir = v.get("is_dir").and_then(|x| x.as_bool()).unwrap_or(false);
    let is_hidden = v
        .get("is_hidden")
        .and_then(|x| x.as_bool())
        .unwrap_or(false);
    let size = v
        .get("size")
        .and_then(|x| x.as_u64())
        .or_else(|| v.get("size").and_then(|x| x.as_f64()).map(|f| f as u64))
        .unwrap_or(0);
    let mtime = v.get("mtime").and_then(|x| x.as_f64()).unwrap_or(0.0);
    Some(FileEntry {
        name,
        is_dir,
        is_hidden,
        size,
        mtime,
    })
}

/// 一个 yazi 后端进程的客户端句柄：负责启动进程、接收事件、发送动作。
pub struct YaziClient {
    client_id: String,
    child: Option<Child>,
}

impl YaziClient {
    /// 启动 yazi（隐藏、无 TTY），返回客户端句柄 + 事件接收端。
    pub fn spawn(cwd: &Path) -> Result<(Self, UnboundedReceiver<YaziEvent>)> {
        let client_id = format!(
            "{}",
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
        );
        let config_home = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("yazi");

        let mut child = Command::new("yazi")
            .args([
                "--client-id",
                client_id.as_str(),
                "--local-events",
                "cd,hover,gui-files",
            ])
            .env("YAZI_CONFIG_HOME", config_home)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdout = child.stdout.take().expect("stdout must be piped");
        let (tx, rx) = unbounded_channel::<YaziEvent>();

        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(l) => {
                        if let Some(ev) = YaziEvent::parse(&l) {
                            if tx.send(ev).is_err() {
                                break;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok((
            Self {
                client_id,
                child: Some(child),
            },
            rx,
        ))
    }

    /// 通过 `ya emit-to <client_id> <action...>` 向 yazi 发送一个动作。
    pub fn send(&self, action: &[&str]) -> Result<String> {
        let mut args = vec!["emit-to", self.client_id.as_str()];
        args.extend_from_slice(action);

        let out = Command::new("ya")
            .args(&args)
            .stdin(Stdio::null())
            .output()?;

        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();

        if out.status.success() {
            Ok(stdout)
        } else {
            Err(anyhow::anyhow!(
                "ya {} failed: {}",
                args.join(" "),
                if stderr.is_empty() { stdout } else { stderr }
            ))
        }
    }
}

impl Drop for YaziClient {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
        }
    }
}
