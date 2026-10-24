//! TeX math to Qt rich text. T3 typesets with KaTeX; Qt's text engine has no math
//! layout, so common constructs map to Unicode, <sup>/<sub> and a fraction slash.
//! Unknown commands keep their TeX source, so nothing is silently dropped.

fn symbol(name: &str) -> Option<&'static str> {
    Some(match name {
        "alpha" => "α", "beta" => "β", "gamma" => "γ", "delta" => "δ", "epsilon" => "ϵ", "varepsilon" => "ε",
        "zeta" => "ζ", "eta" => "η", "theta" => "θ", "vartheta" => "ϑ", "iota" => "ι", "kappa" => "κ",
        "lambda" => "λ", "mu" => "μ", "nu" => "ν", "xi" => "ξ", "pi" => "π", "varpi" => "ϖ", "rho" => "ρ",
        "varrho" => "ϱ", "sigma" => "σ", "varsigma" => "ς", "tau" => "τ", "upsilon" => "υ", "phi" => "ϕ",
        "varphi" => "φ", "chi" => "χ", "psi" => "ψ", "omega" => "ω",
        "Gamma" => "Γ", "Delta" => "Δ", "Theta" => "Θ", "Lambda" => "Λ", "Xi" => "Ξ", "Pi" => "Π",
        "Sigma" => "Σ", "Upsilon" => "Υ", "Phi" => "Φ", "Psi" => "Ψ", "Omega" => "Ω",
        "sum" => "∑", "prod" => "∏", "coprod" => "∐", "int" => "∫", "iint" => "∬", "iiint" => "∭", "oint" => "∮",
        "infty" => "∞", "partial" => "∂", "nabla" => "∇", "forall" => "∀", "exists" => "∃", "nexists" => "∄",
        "emptyset" => "∅", "varnothing" => "∅", "in" => "∈", "notin" => "∉", "ni" => "∋", "subset" => "⊂",
        "supset" => "⊃", "subseteq" => "⊆", "supseteq" => "⊇", "cup" => "∪", "cap" => "∩", "bigcup" => "⋃",
        "bigcap" => "⋂", "setminus" => "∖", "land" => "∧", "wedge" => "∧", "lor" => "∨", "vee" => "∨",
        "neg" => "¬", "lnot" => "¬", "top" => "⊤", "bot" => "⊥",
        "le" => "≤", "leq" => "≤", "ge" => "≥", "geq" => "≥", "ne" => "≠", "neq" => "≠", "approx" => "≈",
        "equiv" => "≡", "sim" => "∼", "simeq" => "≃", "cong" => "≅", "propto" => "∝", "ll" => "≪", "gg" => "≫",
        "pm" => "±", "mp" => "∓", "times" => "×", "div" => "÷", "cdot" => "·", "ast" => "∗", "star" => "⋆",
        "circ" => "∘", "bullet" => "•", "oplus" => "⊕", "otimes" => "⊗", "odot" => "⊙",
        "to" => "→", "rightarrow" => "→", "leftarrow" => "←", "gets" => "←", "leftrightarrow" => "↔",
        "Rightarrow" => "⇒", "Leftarrow" => "⇐", "Leftrightarrow" => "⇔", "implies" => "⟹", "iff" => "⟺",
        "mapsto" => "↦", "uparrow" => "↑", "downarrow" => "↓", "longrightarrow" => "⟶", "longleftarrow" => "⟵",
        "ldots" => "…", "dots" => "…", "cdots" => "⋯", "vdots" => "⋮", "ddots" => "⋱",
        "langle" => "⟨", "rangle" => "⟩", "lceil" => "⌈", "rceil" => "⌉", "lfloor" => "⌊", "rfloor" => "⌋",
        "lbrace" => "{", "rbrace" => "}", "{" => "{", "}" => "}", "|" => "‖", "Vert" => "‖", "vert" => "|", "mid" => "∣",
        "angle" => "∠", "perp" => "⊥", "parallel" => "∥", "degree" => "°", "prime" => "′", "hbar" => "ℏ",
        "ell" => "ℓ", "Re" => "ℜ", "Im" => "ℑ", "aleph" => "ℵ", "therefore" => "∴", "because" => "∵",
        "%" => "%", "$" => "$", "&" => "&amp;", "#" => "#", "_" => "_",
        "," | ":" | ";" | "quad" | "qquad" | " " | "!" => " ",
        _ => return None,
    })
}
fn operator(name: &str) -> bool {
    matches!(name, "sin" | "cos" | "tan" | "cot" | "sec" | "csc" | "log" | "ln" | "exp" | "lim" | "max" | "min" | "sup" | "inf" | "det" | "dim" | "ker" | "gcd" | "arg" | "deg" | "sinh" | "cosh" | "tanh" | "arcsin" | "arccos" | "arctan" | "Pr" | "mod" | "bmod")
}
fn blackboard(c: char) -> Option<char> {
    Some(match c {
        'R' => 'ℝ', 'N' => 'ℕ', 'Z' => 'ℤ', 'Q' => 'ℚ', 'C' => 'ℂ', 'P' => 'ℙ', 'H' => 'ℍ',
        _ => return None,
    })
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

struct Parser<'a> {
    chars: Vec<char>,
    at: usize,
    depth: usize,
    _src: &'a str,
}
impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }
    fn command(&mut self) -> String {
        // Called after a backslash.
        let start = self.at;
        match self.peek() {
            Some(c) if c.is_ascii_alphabetic() => {
                while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                    self.at += 1;
                }
            }
            Some(_) => self.at += 1,
            None => {}
        }
        self.chars[start..self.at].iter().collect()
    }
    fn skip_space(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.at += 1;
        }
    }
    /// One argument: a braced group or a single token.
    fn argument(&mut self) -> String {
        self.skip_space();
        match self.peek() {
            Some('{') => {
                self.at += 1;
                self.group(Some('}'))
            }
            Some('\\') => {
                self.at += 1;
                let name = self.command();
                self.expand(&name)
            }
            Some(c) => {
                self.at += 1;
                escape(&c.to_string())
            }
            None => String::new(),
        }
    }
    /// Raw text of a braced argument, for \text and font commands.
    fn raw_argument(&mut self) -> String {
        self.skip_space();
        if self.peek() != Some('{') {
            return self.peek().map(|c| { self.at += 1; c.to_string() }).unwrap_or_default();
        }
        self.at += 1;
        let start = self.at;
        let mut level = 1;
        while let Some(c) = self.peek() {
            self.at += 1;
            match c {
                '{' => level += 1,
                '}' => {
                    level -= 1;
                    if level == 0 {
                        return self.chars[start..self.at - 1].iter().collect();
                    }
                }
                _ => {}
            }
        }
        self.chars[start..].iter().collect()
    }
    fn expand(&mut self, name: &str) -> String {
        if self.depth > 64 {
            return String::new();
        }
        self.depth += 1;
        let out = match name {
            "frac" | "dfrac" | "tfrac" | "cfrac" => {
                let (a, b) = (self.argument(), self.argument());
                let wrap = |s: String, raw: bool| if raw { s } else { format!("({s})") };
                let simple = |s: &str| s.chars().filter(|c| *c != ';' && *c != '&').count() <= 2 || !s.contains([' ', '+', '-', '−']);
                format!("{}⁄{}", wrap(a.clone(), simple(&a)), wrap(b.clone(), simple(&b)))
            }
            "sqrt" => {
                self.skip_space();
                let root = if self.peek() == Some('[') {
                    self.at += 1;
                    let start = self.at;
                    while self.peek().is_some_and(|c| c != ']') {
                        self.at += 1;
                    }
                    let r: String = self.chars[start..self.at].iter().collect();
                    self.at += 1;
                    format!("<sup>{}</sup>", escape(&r))
                } else {
                    String::new()
                };
                let body = self.argument();
                format!("{root}√<span style=\"text-decoration:overline\">{body}</span>")
            }
            "text" | "textrm" | "mathrm" | "operatorname" | "textnormal" | "mbox" => {
                format!("<span style=\"font-style:normal\">{}</span>", escape(&self.raw_argument()))
            }
            "mathbf" | "textbf" | "boldsymbol" | "bm" => format!("<b>{}</b>", self.argument()),
            "mathit" | "textit" => format!("<i>{}</i>", self.argument()),
            "mathtt" | "texttt" => format!("<tt>{}</tt>", escape(&self.raw_argument())),
            "mathbb" => self.raw_argument().chars().map(|c| blackboard(c).unwrap_or(c).to_string()).collect(),
            "mathcal" | "mathscr" | "mathfrak" | "mathsf" => self.argument(),
            "overline" | "bar" => format!("<span style=\"text-decoration:overline\">{}</span>", self.argument()),
            "underline" => format!("<u>{}</u>", self.argument()),
            "hat" | "widehat" => format!("{}\u{0302}", self.argument()),
            "tilde" | "widetilde" => format!("{}\u{0303}", self.argument()),
            "vec" => format!("{}\u{20D7}", self.argument()),
            "dot" => format!("{}\u{0307}", self.argument()),
            "ddot" => format!("{}\u{0308}", self.argument()),
            "left" | "right" | "big" | "Big" | "bigg" | "Bigg" | "bigl" | "bigr" | "Bigl" | "Bigr" | "displaystyle" | "textstyle" | "limits" | "nolimits" => {
                // Sizing only: the delimiter that follows is kept, "." means none.
                self.skip_space();
                if matches!(name, "left" | "right") && self.peek() == Some('.') {
                    self.at += 1;
                }
                String::new()
            }
            "begin" | "end" => {
                self.raw_argument();
                String::new()
            }
            "\\" | "cr" | "newline" => "<br>".into(),
            _ if operator(name) => format!("<span style=\"font-style:normal\">{name}</span>"),
            _ => match symbol(name) {
                Some(s) => s.into(),
                None => escape(&format!("\\{name}")),
            },
        };
        self.depth -= 1;
        out
    }
    fn group(&mut self, close: Option<char>) -> String {
        let mut out = String::new();
        while let Some(c) = self.peek() {
            self.at += 1;
            match c {
                c if Some(c) == close => return out,
                '{' => out.push_str(&self.group(Some('}'))),
                '}' => {}
                '\\' => {
                    let name = self.command();
                    out.push_str(&self.expand(&name));
                }
                '^' => out.push_str(&format!("<sup>{}</sup>", self.argument())),
                '_' => out.push_str(&format!("<sub>{}</sub>", self.argument())),
                '&' => out.push(' '),
                '~' => out.push(' '),
                '\'' => out.push('′'),
                '-' => out.push('−'),
                '*' => out.push('∗'),
                c if c.is_whitespace() => {
                    if !out.ends_with(' ') {
                        out.push(' ');
                    }
                }
                c if c.is_ascii_alphabetic() => out.push(c),
                c => out.push_str(&escape(&c.to_string())),
            }
        }
        out
    }
}

