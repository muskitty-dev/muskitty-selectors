//! SP-8 §18 matching engine — basic tests.
//!
//! Covers §18 L4878-4919 (Match a Selector Against an Element) and
//! the simple-selector matchers in §3 L858-873 + §5 + §6.

use muskitty_selectors::matching::matches;
use muskitty_selectors::matching::Element;
use muskitty_selectors::parser::parse_a_selector;

/// Minimal in-memory element for unit-testing the matching engine
/// without pulling in muskitty-dom. Tests against muskitty-dom live
/// in `tests/matching_dom.rs`.
#[derive(Clone, Debug)]
struct StubElement {
    local_name: String,
    namespace_uri: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
    attributes: Vec<(String, String)>,
    #[allow(dead_code)]
    parent: Option<Box<StubElement>>,
    #[allow(dead_code)]
    previous_sibling: Option<Box<StubElement>>,
    #[allow(dead_code)]
    next_sibling: Option<Box<StubElement>>,
    #[allow(dead_code)]
    children: Vec<StubElement>,
}

impl StubElement {
    fn new(local_name: &str) -> Self {
        Self {
            local_name: local_name.to_string(),
            namespace_uri: None,
            id: None,
            classes: Vec::new(),
            attributes: Vec::new(),
            parent: None,
            previous_sibling: None,
            next_sibling: None,
            children: Vec::new(),
        }
    }
}

impl Element for StubElement {
    fn local_name(&self) -> String {
        self.local_name.clone()
    }
    fn namespace_uri(&self) -> Option<String> {
        self.namespace_uri.clone()
    }
    fn id(&self) -> Option<String> {
        self.id.clone()
    }
    fn classes(&self) -> Vec<String> {
        self.classes.clone()
    }
    fn get_attribute(&self, name: &str) -> Option<String> {
        self.attributes
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
    }
    fn parent_element(&self) -> Option<Self> {
        self.parent.clone().map(|b| *b)
    }
    fn previous_sibling_element(&self) -> Option<Self> {
        self.previous_sibling.clone().map(|b| *b)
    }
    fn next_sibling_element(&self) -> Option<Self> {
        self.next_sibling.clone().map(|b| *b)
    }
    fn child_elements(&self) -> Vec<Self> {
        self.children.clone()
    }
    fn is_empty(&self) -> bool {
        self.children.is_empty()
    }
    fn index_among_siblings(&self) -> usize {
        // Walk previous_sibling chain. 1-indexed per §13.3 L3982.
        let mut idx = 1;
        let mut cur = self.previous_sibling.clone();
        while let Some(prev) = cur {
            idx += 1;
            cur = prev.previous_sibling.clone();
        }
        idx
    }
    fn count_among_siblings(&self) -> usize {
        let mut count = 1;
        let mut cur = self.previous_sibling.clone();
        while let Some(prev) = cur {
            count += 1;
            cur = prev.previous_sibling;
        }
        let mut cur = self.next_sibling.clone();
        while let Some(next) = cur {
            count += 1;
            cur = next.next_sibling;
        }
        count
    }
    fn index_among_type(&self) -> usize {
        let mut idx = 1;
        let mut cur = self.previous_sibling.clone();
        while let Some(prev) = cur {
            if prev.local_name.eq_ignore_ascii_case(&self.local_name) {
                idx += 1;
            }
            cur = prev.previous_sibling;
        }
        idx
    }
    fn count_among_type(&self) -> usize {
        let mut count = 1;
        let mut cur = self.previous_sibling.clone();
        while let Some(prev) = cur {
            if prev.local_name.eq_ignore_ascii_case(&self.local_name) {
                count += 1;
            }
            cur = prev.previous_sibling;
        }
        let mut cur = self.next_sibling.clone();
        while let Some(next) = cur {
            if next.local_name.eq_ignore_ascii_case(&self.local_name) {
                count += 1;
            }
            cur = next.next_sibling;
        }
        count
    }
}

