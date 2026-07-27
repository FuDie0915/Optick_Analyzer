# Optick Analyzer

Optick `.opt` 性能分析文件解析器，纯 Rust 实现，零外部运行时依赖。

## 功能

- 解析 Optick `.opt` 二进制格式（线程、事件描述、帧数据、Fiber）
- 自动检测并解压 gzip / zlib 压缩的 `.opt` 文件
- 重建调用树并计算每个函数的 self time
- 帧耗时统计：P25/P50/P75/P90/P95/P99 + 均值/标准差
- 帧预算分析：60fps / 30fps / 自定义阈值超标率
- 跨帧热点聚合：函数级 / 模块级 / 调用频次 / 稳定性
- 调用者/被调用者关系图
- Pareto 集中度分析
- 关键路径提取（最慢帧最热函数调用链）
- 趋势分析：
  - 逐帧 self-time 序列，识别 **持续高开销**（PersistentHigh）与 **偶发尖峰**（SporadicSpike）模式
  - 交叉函数 Pearson 相关性分析，输出 |r| > 0.7 的关联函数对
- 自动化优化建议
- 双通道输出：
  - 终端 Unicode 格式化报告
  - 自包含 HTML 报告（内联 SVG 图表 + CSS，无外部依赖）

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

运行后同时输出：
1. **终端报告** — 写入 stdout，包含全部分析结果
2. **HTML 报告** — 写入当前目录，文件名格式 `{unix时间戳}_{原始文件名}.html`，包含帧耗时柱状图、函数 sparkline 趋势图、相关性表格

## 模块结构

```
src/
├── main.rs       编排入口：参数 → 读取 → 解压 → 解析 → 调用树 → 分析 → 终端报告 + HTML
├── binary.rs     二进制读取原语 (little-endian)
├── model.rs      数据模型 (解析产物 + 分析产物 + 趋势结构)
├── parser.rs     .opt 格式解析 (官方顺序格式)
├── call_tree.rs  调用树重建 + self time 计算
├── stats.rs      统计辅助函数 (分位数 / 相关性 / 回归)
├── analyzer.rs   分析聚合 (热点 / Pareto / 模块 / 调用者 / 趋势)
├── trend.rs      趋势分析 (模式分类 + 交叉函数相关性)
├── report.rs     终端报告输出 (Unicode 格式化)
└── html.rs       HTML 报告生成 (内联 SVG + CSS)
```

数据单向流动：

```
.opt 文件 → read_opt(解压) → parse → build_call_trees → analyze(含trend) → print_report + generate_html
```

## 依赖

| crate | 用途 |
|-------|------|
| `flate2` | gzip / zlib 解压 |

## 构建

```bash
cargo build --release
```

Release profile 已启用 `opt-level=3`、`lto=true`、`codegen-units=1`、`panic=abort`。

## 注意事项

- `.opt` 文件可能包含工程内部函数名、源文件路径等信息，注意不要将敏感 `.opt` 文件提交到公开仓库
- 本工具仅读取和分析本地文件，不会上传任何数据
- HTML 报告同样为纯本地生成，不引用任何外部 CDN 或远程资源