import Cocoa

// 获取正在运行的应用（Dock栏中的应用）
let workspace = NSWorkspace.shared
let runningApps = workspace.runningApplications

var id = 0
for app in runningApps {
    // 只显示有UI的应用（Dock栏中可见的）
    if app.activationPolicy == .regular {
        print("[\(id)] \(app.localizedName ?? "Unknown")")
        id += 1
    }
}
