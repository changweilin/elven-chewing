# AGENTS.md

windows-chewing-tsf — Windows 上的注音輸入法 (TSF) 實作。本檔提供給 OpenAI Codex / Cursor / 其他遵循 AGENTS.md 規範的 AI 助手使用,定義各職能領域與工作慣例。Claude Code 使用者請見 `~/.claude/skills/` 與 `~/.claude/agents/` 對應檔案。

## 專案地圖

```
crates/
  chewing_tip_core/   核心邏輯: config (registry/ACL)、ipc (varlink/pipe)、sandbox
  chewing_tip_host/   out-of-process host 程序: UI 視窗、IPC dispatch、更新檢查
  tsfreg/             TSF profile 註冊 helper binary
tip/                  TSF DLL: COM/text service/key event/UI 介面層
  src/text_service/   TextService 主體 + UI 元件介面
  rc/                 Windows resource (icon/bitmap/string table)
installer/            WiX/MSI 安裝程式
xtask/                cargo xtask build/package automation
```

## 通用建置慣例

- 語言: Rust (workspace,Cargo.toml 為 resolver = "3")
- 版本 single source: `Cargo.toml` `[workspace.package].version`
- 建置: `cargo xtask build-installer --target <msvc|mingw-w64-windows|mingw-w64-fedora> --release`
- 打包: `cargo xtask package-installer`
- 不修改不相關的程式碼、不引入新依賴前先檢查 workspace deps
- Windows API 一律用 `windows-rs` crate,不要手刻 FFI
- 所有 COM 物件用 `windows-rs` `#[implement]` 巨集
- 提交前: `cargo check --workspace`

---

## 職能 1: TSF Text Service (TSF/COM)

**何時觸發**: 任務涉及 `tip/src/com.rs`、`msctf.rs`、`text_service/mod.rs`、`edit_session.rs`、`display_attribute.rs`、TSF GUID、COM 介面、IME 生命週期、focus/document manager 事件。

**領域知識**:
- 主要介面: `ITfTextInputProcessorEx`、`ITfThreadMgrEventSink`、`ITfKeyEventSink`、`ITfCompositionSink`
- TSF GUID 集中在 module const,改一處必須同步 `crates/tsfreg/src/main.rs` 跟 `installer/windows-chewing-tsf.wxs`
- Edit session 旗標 (`TF_ES_SYNC/ASYNC` + `TF_ES_READ/READWRITE`) 錯誤會被 TSF 靜默 reject
- Focus 切換時 runtime config (簡繁、中英) 必須保留 — 參考 commit `8d82eb3`
- 不要手動呼叫 `IUnknown::Release`,Drop 自動 release

**檢查清單**:
- [ ] 每個 `AdviseSink` 都有對應 `UnadviseSink` (Deactivate 中)
- [ ] 新 GUID 同步三處: source / tsfreg / wxs
- [ ] `cargo check --workspace`

---

## 職能 2: Key Input + libchewing 引擎

**何時觸發**: 鍵盤事件、新增鍵盤布局、修改 commit 行為、ChewingEngine 設定、片語學習、中英/繁簡切換、IMM32 fallback、應用相容性 (quirk)。

**關鍵檔案**: `tip/src/text_service/key_event.rs`、`chewing.rs`、`tip/src/keybind.rs`、`imm32.rs`、`quirk.rs`,上游 `chewing` crate workspace dep。

**領域知識**:
- VK→Keysym: `SystemKeyboardEvent::new` → `ToAscii` (手動清 `VK_CONTROL` 避開 0x00–0x1F hack) → `chewing::input::KeyboardEvent`
- 鍵盤布局: 注音 (預設) + Dvorak / Colemak / Colemak-DH / Workman / QGMLWY,透過 `INVERTED_*_MAP` 反向表
- 引擎變體: `ChewingEngine` / `FuzzyChewingEngine` / `SimpleEngine`,由 `ChewingTsfConfig.conv_engine` 切換
- WPF 應用送 `scan_code = 0`,用 `MapVirtualKeyW` 補
- `quirk.rs` 針對特定 exe 名 (例 `MyAB.exe`) 停用 IMM32 patch

