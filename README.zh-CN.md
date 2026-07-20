# muskitty-selectors

[English](README.md) | [简体中文](README.zh-CN.md)

[![crates.io](https://img.shields.io/crates/v/muskitty-selectors.svg)](https://crates.io/crates/muskitty-selectors)
[![Documentation](https://docs.rs/muskitty-selectors/badge.svg)](https://docs.rs/muskitty-selectors)
[![License](https://img.shields.io/crates/l/muskitty-selectors.svg)](https://github.com/muskitty-dev/muskitty-selectors/blob/main/LICENSE)

一个从零开始、用纯 Rust 编写的 Selectors Level 4 解析器与匹配引擎，基于
[`muskitty-css-parser`](https://crates.io/crates/muskitty-css-parser) 实现
[Selectors Level 4](https://drafts.csswg.org/selectors-4/) 规范。

属于 [MusKitty](https://github.com/muskitty-dev) 浏览器引擎项目的一部分。

## 状态

| 组件 | 规范覆盖范围 | 测试数 |
|-----------|---------------|-------|
| §3 Data Model | L716-1357 | 6 |
| §5 Elemental selectors | L1805-1995 | 10 |
| §6 Attribute selectors | L1996-2533 | 11 |
| §4 Logical combinations | L1358-1804 | 10 |
| §13 Tree-structural pseudo-classes | L3792-4359 | 12 |
| §15 Combinators | L4360-4532 | 12 |
| §17 Specificity | L4534-4633 | 22 |
| §18 Matching engine | L4816-5026 | 19+ |
| **合计** | §3 / §4 / §5 / §6 / §13 / §14 / §15 / §17 / §18 | **145** |

- 零 `unsafe` 代码
- 零 C/C++ 依赖
- 运行时依赖：`muskitty-css-parser`（该 crate 会再导出 `muskitty-css-tokenizer`）
- 仅使用 Rust 稳定版工具链
- MSRV 1.82

## 安装

将以下内容添加到你的 `Cargo.toml`：

```toml
[dependencies]
muskitty-selectors = "0.1.0"
```

或运行：

```bash
cargo add muskitty-selectors
```

## 快速入门

```rust
use muskitty_selectors::{parse_a_selector, matches, Specificity};

let list = parse_a_selector("div.foo > span").unwrap();
let spec: Specificity = list.specificity_max();
// (0, 1, 2) — 一个类选择器 + 两个类型选择器。
```

若要针对你自己的元素树进行匹配，请实现
`muskitty_selectors::Element` trait：

```rust
use muskitty_selectors::{parse_a_selector, matches, Element};

#[derive(Clone)]
struct MyElement { /* ... */ }

impl Element for MyElement {
    fn local_name(&self) -> String { /* ... */ }
    // ... 其余 13 个 trait 方法
}

let list = parse_a_selector("a:hover").unwrap();
let el = MyElement { /* ... */ };
if matches(&list, &el) {
    // ...
}
```

## 架构

```
muskitty-selectors/
  src/
    types.rs              §3 数据模型：SelectorList、ComplexSelector、
                          CompoundSelector、SubclassSelector、PseudoClass、
                          PseudoElement、AnPlusB、Combinator
    error.rs              SelectorParseError
    specificity.rs        §17 A/B/C 三元组计算
    parser/               §3 文法产生式
      mod.rs              parse_a_selector / parse_a_relative_selector 入口
      simple.rs           §5 / §6.5 / §6.6 类型 / 通配 / 类 / id 选择器
      attribute.rs        §6 属性选择器（存在 / 精确匹配 / ~= / |= / ^= / $= / *=）
      compound.rs         §3 compound-selector 产生式
      complex.rs          §3 complex-selector 产生式 + §15 组合器
      list.rs             §3 selector-list 产生式（逗号分隔）
      an_plus_b.rs        §13.5 An+B 表示法
      relative.rs         §4.5 用于 :has() 的相对选择器
    matching/             §18 匹配引擎
      mod.rs              Element trait + matches / query_selector / query_selector_all
                          + 从右向左遍历 complex-selector（§18 L4902-4919）
      simple_matcher.rs   §5 / §6 simple-selector 匹配
      pseudo_matcher.rs   §13 树结构 + An+B + §4 逻辑组合
      dom_impl.rs         针对 muskitty_dom::Node 的参考实现（仅开发用）
  tests/
    11 个测试文件，共 145 个测试
```

### 规范覆盖范围

**解析器（Parser）** —— 消费由 `muskitty_css_parser::tokenize` 生成的
token 流，并构建选择器数据结构。不依赖 DOM。

- §3 Data Model and Parsing
- §4 Logical Combinations（`:is()` / `:where()` / `:not()` / `:has()`）
- §5 Elemental Selectors（type / universal）
- §6 Attribute Selectors（全部 7 种语法 + 修饰符）
- §6.5 Class selectors、§6.6 ID selectors
- §13 Tree-Structural Pseudo-classes（`:root` / `:empty` / `:first-child` /
  `:nth-child(An+B [of S]?)` 等）
- §15 Combinators（Descendant / Child / NextSibling / SubsequentSibling）
- §3 Compound 与 Complex 选择器

**优先级（Specificity）** —— 按 §17 计算 A/B/C 三元组。

- `:is()` / `:not()` / `:has()` 取其参数中的最大优先级
- `:where()` 永远贡献 0
- `:nth-child(An+B of S)` 会加上 `S` 的优先级

**匹配（Matching）** —— 通过 `Element` trait 将解析后的选择器与
元素树进行匹配。引擎按 §18 L4902-4919 的规定从右向左遍历 complex selector。

- §18 `matches(selector, element)` —— 单元素匹配测试
- §18 `query_selector(root, selector)` —— 按树序返回第一个匹配
- §18 `query_selector_all(root, selector)` —— 按树序返回所有匹配

### 暂未实现

- `:has()` 多 compound 相对选择器（`:has(.a > .b)`）—— SP-8
  目前仅支持单 compound；多 compound 会返回 `false`。
- 严格的命名空间匹配（`ns|tag`）—— 当前按"任意命名空间"处理。
- §7-§12 UI / location / linguistic / resource / display / input
  伪类 —— 支持解析，匹配桩实现返回 `false`。
- WPT 子集集成。

## 构建

```bash
cargo check
cargo build
```

## 测试

```bash
# 全部 145 个测试
cargo test
```

## 设计原则

1. **CSSWG 是唯一事实来源** —— 实现严格遵循规范。
2. **对齐规范，而非对齐测试** —— 测试用于验证代码；除非规范证明测试有误，
   否则绝不为了通过测试而修改代码。
3. **从右向左匹配** —— 按 §18 L4902-4919 的规定从右向左遍历
   complex selector（先匹配主体，再匹配祖先/兄弟）。
4. **零 unsafe** —— 纯 safe Rust。

## 规范参考

本实现参考了以下规范：

- [Selectors Level 4](https://drafts.csswg.org/selectors-4/) —— 主要权威来源
  - §3: Data Model and Selectors Parsing
  - §4: Logical Combinations
  - §5: Elemental Selectors
  - §6: Attribute Selectors
  - §13: Tree-Structural Pseudo-classes
  - §15: Combinators
  - §17: Specificity
  - §18: API Hooks（匹配引擎）

## 许可证

基于 Apache License, Version 2.0 许可。详见 [LICENSE](LICENSE)。

Copyright 2026 MusCat / MusKitty Bit-Torch Community
