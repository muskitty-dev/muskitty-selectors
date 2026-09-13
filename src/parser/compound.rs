//! Compound-selector parsing.
//!
//! Implements the §3 grammar production:
//!
//! ```text
//! <compound-selector> = [ <type-selector>? <subclass-selector>* ]!
//!                        <pseudo-compound-selector>*
//! ```
//!
//! The `!` indicates the inner bracketed group is required to be
//! non-empty *unless* a pseudo-compound selector (pseudo-element +
//! trailing pseudo-classes) is present: `::before` alone is a valid
//! compound selector.
//!
//! SP-4 scope: subclass-selector supports `id`, `class`, `attribute`,
//! and `pseudo-class`; pseudo-compound selectors (modern `::name` and
//! legacy `:before`/`:after`/`:first-line`/`:first-letter`) are parsed
//! here and any trailing pseudo-classes attach to the most recent
//! pseudo-compound per §3 L762-787.
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §3 L4671 + L4684 +
//! L762-787, §13 L3792-4359 (tree-structural pseudo-classes), §14
//! (pseudo-elements).

use crate::error::SelectorParseError;
use crate::parser::simple::{
    parse_attribute_selector, parse_class_selector, parse_id_selector,
    parse_pseudo_class_or_legacy, parse_pseudo_element, parse_type_selector, PseudoClassOrLegacy,
};
use crate::types::{
    CompoundSelector, PseudoClass, PseudoCompoundSelector, PseudoElement, PseudoElementArgument,
    SubclassSelector,
};
use muskitty_css::parser::TokenStream;
use muskitty_css::tokenizer::Token;

/// §3 L4671 + L762-787: Parse a `<compound-selector>`.
///
/// Returns `Ok(CompoundSelector)` containing at least one simple
/// selector (type selector, subclass selector, or pseudo-compound
/// selector). Returns `Err(InvalidSelector)` if the input at the
/// current position does not start a compound selector.
///
/// `pseudo_elements_allowed`（W-3）：`false` 时伪元素（`::before` 等）即
/// **解析失败**。用于 §3 的 `<…-real-selector-list>` 上下文（`:is`/`:where`/
/// `:not`/`:has`/`nth-* of S` 的参数，以及 `::slotted()`/`:host()` 的
/// `<compound-selector>` 参数——`<compound-selector>` 产生式本身不含
/// `pseudo-compound-selector`）。WPT 夹具把这一区分钉在两侧：
/// `:is(::before)` / `:where(::before)` 是 **forgiving-valid**（该 selector
/// 被丢弃、列表仍合法），`:not(::before)` 是 **invalid**（非 forgiving，
/// 错误向上传播）。
///
/// # Phase structure
///
/// 1. **Subclass phase**: parse `id` / `class` / `attribute` /
///    `pseudo-class` until none of them match. A legacy pseudo-element
///    (`:before` / `:after` / `:first-line` / `:first-letter`) detected
///    via [`parse_pseudo_class_or_legacy`] terminates this phase and
///    starts the pseudo-compound phase with that pseudo-element as the
///    first entry.
/// 2. **Pseudo-compound phase**: parse zero or more `::name` modern
///    pseudo-elements, each optionally followed by trailing
///    pseudo-classes that attach to the most recent pseudo-compound per
///    §3 L762-787.
pub fn parse_compound_selector(
    stream: &mut TokenStream,
    has_depth: u8,
    sel_depth: u8,
    pseudo_elements_allowed: bool,
    compound_only: bool,
) -> Result<CompoundSelector, SelectorParseError> {
    // §3 L750-752: type selector (or universal selector) must come
    // first if present.
    let type_selector = parse_type_selector(stream)?;
    let mut compound = CompoundSelector {
        type_selector,
        ..CompoundSelector::default()
    };

    // ── Subclass phase ───────────────────────────────────────────
    // §3 L753-760: subclass selectors may appear in any order after
    // the type selector. §3 L4684: pseudo-class is a subclass-selector.
    // §14: legacy `:before` etc. redirect to the pseudo-compound phase.
    loop {
        if let Some(id) = parse_id_selector(stream)? {
            compound.subclasses.push(SubclassSelector::Id(id));
            continue;
        }
        if let Some(class) = parse_class_selector(stream)? {
            compound.subclasses.push(SubclassSelector::Class(class));
            continue;
        }
        if let Some(attr) = parse_attribute_selector(stream)? {
            compound.subclasses.push(SubclassSelector::Attribute(attr));
            continue;
        }
        match parse_pseudo_class_or_legacy(stream, has_depth, sel_depth, compound_only)? {
            PseudoClassOrLegacy::None => break,
            PseudoClassOrLegacy::Class(pc) => {
                compound.subclasses.push(SubclassSelector::PseudoClass(pc));
                continue;
            }
            PseudoClassOrLegacy::LegacyElement(pe) => {
                // §14 legacy single-colon pseudo-element. Subclasses
                // cannot follow a pseudo-element; switch to the
                // pseudo-compound phase.
                reject_pseudo_element_if_real(pseudo_elements_allowed, &pe.name)?;
                compound.pseudo_compounds.push(PseudoCompoundSelector {
                    pseudo_element: pe,
                    trailing_pseudo_classes: Vec::new(),
                });
                break;
            }
        }
    }

    // ── Pseudo-compound phase ───────────────────────────────────
    // §3 L762-787: zero or more pseudo-compound selectors may follow.
    // Each begins with a pseudo-element (modern `::name` or legacy
    // `:before`); any pseudo-classes appearing after a pseudo-element
    // in source order attach to that pseudo-element's
    // `trailing_pseudo_classes`.
    loop {
        // Try modern `::name` first. parse_pseudo_element leaves the
        // stream unmodified when it returns None.
        if let Some(pe) = parse_pseudo_element(stream)? {
            reject_pseudo_element_if_real(pseudo_elements_allowed, &pe.name)?;
            compound.pseudo_compounds.push(PseudoCompoundSelector {
                pseudo_element: pe,
                trailing_pseudo_classes: Vec::new(),
            });
            continue;
        }
        // Then try pseudo-class or legacy pseudo-element. A regular
        // pseudo-class here attaches to the most recent pseudo-compound
        // (e.g. `::before:hover`); a legacy pseudo-element starts a
        // new pseudo-compound entry.
        match parse_pseudo_class_or_legacy(stream, has_depth, sel_depth, compound_only)? {
            PseudoClassOrLegacy::None => break,
            PseudoClassOrLegacy::Class(pc) => {
                if let Some(last) = compound.pseudo_compounds.last_mut() {
                    check_trailing_pseudo_class(&last.pseudo_element, &pc)?;
                    last.trailing_pseudo_classes.push(pc);
                } else {
                    // Unreachable: the subclass phase only breaks out
                    // via `None` (which means parse_pseudo_class_or_legacy
                    // will return None here too) or `LegacyElement`
                    // (which already pushed one). The only other entry
                    // path is a successful `parse_pseudo_element`
                    // above, which also pushes one. Defensively treat
                    // the stray pseudo-class as a subclass.
                    compound.subclasses.push(SubclassSelector::PseudoClass(pc));
                }
            }
            PseudoClassOrLegacy::LegacyElement(pe) => {
                reject_pseudo_element_if_real(pseudo_elements_allowed, &pe.name)?;
                compound.pseudo_compounds.push(PseudoCompoundSelector {
                    pseudo_element: pe,
                    trailing_pseudo_classes: Vec::new(),
                });
            }
        }
    }

    // The `!` in the grammar requires the compound selector to be
    // non-empty: a type selector, a subclass selector, or a
    // pseudo-compound selector (§3 L762-787 allows a compound selector
    // to consist solely of a pseudo-element, e.g. `::before`).
    if compound.type_selector.is_none()
        && compound.subclasses.is_empty()
        && compound.pseudo_compounds.is_empty()
    {
        let next = stream.next_token();
        let msg = match next {
            Token::Eof => "expected a compound selector, got end of input".into(),
            _ => format!("expected a compound selector, got {:?}", next),
        };
        return Err(SelectorParseError::InvalidSelector(msg));
    }

    Ok(compound)
}

