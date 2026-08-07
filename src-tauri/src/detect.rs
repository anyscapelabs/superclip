const CODE_KEYWORDS: &[&str] = &[
    "fn ",
    "function",
    "const ",
    "let ",
    "var ",
    "return ",
    "import ",
    "from ",
    "export ",
    "class ",
    "def ",
    "pub ",
    "static ",
    "struct ",
    "enum ",
    "interface ",
    "type ",
    "if (",
    "for (",
    "while (",
    "async ",
    "await ",
    "SELECT ",
    "INSERT ",
    "UPDATE ",
    "DELETE ",
    "docker run",
    "npm ",
    "cargo ",
    "#include",
];

pub fn detect_kind(text: &str) -> &'static str {
    let t = text.trim();
    if t.is_empty() {
        return "text";
    }

    let has_nl = t.contains('\n') || t.contains('\r');
    let mut score = 0;

    if has_nl {
        score += 1;
    }
    if t.contains('{') && t.contains('}') {
        score += 1;
    }
    if t.contains(';') {
        score += 1;
    }
    if t.contains('(') && t.contains(')') {
        score += 1;
    }
    if t.contains("=>") || t.contains("->") {
        score += 1;
    }
    if t.contains("</") {
        score += 1;
    }
    if t.lines().any(|l| l.starts_with(char::is_whitespace)) && has_nl {
        score += 1;
    }

    score += CODE_KEYWORDS.iter().filter(|k| t.contains(*k)).count();

    if (has_nl && score >= 2) || score >= 3 {
        "code"
    } else {
        "text"
    }
}
