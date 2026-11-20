# Lint 政策：讓 slop 進不來

這份文件說明 [`analysis_options.yaml`](../analysis_options.yaml) 為什麼這樣設定、每一類規則在擋什麼，以及什麼情況下可以例外。

## 為什麼需要這份政策

AI 協作產出的程式碼通常「編譯會過、畫面會動」，但很容易留下：

- 只為了讓 analyzer 閉嘴的 `dynamic`、`as`、`!`
- 改了 A 卻忘了刪 B 的 import、變數、私有方法
- `catch (e) {}` 或 `catch (e) { print(e); }` 把錯誤吞掉
- 沒有 `await` 的 Future、沒有 `dispose` 的 controller / ticker
- 複製貼上造成的重複 widget、重複常數、重複判斷

單看一處都不致命，累積起來就是 slop。這份政策的重點是**讓它們無法通過 `flutter analyze`**，而不是靠 code review 的人眼去抓。

## 三條鐵律

1. **型別要誠實**
   `strict-casts`、`strict-inference`、`strict-raw-types` 全開，再加上 `avoid_dynamic_calls`、`always_declare_return_types`、`type_annotate_public_apis`。任何「先讓它過」的隱式轉型都會被擋下。

2. **死碼要歸零**
   `unused_import`、`unused_local_variable`、`unused_element`、`dead_code` 直接升級成 **error**，而不是 warning。AI 最常見的足跡就是改了一半的程式碼，這條防線專門抓這個。

3. **錯誤與非同步要顯式**
   `avoid_catches_without_on_clauses`、`avoid_catching_errors`、`only_throw_errors`、`use_rethrow_when_possible` 管例外；`unawaited_futures`、`discarded_futures`、`cancel_subscriptions`、`close_sinks` 管非同步資源。不准吞錯，也不准把 Future 丟著不管。

## 規則分類

| 類別 | 代表規則 | 攔下的 slop |
| --- | --- | --- |
| 型別 | `strict-casts`、`strict-inference`、`strict-raw-types`、`avoid_dynamic_calls` | `dynamic` 偷渡、隱式轉型、raw type |
| 死碼 | `unused_*`、`dead_code`（皆為 error）、`unnecessary_*` | 改一半的程式碼、複製貼上殘留 |
| 例外 | `avoid_catches_without_on_clauses`、`avoid_catching_errors`、`only_throw_errors`、`use_rethrow_when_possible` | 吞錯、亂丟非 Error 物件、遺失原始 stack trace |
| 非同步 | `unawaited_futures`、`discarded_futures`、`cancel_subscriptions`、`close_sinks`、`use_build_context_synchronously` | 放生 Future、忘記取消訂閱、async 後誤用 context |
| 一致性 | `directives_ordering`、`prefer_single_quotes`、`require_trailing_commas`、`eol_at_end_of_file`、`prefer_final_locals` | 風格漂移、無謂的 diff |
| 冗餘 | `avoid_redundant_argument_values`、`unnecessary_parenthesis`、`unnecessary_lambdas`、`avoid_unnecessary_containers` | 傳入預設值、多餘括號、包裝用 Container |
| 控制流 | `exhaustive_cases`、`no_default_cases`、`control_flow_in_finally`、`literal_only_boolean_expressions` | 漏掉的 enum case、finally 裡 return |

## 例外政策

- 只用**單行** `// ignore: rule_name — 理由`，理由要寫「為什麼這裡非這樣不可」，不是「先關掉」。
- 不使用 `ignore_for_file`。產生程式碼若真的需要，用 `exclude` 排除整個檔案，因為它們本來就不該手改。
- 目前基準線是 **0 issues**。新規則若造成大量違規，應該修程式碼或調整規則，而不是加 ignore。
- 若某條規則長期與 Flutter 官方風格衝突，請在 PR 說明後從設定移除，不要讓它變成每個人都要 ignore 的噪音。

## 執行方式

本機與 CI 使用同一組指令：

```bash
flutter pub get
dart format --output=none --set-exit-if-changed lib
flutter analyze --fatal-infos --fatal-warnings
```

`--fatal-infos --fatal-warnings` 是關鍵：lint 預設是 info 等級，不加這兩個參數，CI 永遠不會因為 lint 失敗。

## 為什麼排除某些路徑

| 路徑 | 原因 |
| --- | --- |
| `lib/src/rust/**` | `flutter_rust_bridge` 產生；要改的是 `rust/` 或 `rust/src/api.rs`，重新產生後 ignore 也會被覆蓋 |
| `rust_builder/cargokit/**` | 第三方建置工具，有自己的 `pubspec.yaml` 與 `analysis_options.yaml` |
| `**/*.g.dart`、`**/*.freezed.dart` | codegen 產物 |
| `build/**` | 建置產物 |

## Rust 端

`rust/Cargo.toml` 目前有兩組設定：

- `[lints.rust]`：`unsafe_op_in_unsafe_fn`、`missing_debug_implementations` 等。
- `[lints.clippy]`：`dbg!` 與 `todo!` 直接設為 `deny`，跑 `cargo clippy` 就會失敗，避免 AI 留下除錯輸出或未完成的佔位符。

完整防線建議用：

```bash
cargo clippy --all-targets
cargo clippy --all-targets -- -D warnings
```

目前 `cargo clippy --all-targets -- -D warnings` 已經是 **0 warning**（未使用依賴、多餘轉型、可合併的 `if`、缺 `Debug` 等都已清掉），可以直接設成 blocking。CI 還沒有 Rust job；要加的時候記得先安裝 ALSA 開發套件（`cpal` 在 Linux 上需要），再跑上面兩行。

`cargo fmt --check` 目前仍有 3 個檔案的差異（`cpu.rs`、`instructions/jump.rs`、`timer.rs`），原因是這些地方刻意寫成單行 `if/else`，與 rustfmt 預設的 `single_line_if_else_max_width` 不同。要把它設成 blocking 之前，先決定是改程式碼還是加 `rustfmt.toml`。

## 版本控制範圍

依專案要求，以下屬於本機開發資料，不納入版控（見 [`.gitignore`](../.gitignore)）：

- 任何 `test/`、`tests/`、`integration_test/`、`test_driver/` 資料夾
- 任何 `script/`、`scripts/` 資料夾
- 測試執行檔（`*.test.exe`、`*_test.exe`、`test.exe`、`tests.exe`）

若這些檔案已經被 Git 追蹤，`.gitignore` 不會自動移除，需要手動從索引取出：

```bash
git rm -r --cached test tests integration_test test_driver script scripts
```
