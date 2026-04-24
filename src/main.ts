import "./styles.css";
import { invoke } from "@tauri-apps/api/core";

interface Account {
  id: string;
  name: string;
  password: string;
}

interface WindowInfo {
  id: number;
  title: string;
  app_name: string;
  bundle_id: string | null;
  pid: number;
}

let accounts: Account[] = [];
let windows: WindowInfo[] = [];
let selectedAccountId: string | null = null;
let selectedPid: number | null = null;
let hasAccessibilityPermission = false;
let pendingDeleteAccountId: string | null = null;

async function init() {
  renderMainUI();
  await loadAccounts();
  await checkPermission();
  await loadWindows();
}

function renderMainUI() {
  const app = document.getElementById("app")!;
  app.innerHTML = `
    <div class="container">
      <div class="header">
        <h1>🔐 账户自动填充</h1>
      </div>

      <div class="main-content">
        <div class="panel">
          <h2>账户列表</h2>
          <div class="add-form">
            <input type="text" id="accName" placeholder="账户名称" maxlength="50" autocomplete="off" />
            <input type="password" id="accPassword" placeholder="密码" maxlength="100" autocomplete="new-password" />
            <button id="addBtn">添加</button>
          </div>
          <div id="accountList" class="list"></div>
        </div>

        <div class="panel">
          <h2>目标窗口</h2>
          <div class="hint">💡 选择应用后点击"自动输入"</div>
          <div class="permission-hint" id="permissionHint" style="display:none;">
            <div class="permission-alert">
              <strong>⚠️ 需要辅助功能权限</strong>
              <p>请前往：系统设置 > 隐私与安全 > 辅助功能</p>
              <p>添加并启用此应用</p>
              <button id="openSettingsBtn" class="settings-btn">打开系统设置</button>
            </div>
          </div>
          <button id="refreshBtn" class="refresh-btn">🔄 刷新窗口</button>
          <div id="windowList" class="list"></div>
        </div>
      </div>

      <div class="action-bar">
        <button id="autoFillBtn" class="auto-fill-btn" disabled>
          自动输入密码
        </button>
      </div>

      <div id="status" class="status"></div>
    </div>
  `;

  const addBtn = document.getElementById("addBtn");
  const refreshBtn = document.getElementById("refreshBtn");
  const autoFillBtn = document.getElementById("autoFillBtn");
  const openSettingsBtn = document.getElementById("openSettingsBtn");

  addBtn?.addEventListener("click", () => {
    console.log("添加按钮被点击");
    addAccount();
  });
  refreshBtn?.addEventListener("click", () => {
    console.log("刷新按钮被点击");
    loadWindows();
  });
  autoFillBtn?.addEventListener("click", () => {
    console.log("自动输入按钮被点击", {
      selectedAccountId,
      selectedPid,
      accounts: accounts.length,
      windows: windows.length
    });
    confirmAndAutoFill();
  });
  openSettingsBtn?.addEventListener("click", () => {
    openAccessibilitySettings();
  });
}

async function checkPermission() {
  try {
    hasAccessibilityPermission = await invoke("check_accessibility_permission");
    showPermissionHint(!hasAccessibilityPermission);
    console.log("辅助功能权限状态:", hasAccessibilityPermission);
  } catch (e) {
    console.error("检查权限失败:", e);
    hasAccessibilityPermission = false;
    showPermissionHint(true);
  }
}

async function openAccessibilitySettings() {
  try {
    await invoke("open_accessibility_settings");
  } catch (e: any) {
    showStatus("打开设置失败: " + e, "error");
  }
}

async function loadAccounts() {
  try {
    accounts = await invoke("get_accounts");
    renderAccountList();
  } catch (e: any) {
    showStatus(e, "error");
  }
}

function renderAccountList() {
  const list = document.getElementById("accountList")!;
  if (accounts.length === 0) {
    list.innerHTML = '<div class="empty">暂无账户</div>';
    return;
  }

  list.innerHTML = accounts.map(acc => `
    <div class="item ${acc.id === selectedAccountId ? 'selected' : ''}" data-id="${acc.id}">
      <span class="name">${escapeHtml(acc.name)}</span>
      <button class="delete-btn ${pendingDeleteAccountId === acc.id ? 'confirming' : ''}" data-id="${acc.id}">
        ${pendingDeleteAccountId === acc.id ? "确认删除" : "删除"}
      </button>
    </div>
  `).join("");

  list.querySelectorAll(".delete-btn").forEach(btn => {
    btn.addEventListener("click", (e) => {
      e.stopPropagation();
      const idToDelete = (btn as HTMLElement).dataset.id;
      if (idToDelete) {
        confirmDelete(idToDelete);
      }
    });
  });

  list.querySelectorAll(".item").forEach(item => {
    item.addEventListener("click", (e) => {
      const target = e.target as HTMLElement;
      if (!target.closest(".delete-btn")) {
        selectAccount((item as HTMLElement).dataset.id!);
      }
    });
  });
}

function selectAccount(id: string) {
  selectedAccountId = id;
  renderAccountList();
  updateAutoFillButton();
}

function confirmDelete(id: string) {
  const account = accounts.find(a => a.id === id);
  if (!account) return;

  // Tauri 环境下原生 confirm 可能不弹窗，改为应用内二次确认。
  if (pendingDeleteAccountId !== id) {
    pendingDeleteAccountId = id;
    renderAccountList();
    showStatus(`再次点击“确认删除”以删除 "${account.name}"`, "info");
    return;
  }

  pendingDeleteAccountId = null;
  deleteAccount(id);
}