/// §3 L870: type selector matches local_name (case-insensitive HTML).
#[test]
fn type_selector_matches_case_insensitive() {
    let list = parse_a_selector("div").expect("parses");
    let el = StubElement::new("DIV");
    assert!(matches(&list, &el));
}

/// §5 L1808-1824: type selector matches local_name.
#[test]
fn type_selector_matches() {
    let list = parse_a_selector("div").expect("parses");
    let el = StubElement::new("div");
    assert!(matches(&list, &el));
}

/// §5 L1825-1866: universal selector matches anything.
#[test]
fn universal_selector_matches() {
    let list = parse_a_selector("*").expect("parses");
    let el = StubElement::new("anything");
    assert!(matches(&list, &el));
}

/// §6.5 L2376-2462: class selector matches class list.
#[test]
fn class_selector_matches() {
    let mut el = StubElement::new("div");
    el.classes = vec!["foo".into(), "bar".into()];
    let list = parse_a_selector(".foo").expect("parses");
    assert!(matches(&list, &el));
    let list = parse_a_selector(".baz").expect("parses");
    assert!(!matches(&list, &el));
}

/// §6.6 L2463-2533: id selector matches id attribute.
#[test]
fn id_selector_matches() {
    let mut el = StubElement::new("div");
    el.id = Some("main".into());
    let list = parse_a_selector("#main").expect("parses");
    assert!(matches(&list, &el));
    let list = parse_a_selector("#other").expect("parses");
    assert!(!matches(&list, &el));
}

/// §6 L1996-2533: attribute presence selector.
#[test]
fn attribute_presence_matches() {
    let mut el = StubElement::new("input");
    el.attributes = vec![("disabled".into(), "".into())];
    let list = parse_a_selector("[disabled]").expect("parses");
    assert!(matches(&list, &el));
}

