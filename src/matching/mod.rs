//! Selectors Level 4 §18 matching engine.
//!
//! Matches parsed selectors ([`crate::types::SelectorList`] /
//! [`crate::types::ComplexSelector`]) against an element tree via the
//! [`Element`] trait. The engine walks complex selectors right-to-left
//! per §18 L4902-4919.
//!
//! # Architecture
//!
//! - [`Element`] trait — abstracts the 5 aspects of an element (§3
//!   L865-874: type / namespace / id / classes / attributes) plus
//!   tree traversal required by §13 child-indexed pseudo-classes and
//!   §15 combinators.
//! - [`simple_matcher`] — type / universal / class / id / attribute.
//! - [`pseudo_matcher`] — tree-structural + An+B + logical combinations.
//! - [`dom_impl`] — `Element` impl for `muskitty_dom::Node` (feature
//!   `dom`). Provides [`DomElement`].

#[cfg(feature = "dom")]
pub mod dom_impl;
#[cfg(feature = "dom")]
pub use dom_impl::DomElement;

pub mod pseudo_matcher;
pub mod simple_matcher;

use crate::types::{Combinator, ComplexSelector, ComplexSelectorUnit, SelectorList};
use std::cell::Cell;
use std::collections::HashMap;

/// §3 L858-873 + §18 L4879-4900: read-only view of an element in a
/// tree.
///
/// Implementors provide the 5 aspects of an element (type / namespace
/// / id / classes / attributes) plus the tree-traversal operations
/// required by §13 child-indexed pseudo-classes (parent / sibling
/// iteration) and §15 combinators (parent for Child / ancestor for
/// Descendant / siblings for NextSibling / SubsequentSibling).
///
/// `Self: Clone` is required so that trait methods can return owned
/// copies of the element handle (e.g. `parent_element()` returns
/// `Option<Self>`). For `Rc<RefCell<Node>>` this is a cheap `Rc`
/// clone.
///
/// Methods return owned `String` / `Vec<String>` (not `&str`) because
/// underlying element data is often behind a `RefCell` whose borrow
/// guard cannot escape the function returning the reference.
pub trait Element: Clone {
    /// §3 L870: element type (tag name). Lowercase for HTML.
    fn local_name(&self) -> String;

    /// §3 L871: namespace URI (`None` for no namespace).
    fn namespace_uri(&self) -> Option<String>;

    /// §3 L872: ID attribute value (`None` if absent).
    fn id(&self) -> Option<String>;

    /// §3 L873: classes (space-separated list, may be empty).
    fn classes(&self) -> Vec<String>;

    /// §3 L874: attribute lookup by name. HTML namespace: ASCII
    /// case-insensitive name comparison.
    fn get_attribute(&self, name: &str) -> Option<String>;

    /// Parent element (`None` for root / detached).
    fn parent_element(&self) -> Option<Self>;

    /// Previous sibling element (`None` if first child).
    fn previous_sibling_element(&self) -> Option<Self>;

    /// Next sibling element (`None` if last child).
    fn next_sibling_element(&self) -> Option<Self>;

    /// Iterate child elements (excluding text / comment nodes).
    fn child_elements(&self) -> Vec<Self>;

    /// §13.3 L3820: whether this is the document root (no parent
    /// element). Default impl checks `parent_element().is_none()`.
    fn is_root(&self) -> bool {
        self.parent_element().is_none()
    }

    /// §13.3 L3837-3845: whether the element has no children except
    /// optionally whitespace-only text nodes. Comments and PIs do
    /// not affect emptiness.
    fn is_empty(&self) -> bool;

    /// §13.3 L3982: 1-based index among inclusive siblings (all
    /// element siblings, including self).
    fn index_among_siblings(&self) -> usize;

    /// Total count of inclusive siblings (all element siblings,
    /// including self).
    fn count_among_siblings(&self) -> usize;

    /// §13.3: 1-based index among siblings of the same type (same
    /// `local_name`, case-insensitive).
    fn index_among_type(&self) -> usize;

    /// Total count of siblings of the same type (including self).
    fn count_among_type(&self) -> usize;
}

/// §18 L4878-4919: Match a selector list against an element.
///
/// Returns `true` if any complex selector in `list` matches `element`
/// (right-to-left walk per §18 L4902-4919).
pub fn matches<E: Element>(list: &SelectorList, element: &E) -> bool {
    // SEL-3：建立匹配会话；列表内每个 complex 各享满额栈预算
    // （列表项之间独立，与 SEL-1 每 complex 新建 MatchState 一致）。
    let saved = enter_match_session();
    let result = list.0.iter().any(|cs| {
        reset_match_budget();
        matches_complex(cs, element)
    });
    exit_match_session(saved);
    result
}

