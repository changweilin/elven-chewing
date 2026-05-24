# 精靈語輸入法 (ElvenIME / Windows Chewing TSF)

精靈語輸入法是一套以 Rust 實作的 Windows 注音輸入法，使用 Text Services Framework (TSF) 串接 Windows 文字輸入流程。它繼承新酷音/libchewing 的注音轉換基礎，但本專案的重點在 Windows 使用體驗與自有功能：out-of-process UI、雙排輸入、模糊注音猜詞、最近一次送出文字重轉、WebAssembly 沙盒、WiX/MSI 打包，以及 AppContainer 相容性。

本專案適合想在 Windows 上開發、測試或打包現代 TSF 注音輸入法的人使用。一般使用者可安裝 MSI；開發者則可透過 `cargo xtask` 完成建置、下載外部元件與封裝。

## 核心功能特性

- 以 Rust 與 `windows-rs` 實作 TSF/COM text service，支援 64-bit 與 32-bit 應用程式載入。
- out-of-process host 架構：候選字、通知、雙排預覽等 UI 由獨立程序繪製，TSF DLL 保持輕量。
- 雙排輸入模式：同一串按鍵同時保留中文轉換與英文原文，可在送出前切換輸出軌。
- 最近一次送出文字重轉：預設 `Ctrl+\`` 可在中文與原始英文按鍵結果之間重新轉換。
- 模糊注音猜詞：支援以聲母或前幾碼觸發候選字，適合快速輸入與不完整注音。
- 即時簡繁輸出切換、全形/半形切換、Caps Lock 語言鎖定、短按 Shift 切換等輸入狀態控制。
- 自訂候選字視窗、通知視窗、暗色模式偵測、語言列圖示、字型與色彩設定。
- 自訂快捷鍵，例如 `Ctrl+F12` 切換簡體輸出、`Ctrl+Delete` 反學習候選詞、`Ctrl+F11` 切換雙排輸入軌。
- 針對 Windows 應用相容性處理：AppContainer Registry ACL、WPF `scan_code = 0` 補正、IMM32 fallback 與特定程式 quirk。
- 使用 Windows named pipe + varlink IPC，並支援 host lazy spawn、crash recovery 與簽章驗證。
- 內建更新檢查流程，依 stable/development channel 提示可用版本。
- 提供 `chewing_engine_kit` 與 `crates/web_demo`，可在非 Windows 環境或瀏覽器中驗證精靈語自有輸入體驗。

## 系統需求與安裝步驟

### 一般使用者安裝

下載或自行建置 MSI 後，以 PowerShell 執行：

```powershell
Start-Process -FilePath .\dist\windows-chewing-tsf-unsigned.msi
```

安裝程式會註冊 TSF profile、安裝字典、設定工具與使用者詞庫編輯器。若安裝後輸入法沒有立即出現在輸入法清單中，請開啟 Windows 的「設定 > 時間與語言 > 語言與地區 > 鍵盤」，確認「精靈語輸入法」已加入；必要時登出再登入。

### 開發環境需求

Windows 原生建置建議準備：

- Windows 10/11。
- Rust stable toolchain。
- Visual Studio 2022 Build Tools，需包含 MSVC、Windows SDK 與 MSBuild。
- Git。
- `curl`、`unzip`、`sqv`，供 `cargo xtask download-components` 下載並驗證外部元件。
- Node.js 與 `wasm-pack`，僅在開發 `crates/web_demo` 時需要。

安裝 Rust 與常用工具：

```powershell
winget install Rustlang.Rustup
rustup default stable
rustup target add x86_64-pc-windows-msvc
rustup target add i686-pc-windows-msvc
```

Web Demo 另需：

```powershell
cargo install wasm-pack --locked
```

### 準備 libchewing 暫時 patch

目前根目錄 `Cargo.toml` 使用 `[patch.crates-io]` 指向 `../libchewing-fix-cb`。第一次建置前，請在本 repo 根目錄執行：

```powershell
$repo = (Get-Location).Path
git clone --no-tags https://codeberg.org/chewing/libchewing.git ..\libchewing-fix-cb
git -C ..\libchewing-fix-cb checkout --detach 9363b03f7a4f0c2898213a911f2f71388aeaf571
git -C ..\libchewing-fix-cb apply "$repo\patches\libchewing-fuzzy-partial-prefix-down.patch"
```

CI 會透過 `.github/actions/prepare-libchewing` 自動完成這件事；本機開發則需要手動準備一次。

### 從原始碼建置 MSI

本地未簽章測試包建議加入 `--allow-unsigned-host`，避免 release build 的 host 簽章驗證擋下本機產物：

```powershell
cargo xtask build-installer --target msvc --release --allow-unsigned-host
cargo xtask download-components
cargo xtask package-installer
```

輸出檔案會放在：

```text
dist/windows-chewing-tsf-unsigned.msi
```

