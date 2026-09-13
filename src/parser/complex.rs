//! Complex-selector parsing.
//!
//! Implements the §3 grammar production:
//!
//! ```text
//! <complex-selector> = <complex-selector-unit> [ <combinator>? <complex-selector-unit> ]*
//! ```
//!
//! SP-2..SP-6 scope: parses one or more compound selectors joined by
//! the four §15 combinators (Descendant / Child / NextSibling /
//! SubsequentSibling). Trailing combinators (e.g. `a >`) produce an
//! `InvalidSelector` error. Mixed combinators (`a > b + c`), pseudo-
//! class-terminated compounds (`a > b:hover`), and selector lists
//! with trailing-comma / trailing-combinator / empty-input rejection
//! are all handled (see tests/parser_complex.rs).
//!
//! # Storage convention
//!
//! Per [`crate::types::ComplexSelector`]: storage is rightmost-first.
//! `units[0]` is the subject (rightmost compound in source order);
//! `units[len-1]` is the leftmost compound in source order. The
//! combinator on `units[idx]` links it to `units[idx+1]` (the next
//! leftward compound) and is stored on the rightward unit. The
//! leftmost unit (`units[len-1]`) always has `combinator == None`.
//!
//! For example, `.a > .b` parses to `units = [{ .b, Some(Child) },
//! { .a, None }]` — `.b` is the subject (`units[0]`) carrying the
//! Child combinator that links it to `.a`; `.a` is the leftmost unit
//! with `combinator == None`.
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §3 L4664-4665,
//! §15 L4360-4532 (combinators), §3 L4704-4741 (whitespace rules).

use crate::error::SelectorParseError;
use crate::parser::compound::parse_compound_selector;
use crate::types::{Combinator, ComplexSelector, ComplexSelectorUnit};
use muskitty_css::parser::TokenStream;
use muskitty_css::tokenizer::Token;

/// §3 L4664: Parse a `<complex-selector>`.
///
/// Parses one or more `<compound-selector>`s joined by combinators.
/// Storage is rightmost-first: `units[0]` is the subject (rightmost
/// compound), `units[len-1]` is the leftmost compound. The combinator
/// on each unit links it to the next leftward unit (`units[idx+1]`);
/// the leftmost unit has `combinator == None`.
///
/// # Combinator handling (§15 L4360-4532)
///
/// - `>` (Child), `+` (NextSibling), `~` (SubsequentSibling) —
///   explicit combinators; optional whitespace is allowed before and
///   after.
/// - Whitespace alone — implicit Descendant combinator; only valid
///   when followed by another compound selector. Trailing whitespace
///   (followed by a terminator) is not treated as a combinator.
/// - Trailing combinator (e.g. `a >`, `a +`) — `Err(InvalidSelector)`.
///
/// # Termination
///
/// The parser stops at any token that cannot extend the complex
/// selector: EOF, `,` (list separator), or a block-closing token
/// (`)`, `}`, `]`). The terminator is left unconsumed for the caller
/// (e.g. `parse_selector_list` consumes `,`; the API entry point
/// checks for unexpected trailing tokens).
/// 复杂选择器最大 compound 单元数（审计 F-4）。
///
/// 匹配侧 `walk_leftward` 对每个匹配单元递归（约 2 帧/单元），单元数不受限
/// 时，超长选择器（攻击者可写出任意长度）+ 足够深的 DOM 可栈溢出。解析期
/// 封顶，超限返回 `InvalidSelector`。1024 远超任何真实样式表（Chromium 的
/// 每规则选择器上限也是千级）。
///
/// 注意（SEL-3 复核）：1024 上限是**每个 complex** 的——逻辑组合
/// （`:is`/`:not`/`:where`）嵌套时每层各享一份，栈深 = 嵌套层 × 每层
/// 单元数。嵌套由解析期 [`MAX_SELECTOR_LIST_NESTING`]（simple.rs）与
/// 匹配期共享栈预算（matching/mod.rs）共同封顶，本上限只约束单层。
const MAX_COMPLEX_SELECTOR_UNITS: usize = 1024;

