//! W-3（WPT css/selectors/parsing 对齐第一批）回归测试。
//!
//! 覆盖本轮新增/收紧的解析语义。每条断言的规范或夹具依据见
//! `goal.md` 的"规范依据"表，以及 `src/parser/{simple,compound,complex}.rs`
//! 中对应实现的注释。夹具来源：`tests/data/wpt/*.json`（上游
//! `css/selectors/parsing/parse-*.html`）。
//!
//! 分组：
//! - `::part(<ident>+)` / `::slotted(<compound-selector>)`（css-shadow-1）
//! - `:host(<compound-selector>)`（css-shadow-1），含参数内递归 compound-only
//! - `:state(<custom-ident>)` / `:heading()` / `:has-slotted()`（selectors-5 /
//!   css-shadow-1）
//! - real-selector-list（`:is`/`:where`/`:not`/`:has`/`of S` 内禁伪元素）
//! - 伪元素后置规则与"伪元素只在最右复合选择器"
//! - An+B 空白形态

use muskitty_selectors::parser::parse_a_selector;
use muskitty_selectors::types::{
    PseudoClassArgument, PseudoElementArgument, SelectorList, SubclassSelector,
};

fn valid(selector: &str) {
    assert!(
        parse_a_selector(selector).is_ok(),
        "expected valid: {selector:?} (got {:?})",
        parse_a_selector(selector).err()
    );
}

fn invalid(selector: &str) {
    assert!(
        parse_a_selector(selector).is_err(),
        "expected invalid: {selector:?} (parsed successfully)"
    );
}

/// 取唯一 compound 的第 `idx` 个 subclass 伪类。
fn pseudo_class_at(list: &SelectorList, idx: usize) -> &muskitty_selectors::types::PseudoClass {
    let compound = &list.0[0].units[0].compound;
    match &compound.subclasses[idx] {
        SubclassSelector::PseudoClass(pc) => pc,
        other => panic!("expected PseudoClass at {idx}, got {other:?}"),
    }
}

// ── ::part(<ident>+) ────────────────────────────────────────────────

#[test]
fn part_accepts_one_or_more_ident_names() {
    // css-shadow-1 §part L1163：`::part(<ident>+)`，多个名字顺序无关
    let list = parse_a_selector("::part(foo bar)").expect("valid");
    let pe = &list.0[0].units[0].compound.pseudo_compounds[0].pseudo_element;
    assert_eq!(pe.name, "part");
    assert_eq!(
        pe.argument,
        Some(PseudoElementArgument::Part(vec![
            "foo".to_string(),
            "bar".to_string()
        ]))
    );
    // 单个名字；`--` / `--0` / `-foo` 都是合法 ident（tokenizer §4.3.9）
    valid("::part(foo)");
    valid("::part(--)");
    valid("::part(--0)");
    valid("::part(-foo bar)");
}

#[test]
fn part_allows_trailing_pseudo_classes_and_pseudo_elements() {
    // selectors-4 §3 `<pseudo-compound-selector> = pseudo-element-selector
    // pseudo-class-selector*`；css-shadow-1 §part "fully styleable"
    valid("::part(foo):hover");
    valid("::part(foo):focus-within");
    valid("::part(foo):lang(en)");
    valid("::part(foo):dir(ltr)");
    valid("::part(foo):is(:focus)");
    valid("::part(foo)::before");
    valid("::part(foo)::placeholder");
    valid(":lang(en)::part(foo)");
}

#[test]
fn part_rejects_non_ident_arguments_and_bad_contexts() {
    // 夹具 parse-part.html：`:part()`（单冒号）/ `::part(0)` / `::part('foo')`
    // / `::part([foo])` 均 invalid
    invalid(":part()");
    invalid("::part(0)");
    invalid("::part('foo')");
    invalid("::part([foo])");
    invalid("::part()");
    // `::part(foo):has(li)` invalid（:has() 不能跟在伪元素后，夹具钉死）
    invalid("::part(foo):has(li)");
    // 伪元素只允许出现在最右复合选择器
    invalid("::part(foo) + ::part(bar)");
}

// ── ::slotted(<compound-selector>) ──────────────────────────────────

#[test]
fn slotted_takes_a_compound_selector() {
    // css-shadow-1 §slotted L456
    let list = parse_a_selector("::slotted(.class)").expect("valid");
    let pe = &list.0[0].units[0].compound.pseudo_compounds[0].pseudo_element;
    assert_eq!(pe.name, "slotted");
    assert!(matches!(
        pe.argument,
        Some(PseudoElementArgument::Slotted(_))
    ));
    valid("::slotted(bar)");
    valid("::slotted(*)");
    valid("::slotted([attr=\"foo\"])");
    valid("::slotted(:not(:nth-last-of-type(2)):not([slot=\"foo\"]))");
    valid("::slotted(:has(:first-child:last-child))");
}

#[test]
fn slotted_rejects_pseudo_classes_and_combinators_after_it() {
    // 夹具 parse-slotted.html：`::slotted(foo):hover` 等后置伪类 invalid
    // （css-shadow-1 §slotted L471 只允许后随 tree-abiding 伪元素）
    invalid("::slotted(foo):hover");
    invalid("::slotted(foo):first-child");
    invalid("::slotted(foo):lang(en)");
    invalid("::slotted(foo) + ::slotted(bar)");
    // 裸形式 / 单冒号形式 / 空参数 / 非选择器参数
    invalid("::slotted");
    invalid(":slotted(foo)");
    invalid("::slotted()");
    invalid("::slotted(0)");
    // 后随伪元素合法（同规范句）
    valid("::slotted(foo)::before");
}

// ── :host(<compound-selector>) ──────────────────────────────────────

