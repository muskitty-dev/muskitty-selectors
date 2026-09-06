//! Selector-list parsing.
//!
//! Implements the §3 grammar production:
//!
//! ```text
//! <complex-selector-list> = <complex-selector>#
//! ```
//!
//! Where `#` indicates a comma-separated list of one or more
//! productions. Trailing comma is not allowed.
//!
//! Also implements `forgiving-selector-list` (§3 L4765-4813) used by
//! `:is()` / `:where()`: parses each complex selector independently
//! and silently drops the ones that fail.
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §3 L4651-4653 +
//! L4765-4813.

use crate::error::SelectorParseError;
use crate::parser::complex::parse_complex_selector;
use crate::types::SelectorList;
use muskitty_css::parser::TokenStream;
use muskitty_css::tokenizer::Token;

/// §3 L4651-4653: Parse a `<complex-selector-list>` =
/// `<complex-selector>#`.
///
/// Parses one or more complex selectors separated by commas. Stops at
/// EOF or any token that does not start a complex selector (e.g.
/// `)` for a function argument, `}` for a block). The terminator is
/// left unconsumed.
///
/// Whitespace around commas is discarded (§3 L4704-4727 — whitespace is
/// allowed between a comma and the following complex selector).
///
/// Returns `Err(InvalidSelector)` if no complex selector was parsed
/// (empty list), or if a comma is followed by EOF / a terminator (a
/// trailing comma).
///
/// `has_depth` 是当前 `:has()` 参数嵌套深度（SEL-2，见
/// [`parse_pseudo_class_or_legacy`]/simple.rs），原样向下游透传。
/// `sel_depth` 是选择器列表参数嵌套深度（SEL-3），原样向下游透传。
pub fn parse_selector_list(
    stream: &mut TokenStream,
    has_depth: u8,
    sel_depth: u8,
) -> Result<SelectorList, SelectorParseError> {
    let mut selectors = Vec::new();

    // Required first complex selector.
    stream.discard_whitespace();
    selectors.push(parse_complex_selector(stream, has_depth, sel_depth)?);

    // Optional trailing complex selectors separated by commas.
    loop {
        stream.discard_whitespace();
        match stream.next_token() {
            Token::Comma => {
                stream.discard_token(); // consume ','
                stream.discard_whitespace();
                // Must be followed by another complex selector.
                if is_terminator(&stream.next_token()) {
                    return Err(SelectorParseError::InvalidSelector(
                        "trailing comma in selector list".into(),
                    ));
                }
                selectors.push(parse_complex_selector(stream, has_depth, sel_depth)?);
            }
            _ => break, // terminator or other token — stop, leave it unconsumed.
        }
    }

    Ok(SelectorList(selectors))
}

/// §3 L4765-4813: Parse a `forgiving-selector-list`.
///
/// Like [`parse_selector_list`], but each complex selector is parsed
/// independently. Selectors that fail to parse are silently dropped
/// instead of failing the whole list. If every selector fails, the
/// returned list is empty (this is permitted by the forgiving
/// production).
///
/// SEL-2：失败 selector 的残留在复杂构造（如被拒绝的嵌套 `:has(...)`，
/// 经 `parse_pseudo_class_or_legacy` 的 restore 回退到 `:` 起点）时不止
/// 一个 token，逐 token 跳过会让外层 `)` 配对错位。恢复改为配平跳过：
/// 消费失败 selector 的全部剩余 token，在括号深度 0 的 `,`（列表分隔
/// 符）或 `)`（参数结束符）之前停下（WPT parse-has-forgiving-selector）。
pub fn parse_forgiving_selector_list(
    stream: &mut TokenStream,
    has_depth: u8,
    sel_depth: u8,
) -> Result<SelectorList, SelectorParseError> {
    let mut selectors = Vec::new();

    stream.discard_whitespace();
    // First selector — if it fails, just skip it (no preceding comma
    // to consume, the caller manages stream state for non-forgiving
    // invocations).
    match parse_complex_selector(stream, has_depth, sel_depth) {
        Ok(cs) => selectors.push(cs),
        Err(_) => {
            // Best-effort recovery: the spec doesn't precisely
            // describe recovery, but per §3 L4789-4799 "parse as a
            // forgiving selector list" delegates to "parse a list of
            // complex-real-selectors" which itself drops failures.
            skip_failed_selector_remnants(stream);
        }
    }

    loop {
        stream.discard_whitespace();
        match stream.next_token() {
            Token::Comma => {
                stream.discard_token(); // consume ','
                stream.discard_whitespace();
                if is_terminator(&stream.next_token()) {
                    // Trailing comma in forgiving mode: treat as end
                    // of list, do not error.
                    break;
                }
                match parse_complex_selector(stream, has_depth, sel_depth) {
                    Ok(cs) => selectors.push(cs),
                    Err(_) => skip_failed_selector_remnants(stream),
                }
            }
            _ => break,
        }
    }

    Ok(SelectorList(selectors))
}

/// Forgiving 恢复：跳过失败 complex selector 的全部剩余 token。
///
/// 在**括号深度 0** 的 `,`（下一个列表项的分隔符，留给外层循环消费）
/// 或 `)`（所在伪类参数的结束符，留给参数解析的收尾检查）之前停下；
/// `Function`/`OpenParen` 深度 +1、配对的 `)` 深度 -1，保证跨整个失败
/// 构造（如 `:has(> .a .b)`）。EOF 直接终止（防御未闭合输入）。
fn skip_failed_selector_remnants(stream: &mut TokenStream) {
    let mut depth: usize = 0;
    loop {
        match stream.next_token() {
            Token::Eof => return,
            Token::Comma | Token::CloseParen if depth == 0 => return,
            Token::Function(_) | Token::OpenParen => {
                depth += 1;
                stream.discard_token();
            }
            Token::CloseParen => {
                depth = depth.saturating_sub(1);
                stream.discard_token();
            }
            _ => stream.discard_token(),
        }
    }
}

/// Heuristic: a token that cannot start a complex selector and
/// therefore terminates a selector list. Anything not in this set is
/// treated as a potential complex-selector starter.
fn is_terminator(token: &Token) -> bool {
    matches!(
        token,
        Token::Eof | Token::CloseParen | Token::CloseBrace | Token::CloseBracket
    )
}