/// W-3：real-selector-list（或 `<compound-selector>` 参数）上下文中出现
/// 伪元素即解析失败。
///
/// 失败语义由调用方决定：forgiving 列表丢弃该 selector（`:is(::before)` 合法），
/// 非 forgiving 列表/参数把错误向上传播（`:not(::before)` 无效）。
fn reject_pseudo_element_if_real(
    pseudo_elements_allowed: bool,
    name: &str,
) -> Result<(), SelectorParseError> {
    if pseudo_elements_allowed {
        return Ok(());
    }
    Err(SelectorParseError::InvalidSelector(format!(
        "pseudo-element ::{name} is not allowed in a real selector list \
         (:is/:where/:not/:has/of S) or a compound-selector argument"
    )))
}

/// W-3：伪元素之后允许出现的伪类（夹具钉死的三条规则）。
///
/// 规范与夹具依据（详见 goal.md 规范依据表）：
/// - `<pseudo-compound-selector> = pseudo-element-selector pseudo-class-selector*`
///   原则上允许后随伪类（selectors-4 §3 L4665-4672，例 `.foo::before:hover`
///   L780-784），但：
/// - **`::slotted()` 后不允许伪类**：css-shadow-1 §slotted L471 只允许后随
///   *tree-abiding 伪元素*；夹具 `::slotted(foo):hover` / `::slotted(foo):first-child`
///   均为 invalid。
/// - **`:has()` 不允许后随伪元素**：夹具 `::part(foo):has(li)` invalid
///   （selectors-4 §4.5 note L1770-1775：`:has()` 取 relative-selector-list，
///   无法用于不允许 complex selector 的上下文）。
/// - **`:state()` 仅可紧跟 `::part()`**：夹具 `::part(inner):state(bar)` valid，
///   而 `::after:state()` / `::first-letter:state()` / `::slotted(foo):state()`
///   均 invalid（HTML custom state + css-shadow-1 §part L1201 "fully styleable"）。
fn check_trailing_pseudo_class(
    pseudo_element: &PseudoElement,
    pc: &PseudoClass,
) -> Result<(), SelectorParseError> {
    let is_slotted = matches!(
        pseudo_element.argument,
        Some(PseudoElementArgument::Slotted(_))
    );
    if is_slotted {
        return Err(SelectorParseError::InvalidSelector(format!(
            "::slotted() cannot be followed by a pseudo-class (:{}); only a \
             tree-abiding pseudo-element may follow it",
            pc.name
        )));
    }
    if pc.name == "has" {
        return Err(SelectorParseError::InvalidSelector(
            ":has() cannot be used after a pseudo-element".into(),
        ));
    }
    if pc.name == "state" {
        let is_part = matches!(
            pseudo_element.argument,
            Some(PseudoElementArgument::Part(_))
        ) && pseudo_element.name == "part";
        if !is_part {
            return Err(SelectorParseError::InvalidSelector(format!(
                ":state() may only follow ::part(), not ::{}",
                pseudo_element.name
            )));
        }
    }
    Ok(())
}
