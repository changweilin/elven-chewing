# 不重開機的小範圍打字測試

TSF DLL 是 per-process 載入,只要關掉「載入舊 DLL 的程序」,新開的程序就會載入新 DLL。

## 建議流程

```powershell
# 1. 建置
cargo xtask build-installer --target msvc --release
# (或只 cargo build -p chewing_tip --release 拿 DLL)

# 2. 關掉會載入 IME 的測試程序 + host
taskkill /F /IM notepad.exe 2>$null
taskkill /F /IM chewing_tip_host.exe 2>$null

# 3. 覆寫安裝目錄的 DLL (前提:之前用 MSI 裝過一次)
Copy-Item target\release\chewing_tip.dll "C:\Program Files\ChewingTextService\" -Force

# 4. 開新 notepad 測
notepad
```

## 重點與限制

- **第一次要先用 MSI 裝**,讓 `tsfreg.exe register` 把 CLSID / Profile / Categories 寫進 registry。之後改 DLL 不需要再跑 tsfreg,**除非動到 GUID 或 categories** (`tip/src/text_service/mod.rs` / `crates/tsfreg/src/main.rs`)。
- **檔案鎖定**: 如果 `Copy-Item` 報 access denied,代表還有程序載著舊 DLL。用 `handle64.exe chewing_tip.dll` (Sysinternals) 找出來關掉。Explorer 偶爾也會載入,必要時 `taskkill /F /IM explorer.exe` 然後 `start explorer`。
- **測試程序選擇**:
  - 推薦 `notepad`、`wordpad`、`write`、VS Code — 非 AppContainer,行為單純
  - 避開 Edge / UWP / 新版 Office — AppContainer,讀 registry 要走 ACL,若懷疑 sandbox 問題才測這些
- **host 程序**: 候選字視窗、語言列在 `chewing_tip_host.exe`。改 host 端只需 `taskkill /F /IM chewing_tip_host.exe`,tip 下次需要時會 lazy spawn,**完全不用碰 DLL**。
- **改 GUID / 改 IPC schema / 改 categories** 才需要重跑 `tsfreg.exe register` (或重裝 MSI)。
- **改 registry schema** (`config.rs`) → 不用重開機,但要記得清舊值: `Remove-Item HKCU:\Software\ChewingTextService -Recurse`。

## 最快迴圈

改 host UI 最快: kill host → 開新 notepad → 切到注音,就會看到新版本,完全不用碰 DLL。