**檢查清單**:
- [ ] 改 key event 後手動測 modifier 矩陣 (Shift/Ctrl/Alt/CapsLock)
- [ ] 升級 chewing crate 時 review `Editor` API breaking change

---

## 職能 3: Candidate UI / 主題

**何時觸發**: 候選字視窗、語言列、選單、通知、字型/顏色、暗色模式、icon 變體、視窗定位、composition 視覺屬性。

**關鍵檔案**:
- tip 端: `tip/src/text_service/ui_elements/`、`lang_bar.rs`、`menu.rs`、`theme.rs`、`display_attribute.rs`、`resources.rs`
- host 端: `crates/chewing_tip_host/src/ui/`、`ui_elements/`
- 資源: `tip/rc/*.ico`、`*.bmp`

**領域知識**:
- 候選字視窗是 host 程序 win32 子視窗,**不是** TSF candidate UI element。改 UI = 改 tip + host + IPC schema 三處
- 暗色模式: `SystemUsesLightTheme` registry + 視窗背景亮度雙重判斷
- 圖示 6 變體 = 語系 (chi/eng/simp) × 主題 (light/dark) × 狀態 (normal/dot)
- 字型/顏色 100% 從 `ChewingTsfConfig` 來,**不可 hardcode**
- 視窗 clamping 參考 commit `4427ef3`

**檢查清單**:
- [ ] 改 UI 邏輯時同步檢查 IPC `messages.rs`
- [ ] 改 icon 時 6 變體都要存在
- [ ] 主題切換後 force redraw

---

## 職能 4: Config / Registry / ACL

**何時觸發**: `ChewingTsfConfig` 欄位、Registry 讀寫、AppContainer/UWP 沙箱相容、DACL 設定、設定升級遷移、預設值。

**關鍵檔案**: `crates/chewing_tip_core/src/config.rs`、`sandbox.rs`、`shell.rs`,WiX `.wxs` 中 RegistryKey component。

**領域知識**:
- 儲存於 `HKEY_CURRENT_USER\Software\ElvenIME`,view = `KEY_WOW64_64KEY`
- 舊版 `HKEY_CURRENT_USER\Software\ChewingTextService` 僅作讀取 migration fallback,新寫入一律走 `ElvenIME`
- 序列化用 `serde::{Serialize, Deserialize}` (字串 / JSON)
- AppContainer (Edge、Store App、新 Office) 需要 `ALL_APPLICATION_PACKAGES` DACL,否則沙箱程序讀預設值
- DACL 流程: `AllocateAndInitializeSid(SECURITY_APP_PACKAGE_AUTHORITY)` → `EXPLICIT_ACCESS_W` → `SetEntriesInAclW` → `SetNamedSecurityInfoW`

**檢查清單**:
- [ ] 新欄位有 `#[serde(default)]`
- [ ] 重新命名用 `#[serde(rename)]`,不硬改 Registry key 名稱
- [ ] 需要 AppContainer 存取的 key 走 ACL helper
- [ ] 改 Registry path 同步 WiX `.wxs`

---

## 職能 5: IPC / Host 程序

**何時觸發**: 新增 IPC 方法、修改訊息 payload、debug pipe 連線、host crash recovery、host 程序 lifecycle、varlink schema 相容性。

**關鍵檔案**: `crates/chewing_tip_core/src/ipc/` (`messages.rs`、`varlink.rs`、`named_pipe.rs`、`client.rs`)、`crates/chewing_tip_host/src/main.rs`、`ipc.rs`、`ui/event_loop.rs`。

**領域知識**:
- 協議: varlink (JSON-RPC 變體) over Windows named pipe,訊息 `\0` 結尾
- Pipe 名稱跟使用者 SID/session 綁定,Pipe ACL 必須允許 AppContainer
- Host 由 tip lazy spawn,host crash 後 tip 必須 respawn
- 主執行緒禁止 blocking pipe I/O,用 overlapped 或背景 thread
- Schema **必須** forward/backward compatible (升級期間新 tip + 舊 host 同時存在)

