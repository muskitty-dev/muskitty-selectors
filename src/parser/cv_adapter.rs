//! Component-value → token adapter.
//!
//! The §5.4.1 `parse_a_grammar` entry point produces a
//! `Vec<ComponentValue>` (per §5.5.7 "consume a list of component
//! values"), and the [`Grammar`](muskitty_css::parser::Grammar) trait
//! hands that list to its `parse` method. The Selectors grammar is
//! defined in terms of tokens (§3 L4647-4653 references
//! `<ident-token>`, `<hash-token>`, etc.), and the existing selectors
//! parser consumes a [`TokenStream`] — so this module adapts a slice
//! of component values back into the equivalent token stream.
//!
//! The adaptation is lossless for selectors: every selector-valid
//! component value (preserved token, function, simple block) expands
//! back to its original token sequence:
//!
//! - [`ComponentValue::PreservedToken(t)`] → `t`
//! - [`ComponentValue::Function(f)`] → `Function(f.name)`, `f.value...`,
//!   `CloseParen` — the `Token::Function(name)` already represents
//!   `name(` (the `(` is absorbed into the Function token per §4.3.4
//!   "consume an ident-like token"), so we do NOT re-emit a separate
//!   `OpenParen` token. §5.5.6 "consume a function" absorbs only the
//!   trailing `)`, which we re-emit here.
//! - [`ComponentValue::SimpleBlock(b)`] → opening token, `b.value...`,
//!   closing token (matched to `b.kind`) — §5.5.5 "consume a simple
//!   block" absorbs BOTH the opening and closing tokens, so we re-emit
//!   both here.
//!
//! [`ComponentValue::PreservedToken(t)`]: muskitty_css::parser::ComponentValue::PreservedToken
//! [`ComponentValue::Function(f)`]: muskitty_css::parser::ComponentValue::Function
//! [`ComponentValue::SimpleBlock(b)`]: muskitty_css::parser::ComponentValue::SimpleBlock
//! [`TokenStream`]: muskitty_css::parser::TokenStream

use muskitty_css::parser::{BlockKind, ComponentValue};
use muskitty_css::tokenizer::Token;

/// Adapt a slice of component values into the equivalent token
/// stream.
///
/// Returns a `Vec<Token>` suitable for wrapping in a
/// [`TokenStream`](muskitty_css::parser::TokenStream). The trailing
/// `<EOF-token>` is appended by `TokenStream::new` (§5.3 L1811-1813),
/// so callers should NOT include `Token::Eof` in the returned vec.
pub fn cv_to_tokens(cvs: &[ComponentValue]) -> Vec<Token> {
    let mut out = Vec::with_capacity(cvs.len());
    extend_with_tokens(cvs, &mut out);
    out
}

fn extend_with_tokens(cvs: &[ComponentValue], out: &mut Vec<Token>) {
    for cv in cvs {
        match cv {
            ComponentValue::PreservedToken(t) => out.push(t.clone()),
            ComponentValue::Function(f) => {
                // §4.3.4: `Token::Function(name)` represents `name(` —
                // the `(` is absorbed into the Function token. §5.5.6
                // "consume a function" absorbs only the trailing `)`,
                // which we re-emit here. Do NOT emit a separate
                // `OpenParen`.
                out.push(Token::Function(f.name.clone()));
                extend_with_tokens(&f.value, out);
                out.push(Token::CloseParen);
            }
            ComponentValue::SimpleBlock(b) => {
                // §5.5.5: simple-block CV absorbs both the opening and
                // closing tokens — re-emit both, choosing the pair that
                // matches `b.kind`.
                let (open, close) = block_tokens(b.kind);
                out.push(open);
                extend_with_tokens(&b.value, out);
                out.push(close);
            }
        }
    }
}

/// §5.5.5: choose the open/close token pair for a simple block.
fn block_tokens(kind: BlockKind) -> (Token, Token) {
    match kind {
        BlockKind::Curly => (Token::OpenBrace, Token::CloseBrace),
        BlockKind::Square => (Token::OpenBracket, Token::CloseBracket),
        BlockKind::Paren => (Token::OpenParen, Token::CloseParen),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Preserved tokens pass through unchanged.
    #[test]
    fn preserved_tokens_pass_through() {
        let cvs = vec![
            ComponentValue::PreservedToken(Token::Ident("a".into())),
            ComponentValue::PreservedToken(Token::Whitespace),
            ComponentValue::PreservedToken(Token::Ident("b".into())),
        ];
        let tokens = cv_to_tokens(&cvs);
        assert_eq!(
            tokens,
            vec![
                Token::Ident("a".into()),
                Token::Whitespace,
                Token::Ident("b".into())
            ]
        );
    }

    /// Function CV expands to Function + value + CloseParen (NO
    /// separate OpenParen — the Function token itself represents
    /// `name(` per §4.3.4).
    #[test]
    fn function_expands_to_function_value_close() {
        let cvs = vec![ComponentValue::Function(muskitty_css::parser::Function {
            name: "not".into(),
            value: vec![ComponentValue::PreservedToken(Token::Ident("a".into()))],
        })];
        let tokens = cv_to_tokens(&cvs);
        assert_eq!(
            tokens,
            vec![
                Token::Function("not".into()),
                Token::Ident("a".into()),
                Token::CloseParen,
            ]
        );
    }

    /// Square-block CV expands to bracket pair.
    #[test]
    fn square_block_expands_to_brackets() {
        let cvs = vec![ComponentValue::SimpleBlock(
            muskitty_css::parser::SimpleBlock {
                kind: BlockKind::Square,
                value: vec![ComponentValue::PreservedToken(Token::Ident("attr".into()))],
            },
        )];
        let tokens = cv_to_tokens(&cvs);
        assert_eq!(
            tokens,
            vec![
                Token::OpenBracket,
                Token::Ident("attr".into()),
                Token::CloseBracket
            ]
        );
    }

    /// Nested CVs (function inside block) expand recursively.
    #[test]
    fn nested_cvs_expand_recursively() {
        let inner = ComponentValue::Function(muskitty_css::parser::Function {
            name: "f".into(),
            value: vec![ComponentValue::PreservedToken(Token::Ident("x".into()))],
        });
        let outer = ComponentValue::SimpleBlock(muskitty_css::parser::SimpleBlock {
            kind: BlockKind::Paren,
            value: vec![inner],
        });
        let tokens = cv_to_tokens(&[outer]);
        assert_eq!(
            tokens,
            vec![
                Token::OpenParen,
                Token::Function("f".into()),
                Token::Ident("x".into()),
                Token::CloseParen,
                Token::CloseParen,
            ]
        );
    }

    /// Empty input → empty output.
    #[test]
    fn empty_input_yields_empty_output() {
        let tokens = cv_to_tokens(&[]);
        assert!(tokens.is_empty());
    }
}