// ── SEL-3：匹配期共享栈预算 ─────────────────────────────────────
//
// 逻辑组合参数（`:is`/`:not`/`:where` 列表、`:has` 相对选择器、
// `:nth-* of S` 过滤）在匹配期重新进入 complex 匹配，每层各享一份
// 1024 单元上限（解析期 `MAX_COMPLEX_SELECTOR_UNITS` 只约束单个
// complex）。栈深 = 嵌套层 × 每层单元数：解析期嵌套封顶 32 后仍有
// 32 × 1024 ≈ 32k 帧的溢出窗口。本预算把（嵌套重入 + 左向单元
// 消费）合并到单一池中，超限按不匹配降级（与 SEL-1 步数预算同
// 语义），令单次（元素 × complex）匹配的活跃栈帧有硬上界。

/// SEL-3：单次（元素 × complex selector）匹配的共享栈预算上限
/// （嵌套重入 + 左向单元消费合计）。
const MAX_MATCH_STACK_BUDGET: usize = 2048;

thread_local! {
    /// 剩余预算。`None` = 当前线程无进行中的顶层匹配会话。顶层
    /// [`matches`] 进入时建立并保存旧值；嵌套重入点在 `None` 时
    /// 惰性建立（覆盖绕过 `matches` 直接调用 `matches_pseudo_class`
    /// 等 pub 子匹配器的场景——该路径不复位，多次调用共享递减，
    /// 仅影响预算耗尽后的降级，无安全问题）。
    static MATCH_STACK_BUDGET: Cell<Option<usize>> = const { Cell::new(None) };
}

/// 进入顶层匹配会话，返回需在退出时恢复的旧值。
fn enter_match_session() -> Option<usize> {
    MATCH_STACK_BUDGET.with(|c| {
        let old = c.get();
        c.set(Some(MAX_MATCH_STACK_BUDGET));
        old
    })
}

/// 会话内将预算重置为满额（每个 complex 一次）。
fn reset_match_budget() {
    MATCH_STACK_BUDGET.with(|c| {
        if c.get().is_some() {
            c.set(Some(MAX_MATCH_STACK_BUDGET));
        }
    });
}

/// 退出顶层匹配会话，恢复旧值。
fn exit_match_session(saved: Option<usize>) {
    MATCH_STACK_BUDGET.with(|c| c.set(saved));
}

/// SEL-3：消费 1 单位栈预算。返回 `false` = 预算耗尽，调用方按不
/// 匹配降级。会话不存在时惰性建立满额会话。
pub(crate) fn consume_match_budget() -> bool {
    MATCH_STACK_BUDGET.with(|c| {
        let budget = match c.get() {
            None => {
                c.set(Some(MAX_MATCH_STACK_BUDGET));
                MAX_MATCH_STACK_BUDGET
            }
            Some(b) => b,
        };
        if budget == 0 {
            return false;
        }
        c.set(Some(budget - 1));
        true
    })
}

/// `pub(crate)` wrapper around [`matches`] for use by sibling modules
/// (e.g. `pseudo_matcher` resolving `:nth-child(An+B of S)` filters).
/// Kept separate from [`matches`] so the public API surface stays
/// flat.
///
/// SEL-3：这是逻辑组合参数重入 complex 匹配的入口之一，消费 1 单位
/// 栈预算（预算耗尽按不匹配降级）。
pub(crate) fn matches_complex_list<E: Element>(list: &SelectorList, element: &E) -> bool {
    if !consume_match_budget() {
        return false;
    }
    list.0.iter().any(|cs| matches_complex(cs, element))
}

/// §18 L4955-5026: Match a selector list against a tree, returning
/// the first matching element in tree order. Returns `None` if no
/// descendant of `root` matches.
pub fn query_selector<E: Element>(root: &E, list: &SelectorList) -> Option<E> {
    query_selector_all(root, list).into_iter().next()
}

/// §18 L4955-5026: Match a selector list against a tree, returning
/// all matching elements in tree order (depth-first, pre-order).
pub fn query_selector_all<E: Element>(root: &E, list: &SelectorList) -> Vec<E> {
    let mut out = Vec::new();
    walk_tree(root, &mut |el: &E| {
        if matches(list, el) {
            out.push(el.clone());
        }
    });
    out
}

/// Depth-first pre-order walk of `root`'s subtree (including root
/// itself).
fn walk_tree<E: Element, F: FnMut(&E)>(root: &E, f: &mut F) {
    f(root);
    for child in root.child_elements() {
        walk_tree(&child, f);
    }
}

