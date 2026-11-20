# flutter_gameboy

flutter_gameboy 是一個以 Flutter 為前端、Rust 為模擬核心的跨平台 Game Boy 模擬器專案，也是一個 A.I. 協作開發案例。專案目標不是只做出可執行的播放器，而是把 UI、輸入、音訊、模擬器核心與桌面整合流程拆清楚，讓 Flutter 與 Rust 可以各自負責擅長的部分。

## 專案定位

這份專案可視為一個 A.I. 專案實作案例：

- 使用 Flutter 建立跨平台操作介面與輸入層。
- 使用 Rust 實作 Game Boy 模擬核心，處理 CPU、MMU、PPU、APU、Timer 與 Joypad。
- 使用 flutter_rust_bridge 連接 Dart 與 Rust，讓 UI 與核心分工明確。
- 在開發過程中透過 A.I. 協作進行除錯、重構、測試補強與 README 整理。

如果你想展示「A.I. 如何協助完成跨語言系統整合、模擬器除錯與專案文件化」，這個 repo 就是一個很直接的案例。

## 功能概覽

- 支援 Flutter 桌面與行動端介面。
- 支援載入 Game Boy ROM。
- 提供虛擬按鍵、D-pad 與鍵盤輸入。
- Rust 核心負責模擬 Game Boy 硬體行為。
- 透過 CPAL 處理音訊輸出。
- 具備部分 Rust 測試，用於驗證 Joypad、FF00 與中斷相關行為。

## 技術架構

### Flutter 端

- `lib/main.dart`
	- 負責畫面渲染、ROM 載入流程、焦點控制、鍵盤與觸控輸入。
- `lib/src/input_mask.dart`
	- 將 Flutter 端按鍵狀態轉成 Rust 核心可讀的 bitmask。
- `lib/src/rust/`
	- flutter_rust_bridge 產生的 Dart 綁定層。

### Rust 端

- `rust/src/cpu.rs`
	- CPU 指令執行與中斷處理。
- `rust/src/mmu.rs`
	- 記憶體映射、I/O 暫存器、Serial 與 DMA 行為。
- `rust/src/ppu.rs`
	- 畫面更新與 LCD 時序。
- `rust/src/apu.rs`
	- 音訊模擬。
- `rust/src/joypad.rs`
	- 按鍵輸入、FF00 暫存器與 Joypad interrupt 行為。
- `rust/src/gameboy.rs`
	- 整合 CPU、MMU、PPU、APU、Timer 與 Joypad 的核心執行流程。

## 專案亮點

- Flutter 負責互動與跨平台 UI，Rust 負責效能敏感的模擬邏輯。
- 專案不是單純包一層 FFI，而是有完整的輸入同步與 frame stepping 設計。
- 曾針對 Game Boy Joypad `FF00`、中斷旗標 `FF0F`、Serial interrupt 等問題做過深入除錯，對模擬器開發很有參考價值。
- 適合拿來展示 A.I. 協作開發如何參與除錯、測試與文件整理。

## 執行方式

### Flutter

在專案根目錄執行：

```bash
flutter pub get
flutter run -d windows
```

你也可以依平台改成 `android`、`linux`、`macos` 或其他 Flutter 支援的 target。

### Rust 核心

在 `rust` 目錄執行：

```bash
cargo build
cargo test
```

## 開發說明

- Flutter 負責輸入與畫面顯示，但實際模擬邏輯在 Rust。
- 桌面版會載入打包後的 Rust DLL，因此修改 Rust 後需要重新 build 並確認 runner 使用的是最新 DLL。
- 若是輸入異常，通常要同時檢查 Flutter mask、Joypad bit 對應、FF00 輪詢與中斷時序。

## 程式碼品質（Anti-Slop Lint）

本專案用嚴格的 analyzer / lint 規則防止程式碼退化成 AI slop。設定在 [`analysis_options.yaml`](analysis_options.yaml)，規則分類與例外政策見 [`docs/lint-policy.md`](docs/lint-policy.md)。

提交前請在本機執行：

```bash
dart format --output=none --set-exit-if-changed lib
flutter analyze --fatal-infos --fatal-warnings
```

CI（[`code-quality.yml`](.github/workflows/code-quality.yml)）會執行同樣的檢查；只要出現一個 lint info 就會擋下 PR。

重點規則：

- `strict-casts` / `strict-inference` / `strict-raw-types`：不允許隱式轉型、模糊推斷與 raw type。
- 未使用的 import、區域變數、私有成員與死碼直接視為 **error**。
- `unawaited_futures` / `discarded_futures`：Future 必須 `await` 或明確包成 `unawaited()`。
- `avoid_catches_without_on_clauses`：不准用無型別的 `catch` 吞錯。

`test/`、`integration_test/`、`test_driver/`、`script/`、`scripts/` 與測試執行檔屬本機開發用，不納入版本控制。

## ROM 與版權說明

本專案僅作為技術研究、模擬器開發與 A.I. 協作案例展示。請僅使用你合法持有或可合法使用的 ROM 檔案。

## 適合用來展示什麼

- Flutter 與 Rust 混合架構設計。
- 模擬器專案的系統切分方式。
- A.I. 協作開發流程。
- 跨語言除錯與測試補強。
- 桌面應用與原生模組整合。