/// §6.1 L2037-2054: `[attr=value]` exact match.
#[test]
fn attribute_exact_match() {
    let mut el = StubElement::new("a");
    el.attributes = vec![("href".into(), "https://example.com".into())];
    let list = parse_a_selector(r#"[href="https://example.com"]"#).expect("parses");
    assert!(matches(&list, &el));
}

/// §6.2 L2137-2162: `[attr~=value]` whitespace-list contains.
#[test]
fn attribute_includes_match() {
    let mut el = StubElement::new("div");
    el.attributes = vec![("class".into(), "foo bar baz".into())];
    let list = parse_a_selector("[class~=bar]").expect("parses");
    assert!(matches(&list, &el));
}

/// §6.2 L2137-2162: `[attr^=value]` prefix match.
#[test]
fn attribute_prefix_match() {
    let mut el = StubElement::new("a");
    el.attributes = vec![("href".into(), "https://foo".into())];
    let list = parse_a_selector(r#"[href^="https://"]"#).expect("parses");
    assert!(matches(&list, &el));
}

/// §6.2 L2137-2162: `[attr$=value]` suffix match.
#[test]
fn attribute_suffix_match() {
    let mut el = StubElement::new("a");
    el.attributes = vec![("href".into(), "doc.pdf".into())];
    let list = parse_a_selector(r#"[href$=".pdf"]"#).expect("parses");
    assert!(matches(&list, &el));
}

/// §6.2 L2137-2162: `[attr*=value]` substring match.
#[test]
fn attribute_substring_match() {
    let mut el = StubElement::new("div");
    el.attributes = vec![("data-x".into(), "foobar".into())];
    let list = parse_a_selector("[data-x*=oob]").expect("parses");
    assert!(matches(&list, &el));
}

/// §6.1 L2055-2080: `[attr|=value]` exact match or hyphen-prefix.
#[test]
fn attribute_dash_match() {
    let mut el = StubElement::new("html");
    el.attributes = vec![("lang".into(), "en-US".into())];
    let list = parse_a_selector("[lang|=en]").expect("parses");
    assert!(matches(&list, &el));
}

/// Compound: `div.foo#bar[baz]` matches when all components match.
#[test]
fn compound_all_components_match() {
    let mut el = StubElement::new("div");
    el.id = Some("bar".into());
    el.classes = vec!["foo".into()];
    el.attributes = vec![("baz".into(), "".into())];
    let list = parse_a_selector("div.foo#bar[baz]").expect("parses");
    assert!(matches(&list, &el));
    // Missing id → no match
    let mut el2 = el.clone();
    el2.id = None;
    assert!(!matches(&list, &el2));
}

// ---------------------------------------------------------------------------
// Task 7: combinator matching (right-to-left complex-selector walk)
// ---------------------------------------------------------------------------

/// Build three siblings `a`, `b`, `c` under a `root` parent. Wires
/// `previous_sibling` (forward) and `next_sibling` (forward) and
/// populates `parent.children`. Each child's `parent` snapshot has
/// `children` populated to keep `count_among_siblings` consistent
/// when needed by other tests reusing this helper.
fn build_three_siblings() -> Vec<StubElement> {
    let mut parent = StubElement::new("root");
    let names = ["a", "b", "c"];
    let mut children: Vec<StubElement> = Vec::new();
    for name in names {
        let mut child = StubElement::new(name);
        child.parent = Some(Box::new(parent.clone()));
        if let Some(prev) = children.last() {
            child.previous_sibling = Some(Box::new(prev.clone()));
        }
        children.push(child);
    }
    // Wire next_sibling forward links. REVERSE iteration so each
    // snapshot captures the already-populated forward chain (matches
    // the convention used by `build_siblings` in matching_pseudo.rs).
    for i in (0..children.len()).rev() {
        if i + 1 < children.len() {
            children[i].next_sibling = Some(Box::new(children[i + 1].clone()));
        }
    }
    parent.children = children.clone();
    for child in &mut children {
        child.parent = Some(Box::new(parent.clone()));
    }
    children
}

/// §15 L4369: descendant combinator (whitespace).
#[test]
fn descendant_combinator_matches() {
    // `parent > child` structure.
    let mut parent = StubElement::new("div");
    let mut child = StubElement::new("span");
    child.parent = Some(Box::new(parent.clone()));
    parent.children = vec![child.clone()];

    // `div span` — descendant combinator.
    let list = parse_a_selector("div span").expect("parses");
    assert!(matches(&list, &child));
    assert!(!matches(&list, &parent));
}

/// §15 L4376: child combinator (`>`).
#[test]
fn child_combinator_matches_only_direct() {
    // `root > middle > leaf`.
    let mut root = StubElement::new("root");
    let mut middle = StubElement::new("middle");
    let mut leaf = StubElement::new("leaf");
    middle.parent = Some(Box::new(root.clone()));
    leaf.parent = Some(Box::new(middle.clone()));
    middle.children = vec![leaf.clone()];
    root.children = vec![middle.clone()];

    // `root > leaf` — does NOT match (leaf's direct parent is middle,
    // not root).
    let list = parse_a_selector("root > leaf").expect("parses");
    assert!(!matches(&list, &leaf));

    // `root > middle` — matches.
    let list = parse_a_selector("root > middle").expect("parses");
    assert!(matches(&list, &middle));
}

/// §15 L4383: next-sibling combinator (`+`).
#[test]
fn next_sibling_combinator_matches() {
    // Build siblings a, b, c.
    let sibs = build_three_siblings();
    // `a + b` matches b (b's previous sibling is a).
    let list = parse_a_selector("a + b").expect("parses");
    assert!(matches(&list, &sibs[1]));
    // `a + c` does NOT match c (c's previous sibling is b, not a).
    let list = parse_a_selector("a + c").expect("parses");
    assert!(!matches(&list, &sibs[2]));
}

/// §15 L4390: subsequent-sibling combinator (`~`).
#[test]
fn subsequent_sibling_combinator_matches() {
    let sibs = build_three_siblings();
    // `a ~ c` matches c (c has an earlier sibling a).
    let list = parse_a_selector("a ~ c").expect("parses");
    assert!(matches(&list, &sibs[2]));
    // `b ~ a` does NOT match a (a has no earlier sibling b).
    let list = parse_a_selector("b ~ a").expect("parses");
    assert!(!matches(&list, &sibs[0]));
}

/// Mixed combinators: `a > b + c` (a is parent of b; b is preceding
/// sibling of c).
#[test]
fn mixed_combinators_match() {
    let mut a = StubElement::new("a");
    let mut b = StubElement::new("b");
    let mut c = StubElement::new("c");
    b.parent = Some(Box::new(a.clone()));
    c.parent = Some(Box::new(a.clone()));
    c.previous_sibling = Some(Box::new(b.clone()));
    b.next_sibling = Some(Box::new(c.clone()));
    a.children = vec![b.clone(), c.clone()];

    let list = parse_a_selector("a > b + c").expect("parses");
    assert!(matches(&list, &c));
    assert!(!matches(&list, &b));
}

/// Three-part descendant: `a b c` — c is descendant of b is
/// descendant of a.
#[test]
fn three_part_descendant_matches() {
    let mut a = StubElement::new("a");
    let mut b = StubElement::new("b");
    let mut c = StubElement::new("c");
    b.parent = Some(Box::new(a.clone()));
    c.parent = Some(Box::new(b.clone()));
    b.children = vec![c.clone()];
    a.children = vec![b.clone()];

    let list = parse_a_selector("a b c").expect("parses");
    assert!(matches(&list, &c));
    assert!(!matches(&list, &b));
}

// ---------------------------------------------------------------------------
// SEL-1: 左向匹配回溯的记忆化与步数预算（审计 2026-09-06 P0）
// ---------------------------------------------------------------------------

/// 构造 `depth` 层全 `.x` 的 div 链，返回**最深**元素（其 parent
/// 快照逐层内嵌完整祖先链）。自顶向下构建：每轮把当前链顶作为新
/// child 的 parent 快照。
fn deep_chain_leaf(depth: usize, class: &str) -> StubElement {
    let mk = || {
        let mut e = StubElement::new("div");
        e.classes = vec![class.into()];
        e
    };
    let mut node = mk(); // 根
    for _ in 1..depth {
        let mut child = mk();
        child.parent = Some(Box::new(node));
        node = child;
    }
    node
}

/// 构造 `n` 个 `div` 兄弟（同一 parent），返回带完整
/// previous_sibling 链的兄弟列表。
fn build_div_siblings(n: usize) -> Vec<StubElement> {
    let mut parent = StubElement::new("root");
    let mut children: Vec<StubElement> = Vec::new();
    for _ in 0..n {
        let mut child = StubElement::new("div");
        child.parent = Some(Box::new(parent.clone()));
        if let Some(prev) = children.last() {
            child.previous_sibling = Some(Box::new(prev.clone()));
        }
        children.push(child);
    }
    parent.children = children.clone();
    for child in &mut children {
        child.parent = Some(Box::new(parent.clone()));
    }
    children
}

/// 构造 `depth` 层链：leaf（最深层）与最顶 `x_ancestors` 层祖先为
/// `.x`，中间层为无 class 的 div。返回最深元素。
fn deep_chain_mixed(depth: usize, x_ancestors: usize) -> StubElement {
    let x = || {
        let mut e = StubElement::new("div");
        e.classes = vec!["x".into()];
        e
    };
    let mut node = x(); // 根（.x）
    for level in 1..depth {
        let mut child = if level < x_ancestors || level == depth - 1 {
            x()
        } else {
            StubElement::new("div")
        };
        child.parent = Some(Box::new(node));
        node = child;
    }
    node
}

/// SEL-1 ②：`.x` × 8（7 个 Descendant 组合器）对 60 深 DOM 的全路径
/// 回溯在修复前为 C(59,7) ≈ 3×10⁸ 条候选路径（单条规则挂起数分钟）；
/// 祖先链 + 记忆化后每 (祖先, 剩余后缀) 只求值一次（≤ 60×8 次），
/// 即刻返回正确结果。
#[test]
fn descendant_backtracking_is_memoized() {
    let leaf = deep_chain_leaf(60, "x");
    let selector = ".x .x .x .x .x .x .x .x";
    let list = parse_a_selector(selector).expect("parses");
    assert!(
        matches(&list, &leaf),
        "60-level all-.x chain must match the 8-compound .x selector"
    );
    // 负例：只有 6 个 .x 祖先 < 需要的 7 个 —— 全路径探索同样被
    // 记忆化压到 D×k，即刻返回不匹配。
    let broken = deep_chain_mixed(60, 6);
    let list_neg = parse_a_selector(selector).expect("parses");
    assert!(
        !matches(&list_neg, &broken),
        "chain with only 6 .x ancestors must not match the 8-compound selector"
    );
}

/// SEL-1 ①：兄弟组合器无记忆化，匹配**失败**时全路径探索 ——
/// `span ~ div ~ div ~ div ~ div`（subject=div，最左 span 不存在）对
/// 150 兄弟的路径计数为 C(149,4) ≈ 1.9×10⁷ > 100_000 步数预算 →
/// 按不匹配降级并即刻返回（语义答案同为 false；回归信号是耗时：
/// 预算被移除时本测试需 ~2×10⁷ 次 compound 匹配，数十秒级）。
#[test]
fn sibling_path_explosion_is_bounded_by_step_budget() {
    let sibs = build_div_siblings(150);
    let list = parse_a_selector("span ~ div ~ div ~ div ~ div").expect("parses");
    assert!(
        !matches(&list, &sibs[149]),
        "C(149,4) ~= 1.9e7 path exploration must be bounded by MAX_MATCH_STEPS and return no-match"
    );
    // 浅兄弟树（预算内）语义不受影响：3 兄弟 × 1 组合器路径极少。
    let small = build_div_siblings(3);
    let list_small = parse_a_selector("div ~ div").expect("parses");
    assert!(matches(&list_small, &small[2]));
    assert!(!matches(&list_small, &small[0]));
}

/// SEL-1：跨"链外 → 链上"的组合器路径正确性 —— SubsequentSibling
/// 走到祖先的兄弟（链外元素，层级 = 该祖先层级）后，其 Descendant
/// 候选必须是**该兄弟自己的**祖先（= subject 祖先链后缀），验证
/// 层级线索（level）在链外分支的传递。
#[test]
fn sibling_then_descendant_walks_siblings_own_ancestors() {
    // root
    // ├─ a.a
    // └─ b.b
    //    └─ c.c   ← subject
    let mut root = StubElement::new("root");
    let mut a = StubElement::new("div");
    a.classes = vec!["a".into()];
    let mut b = StubElement::new("div");
    b.classes = vec!["b".into()];
    let mut c = StubElement::new("span");
    c.classes = vec!["c".into()];

    a.parent = Some(Box::new(root.clone()));
    b.parent = Some(Box::new(root.clone()));
    b.previous_sibling = Some(Box::new(a.clone()));
    a.next_sibling = Some(Box::new(b.clone()));
    c.parent = Some(Box::new(b.clone()));
    b.children = vec![c.clone()];
    root.children = vec![a.clone(), b.clone()];

    // `a ~ b .c`：c 的祖先 b，b 的先前兄弟 a。
    let list = parse_a_selector(".a ~ .b .c").expect("parses");
    assert!(matches(&list, &c));
    // 兄弟不匹配 → 整体不匹配。
    let list = parse_a_selector(".zz ~ .b .c").expect("parses");
    assert!(!matches(&list, &c));
    // b 的祖先链上无 .a（.a 是兄弟不是祖先）→ `a b .c` 的
    // Descendant 版本不匹配。
    let list = parse_a_selector(".a .b .c").expect("parses");
    assert!(!matches(&list, &c));
}

/// SEL-1：subject 为根（无祖先）时的边界 —— Descendant/Child 无候选。
#[test]
fn root_subject_has_no_leftward_candidates() {
    let mut root = StubElement::new("div");
    root.classes = vec!["x".into()];
    let list = parse_a_selector(".x .x").expect("parses");
    assert!(!matches(&list, &root));
    let list = parse_a_selector(".x > .x").expect("parses");
    assert!(!matches(&list, &root));
}