pub fn parse_complex_selector(
    stream: &mut TokenStream,
    has_depth: u8,
    sel_depth: u8,
    pseudo_elements_allowed: bool,
    compound_only: bool,
) -> Result<ComplexSelector, SelectorParseError> {
    // Build in source order (left-to-right), then reverse so storage
    // is rightmost-first. The combinator goes on the rightward unit
    // (the one just parsed), per the storage convention documented on
    // [`crate::types::ComplexSelector`].
    let mut units: Vec<ComplexSelectorUnit> = Vec::new();
    let first_compound = parse_compound_selector(
        stream,
        has_depth,
        sel_depth,
        pseudo_elements_allowed,
        compound_only,
    )?;
    units.push(ComplexSelectorUnit {
        compound: first_compound,
        combinator: None,
    });

    // W-3：compound-only 模式 —— 用于 `:host(<compound-selector>)` 参数及其
    // **嵌套**的 `:is()`/`:where()`/`:not()` 参数。此时只允许一个复合选择器，
    // 后面除终止符外任何东西（组合器或另一个复合选择器）都令解析失败。
    // 夹具依据（parse-is-where.html / parse-not.html）：
    // `:host(:not(.a))` valid、`:host(:not(.a .b))` invalid、
    // `:host(:is(div .foo))` forgiving-valid（失败的项被丢弃）、
    // `:host(:is(.a, .b+.c, .d))` forgiving-valid。
    if compound_only {
        stream.discard_whitespace();
        if !is_complex_terminator(&stream.next_token()) {
            return Err(SelectorParseError::InvalidSelector(
                "expected a single compound selector (this argument only accepts \
                 compound selectors)"
                    .into(),
            ));
        }
        units.reverse();
        return Ok(ComplexSelector { units });
    }

    loop {
        // F-4：单元数封顶（每个循环迭代至多追加一个单元）。
        if units.len() >= MAX_COMPLEX_SELECTOR_UNITS {
            return Err(SelectorParseError::InvalidSelector(format!(
                "complex selector exceeds {MAX_COMPLEX_SELECTOR_UNITS} compound units"
            )));
        }
        // Detect leading whitespace (potential implicit descendant
        // combinator). §3 L4724-4727: whitespace between two
        // complex-selector-units is required if no explicit combinator
        // is present.
        let had_whitespace = matches!(stream.next_token(), Token::Whitespace);
        if had_whitespace {
            stream.discard_whitespace();
        }

        // Try an explicit combinator (§15 L4422 / L4463 / L4505).
        let explicit_combinator = match stream.next_token() {
            Token::Delim('>') => {
                stream.discard_token();
                Some(Combinator::Child)
            }
            Token::Delim('+') => {
                stream.discard_token();
                Some(Combinator::NextSibling)
            }
            Token::Delim('~') => {
                stream.discard_token();
                Some(Combinator::SubsequentSibling)
            }
            _ => None,
        };

        if let Some(comb) = explicit_combinator {
            // Explicit combinator: consume optional trailing whitespace,
            // then require another compound selector. A terminator
            // here means a trailing combinator (e.g. `a >`).
            stream.discard_whitespace();
            if is_complex_terminator(&stream.next_token()) {
                return Err(SelectorParseError::InvalidSelector(
                    "trailing combinator in complex selector".into(),
                ));
            }
            let next_compound = parse_compound_selector(
                stream,
                has_depth,
                sel_depth,
                pseudo_elements_allowed,
                compound_only,
            )?;
            // Combinator goes on the new (rightward) unit.
            units.push(ComplexSelectorUnit {
                compound: next_compound,
                combinator: Some(comb),
            });
            continue;
        }

        // No explicit combinator. Without preceding whitespace this
        // is the end of the complex selector.
        if !had_whitespace {
            break;
        }

        // Had whitespace but no explicit combinator: either an
        // implicit descendant combinator (followed by another
        // compound) or trailing whitespace (followed by a
        // terminator).
        if is_complex_terminator(&stream.next_token()) {
            break; // trailing whitespace
        }

        // Implicit descendant combinator (§15 L4363). Parse the next
        // compound.
        let next_compound = parse_compound_selector(
            stream,
            has_depth,
            sel_depth,
            pseudo_elements_allowed,
            compound_only,
        )?;
        units.push(ComplexSelectorUnit {
            compound: next_compound,
            combinator: Some(Combinator::Descendant),
        });
    }

    // Reverse so storage is rightmost-first: units[0] = subject
    // (rightmost in source), units[len-1] = leftmost in source.
    units.reverse();

    // W-3：伪元素（pseudo-compound）只允许出现在**最右**复合选择器（subject，
    // units[0]）里。依据：selectors-4 §3 L790-800（pseudo-compound selector 不是
    // compound selector，"表现得像自带一个组合器"），WPT 夹具把
    // `::part(foo) + ::part(bar)`、`::slotted(foo) + ::slotted(bar)` 钉为
    // invalid——浏览器同样只接受 subject 位置的伪元素。反转后统一校验
    // units[1..]，中间单元里的伪元素（如 `.a .b::before .c`）也能被抓到。
    for unit in &units[1..] {
        if !unit.compound.pseudo_compounds.is_empty() {
            return Err(SelectorParseError::InvalidSelector(
                "pseudo-elements are only allowed in the rightmost compound \
                 selector of a complex selector"
                    .into(),
            ));
        }
    }

    Ok(ComplexSelector { units })
}

/// A token that cannot follow a compound selector in a complex
/// selector without an intervening combinator. Includes the
/// selector-list separator (`,`) and block-closing tokens that
/// terminate a selector list.
fn is_complex_terminator(token: &Token) -> bool {
    matches!(
        token,
        Token::Eof | Token::Comma | Token::CloseParen | Token::CloseBrace | Token::CloseBracket
    )
}
