# AccountAutoFill (账户自动填充工具)

[English](#english) | [简体中文](#简体中文)

---

## English

AccountAutoFill is a cross-platform desktop application built with Tauri, Rust, and React. It helps users securely manage their accounts and passwords, providing a convenient one-click auto-fill feature for other desktop applications.

### Features

- **Secure Storage**: Local storage with basic obfuscation for sensitive data.
- **Cross-Platform**: Supports macOS (Apple Silicon & Intel) and Windows.
- **Auto-Fill**: Automatically fills passwords into the focused window/application.
- **Window Management**: Detects active windows and allows users to select target applications for filling.
- **Privacy Focused**: Works offline; your data stays on your machine.

### Tech Stack

- **Frontend**: React, TypeScript, Vite, Tailwind CSS.
- **Backend**: Rust, Tauri.
- **System Integration**: 
  - **macOS**: AppleScript (UI Automation) for keystroke simulation.
  - **Windows**: Win32 API for window detection and input simulation.

### Getting Started

#### Prerequisites

- Node.js (LTS)
- Rust toolchain
- (macOS) Xcode Command Line Tools
- (Windows) C++ Build Tools

#### Development

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev
```

#### Build

```bash
# Build the application
npm run tauri build
```

---

## 简体中文

AccountAutoFill 是一款基于 Tauri、Rust 和 React 开发的跨平台桌面应用。它旨在帮助用户安全地管理账户和密码，并为其他桌面应用程序提供便捷的一键自动填充功能。

### 功能特性

- **安全存储**: 本地存储，对敏感数据进行基础混淆加密。
- **跨平台支持**: 支持 macOS (Apple Silicon & Intel) 和 Windows 系统。
- **自动填充**: 自动将密码输入到当前获取焦点的窗口或应用程序中。
- **窗口管理**: 智能检测当前运行的窗口，允许用户选择目标应用进行填充。
- **隐私保护**: 离线工作，数据完全保留在本地。

### 技术栈

- **前端**: React, TypeScript, Vite, Tailwind CSS。
- **后端**: Rust, Tauri。
- **系统集成**:
  - **macOS**: 使用 AppleScript (UI Automation) 模拟按键输入。
  - **Windows**: 使用 Win32 API 进行窗口检测和输入模拟。

### 快速开始

#### 环境要求

- Node.js (LTS)
- Rust 编译环境
- (macOS) Xcode Command Line Tools
- (Windows) C++ Build Tools

#### 开发环境运行

```bash
# 安装依赖
npm install

# 启动开发模式
npm run tauri dev
```

#### 打包编译

```bash
# 打包应用
npm run tauri build
```

### 许可证

[MIT License](LICENSE)
