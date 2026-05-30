//! 系统托盘管理

use crate::{Error, Result};
use serde_json::Value;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;
use tracing::warn;
use tray_icon::{
    icon::Icon,
    menu::{menu_event_receiver, Menu, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};

#[cfg(windows)]
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};

#[cfg(windows)]
use windows::Win32::System::Threading::GetCurrentThreadId;

#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PeekMessageW, PostThreadMessageW, TranslateMessage, MSG,
    PM_NOREMOVE, WM_APP, WM_QUIT,
};

const TRAY_ICON_SIZE: u32 = 16;
const TRAY_COMMAND_MESSAGE: u32 = WM_APP + 0x351;

/// 托盘菜单动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    OpenAdmin,
    ToggleService,
    CopyStreamUrl,
    OpenLogs,
    Exit,
}

/// 托盘菜单项描述。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrayMenuEntry {
    pub action: TrayAction,
    pub label: &'static str,
    pub enabled: bool,
}

enum TrayThreadCommand {
    UpdateStatus(bool),
}

struct TrayRuntime {
    command_sender: Sender<TrayThreadCommand>,
    thread: Option<JoinHandle<()>>,
    #[cfg(windows)]
    thread_id: u32,
}

/// 系统托盘管理器
pub struct TrayManager {
    admin_port: u16,
    runtime: Option<TrayRuntime>,
}

impl TrayManager {
    pub fn new(admin_port: u16) -> Result<Self> {
        Ok(Self {
            admin_port,
            runtime: None,
        })
    }

    /// 创建托盘菜单项
    pub fn create_menu(&mut self) -> Result<()> {
        if self.runtime.is_some() {
            return Ok(());
        }

        let (command_sender, command_receiver) = mpsc::channel();
        let (ready_sender, ready_receiver) = mpsc::channel();
        let admin_port = self.admin_port;

        let thread = std::thread::Builder::new()
            .name("webkeylayer-tray".to_string())
            .spawn(move || {
                #[cfg(windows)]
                {
                    let thread_id = unsafe { GetCurrentThreadId() };
                    unsafe {
                        let mut message = MSG::default();
                        let _ = PeekMessageW(&mut message, HWND(0), 0, 0, PM_NOREMOVE);
                    }

                    run_tray_thread(admin_port, command_receiver, ready_sender, thread_id);
                }

                #[cfg(not(windows))]
                {
                    let _ = command_receiver;
                    let _ = ready_sender.send(Err(
                        "system tray is currently implemented only on Windows".to_string(),
                    ));
                }
            })
            .map_err(|error| Error::UI(format!("failed to spawn tray thread: {error}")))?;

        let thread_id = match ready_receiver.recv() {
            Ok(Ok(thread_id)) => thread_id,
            Ok(Err(error)) => {
                let _ = thread.join();
                return Err(Error::UI(error));
            }
            Err(error) => {
                let _ = thread.join();
                return Err(Error::UI(format!(
                    "failed to receive tray startup result: {error}"
                )));
            }
        };

        self.runtime = Some(TrayRuntime {
            command_sender,
            thread: Some(thread),
            #[cfg(windows)]
            thread_id,
        });
        Ok(())
    }

    /// 更新托盘图标状态
    pub fn update_status(&self, _running: bool) -> Result<()> {
        if let Some(runtime) = &self.runtime {
            runtime
                .command_sender
                .send(TrayThreadCommand::UpdateStatus(_running))
                .map_err(|error| Error::UI(format!("failed to send tray status: {error}")))?;

            #[cfg(windows)]
            unsafe {
                let posted = PostThreadMessageW(
                    runtime.thread_id,
                    TRAY_COMMAND_MESSAGE,
                    WPARAM(0),
                    LPARAM(0),
                );
                if !posted.as_bool() {
                    return Err(Error::UI("failed to wake tray thread".to_string()));
                }
            }
        }
        Ok(())
    }

    /// 停止托盘线程。
    pub fn stop(&mut self) -> Result<()> {
        let Some(mut runtime) = self.runtime.take() else {
            return Ok(());
        };

        #[cfg(windows)]
        unsafe {
            let posted = PostThreadMessageW(runtime.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            if !posted.as_bool() {
                warn!(
                    thread_id = runtime.thread_id,
                    "failed to post tray quit message"
                );
            }
        }

        if let Some(thread) = runtime.thread.take() {
            thread
                .join()
                .map_err(|_| Error::UI("tray thread panicked".to_string()))?;
        }

        Ok(())
    }
}

impl Drop for TrayManager {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            warn!(%error, "failed to stop tray manager");
        }
    }
}

