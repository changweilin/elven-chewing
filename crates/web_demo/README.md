# 精靈語輸入法 Web Demo

這個 Web Demo 是精靈語輸入法的瀏覽器沙盒，重點放在驗證本專案自有的輸入體驗：模糊注音猜詞、最近送出文字重轉、簡繁即時切換、候選面板樣式、鍵盤配置與行為細節。

沙盒使用 `wasm-bindgen` 編譯成 WebAssembly，靜態頁面即可執行，不需要後端服務。它共用 `chewing_engine_kit`，讓桌面版與瀏覽器版能盡量維持一致的輸入邏輯。

## 事前準備

每台機器安裝一次：

```sh
cargo install wasm-pack --locked
npm install
```

WebAssembly bundle 會內嵌字典檔，因此建置前需先準備 `word.dat`、`tsi.dat`、`symbols.dat`：

```sh
cargo xtask download-components
```

字典預設會放在 `build/installer/Dictionary/`。若要改用其他位置，可設定 `CHEWING_DICT_DIR`。

## 本機 / LAN / Tailscale 開發

```sh
npm run dev
```

這個指令會先重新編譯 wasm，再用 Vite 服務 `static/`。Vite 會綁定所有網路介面，因此可用下列網址開啟：

```text
http://localhost:5173
http://<LAN-IP>:5173
http://<Tailscale-IP>:5173
http://<machine>.<tailnet>.ts.net:5173
```

`vite.config.js` 會允許 localhost、IP 位址、Windows 主機名、`.local` / `.lan` / `.home.arpa` 與 Tailscale MagicDNS (`.ts.net`)。若你使用自訂網域，可用逗號分隔補上：

```powershell
$env:VITE_ALLOWED_HOSTS="demo.example.com,my-pc.office"
npm run dev
```

若其他裝置仍連不上，請確認 Windows 防火牆允許 TCP 5173 進站，尤其是 Tailscale 介面。

## Production Build

```sh
npm run build
```

輸出會放在 `crates/web_demo/dist/`，內容是可直接部署的靜態網站。

若要在本機預覽 production build：

```sh
npm run preview
```

預覽伺服器同樣支援 LAN / Tailscale，預設 port 是 `4173`。

## GitHub Pages

此 demo 由 `.github/workflows/pages.yml` 部署。Repository Settings 中啟用 Pages，來源選擇 **GitHub Actions** 後，推送到 `main` 或手動執行 workflow 即可更新。

## 沙盒範圍

支援展示：

- 模糊注音猜詞與部分音節查詢流程。
- 最近一次送出文字重轉。
- 簡繁即時切換。
- 候選字數量、排列、色彩與字型設定。
- 多種注音鍵盤配置與觸控用虛擬鍵盤。

不模擬：

- TSF focus、composition、display attribute 等 Windows 文字服務事件。
- out-of-process host、named pipe IPC、AppContainer ACL 與程式簽章驗證。
- 真實桌面候選字視窗的 DPI、螢幕邊界與多螢幕定位。

完整 Windows 整合測試仍需使用桌面版與 self-hosted Windows runner；設定方式請見 `design/ci-self-hosted-runner.md`。
