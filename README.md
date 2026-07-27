# Optick Analyzer

Optick `.opt` 性能分析文件解析器，纯 Rust 实现，零外部依赖。

## 功能

- 解析 Optick `.opt` 二进制格式（线程、事件描述、帧数据）
- 重建调用树并计算每个函数的 self time
- 帧耗时统计：P25/P50/P75/P90/P95/P99 + 均值/标准差
- 帧预算分析：60fps / 30fps / 自定义阈值超标率
- 跨帧热点聚合：函数级 / 模块级 / 调用频次 / 稳定性
- 调用者/被调用者关系图
- Pareto 集中度分析
- 关键路径提取（最慢帧最热函数调用链）
- 自动化优化建议

## 用法

```bash
# 编译
cargo build --release

# 运行（默认阈值 100ms）
./target/release/opt_analyze <file.opt> [阈值ms]

# 示例
./target/release/opt_analyze capture.opt 100
./target/release/opt_analyze capture.opt 16.7   # 60fps 预算
```

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `file.opt` | Optick 导出的 `.opt` 文件路径 | `capture.opt` |
| `阈值ms` | 卡顿帧判定阈值（毫秒） | `100.0` |

## 模块结构

```
src/
├── main.rs       编排入口
├── binary.rs     二进制读取原语 (little-endian)
├── model.rs      数据模型 (解析产物 + 分析产物)
├── parser.rs     .opt 格式解析
├── call_tree.rs  调用树重建 + self time 计算
├── stats.rs      统计辅助函数
├── analyzer.rs   分析聚合
└── report.rs     报告输出
```

数据单向流动：

```
.opt 文件 → parse → build_call_trees → analyze → print_report
```

## 构建

```bash
cargo build --release
```

Release profile 已启用 `opt-level=3`、`lto=true`、`codegen-units=1`、`panic=abort`。

## 注意事项

- `.opt` 文件可能包含工程内部函数名、源文件路径等信息，注意不要将敏感 `.opt` 文件提交到公开仓库
- 本工具仅读取和分析本地文件，不会上传任何数据