/// 构造当前状态下的托盘菜单描述。
pub fn tray_menu_entries(service_running: bool) -> Vec<TrayMenuEntry> {
    vec![
        TrayMenuEntry {
            action: TrayAction::OpenAdmin,
            label: "打开管理面板",
            enabled: true,
        },
        TrayMenuEntry {
            action: TrayAction::ToggleService,
            label: if service_running {
                "停止服务"
            } else {
                "启动服务"
            },
            enabled: true,
        },
        TrayMenuEntry {
            action: TrayAction::CopyStreamUrl,
            label: "复制连接地址",
            enabled: true,
        },
        TrayMenuEntry {
            action: TrayAction::OpenLogs,
            label: "查看日志",
            enabled: true,
        },
        TrayMenuEntry {
            action: TrayAction::Exit,
            label: "退出程序",
            enabled: true,
        },
    ]
}

#[cfg(windows)]
fn run_tray_thread(
    admin_port: u16,
    command_receiver: Receiver<TrayThreadCommand>,
    ready_sender: Sender<std::result::Result<u32, String>>,
    thread_id: u32,
) {
    let mut service_running = fetch_service_running(admin_port).unwrap_or(true);
    let tray_menu = Menu::new();
    let open_admin_item = MenuItem::new("打开管理面板", true, None);
    let service_item = MenuItem::new(service_label(service_running), true, None);
    let copy_stream_url_item = MenuItem::new("复制连接地址", true, None);
    let open_logs_item = MenuItem::new("查看日志", true, None);
    let exit_item = MenuItem::new("退出程序", true, None);

    tray_menu.append_items(&[
        &open_admin_item,
        &service_item,
        &copy_stream_url_item,
        &open_logs_item,
        &PredefinedMenuItem::separator(),
        &exit_item,
    ]);

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("WebKeyLayer")
        .with_icon(match tray_icon_for_status(service_running) {
            Ok(icon) => icon,
            Err(error) => {
                let _ = ready_sender.send(Err(error.to_string()));
                return;
            }
        })
        .build()
        .map_err(|error| Error::UI(format!("failed to create tray icon: {error}")));

    let mut tray_icon = match tray_icon {
        Ok(tray_icon) => tray_icon,
        Err(error) => {
            let _ = ready_sender.send(Err(error.to_string()));
            return;
        }
    };

    let menu_channel = menu_event_receiver();
    let _ = ready_sender.send(Ok(thread_id));

    unsafe {
        let mut message = MSG::default();
        loop {
            let result = GetMessageW(&mut message, HWND(0), 0, 0).0;
            if result == -1 {
                warn!("tray message loop failed");
                return;
            }
            if result == 0 {
                break;
            }

            if message.message == TRAY_COMMAND_MESSAGE {
                if let Err(error) = drain_tray_commands(
                    &command_receiver,
                    &mut tray_icon,
                    &service_item,
                    &mut service_running,
                ) {
                    warn!(%error, "failed to apply tray command");
                }
                continue;
            }

            TranslateMessage(&message);
            DispatchMessageW(&message);

            while let Ok(event) = menu_channel.try_recv() {
                if event.id == open_admin_item.id() {
                    open_url(&admin_browser_url(admin_port));
                } else if event.id == service_item.id() {
                    service_running = toggle_service(admin_port, service_running);
                    if let Err(error) =
                        apply_tray_status(&mut tray_icon, &service_item, service_running)
                    {
                        warn!(%error, "failed to apply tray service status");
                    }
                } else if event.id == copy_stream_url_item.id() {
                    if let Some(url) = fetch_stream_url(admin_port) {
                        copy_text_to_clipboard(&url);
                    }
                } else if event.id == open_logs_item.id() {
                    open_url(&format!("{}#logs", admin_url(admin_port)));
                } else if event.id == exit_item.id() {
                    std::process::exit(0);
                }
            }
        }
    }
}

fn drain_tray_commands(
    command_receiver: &Receiver<TrayThreadCommand>,
    tray_icon: &mut TrayIcon,
    service_item: &MenuItem,
    service_running: &mut bool,
) -> Result<()> {
    while let Ok(command) = command_receiver.try_recv() {
        match command {
            TrayThreadCommand::UpdateStatus(running) => {
                *service_running = running;
                apply_tray_status(tray_icon, service_item, running)?;
            }
        }
    }

    Ok(())
}

fn apply_tray_status(
    tray_icon: &mut TrayIcon,
    service_item: &MenuItem,
    service_running: bool,
) -> Result<()> {
    service_item.set_text(service_label(service_running));
    tray_icon
        .set_icon(Some(tray_icon_for_status(service_running)?))
        .map_err(|error| Error::UI(format!("failed to update tray icon: {error}")))?;
    tray_icon
        .set_tooltip(Some(if service_running {
            "WebKeyLayer 正在运行"
        } else {
            "WebKeyLayer 已停止"
        }))
        .map_err(|error| Error::UI(format!("failed to update tray tooltip: {error}")))
}

