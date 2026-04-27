use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;
use base64::{Engine as _, engine::general_purpose};

fn obfuscate(data: &str) -> String {
    general_purpose::STANDARD.encode(data)
}

fn deobfuscate(data: &str) -> Result<String, String> {
    let bytes = general_purpose::STANDARD.decode(data)
        .map_err(|e| format!("解码失败: {}", e))?;
    String::from_utf8(bytes).map_err(|e| format!("转换字符串失败: {}", e))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: u64,
    pub title: String,
    pub app_name: String,
    pub bundle_id: Option<String>,
    pub pid: i32,
}

pub struct AppState {
    accounts: Mutex<Vec<Account>>,
    data_dir: Mutex<Option<PathBuf>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            accounts: Mutex::new(Vec::new()),
            data_dir: Mutex::new(None),
        }
    }

    fn get_data_file(&self) -> PathBuf {
        let dir = self.data_dir.lock().unwrap();
        dir.as_ref().unwrap().join("accounts.json")
    }

    fn load(&self) -> Result<(), String> {
        let file = self.get_data_file();
        if file.exists() {
            let content = fs::read_to_string(&file)
                .map_err(|e| format!("读取文件失败: {}", e))?;
            let mut accounts: Vec<Account> = serde_json::from_str(&content)
                .map_err(|e| format!("解析文件失败: {}", e))?;
            
            // 解密密码
            for acc in accounts.iter_mut() {
                if let Ok(decrypted) = deobfuscate(&acc.password) {
                    acc.password = decrypted;
                }
            }
            
            *self.accounts.lock().unwrap() = accounts;
        }
        Ok(())
    }

    fn save(&self) -> Result<(), String> {
        let file = self.get_data_file();
        let mut accounts = self.accounts.lock().unwrap().clone();
        
        // 加密密码
        for acc in accounts.iter_mut() {
            acc.password = obfuscate(&acc.password);
        }
        
        let content = serde_json::to_string_pretty(&accounts)
            .map_err(|e| format!("序列化失败: {}", e))?;
        fs::write(file, content)
            .map_err(|e| format!("写入文件失败: {}", e))?;
        Ok(())
    }
}

fn init_state(app: &AppHandle) -> Result<(), String> {
    let state: State<AppState> = app.state();
    let app_dir = app.path().app_data_dir()
        .map_err(|e| format!("获取应用目录失败: {}", e))?;
    fs::create_dir_all(&app_dir)
        .map_err(|e| format!("创建目录失败: {}", e))?;
    *state.data_dir.lock().unwrap() = Some(app_dir);
    state.load()
}

#[tauri::command]
fn get_accounts(state: State<AppState>) -> Result<Vec<Account>, String> {
    let accounts = state.accounts.lock().unwrap();
    Ok(accounts.clone())
}

#[tauri::command]
fn add_account(name: String, password: String, state: State<AppState>) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("账户名称不能为空".to_string());
    }

    if password.is_empty() {
        return Err("密码不能为空".to_string());
    }

    let account = Account {
        id: Uuid::new_v4().to_string(),
        name: name.trim().to_string(),
        password,
    };

    state.accounts.lock().unwrap().push(account);
    state.save()
}

#[tauri::command]
fn delete_account(id: String, state: State<AppState>) -> Result<(), String> {
    let mut accounts = state.accounts.lock().unwrap();
    accounts.retain(|a| a.id != id);
    drop(accounts);
    state.save()
}

#[tauri::command(rename_all = "snake_case")]
fn auto_fill_password(account_id: String, pid: i32, state: State<AppState>) -> Result<(), String> {
    eprintln!("收到 auto_fill_password 请求：account_id={}, pid={}", account_id, pid);

    let accounts = state.accounts.lock().unwrap();
    let account = accounts
        .iter()
        .find(|a| a.id == account_id)
        .ok_or("账户不存在")?;

    eprintln!("找到账户：{}, 准备输入密码", account.name);
    let password = account.password.clone();

    drop(accounts);

    eprintln!("调用 platform::auto_fill_password_by_pid");
    platform::auto_fill_password_by_pid(pid, &password)
}