正式發行版不應使用 `--allow-unsigned-host`；簽章由 CI/SignPath 流程處理。

## 快速上手與使用範例

安裝後，切換到「精靈語輸入法」即可在支援 TSF 的 Windows 應用程式中輸入。除了基本注音輸入，建議先試以下幾個本專案自有功能：

常用操作：

- 短按 `Shift`：預設切換中文/英文模式。
- 語言列圖示：切換中文/英文、全形/半形，或開啟設定/工具選單。
- 數字鍵 `1` 到 `9`：選擇候選字。
- 模糊注音猜詞：開啟後輸入不完整注音，再按 `↓` 觸發候選。
- 雙排輸入模式：可在選單或設定中開啟，輸入時同時預覽中文與英文軌。
- `Ctrl+F12`：切換簡體中文輸出。
- `Ctrl+Delete`：在候選字選擇期間反學習片語。
- `Ctrl+F11`：雙排輸入模式開啟時切換中文/英文輸出軌。
- `Ctrl+\``：重新轉換最近一次送出的文字。

開啟設定與詞庫工具：

```powershell
Start-Process "elven-ime-preferences://config"
Start-Process "elven-ime-editor://open"
```

執行基本開發檢查：

```powershell
cargo check --workspace
cargo test -p chewing_engine_kit
```

啟動 Web Demo：

```powershell
cargo xtask download-components
Set-Location crates\web_demo
npm install
npm run dev
```

Vite 預設會開在：

```text
http://localhost:5173
```

開發伺服器也會綁定 LAN / Tailscale 介面，可改用 `http://<LAN-IP>:5173`、`http://<Tailscale-IP>:5173` 或 Tailscale MagicDNS hostname。若使用自訂網域，請設定 `VITE_ALLOWED_HOSTS` 後再啟動。

## 專案架構說明

```text
.
├── Cargo.toml                 # Rust workspace 與共用依賴、版本來源
├── tip/                       # TSF DLL：COM、TextService、鍵盤事件與語言列整合
│   ├── src/text_service/      # IME 核心流程、edit session、候選字、選單、主題
│   └── rc/                    # Windows resource：icon、bitmap、字串表、version.rc
├── crates/
│   ├── chewing_tip_core/      # 共用核心：config、Registry、ACL、IPC、shell helper
│   ├── chewing_tip_host/      # out-of-process host：UI 視窗、IPC dispatch、更新檢查
│   ├── chewing_engine_kit/    # 可攜式輸入引擎 adapter，供測試與 Web Demo 使用
│   ├── tsfreg/                # TSF profile 註冊、啟用、停用與 host 停止工具
│   └── web_demo/              # wasm-bindgen + Vite 的瀏覽器引擎測試沙盒
├── installer/                 # WiX/MSI 安裝程式定義、zh-TW 字串與版本檔
├── xtask/                     # build-installer、download-components、package-installer
├── patches/                   # 暫時套用到 libchewing 的 patch
├── docs/                      # 開發設計文件
├── design/                    # CI、logo 與設計相關資料
├── build/                     # 建置中間產物，包含 installer staging 目錄
└── dist/                      # 最終 MSI 輸出目錄
```

主要執行元件：

| 元件 | 說明 |
| --- | --- |
| `chewing_tip.dll` | Windows TSF text service，負責接收按鍵、維護 composition、送出文字。 |
| `chewing_tip_host.exe` | 顯示候選字、通知與雙排預覽等 UI，並處理更新檢查。 |
| `tsfreg.exe` | 安裝/解除安裝時註冊 TSF CLSID、profile 與 categories；`tsfreg -x` 可匯出診斷 JSON。 |
| `ChewingPreferences.exe` | 外部下載的設定工具，由 MSI 一併安裝。 |
| `ChewingEditor.exe` | 外部下載的使用者詞庫編輯工具，由 MSI 一併安裝。 |

重要路徑：

- 系統字典：`%ProgramFiles%\ElvenIME\Dictionary`
- 使用者資料：`%APPDATA%\ElvenIME\ChewingTextService`
- 使用者設定：`HKCU\Software\ElvenIME`
- MSI 輸出：`dist\windows-chewing-tsf-unsigned.msi`

中期功能支援入口：

```powershell
tsfreg -x
tsfreg --export-profile .\elven-ime-profile.json
tsfreg --import-profile .\elven-ime-profile.json
```

更新檢查除了 `UpdateInfoUrl`，也會在 registry 保留版本、artifact URL、checksum 與 release description，供設定工具或未來 UI 顯示。

## 授權條款

本專案授權條款繼承新酷音專案，原始碼以 GNU General Public License v3.0 or later (`GPL-3.0-or-later`) 釋出。完整授權文字請見 `COPYING.txt`。

由 `download-components` 取得的外部元件與字典資料，請同時遵守其上游專案的授權條款。
