use anyhow::Result;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// 一个文件条目（来自 yazi 插件发布的文件列表）。
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub mtime: f64,
}

/// 一个从 yazi `--local-events` 流解析出来的事件。
#[derive(Debug, Clone)]
pub enum YaziEvent {
    Cd {
        url: Option<String>,
    },
    /// 完整目录列表（由 gui-files 插件通过 ps.pub 发布）。
    GuiFiles {
        cwd: String,
        files: Vec<FileEntry>,
    },
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
            "cd" if tab == 1 => Some(YaziEvent::Cd { url }),
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
                Some(YaziEvent::GuiFiles { cwd, files })
            }
            _ => None,
        }
    }
}

fn parse_file_entry(v: &serde_json::Value) -> Option<FileEntry> {
    let name = v.get("name")?.as_str()?.to_string();
    let is_dir = v.get("is_dir").and_then(|x| x.as_bool()).unwrap_or(false);
    let size = v
        .get("size")
        .and_then(|x| x.as_u64())
        .or_else(|| v.get("size").and_then(|x| x.as_f64()).map(|f| f as u64))
        .unwrap_or(0);
    let mtime = v.get("mtime").and_then(|x| x.as_f64()).unwrap_or(0.0);
    Some(FileEntry {
        name,
        is_dir,
        size,
        mtime,
    })
}

#[cfg(debug_assertions)]
fn runtime_config_home() -> Result<PathBuf> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("yazi");
    ensure_directory(path)
}

#[cfg(not(debug_assertions))]
fn runtime_config_home() -> Result<PathBuf> {
    let executable = std::env::current_exe()?;
    let parent = executable
        .parent()
        .ok_or_else(|| anyhow::anyhow!("release executable has no parent directory"))?;
    ensure_directory(parent.join("assets").join("yazi"))
}

fn ensure_directory(path: PathBuf) -> Result<PathBuf> {
    if path.is_dir() {
        Ok(path)
    } else {
        Err(anyhow::anyhow!(
            "yazi config directory not found: {}",
            path.display()
        ))
    }
}

#[cfg(feature = "bundled-yazi")]
fn yazi_binary() -> Result<PathBuf> {
    bundled_binary("yazi.exe")
}

#[cfg(not(feature = "bundled-yazi"))]
fn yazi_binary() -> Result<PathBuf> {
    Ok(PathBuf::from("yazi"))
}

#[cfg(feature = "bundled-yazi")]
fn ya_binary() -> Result<PathBuf> {
    bundled_binary("ya.exe")
}

#[cfg(not(feature = "bundled-yazi"))]
fn ya_binary() -> Result<PathBuf> {
    Ok(PathBuf::from("ya"))
}

#[cfg(feature = "bundled-yazi")]
fn bundled_binary(name: &str) -> Result<PathBuf> {
    let executable = std::env::current_exe()?;
    let parent = executable
        .parent()
        .ok_or_else(|| anyhow::anyhow!("bundled executable has no parent directory"))?;
    let path = parent.join(name);
    if path.is_file() {
        Ok(path)
    } else {
        Err(anyhow::anyhow!(
            "bundled yazi binary not found: {}",
            path.display()
        ))
    }
}

fn hide_console(command: &mut Command) {
    #[cfg(windows)]
    command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);

    #[cfg(not(windows))]
    let _ = command;
}

/// 一个共享 Yazi Session：负责启动后端、接收事件和发送目录动作。
pub struct YaziSession {
    client_id: String,
    child: Option<Child>,
}

#[derive(Clone)]
pub(crate) struct YaziSessionHandle {
    client_id: String,
}

impl YaziSessionHandle {
    pub(crate) fn change_directory(&self, cwd: String) -> Result<String> {
        YaziSession::send_with_client_id(&self.client_id, &[String::from("cd"), cwd])
    }
}

impl YaziSession {
    /// 启动 yazi（隐藏、无 TTY），返回客户端句柄 + 事件接收端。
    pub fn spawn(cwd: &Path) -> Result<(Self, UnboundedReceiver<YaziEvent>)> {
        let client_id = format!(
            "{}",
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
        );
        let config_home = runtime_config_home()?;
        let yazi = yazi_binary()?;

        let mut command = Command::new(yazi);
        command
            .args([
                "--client-id",
                client_id.as_str(),
                "--local-events",
                "cd,gui-files",
            ])
            .env("YAZI_CONFIG_HOME", config_home)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        hide_console(&mut command);
        let mut child = command.spawn()?;

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

    pub fn change_directory(&self, cwd: &str) -> Result<String> {
        Self::send_with_client_id(&self.client_id, &[String::from("cd"), cwd.to_string()])
    }

    pub(crate) fn handle(&self) -> YaziSessionHandle {
        YaziSessionHandle {
            client_id: self.client_id.clone(),
        }
    }

    fn send_with_client_id(client_id: &str, action: &[String]) -> Result<String> {
        let mut args = vec!["emit-to", client_id];
        args.extend(action.iter().map(String::as_str));

        let ya = ya_binary()?;
        let mut command = Command::new(ya);
        command.args(&args).stdin(Stdio::null());
        hide_console(&mut command);
        let out = command.output()?;

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

impl Drop for YaziSession {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::YaziEvent;

    #[test]
    fn parses_primary_cd_event_without_backend_tab_detail() {
        let event = YaziEvent::parse(r#"cd,gui,ya,{"tab":1,"url":"D:/Projects"}"#);
        match event {
            Some(YaziEvent::Cd { url: Some(url) }) => assert_eq!(url, "D:/Projects"),
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn ignores_non_primary_and_unknown_events() {
        assert!(YaziEvent::parse(r#"cd,gui,ya,{"tab":2,"url":"D:/Other"}"#).is_none());
        assert!(YaziEvent::parse(r#"hover,gui,ya,{"tab":1,"url":"D:/Other"}"#).is_none());
    }

    #[test]
    fn parses_only_consumed_gui_files_fields() {
        let event = YaziEvent::parse(
            r#"gui-files,gui,plugin,{"cwd":"D:/Projects","files":[{"name":"a.txt","is_dir":false,"is_hidden":true,"size":12,"mtime":4.5}],"hovered":"a.txt"}"#,
        );
        match event {
            Some(YaziEvent::GuiFiles { cwd, files }) => {
                assert_eq!(cwd, "D:/Projects");
                assert_eq!(files.len(), 1);
                assert_eq!(files[0].name, "a.txt");
                assert!(!files[0].is_dir);
                assert_eq!(files[0].size, 12);
                assert_eq!(files[0].mtime, 4.5);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