/// SEL-1：单次（元素 × complex selector）匹配的左向匹配状态。
///
/// 三件事：
/// - **祖先链**（`chain`）：subject 的祖先，`chain[0]` = 父元素，
///   `chain[i]` 再向上，**惰性**增长（`ensure_chain` 只在索引被访问时
///   才向上走一步，浅组合器规则不为深 DOM 付全程 walk）。左向匹配
///   到访的任何元素的 parent 必为 subject 的祖先（归纳：从 subject
///   出发，Child/Descendant 步上移一层、sibling 步同层，而同层兄弟的
///   parent 同为链上元素），因此 Descendant/Child 候选可按
///   `chain[level..]` 索引访问，元素无需身份标识。
/// - **Descendant 记忆化**（`memo`）：`continues_leftward(remaining,
///   chain[i])` 的结果以 `(i, remaining 起始下标)` 为 key 缓存。
///   `.x .x … .x`（k 个组合器）对全 `.x` 的 D 深 DOM 原始回溯为
///   C(D-1, k-1) 条候选路径（k=10、D=100 ≈ 1.7×10¹²，单条规则即可
///   挂起数分钟）；记忆化后每 (祖先, 剩余后缀) 只求值一次，D×k 封顶。
/// - **步数预算**（`budget`）：compound 匹配计数，超限按不匹配降级
///   （审计 SEL-1 ①，10 万/元素×规则）。兄弟组合器（`~`）无记忆化，
///   宽兄弟树 × `a ~ a ~ …` 仍是指数路径计数，由预算兜底。
struct MatchState<E: Element> {
    /// subject 的祖先链（惰性）。`chain[i]` 的层级（距 subject 的
    /// parent 步数）为 `i + 1`。
    chain: Vec<E>,
    /// `chain` 已延伸到根，不再增长。
    chain_complete: bool,
    /// `(chain 索引, remaining 起始下标)` → `continues_leftward` 结果。
    memo: HashMap<(usize, usize), bool>,
    /// 剩余 compound 匹配步数。
    budget: usize,
}

/// SEL-1：每次（元素 × complex selector）匹配的 compound 匹配步数
/// 上限。超限按不匹配降级 —— 真实页面单条规则的左向匹配远达不到
/// （Descendant 已被记忆化压到 D×k），仅拦截敌意的兄弟组合器路径
/// 爆炸与病态构造。
const MAX_MATCH_STEPS: usize = 100_000;

impl<E: Element> MatchState<E> {
    fn new() -> Self {
        Self {
            chain: Vec::new(),
            chain_complete: false,
            memo: HashMap::new(),
            budget: MAX_MATCH_STEPS,
        }
    }

    /// 预算内做一次 compound 匹配；预算耗尽按不匹配降级。
    fn try_compound(&mut self, compound: &crate::types::CompoundSelector, element: &E) -> bool {
        if self.budget == 0 {
            return false;
        }
        self.budget -= 1;
        simple_matcher::matches_compound(compound, element)
    }

    /// 确保祖先链覆盖到索引 `idx`（含）。从已知最深处向上走一步，
    /// 到根即标记 `chain_complete`。
    ///
    /// 空链分支（`chain.last() == None`）只在层级 0 被调用时可达：
    /// 层级 ≥ 1 的元素必然经由某个链上元素到达（chain 已非空）。
    /// 层级 0 的 `element` 是 subject 或其兄弟，二者 parent 同为
    /// `chain[0]`，故从 `element` 起步等价于从 subject 起步。
    fn ensure_chain(&mut self, level0_element: &E, idx: usize) {
        while self.chain.len() <= idx && !self.chain_complete {
            let next = match self.chain.last() {
                Some(deepest) => deepest.parent_element(),
                None => level0_element.parent_element(),
            };
            match next {
                Some(ancestor) => self.chain.push(ancestor),
                None => self.chain_complete = true,
            }
        }
    }
}

/// §18 L4902-4919: Match a complex selector against an element,
/// processing compound selectors right-to-left.
///
/// `units[0]` is the subject (rightmost compound in source order).
/// The combinator on `units[idx]` links it to `units[idx+1]` (the
/// next leftward compound): e.g. for `a > b`, `units = [{b, Child},
/// {a, None}]` and `b`'s parent must match `a`. We match the subject
/// first, then walk leftward checking combinators against related
/// elements.
fn matches_complex<E: Element>(cs: &ComplexSelector, element: &E) -> bool {
    if cs.units.is_empty() {
        return false;
    }
    // §18 L4908: the rightmost compound (units[0], the subject)
    // must match `element`.
    let subject = &cs.units[0];
    // 单 compound 选择器不做左向 walk，无需构建匹配状态（热路径）。
    if cs.units.len() == 1 {
        return simple_matcher::matches_compound(&subject.compound, element);
    }
    let mut state = MatchState::new();
    if !state.try_compound(&subject.compound, element) {
        return false;
    }
    // §18 L4914-4919: otherwise, walk leftward using the subject's
    // combinator to find candidates for units[1..].
    let combinator = match subject.combinator {
        Some(c) => c,
        None => return true, // shouldn't happen for len > 1
    };
    walk_leftward(&cs.units[1..], 1, element, 0, combinator, &mut state)
}

