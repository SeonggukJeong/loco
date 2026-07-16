//! 상품 카테고리 계층 모델.
//!
//! 카테고리는 트리 구조(부모-자식)를 가질 수 있다. 이 파일은 평면 목록에서
//! 부모/자식 관계를 조회하는 헬퍼만 제공한다(트리 자료구조 자체를 별도
//! 타입으로 만들지 않고, `Vec<Category>` + parent 코드 참조로 표현한다 —
//! 사내에서 실제로 자주 보이는 "얕은 트리" 표현 방식이다).

/// 카테고리 한 개.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    pub code: String,
    pub name: String,
    pub parent: Option<String>,
}

impl Category {
    pub fn root(code: impl Into<String>, name: impl Into<String>) -> Self {
        Category { code: code.into(), name: name.into(), parent: None }
    }

    pub fn child(code: impl Into<String>, name: impl Into<String>, parent: impl Into<String>) -> Self {
        Category { code: code.into(), name: name.into(), parent: Some(parent.into()) }
    }

    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }
}

/// 코드로 카테고리를 찾는다.
pub fn find_by_code<'a>(categories: &'a [Category], code: &str) -> Option<&'a Category> {
    categories.iter().find(|c| c.code == code)
}

/// 특정 부모의 직계 자식만 걸러낸다.
pub fn children_of<'a>(categories: &'a [Category], parent_code: &str) -> Vec<&'a Category> {
    categories.iter().filter(|c| c.parent.as_deref() == Some(parent_code)).collect()
}

/// 최상위(루트) 카테고리만 걸러낸다.
pub fn roots(categories: &[Category]) -> Vec<&Category> {
    categories.iter().filter(|c| c.is_root()).collect()
}

/// 카테고리의 조상 코드 체인을 루트까지 거슬러 올라가며 수집한다.
///
/// 순환 참조가 있으면 무한루프를 막기 위해 이미 방문한 코드에서 멈춘다.
pub fn ancestor_chain(categories: &[Category], code: &str) -> Vec<String> {
    let mut chain = Vec::new();
    let mut current = code.to_string();
    loop {
        match find_by_code(categories, &current) {
            Some(cat) => match &cat.parent {
                Some(parent) if !chain.contains(parent) => {
                    chain.push(parent.clone());
                    current = parent.clone();
                }
                _ => break,
            },
            None => break,
        }
    }
    chain
}

/// 카테고리 트리의 깊이(루트=0)를 계산한다.
pub fn depth_of(categories: &[Category], code: &str) -> usize {
    ancestor_chain(categories, code).len()
}

/// 특정 카테고리가 다른 카테고리의 하위(자손)인지 판정한다.
pub fn is_descendant_of(categories: &[Category], code: &str, ancestor_code: &str) -> bool {
    ancestor_chain(categories, code).iter().any(|c| c == ancestor_code)
}

/// 카테고리 코드 목록 중 존재하지 않는 코드를 걸러낸다(참조 무결성 점검용).
pub fn missing_codes(categories: &[Category], codes: &[String]) -> Vec<String> {
    codes.iter().filter(|c| find_by_code(categories, c).is_none()).cloned().collect()
}

/// 특정 카테고리의 모든 자손(자식, 손자, ...) 코드를 재귀적으로 수집한다.
pub fn all_descendants(categories: &[Category], code: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut stack: Vec<String> = children_of(categories, code).into_iter().map(|c| c.code.clone()).collect();
    while let Some(current) = stack.pop() {
        if !result.contains(&current) {
            let child_codes: Vec<String> = children_of(categories, &current).into_iter().map(|c| c.code.clone()).collect();
            stack.extend(child_codes);
            result.push(current);
        }
    }
    result.sort();
    result
}

/// 카테고리 이름으로 대소문자 무시 부분 검색을 한다.
pub fn search_by_name<'a>(categories: &'a [Category], query: &str) -> Vec<&'a Category> {
    let q = query.to_ascii_lowercase();
    categories.iter().filter(|c| c.name.to_ascii_lowercase().contains(&q)).collect()
}

/// 카테고리 목록에 순환 참조가 있는지 검사한다(각 카테고리의 조상 체인이
/// 유한한 길이 안에 끝나는지 확인).
pub fn has_cycle(categories: &[Category]) -> bool {
    categories.iter().any(|c| {
        let chain = ancestor_chain(categories, &c.code);
        // 정상이라면 체인 길이가 전체 카테고리 수를 넘을 수 없다.
        chain.len() > categories.len()
    })
}

/// 카테고리를 코드 오름차순으로 정렬한다.
pub fn sort_by_code(mut categories: Vec<Category>) -> Vec<Category> {
    categories.sort_by(|a, b| a.code.cmp(&b.code));
    categories
}

/// 두 카테고리가 같은 부모를 갖는 형제 관계인지 판정한다.
pub fn are_siblings(categories: &[Category], code_a: &str, code_b: &str) -> bool {
    match (find_by_code(categories, code_a), find_by_code(categories, code_b)) {
        (Some(a), Some(b)) => a.parent.is_some() && a.parent == b.parent,
        _ => false,
    }
}

/// 카테고리 코드 포맷이 유효한지 검사한다(영문 대문자 2~4자).
pub fn is_valid_category_code(code: &str) -> bool {
    (2..=4).contains(&code.len()) && code.chars().all(|c| c.is_ascii_uppercase())
}