/// Rich text dialect a formula is rendered for. Only Qt's exists today; another
/// surface gets its own variant once its renderer is designed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Surface {
    #[default]
    Qt,
}
/// One formula for `surface`; `Surface::Qt` is exactly [`to_rich_text`].
pub fn render(tex: &str, display: bool, surface: Surface) -> String {
    match surface {
        Surface::Qt => to_rich_text(tex, display),
    }
}
/// Qt rich text for one formula; `display` centers it on its own line.
pub fn to_rich_text(tex: &str, display: bool) -> String {
    let mut parser = Parser { chars: tex.trim().chars().collect(), at: 0, depth: 0, _src: tex };
    let body = parser.group(None).trim().to_string();
    let body = body.replace("  ", " ");
    if display {
        format!("<p align=\"center\"><span style=\"font-family:serif;font-style:italic;font-size:large\">{body}</span></p>")
    } else {
        format!("<span style=\"font-family:serif;font-style:italic\">{body}</span>")
    }
}

/// Rewrites \[..\] and \(..\) outside code to $$..$$ and $..$ so the Markdown
/// parser's math extension sees them, as T3's remark-math setup does.
pub fn normalize_delimiters(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut fence: Option<String> = None;
    for line in markdown.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if let Some(open) = &fence {
            if trimmed.starts_with(open.as_str()) {
                fence = None;
            }
            out.push_str(line);
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = Some(trimmed[..3].to_string());
            out.push_str(line);
            continue;
        }
        // Inline code spans keep their backslashes.
        let mut in_code = false;
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '`' {
                in_code = !in_code;
                out.push(c);
                continue;
            }
            if c == '\\' && !in_code {
                match chars.peek() {
                    Some('[') | Some(']') => {
                        chars.next();
                        out.push_str("$$");
                        continue;
                    }
                    Some('(') | Some(')') => {
                        chars.next();
                        out.push('$');
                        continue;
                    }
                    _ => {}
                }
            }
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn common_constructs_become_rich_text() {
        assert_eq!(to_rich_text("x^2", false), "<span style=\"font-family:serif;font-style:italic\">x<sup>2</sup></span>");
        assert!(to_rich_text("\\alpha + \\beta_i", false).contains("α + β<sub>i</sub>"));
        assert!(to_rich_text("\\frac{a}{b}", false).contains("a⁄b"));
        assert!(to_rich_text("\\frac{a+1}{b}", false).contains("(a+1)⁄b"));
        assert!(to_rich_text("\\sqrt{x}", false).contains("√<span style=\"text-decoration:overline\">x</span>"));
        assert!(to_rich_text("\\mathbb{R}^n", false).contains("ℝ<sup>n</sup>"));
        assert!(to_rich_text("\\sum_{i=1}^{n} i", true).starts_with("<p align=\"center\">"));
        assert!(to_rich_text("\\sin x", false).contains("font-style:normal\">sin"));
        assert!(to_rich_text("\\unknowncmd", false).contains("\\unknowncmd"));
        assert!(to_rich_text("a<b", false).contains("a&lt;b"));
        assert!(to_rich_text("\\left( x \\right.", false).contains("( x"));
    }
    #[test]
    fn bracket_delimiters_outside_code_become_dollars() {
        assert_eq!(normalize_delimiters("see \\(x\\) and \\[y\\]"), "see $x$ and $$y$$");
        assert_eq!(normalize_delimiters("`\\(x\\)`"), "`\\(x\\)`");
        assert_eq!(normalize_delimiters("```\n\\[x\\]\n```\n"), "```\n\\[x\\]\n```\n");
    }
}
