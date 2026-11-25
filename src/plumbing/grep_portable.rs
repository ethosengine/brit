//! Byte-oriented regex matching on platforms without the POSIX regex API.
use gix::{
    Result,
    error::{OptionExt, ResultExt, bail, message},
};

pub(super) struct Regex(regex::bytes::Regex);

impl Regex {
    pub(super) fn new(pattern: &[u8], extended: bool, ignore_case: bool) -> Result<Self> {
        let refusal = || {
            if extended {
                message("Unsupported POSIX extended grep expression; use -F for literal text")
            } else {
                message(
                    "Unsupported POSIX basic grep expression; use -F for literal text or -E for extended expressions",
                )
            }
            .unsupported()
        };
        let pattern = std::str::from_utf8(pattern).or_raise(refusal)?;
        let mut expression = String::new();
        let mut chars = pattern.chars().enumerate().peekable();
        while let Some((index, ch)) = chars.next() {
            match ch {
                '^' if !extended && index != 0 => bail!(refusal()),
                '$' if !extended && chars.peek().is_some() => bail!(refusal()),
                '\\' => {
                    let (_, escaped) = chars.next().ok_or_raise(refusal)?;
                    if !extended && "(){}+?|".contains(escaped) {
                        expression.push(escaped);
                    } else if ".[\\*^$]".contains(escaped) || extended && "(){}+?|".contains(escaped) {
                        expression.push('\\');
                        expression.push(escaped);
                    } else {
                        bail!(refusal());
                    }
                }
                '[' => {
                    expression.push(ch);
                    if chars.next_if(|(_, ch)| *ch == '^').is_some() {
                        expression.push('^');
                    }
                    if chars.next_if(|(_, ch)| *ch == ']').is_some() {
                        expression.push_str(r"\]");
                    }
                    loop {
                        match chars.next().map(|(_, ch)| ch) {
                            Some(']') => {
                                expression.push(']');
                                break;
                            }
                            Some('[' | '\\') | None => bail!(refusal()),
                            Some(ch)
                                if matches!(ch, '&' | '~' | '-')
                                    && chars.peek().is_some_and(|(_, next)| *next == ch) =>
                            {
                                bail!(refusal())
                            }
                            Some(ch) => expression.push(ch),
                        }
                    }
                }
                '(' | ')' | '{' | '}' | '+' | '?' | '|' if !extended => {
                    expression.push('\\');
                    expression.push(ch);
                }
                '(' if chars.peek().is_some_and(|(_, ch)| *ch == '?') => bail!(refusal()),
                // POSIX treats a CR as part of the line, including when using an end anchor.
                ch => expression.push(ch),
            }
        }
        regex::bytes::RegexBuilder::new(&expression)
            .unicode(false)
            .case_insensitive(ignore_case)
            .build()
            .map(Self)
            .or_raise(refusal)
    }

    pub(super) fn is_match(&self, line: &[u8]) -> Result<bool> {
        Ok(self.0.is_match(line))
    }
}

pub(super) struct Locale;

pub(super) fn use_environment_locale() -> Locale {
    Locale
}
