mod util;

pub use util::normalize;

pub fn title(s: &str) -> String {
    let n = normalize(s);
    let mut c = n.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => n,
    }
}

pub fn slug(s: &str) -> String {
    normalize(s).replace(' ', "-")
}

pub fn compare(a: &str, b: &str) -> bool {
    normalize(a) == normalize(b)
}