/// Walk leftward from `element` by `combinator` to find candidates
/// for `remaining[0]`. `combinator` was carried by the previous
/// (rightward) unit, so it describes how `remaining[0]` is related
/// to `element` (e.g. for `Child`, `remaining[0]` is the parent of
/// `element`).
///
/// SEL-1：`start` 是 `remaining` 在完整单元序列中的起始下标（记忆化
/// key 的一部分）；`level` 是 `element` 的层级（距 subject 的 parent
/// 步数，sibling 步不改变层级），Descendant/Child 候选 = `state.chain`
/// 自 `level` 起的后缀。
fn walk_leftward<E: Element>(
    remaining: &[ComplexSelectorUnit],
    start: usize,
    element: &E,
    level: usize,
    combinator: Combinator,
    state: &mut MatchState<E>,
) -> bool {
    // SEL-3：每帧消费 1 单位栈预算，令活跃递归深度有硬上界。
    if !consume_match_budget() {
        return false;
    }
    let next_unit = &remaining[0];
    match combinator {
        Combinator::Descendant => {
            // §15 L4369: any ancestor of `element` = chain[level..]。
            let mut idx = level;
            loop {
                state.ensure_chain(element, idx);
                if idx >= state.chain.len() {
                    return false;
                }
                let ancestor = state.chain[idx].clone();
                if state.try_compound(&next_unit.compound, &ancestor)
                    && continues_on_chain(remaining, start, idx, &ancestor, state)
                {
                    return true;
                }
                idx += 1;
            }
        }
        Combinator::Child => {
            // §15 L4376: direct parent only = chain[level]。
            state.ensure_chain(element, level);
            if level < state.chain.len() {
                let parent = state.chain[level].clone();
                if state.try_compound(&next_unit.compound, &parent)
                    && continues_on_chain(remaining, start, level, &parent, state)
                {
                    return true;
                }
            }
            false
        }
        Combinator::NextSibling => {
            // §15 L4383: direct previous sibling only. 兄弟不在祖先链
            // 上（无记忆化，预算兜底），层级与 `element` 相同。
            if let Some(prev) = element.previous_sibling_element() {
                if state.try_compound(&next_unit.compound, &prev)
                    && continues_leftward(remaining, start, &prev, level, state)
                {
                    return true;
                }
            }
            false
        }
        Combinator::SubsequentSibling => {
            // §15 L4390: any previous sibling.
            let mut prev = element.previous_sibling_element();
            while let Some(sibling) = prev {
                if state.try_compound(&next_unit.compound, &sibling)
                    && continues_leftward(remaining, start, &sibling, level, state)
                {
                    return true;
                }
                prev = sibling.previous_sibling_element();
            }
            false
        }
    }
}

/// After matching `remaining[0]` against an element, continue the
/// leftward walk if there are more units. If `remaining[0]` is the
/// leftmost unit, the match is complete.
fn continues_leftward<E: Element>(
    remaining: &[ComplexSelectorUnit],
    start: usize,
    element: &E,
    level: usize,
    state: &mut MatchState<E>,
) -> bool {
    if remaining.len() == 1 {
        // remaining[0] is leftmost; we've already matched it.
        return true;
    }
    // Recurse using remaining[0].combinator to find candidates for
    // remaining[1].
    let next_combinator = match remaining[0].combinator {
        Some(c) => c,
        None => return true, // leftmost, already matched
    };
    walk_leftward(
        &remaining[1..],
        start + 1,
        element,
        level,
        next_combinator,
        state,
    )
}

/// SEL-1：`continues_leftward` 的记忆化入口，仅用于**链上**元素
/// （`state.chain[idx]`，层级 `idx + 1`）。key = (链索引, remaining
/// 起始下标)：同一 (祖先元素, 剩余后缀) 子问题在回溯中被重复求值，
/// 结果只取决于这两者。兄弟分支的元素不在链上，直接走
/// [`continues_leftward`]（无记忆化，由步数预算兜底）。
fn continues_on_chain<E: Element>(
    remaining: &[ComplexSelectorUnit],
    start: usize,
    idx: usize,
    element: &E,
    state: &mut MatchState<E>,
) -> bool {
    let key = (idx, start);
    if let Some(&cached) = state.memo.get(&key) {
        return cached;
    }
    let result = continues_leftward(remaining, start, element, idx + 1, state);
    state.memo.insert(key, result);
    result
}