fn service_label(service_running: bool) -> &'static str {
    if service_running {
        "停止服务"
    } else {
        "启动服务"
    }
}

fn toggle_service(admin_port: u16, service_running: bool) -> bool {
    let endpoint = if service_running {
        "/api/service/stop"
    } else {
        "/api/service/start"
    };

    match send_admin_request(admin_port, "POST", endpoint, "") {
        Ok(response) => service_running_from_response(&response).unwrap_or(!service_running),
        Err(error) => {
            warn!(%error, endpoint, "failed to toggle input service from tray");
            service_running
        }
    }
}

fn fetch_service_running(admin_port: u16) -> Option<bool> {
    send_admin_request(admin_port, "GET", "/api/status", "")
        .ok()
        .and_then(|response| service_running_from_response(&response))
}

fn fetch_stream_url(admin_port: u16) -> Option<String> {
    send_admin_request(admin_port, "GET", "/api/network/ip", "")
        .ok()
        .and_then(|response| {
            response
                .get("data")
                .and_then(|data| data.get("stream_url"))
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
}

fn service_running_from_response(response: &Value) -> Option<bool> {
    response
        .get("data")
        .and_then(|data| data.get("service_running"))
        .and_then(Value::as_bool)
}

fn send_admin_request(admin_port: u16, method: &str, path: &str, body: &str) -> Result<Value> {
    let mut stream = TcpStream::connect(("127.0.0.1", admin_port))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;

    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{admin_port}\r\nConnection: close\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes())?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    let (_, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| Error::UI("invalid admin http response".to_string()))?;
    serde_json::from_str(body).map_err(Error::from)
}

fn admin_url(admin_port: u16) -> String {
    admin_browser_url(admin_port)
}

fn admin_browser_url(admin_port: u16) -> String {
    format!("http://127.0.0.1:{admin_port}/admin")
}

fn open_url(url: &str) {
    if let Err(error) = std::process::Command::new("explorer").arg(url).spawn() {
        warn!(%error, %url, "failed to open url");
    }
}

fn copy_text_to_clipboard(text: &str) {
    if let Err(error) = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Set-Clipboard -Value $args[0]",
            text,
        ])
        .spawn()
    {
        warn!(%error, "failed to copy text to clipboard");
    }
}

fn tray_icon_for_status(service_running: bool) -> Result<Icon> {
    let color = if service_running {
        [84, 214, 168, 255]
    } else {
        [130, 138, 148, 255]
    };
    let rgba = create_tray_icon_rgba(color);
    Icon::from_rgba(rgba, TRAY_ICON_SIZE, TRAY_ICON_SIZE)
        .map_err(|error| Error::UI(format!("failed to create tray icon image: {error}")))
}

fn create_tray_icon_rgba(color: [u8; 4]) -> Vec<u8> {
    let mut rgba = vec![0; (TRAY_ICON_SIZE * TRAY_ICON_SIZE * 4) as usize];
    let radius = TRAY_ICON_SIZE as f32 / 2.0 - 1.0;
    let center = TRAY_ICON_SIZE as f32 / 2.0 - 0.5;

    for y in 0..TRAY_ICON_SIZE {
        for x in 0..TRAY_ICON_SIZE {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            if (dx * dx + dy * dy).sqrt() <= radius {
                let index = ((y * TRAY_ICON_SIZE + x) * 4) as usize;
                rgba[index..index + 4].copy_from_slice(&color);
            }
        }
    }

    rgba
}

#[cfg(test)]
mod tests {
    use super::{admin_browser_url, tray_menu_entries, TrayAction};

    #[test]
    fn tray_menu_contains_required_actions() {
        let actions = tray_menu_entries(true)
            .into_iter()
            .map(|entry| entry.action)
            .collect::<Vec<_>>();

        assert_eq!(
            actions,
            vec![
                TrayAction::OpenAdmin,
                TrayAction::ToggleService,
                TrayAction::CopyStreamUrl,
                TrayAction::OpenLogs,
                TrayAction::Exit,
            ]
        );
    }

    #[test]
    fn tray_menu_toggle_label_reflects_service_state() {
        let running = tray_menu_entries(true);
        let stopped = tray_menu_entries(false);

        assert_eq!(running[1].label, "停止服务");
        assert_eq!(stopped[1].label, "启动服务");
    }

    #[test]
    fn admin_panel_opens_through_default_browser_url() {
        assert_eq!(
            admin_browser_url(8888),
            "http://127.0.0.1:8888/admin".to_string()
        );
    }
}