async function addAccount() {
  const nameInput = document.getElementById("accName") as HTMLInputElement;
  const passwordInput = document.getElementById("accPassword") as HTMLInputElement;

  const name = nameInput.value.trim();
  const password = passwordInput.value;

  if (!name) {
    showStatus("请输入账户名称", "error");
    return;
  }

  if (!password) {
    showStatus("请输入密码", "error");
    return;
  }

  try {
    await invoke("add_account", { name, password });
    nameInput.value = "";
    passwordInput.value = "";
    await loadAccounts();
    showStatus("账户添加成功", "success");
  } catch (e: any) {
    showStatus(e, "error");
  }
}

async function deleteAccount(id: string) {
  try {
    await invoke("delete_account", { id });
    pendingDeleteAccountId = null;
    if (selectedAccountId === id) {
      selectedAccountId = null;
    }
    await loadAccounts();
    updateAutoFillButton();
    showStatus("账户已删除", "success");
  } catch (e: any) {
    showStatus(e, "error");
  }
}

async function loadWindows() {
  showStatus("正在获取窗口列表...", "info");
  try {
    windows = await invoke("get_windows");
    renderWindowList();
    showStatus(`找到 ${windows.length} 个窗口`, "success");
  } catch (e: any) {
    showStatus(e, "error");
  }
}

function renderWindowList() {
  const list = document.getElementById("windowList")!;
  if (windows.length === 0) {
    list.innerHTML = '<div class="empty">未找到窗口</div>';
    return;
  }

  list.innerHTML = windows.map(win => {
    const displayTitle = win.title || "无标题";
    const displayApp = win.app_name || "未知应用";
    const isSelected = win.pid === selectedPid;

    return `
      <div class="item ${isSelected ? 'selected' : ''}" data-pid="${win.pid}" title="PID: ${win.pid}">
        <div class="win-app">${escapeHtml(displayApp)}</div>
        <div class="win-title">${escapeHtml(displayTitle)}</div>
      </div>
    `;
  }).join("");

  list.querySelectorAll(".item").forEach(item => {
    item.addEventListener("click", () => {
      const pid = parseInt((item as HTMLElement).dataset.pid!);
      selectWindow(pid);
    });
  });
}

function selectWindow(pid: number) {
  selectedPid = pid;
  renderWindowList();
  updateAutoFillButton();

  const win = windows.find(w => w.pid === pid);
  if (win) {
    showStatus(`已选择: ${win.app_name}${win.title ? ' - ' + win.title : ''}`, "info");
  }
}

function updateAutoFillButton() {
  const btn = document.getElementById("autoFillBtn") as HTMLButtonElement;
  if (btn) {
    btn.disabled = !selectedAccountId || selectedPid === null || !hasAccessibilityPermission;
  }
}

function confirmAndAutoFill() {
  console.log("confirmAndAutoFill 被调用");
  console.log("selectedAccountId:", selectedAccountId);
  console.log("selectedPid:", selectedPid);
  console.log("accounts:", accounts);
  console.log("windows:", windows);

  if (!hasAccessibilityPermission) {
    showStatus("请先授予辅助功能权限", "error");
    showPermissionHint(true);
    return;
  }

  if (!selectedAccountId || selectedPid === null) {
    console.log("条件触发：账户或窗口未选择");
    showStatus("请先选择账户和窗口", "error");
    return;
  }
  console.log("条件通过，继续执行");

  const account = accounts.find(a => a.id === selectedAccountId);
  const win = windows.find(w => w.pid === selectedPid);

  if (!account) {
    console.log("未找到账户");
    showStatus("未找到选中的账户", "error");
    return;
  }

  if (!win) {
    console.log("未找到窗口");
    showStatus("未找到选中的窗口，请刷新窗口列表", "error");
    return;
  }

  console.log("准备显示确认对话框");
  console.log("直接调用 autoFill");
  autoFill();
}

async function autoFill() {
  if (!selectedAccountId || selectedPid === null) {
    showStatus("请先选择账户和窗口", "error");
    return;
  }

  if (!hasAccessibilityPermission) {
    showStatus("请先授予辅助功能权限", "error");
    showPermissionHint(true);
    return;
  }

  console.log("开始调用 auto_fill_password 命令");
  console.log("参数: account_id =", selectedAccountId, "pid =", selectedPid);

  const statusEl = document.getElementById("status")!;
  statusEl.textContent = "正在激活窗口并输入密码...";
  statusEl.className = "status info";

  try {
    console.log("调用 invoke 前...");
    const result = await invoke("auto_fill_password", {
      account_id: selectedAccountId,
      pid: selectedPid
    });
    console.log("invoke 返回:", result);
    statusEl.textContent = "密码输入完成";
    statusEl.className = "status success";
  } catch (e: any) {
    console.error("invoke 失败:", e);
    console.error("错误类型:", typeof e);
    console.error("错误消息:", e?.message || e?.toString() || String(e));
    const errorMsg = String(e);
    if (errorMsg.includes("辅助功能权限") || errorMsg.includes("privilege")) {
      showPermissionHint(true);
      hasAccessibilityPermission = false;
      updateAutoFillButton();
    }
    statusEl.textContent = "输入失败：" + errorMsg;
    statusEl.className = "status error";
  }
}

function showPermissionHint(show: boolean) {
  const hint = document.getElementById("permissionHint");
  if (hint) {
    hint.style.display = show ? "block" : "none";
  }
}

function showStatus(message: string, type: "info" | "success" | "error") {
  const status = document.getElementById("status")!;
  status.textContent = message;
  status.className = `status ${type}`;
  if (type !== "error") {
    setTimeout(() => {
      status.textContent = "";
      status.className = "status";
    }, 3000);
  }
}

function escapeHtml(text: string): string {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

window.addEventListener("error", (e) => {
  console.error("全局错误:", e.error);
});

window.addEventListener("unhandledrejection", (e) => {
  console.error("未处理的 Promise 拒绝:", e.reason);
});

init();