#[tauri::command]
fn get_windows() -> Result<Vec<WindowInfo>, String> {
    platform::get_windows()
}

#[tauri::command]
fn get_foreground_window() -> Result<WindowInfo, String> {
    platform::get_foreground_window()
}

#[tauri::command]
fn check_accessibility_permission() -> Result<bool, String> {
    platform::check_accessibility_permission()
}

#[tauri::command]
fn open_accessibility_settings() -> Result<(), String> {
    platform::open_accessibility_settings()
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use std::process::Command;

    pub fn open_accessibility_settings() -> Result<(), String> {
        Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn()
            .map_err(|e| format!("无法打开设置: {}", e))?;
        Ok(())
    }

    pub fn check_accessibility_permission() -> Result<bool, String> {
        let script = r#"
            tell application "System Events"
                return UI elements enabled
            end tell
        "#;
        
        let output = Command::new("osascript")
            .arg("-e")
            .arg(script)
            .output()
            .map_err(|e| format!("检查权限失败: {}", e))?;
        
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            if err.contains("privilege violation") || err.contains("not allowed") {
                return Ok(false);
            }
            return Err(format!("AppleScript 错误: {}", err));
        }
        
        let result = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
        Ok(result == "true")
    }

    pub fn get_windows() -> Result<Vec<WindowInfo>, String> {
        // 方案1：使用 Swift CGWindowListCopyWindowInfo（不需要辅助功能权限，获取真正有窗口的应用）
        match get_windows_cg() {
            Ok(windows) if !windows.is_empty() => return Ok(windows),
            Err(e) => eprintln!("CGWindowList 获取失败: {}", e),
            _ => {}
        }

        // 方案2：使用 lsappinfo（不需要辅助功能权限，但可能包含没有窗口的进程）
        match get_windows_lsappinfo() {
            Ok(windows) if !windows.is_empty() => return Ok(windows),
            Err(e) => eprintln!("lsappinfo 获取失败: {}", e),
            _ => {}
        }

        // 方案3：使用 ps 命令（最后的备用）
        match get_windows_ps() {
            Ok(windows) if !windows.is_empty() => return Ok(windows),
            Err(e) => eprintln!("ps 获取失败: {}", e),
            _ => {}
        }

        Ok(Vec::new())
    }

    fn get_windows_cg() -> Result<Vec<WindowInfo>, String> {
        let output = Command::new("swift")
            .args(&["-e", r#"
import Cocoa
let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
if let windowList = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] {
    for window in windowList {
        if let ownerName = window[kCGWindowOwnerName as String] as? String {
            let windowName = window[kCGWindowName as String] as? String ?? ""
            let pid = window[kCGWindowOwnerPID as String] as? Int32 ?? 0
            if !["Control Center", "Window Server", "Dock"].contains(ownerName) {
                print("\(ownerName)|\(windowName)|\(pid)")
            }
        }
    }
}
"#])
            .output()
            .map_err(|e| format!("Swift 执行失败: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Swift 错误: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut windows = Vec::new();
        let mut seen_pids = std::collections::HashSet::new();
        let mut id_counter = 1u64;

        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() < 3 {
                continue;
            }

            let app_name = parts[0].to_string();
            let title = parts[1].to_string();
            let pid: i32 = parts[2].parse().unwrap_or(0);

            // 过滤系统应用
            let skip_list = [
                "Window Server", "Dock", "SystemUIServer", "Control Center",
                "loginwindow", "Finder", "Spotlight",
            ];
            if skip_list.contains(&app_name.as_str()) {
                continue;
            }

            if seen_pids.contains(&pid) {
                continue;
            }
            seen_pids.insert(pid);

            windows.push(WindowInfo {
                id: id_counter,
                app_name,
                title,
                bundle_id: None,
                pid,
            });
            id_counter += 1;
        }

        Ok(windows)
    }

    fn get_windows_lsappinfo() -> Result<Vec<WindowInfo>, String> {
        let output = Command::new("lsappinfo")
            .args(&["list", "-a"])
            .output()
            .map_err(|e| format!("lsappinfo 执行失败: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut windows = Vec::new();
        let mut id_counter = 1u64;
        let mut current_app: Option<String> = None;
        let mut current_bundle_id: Option<String> = None;
        let mut current_pid: i32 = 0;
        let mut current_type: Option<String> = None;
        let mut seen_bundle_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

        for line in stdout.lines() {
            let line = line.trim();

            // 解析应用名：如 1) "Google Chrome" ASN:0x0-0x1a01a:
            if line.contains(")") && line.contains("ASN:") {
                // 使用引号来提取应用名
                if let Some(first_quote) = line.find('"') {
                    if let Some(second_quote) = line[first_quote+1..].find('"') {
                        let app_name = line[first_quote+1..first_quote+1+second_quote].to_string();
                        current_app = Some(app_name);
                        current_bundle_id = None;
                        current_pid = 0;
                        current_type = None;
                    }
                }
            }
            // 解析 bundle ID：如 bundleID="com.google.Chrome"
            else if line.starts_with("bundleID=") {
                let bundle_id = line.trim_matches('"')
                    .trim_start_matches("bundleID=")
                    .trim();
                if !bundle_id.is_empty() && bundle_id != "[ NULL ]" && !bundle_id.starts_with("com.apple") {
                    current_bundle_id = Some(bundle_id.to_string());
                }
            }
            // 解析 PID 和 type：如 pid = 1234 type="Foreground" ...
            else if line.starts_with("pid") && line.contains("=") {
                if let Some(start) = line.find("=") {
                    let after_eq = &line[start+1..];
                    let pid_str = after_eq.trim().split_whitespace().next().unwrap_or("0");
                    current_pid = pid_str.parse().unwrap_or(0);
                    
                    // 解析 type
                    if let Some(type_start) = line.find("type=\"") {
                        let type_begin = type_start + 6;
                        if let Some(type_end) = line[type_begin..].find('"') {
                            let app_type = line[type_begin..type_begin+type_end].to_string();
                            current_type = Some(app_type);
                        }
                    }
                }
            }
            // 空行表示一个应用信息结束
            else if line.is_empty() {
                if let Some(app_name) = current_app.take() {
                    // 更严格的过滤：排除所有系统进程
                    let skip_list = [
                        "loginwindow", "SystemUIServer", "Dock", "Window Server",
                        "Control Center", "Finder", "universalaccessd",
                        "CoreServicesUIAgent", "WindowManager", "BackgroundTaskManagementAgent",
                        "ViewBridgeAuxiliary", "talagentd", "cfprefsd", "containermanagerd",
                        "trustd", "securityd", "kernel_task", "launchd",
                        "WindowServer", "coreauthd", "configd", "powerd", "diskarbitrationd",
                        "fseventsd", "logd", "notifyd", "opendirectoryd", "syslogd",
                        "mdworker", "mdworker_shared", "syspolicyd",
                        "coreservicesd", "distnoted", "gpsd", "wifid", "bluetoothd",
                        "sharingd", "airplayd", "appleeventsd", "System Events", "Spotlight",
                    ];
                    // 额外的过滤：排除以 com.apple 开头的 bundle ID
                    // 排除包含空格的系统进程名（通常是系统应用）
                    // 排除以 d 结尾的守护进程
                    let is_system_process = app_name.starts_with("com.")
                        || app_name.contains(".framework")
                        || app_name.contains("/System/")
                        || (app_name.ends_with('d') && app_name.chars().next().map(|c| c.is_ascii_lowercase()).unwrap_or(false))
                        || skip_list.contains(&app_name.as_str());
                    
                    // 检查 type：只有 Foreground 类型的应用才有窗口
                    // UIElement 和 BackgroundOnly 没有窗口
                    let has_window = current_type.as_deref() == Some("Foreground");
                    
                    if !is_system_process
                        && !app_name.is_empty()
                        && current_pid > 0
                        && has_window
                    {
                        // 去重：使用 bundle_id 作为 key
                        let dedup_key = current_bundle_id.clone().unwrap_or_else(|| app_name.clone());
                        if seen_bundle_ids.contains(&dedup_key) {
                            current_bundle_id = None;
                            current_pid = 0;
                            current_type = None;
                            continue;
                        }
                        seen_bundle_ids.insert(dedup_key);
                        
                        windows.push(WindowInfo {
                            id: id_counter,
                            app_name,
                            title: String::new(),
                            bundle_id: current_bundle_id.take(),
                            pid: current_pid,
                        });
                        id_counter += 1;
                    }
                }
                current_bundle_id = None;
                current_pid = 0;
            }
        }

        // 处理最后一个应用
        if let Some(app_name) = current_app {
            let skip_list = [
                "loginwindow", "SystemUIServer", "Dock", "Window Server",
                "Control Center", "Finder", "universalaccessd",
                "CoreServicesUIAgent", "WindowManager", "BackgroundTaskManagementAgent",
                "ViewBridgeAuxiliary", "talagentd", "cfprefsd", "containermanagerd",
                "trustd", "securityd", "kernel_task", "launchd",
                "WindowServer", "coreauthd", "configd", "powerd", "diskarbitrationd",
                "fseventsd", "logd", "notifyd", "opendirectoryd", "syslogd",
                "mdworker", "mdworker_shared", "syspolicyd",
                "coreservicesd", "distnoted", "gpsd", "wifid", "bluetoothd",
                "sharingd", "airplayd", "appleeventsd", "System Events", "Spotlight",
            ];
            let is_system_process = app_name.starts_with("com.")
                || app_name.contains("framework")
                || app_name.contains("/System/")
                || (app_name.ends_with('d') && app_name.chars().next().map(|c| c.is_ascii_lowercase()).unwrap_or(false))
                || skip_list.contains(&app_name.as_str());
            
            // 检查 type：只有 Foreground 类型的应用才有窗口
            let has_window = current_type.as_deref() == Some("Foreground");
            
            if !is_system_process
                && !app_name.is_empty()
                && current_pid > 0
                && has_window
            {
                // 去重
                let dedup_key = current_bundle_id.clone().unwrap_or_else(|| app_name.clone());
                if !seen_bundle_ids.contains(&dedup_key) {
                    seen_bundle_ids.insert(dedup_key);
                    windows.push(WindowInfo {
                        id: id_counter,
                        app_name,
                        title: String::new(),
                        bundle_id: current_bundle_id.take(),
                        pid: current_pid,
                    });
                }
            }
        }

        Ok(windows)
    }

    fn get_windows_ps() -> Result<Vec<WindowInfo>, String> {
        let output = Command::new("ps")
            .args(&["-eo", "pid,comm"])
            .output()
            .map_err(|e| format!("ps 执行失败: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut windows = Vec::new();
        let mut id_counter = 1u64;

        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.trim().split_whitespace().collect();
            if parts.len() >= 2 {
                let pid: i32 = parts[0].parse().unwrap_or(0);
                let process_name = parts[1];

                // 只保留有窗口的应用（排除系统进程）
                let app_name = if process_name.contains("/") {
                    process_name.split('/').last().unwrap_or(process_name).to_string()
                } else {
                    process_name.to_string()
                };

                let skip_list = [
                    "loginwindow", "SystemUIServer", "Dock", "Window Server",
                    "universalaccessd", "Control Center", "Finder",
                    "CoreServicesUIAgent", "WindowManager", "BackgroundTaskManagementAgent",
                    "ViewBridgeAuxiliary", "bash", "zsh", "sh", "ssh", "tmux",
                    "node", "python", "python3", "ruby", "perl",
                    "mdworker", "mdworker_shared", "syspolicyd", "trustd", "securityd",
                    "kernel_task", "launchd", "WindowServer", "coreauthd", "configd",
                    "powerd", "diskarbitrationd", "fseventsd", "logd", "notifyd",
                    "opendirectoryd", "syslogd", "coreservicesd", "distnoted",
                    "gpsd", "wifid", "bluetoothd", "sharingd", "airplayd", "appleeventsd",
                    "cfprefsd", "containermanagerd", "talagentd",
                ];

                let is_system_process = app_name.starts_with("com.")
                    || app_name.contains(".framework")
                    || app_name.contains("/System/Library/")
                    || app_name.contains("/usr/libexec/")
                    || app_name.contains("/usr/sbin/")
                    || (app_name.ends_with('d') && app_name.chars().next().map(|c| c.is_ascii_lowercase()).unwrap_or(false))
                    || skip_list.contains(&app_name.as_str());

                if !is_system_process
                    && !app_name.is_empty()
                    && pid > 0
                {
                    windows.push(WindowInfo {
                        id: id_counter,
                        app_name,
                        title: String::new(),
                        bundle_id: None,
                        pid,
                    });
                    id_counter += 1;
                }
            }
        }

        Ok(windows)
    }

    pub fn get_foreground_window() -> Result<WindowInfo, String> {
        let output = Command::new("osascript")
            .arg("-e")
            .arg(r#"
tell application "System Events"
    set frontProc to first process whose frontmost is true
    set procName to name of frontProc
    set procPID to unix id of frontProc
    set winTitle to ""
    try
        set winTitle to name of first window of frontProc
    end try
    return procName & "|" & winTitle & "|" & procPID
end tell
"#)
            .output()
            .map_err(|e| format!("osascript 执行失败: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if stdout.is_empty() {
            return Err("无法获取前台窗口".to_string());
        }

        let parts: Vec<&str> = stdout.split('|').collect();
        let app_name = parts.get(0).unwrap_or(&"").to_string();
        let title = parts.get(1).unwrap_or(&"").to_string();
        let pid = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

        Ok(WindowInfo {
            id: 0,
            app_name,
            title,
            bundle_id: None,
            pid,
        })
    }

    pub fn auto_fill_password_by_pid(pid: i32, password: &str) -> Result<(), String> {
        eprintln!("auto_fill_password_by_pid 开始执行，pid={}", pid);

        match check_accessibility_permission() {
            Ok(false) => {
                return Err("需要辅助功能权限。请前往：系统设置 > 隐私与安全 > 辅助功能，添加并启用此应用".to_string());
            }
            Ok(true) => {}
            Err(e) => {
                eprintln!("检查权限失败: {}", e);
            }
        }

        // 直接使用 PID 激活应用
        eprintln!("使用 PID {} 激活应用", pid);
        activate_by_pid(pid)?;

        // 等待应用激活
        std::thread::sleep(std::time::Duration::from_millis(1500));

        // 输入密码
        eprintln!("准备输入密码，长度: {}", password.len());

        let escaped_password = password
            .replace('\\', "\\\\")
            .replace('"', "\\\"");

        let script = format!(
            r#"tell application "System Events" to keystroke "{}""#,
            escaped_password
        );

        eprintln!("执行 keystroke 命令");
        let output = Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| {
                eprintln!("keystroke 命令执行失败: {}", e);
                format!("启动失败: {}", e)
            })?;

        eprintln!("keystroke 状态码: {:?}", output.status.code());
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            eprintln!("keystroke 错误: {}", err);
            if err.contains("not allowed") || err.contains("assistive") || err.contains("privilege violation") {
                return Err("需要辅助功能权限。请前往：系统设置 > 隐私与安全 > 辅助功能，添加并启用此应用".to_string());
            }
            return Err(format!("AppleScript 错误: {}", err));
        }

        eprintln!("auto_fill_password 完成");
        Ok(())
    }

    fn activate_by_pid(pid: i32) -> Result<(), String> {
        eprintln!("使用 PID 激活: {}", pid);
        
        let script = format!(
            r#"tell application "System Events" to set frontmost of first process whose unix id is {} to true"#,
            pid
        );

        let output = Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| format!("PID 激活失败: {}", e))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            Err(format!("PID 激活失败: {}", err))
        } else {
            Ok(())
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use windows::Win32::Foundation::{HWND, LPARAM, BOOL};
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, IsWindowVisible, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow};
    use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_UNICODE, INPUT_0, VIRTUAL_KEY};
    use std::os::windows::ffi::OsStringExt;
    use std::ffi::OsString;
    use std::process::Command;

    pub fn check_accessibility_permission() -> Result<bool, String> {
        Ok(true)
    }

    pub fn open_accessibility_settings() -> Result<(), String> {
        Command::new("cmd")
            .args(&["/C", "start", "ms-settings:easeofaccess-keyboard"])
            .spawn()
            .map_err(|e| format!("无法打开设置: {}", e))?;
        Ok(())
    }

    struct FindWindowData {
        target_pid: u32,
        result: HWND,
    }

    unsafe fn FindWindowByPID(target_pid: u32) -> HWND {
        let mut data = FindWindowData {
            target_pid,
            result: HWND(0),
        };

        let _ = EnumWindows(Some(find_window_callback), LPARAM(&mut data as *mut _ as isize));

        data.result
    }

    unsafe extern "system" fn find_window_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let data = &mut *(lparam.0 as *mut FindWindowData);
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == data.target_pid && IsWindowVisible(hwnd).as_bool() {
            data.result = hwnd;
            return BOOL(0);
        }
        BOOL(1)
    }

    pub fn get_windows() -> Result<Vec<WindowInfo>, String> {
        let mut windows: Vec<WindowInfo> = Vec::new();

        unsafe {
            let _ = EnumWindows(Some(enum_window_callback), LPARAM(&mut windows as *mut _ as isize));
        }

        Ok(windows)
    }

    unsafe extern "system" fn enum_window_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let windows = &mut *(lparam.0 as *mut Vec<WindowInfo>);

        if IsWindowVisible(hwnd).as_bool() {
            let mut text: [u16; 512] = [0; 512];
            let len = GetWindowTextW(hwnd, &mut text);

            if len > 0 {
                let title = OsString::from_wide(&text[..len as usize])
                    .to_string_lossy()
                    .to_string();

                // 过滤掉一些常见的系统窗口
                if ["Settings", "Microsoft Store", "Program Manager", "Calculators"].contains(&title.as_str()) {
                    return BOOL(1);
                }

                let mut pid: u32 = 0;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));

                windows.push(WindowInfo {
                    id: hwnd.0 as u64,
                    title: title.clone(),
                    app_name: title,
                    bundle_id: None,
                    pid: pid as i32,
                });
            }
        }

        BOOL(1)
    }

    pub fn get_foreground_window() -> Result<WindowInfo, String> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0 == 0 {
                return Err("无法获取当前窗口".to_string());
            }

            let mut text: [u16; 512] = [0; 512];
            let len = GetWindowTextW(hwnd, &mut text);
            let title = OsString::from_wide(&text[..len as usize])
                .to_string_lossy()
                .to_string();

            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));

            Ok(WindowInfo {
                id: hwnd.0 as u64,
                app_name: title.clone(),
                title,
                bundle_id: None,
                pid: pid as i32,
            })
        }
    }

    pub fn auto_fill_password_by_pid(pid: i32, password: &str) -> Result<(), String> {
        eprintln!("Windows auto_fill_password_by_pid 开始执行，pid={}", pid);

        unsafe {
            let hwnd = FindWindowByPID(pid as u32);
            if hwnd.0 == 0 {
                return Err("找不到窗口".to_string());
            }

            let _ = SetForegroundWindow(hwnd);
            std::thread::sleep(std::time::Duration::from_millis(500));

            for ch in password.chars() {
                let input = INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VIRTUAL_KEY(0),
                            wScan: ch as u16,
                            dwFlags: KEYEVENTF_UNICODE,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                };

                let mut inputs = [input];
                SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            }
        }

        Ok(())
    }

    pub fn auto_fill_password(window_id: u64, password: &str) -> Result<(), String> {
        unsafe {
            let hwnd = HWND(window_id as isize);
            
            let _ = SetForegroundWindow(hwnd);
            
            std::thread::sleep(std::time::Duration::from_millis(500));

            for ch in password.chars() {
                let input = INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VIRTUAL_KEY(0),
                            wScan: ch as u16,
                            dwFlags: KEYEVENTF_UNICODE,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                };

                let mut inputs = [input];
                SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            }
        }

        Ok(())
    }
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .setup(|app| {
            init_state(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_accounts,
            add_account,
            delete_account,
            get_windows,
            get_foreground_window,
            auto_fill_password,
            check_accessibility_permission,
            open_accessibility_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