#[test]
fn host_bare_and_functional_forms() {
    valid(":host");
    valid(":host(.a)");
    valid(":host(.a.b)");
    // 参数是 compound-selector：内部伪类自带选择器参数照常解析
    let list = parse_a_selector(":host(:is(div))").expect("valid");
    assert!(matches!(
        pseudo_class_at(&list, 0).argument,
        Some(PseudoClassArgument::Compound(_))
    ));
    valid(":host(:not(.a))");
    valid(":not(:host(:not(.a)))");
}

#[test]
fn host_argument_is_compound_only_recursively() {
    // 夹具 parse-not.html / parse-is-where.html：
    // `:host(:not(.a .b))` invalid（非 forgiving，内层复合限制的失败传播）
    invalid(":host(:not(.a .b))");
    // `:host(.a .b)` invalid（参数本身不接受组合器）
    invalid(":host(.a .b)");
    invalid(":host()");
    // forgiving 参数内的失败项被丢弃 → 整体仍合法
    valid(":host(:is(div .foo))");
    valid(":host(:where(div .foo))");
    valid(":host(:is(,,,))");
    valid(":host(:is(.a, .b+.c, .d))");
}

// ── :state(<custom-ident>) ──────────────────────────────────────────

#[test]
fn state_takes_a_single_custom_ident() {
    // selectors-5 §state L271-296 + HTML custom state
    let list = parse_a_selector(":state(--foo)").expect("valid");
    assert!(matches!(
        pseudo_class_at(&list, 0).argument,
        Some(PseudoClassArgument::Raw(_))
    ));
    valid(":state(bar)");
    valid(":state(--)");
    valid(":state(--0)");
    valid("my-input[type=\"foo\"]:state(checked)");
    // 裸形式 / 空 / 非 ident 参数
    invalid(":state");
    invalid(":state(");
    invalid(":state()");
    invalid(":state(0)");
    invalid(":state(0rem)");
    invalid(":state(url())");
    invalid(":state(foo(1))");
    invalid(":state(:host)");
}

#[test]
fn state_only_follows_part_among_pseudo_elements() {
    // 夹具 parse-state.html：:state 仅可紧跟 ::part()
    valid("::part(inner):state(bar)");
    valid("::part(inner):state(bar)::before");
    valid("my-input[type=\"foo\"]:state(--0)::part(inner):state(bar)");
    invalid("my-input::after:state(foo)");
    invalid("my-input::first-letter:state(foo)");
    invalid("::slotted(foo):state(foo)");
    invalid("::part(inner):state(bar)::before:state(foo)");
}

// ── :heading / :has-slotted ─────────────────────────────────────────

#[test]
fn heading_bare_and_integer_list_forms() {
    // selectors-5 §heading L315-330：`<level>` 是 type flag 为 integer 的
    // number-token；裸形式亦合法
    valid(":heading");
    valid("h1:heading");
    valid(":heading(2)");
    valid(":heading(99999)");
    valid(":heading(0)");
    valid(":heading(-1)");
    valid(":heading(0, 1, 2, 3, 4, 5, 6, 7, 8, 9)");
    invalid(":heading()");
    invalid(":heading(1.0)");
    invalid(":heading(1.4)");
    invalid(":heading(n)");
    invalid(":heading(odd)");
    invalid(":heading(2n)");
    invalid(":heading(2n+1)");
    invalid(":heading(2n, 3n)");
    invalid(":heading(2 of .foo)");
    invalid(":heading(calc(1))");
    invalid(":heading(var(--level))");
}

#[test]
fn has_slotted_bare_and_compound_argument() {
    // css-shadow-1 §has-slotted L548-575（功能性形式为 tentative）
    valid(":has-slotted");
    valid(":has-slotted(bar)");
    valid(":has-slotted([attr=\"foo\"])");
    valid(":has-slotted(*)");
    valid(":has-slotted(#id)");
    valid(":has-slotted(div:has(> span))");
    valid(":has-slotted(foo):hover");
    valid(":has-slotted(foo) + :has-slotted(bar)");
    valid(":not(:has-slotted(foo))");
    invalid("::has-slotted(foo)");
    invalid(":has-slotted()");
    invalid(":has-slotted(0)");
    // 参数是复合选择器 → 组合器无效（夹具 `div > span` invalid）
    invalid(":has-slotted(div > span)");
    // 已知偏差：夹具称 `:has-slotted(div + div)` valid，但同一文法不可能
    // 同时接受 `+` 而拒绝 `>`——判定为 tentative 夹具自相矛盾，见 goal.md。
    assert!(
        parse_a_selector(":has-slotted(div + div)").is_err(),
        "documented deviation: compound-only argument rejects the sibling combinator"
    );
}

// ── real-selector-list（伪元素禁止） ────────────────────────────────

#[test]
fn pseudo_elements_are_rejected_in_real_selector_lists() {
    // selectors-4 §4.3 `:not()` 取 complex-**real**-selector-list
    invalid(":not(::before)");
    // forgiving 列表丢弃失败项 → 整体仍合法（夹具 parse-is-where.html）
    valid(":is(::before)");
    valid(":where(::before)");
    valid(":is(::before:HOVER, .a)");
    // 顶层仍然允许伪元素
    valid("div::before");
}

// ── An+B 空白形态 ───────────────────────────────────────────────────

#[test]
fn an_plus_b_allows_whitespace_around_signs() {
    // CSS Syntax §7：An 与 B 之间的 `+`/`-` 两侧都允许空白
    valid(":nth-of-type( +n + 7 )");
    valid(":nth-last-of-type( +n + 7 )");
    valid(":nth-of-type( 23n\n\n+\n\n123 )");
    valid(":nth-child(2n + 1)");
    valid(":nth-child(2 )");
}