**新增 IPC 方法步驟**:
1. `messages.rs` 加 struct (`#[serde(default)]` 新欄位)
2. `client.rs` 加方法
3. `chewing_tip_host/src/ipc.rs` 加 dispatch case
4. `chewing_tip_host/src/ui_elements/` 或 `update/` 加實作

---

## 職能 6: Build / Installer / Release

**何時觸發**: build pipeline、WiX component、版本 bump、cross compile 設定、TSF 註冊邏輯、SignPath 簽章。

**關鍵檔案**: `xtask/src/` (build automation)、`installer/` (WiX)、`crates/tsfreg/`、root `Cargo.toml`。

**領域知識**:
- 版本 single source: `Cargo.toml` workspace.package.version → 自動同步 `installer/version.json` / `version.wxi`
- 命令: `cargo xtask build-installer --target <msvc|mingw-w64-windows|mingw-w64-fedora> --release` → `cargo xtask package-installer`
- TSF CLSID/GUID 改一處必須同步: `text_service/mod.rs` + `tsfreg` + `installer/*.wxs`
- 安裝後跑 `tsfreg.exe register`,反安裝跑 `tsfreg.exe unregister`
- per-user + per-machine 兩種 scope
- **WiX component GUID 一旦發布不可改**,只能新增
- 32-bit + 64-bit registry (`Wow6432Node`) 兩邊都要寫
- 簽章由 SignPath CI 處理,本地不需要

**檢查清單**:
- [ ] 新 component 有獨立 GUID + 加到 Feature
- [ ] 跑 `cargo xtask build-installer` 驗證
- [ ] cross compile 兩個 target (msvc + mingw) 都試

---

## 職能 7: i18n / Localization

**何時觸發**: 使用者可見字串、新增語系、codepage 問題、字串外部化、繁簡轉換需求。

**關鍵檔案**: `installer/windows-chewing-tsf.wxl` (WiX, zh-tw, cp 950)、`tip/rc/ChewingTextService.rc`、`version.rc`、`resource.h`、`tip/src/text_service/resources.rs`。

**領域知識**:
- **安裝程式語系 ≠ IME 輸出語系**: WiX `.wxl` 只影響安裝畫面; IME runtime 字串走 `tip/rc/`
- **繁簡是引擎功能不是 i18n**: 走 chewing engine + `output_simp_chinese` config,不要做字串對應表
- 一個語系一個 `.wxl`,Codepage: zh-tw=950、zh-cn=936、en-us=1252
- `.rc` 編碼: UTF-16 LE
- Resource string 上限 4096 字元,長文案 (EULA) 用 `.rtf`
- Hardcode 字串在 Rust source 是 anti-pattern,走 resource ID + `LoadStringW`

**新增使用者字串步驟**:
1. `resource.h` 加 ID
2. `tip/rc/ChewingTextService.rc` STRINGTABLE 加字串
3. `resources.rs` 加 helper (`LoadStringW`)
4. 程式碼呼叫

**注意**: `.wxl` 字串 ID 寫錯不會 build error,必須**實際打開 MSI** 確認畫面。

---

## 跨職能協作矩陣

| 任務範例 | 涉及職能 |
|---|---|
| 改候選字外觀 | candidate-ui + ipc-host (因為要傳訊息) |
| 新增使用者偏好欄位 | config-registry-acl + ui? + i18n (若需 UI 標籤) |
| 新鍵盤布局 | key-input-chewing (+ chewing crate upstream) |
| 改 TSF event 處理 | tsf-text-service |
| 發版 | build-installer + 確認版本同步所有檔案 |
| 新增 IPC 方法 | ipc-host |
| 修暗色模式 bug | candidate-ui |

## 風險紅旗 (任何 agent 看到都要警告使用者)

- 改 TSF GUID / WiX component GUID — 影響升級路徑,只能新增不能改
- 改 Registry path 或 key 名 — 舊使用者設定遺失
- 改 IPC schema 但不維持向後相容 — 升級期間新舊版本同跑會 crash
- 移除 AppContainer ACL — UWP 應用悄悄退回預設值
- 簽章流程改動 — 不要在本地簽,SignPath 處理
