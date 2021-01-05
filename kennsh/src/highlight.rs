use rustyline::{Helper, validate::Validator, completion::Completer, hint::Hinter, highlight::Highlighter};

#[derive(Debug, Copy, Clone)]
struct Highlight;

impl Highlighter for Highlight {
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> std::borrow::Cow<'l, str> {
        let _ = pos;
        std::borrow::Cow::Borrowed(line)
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> std::borrow::Cow<'h, str> {
        std::borrow::Cow::Borrowed(hint)
    }

    fn highlight_candidate<'c>(
        &self,
        candidate: &'c str,
        completion: rustyline::CompletionType,
    ) -> std::borrow::Cow<'c, str> {
        let _ = completion;
        std::borrow::Cow::Borrowed(candidate)
    }

    fn highlight_char(&self, line: &str, pos: usize) -> bool {
		if char::from(line.as_bytes()[pos]) == '`' {
			true
		}
		else if pos != 0 && char::from(line.as_bytes()[pos - 1]) == '`' {
			true
		}
		else {
			false
		}
    }
}
