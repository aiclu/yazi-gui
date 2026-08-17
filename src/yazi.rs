use anyhow::Result;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{SystemTime, UNIX_EPOCH};

/// 一个从 yazi `--local-events` 流解析出来的事件。
#[derive(Debug, Clone)]
pub enum YaziEvent {
    /// 目录切换。url 是新的工作目录。
    Cd { tab: usize, url: Option<String> },
    /// 光标悬停到某个文件。url 为 None 表示没有悬停。
    Hover { tab: usize, url: Option<String> },
    /// 其他事件（rename/trash/delete/move/bulk 等），保留原始 kind 和 body。
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
        let url = body
            .get("url")
            .and_then(|v| v.as_str())
            .map(str::to_owned);

        match kind {
            "cd" => Some(YaziEvent::Cd { tab, url }),
            "hover" => Some(YaziEvent::Hover { tab, url }),
            _ => Some(YaziEvent::Other {
                kind: kind.to_string(),
                body,
            }),
        }
    }

    pub fn label(&self) -> String {
        match self {
            YaziEvent::Cd { url, .. } => format!("cd  -> {:?}", url),
            YaziEvent::Hover { url, .. } => format!("hover -> {:?}", url),
            YaziEvent::Other { kind, .. } => format!("{}", kind),
        }
    }
}

/// 一个 yazi 后端进程的客户端句柄：负责启动进程、接收事件、发送动作。
pub struct YaziClient {
    client_id: String,
    child: Option<Child>,
}

impl YaziClient {
    /// 启动 yazi（隐藏、无 TTY），返回客户端句柄 + 事件接收端。
    pub fn spawn(cwd: &Path) -> Result<(Self, Receiver<YaziEvent>)> {
        let client_id = format!(
            "{}",
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
        );

        let mut child = Command::new("yazi")
            .args([
                "--client-id",
                client_id.as_str(),
                "--local-events",
                "cd,hover,rename,trash,delete,move,bulk",
            ])
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdout = child.stdout.take().expect("stdout must be piped");
        let (tx, rx) = channel::<YaziEvent>();

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
