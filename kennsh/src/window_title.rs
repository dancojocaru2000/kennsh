use std::{fmt::Display, io::Write};

use c_wrapper::cwd::{getcwd, getcwd_tilde};

pub(crate) fn set_window_title<Elems>(elements: Elems) 
where Elems: IntoIterator<Item = WindowTitleElement> {
	if !c_wrapper::file::FileDescriptor::wrap_stdout(|s| s.is_a_tty()) {
		return;
	}

	// ANSI Beginning 
	print!("\x1b]0;");
	for element in elements {
		print!("{}", element);
	}
    // ANSI Ending
	print!("\x1b\\");
	let _ = std::io::stdout().flush();
}

pub(crate) enum WindowTitleElement {
	Separator,
	ShellName,
	LastExitCode(u8),
	AbsoluteCurrentWorkingDirectory,
	FullCurrentWorkingDirectory,
	ShortCurrentWorkingDirectory,
	CustomText(String),
}

static DEFAULT_SEPARATOR: &str = " - ";

impl Display for WindowTitleElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WindowTitleElement::Separator => write!(f, "{}", DEFAULT_SEPARATOR),
			WindowTitleElement::ShellName => write!(f, "{}", crate::SHELL_NAME),            
			WindowTitleElement::LastExitCode(code) => write!(f, "Last Status: {}", code),
            WindowTitleElement::AbsoluteCurrentWorkingDirectory => {
				write!(f, "{}", getcwd().unwrap_or("".into()))			
			}
            WindowTitleElement::FullCurrentWorkingDirectory => {
				write!(f, "{}", getcwd_tilde().unwrap_or("".into()))
			}
            WindowTitleElement::ShortCurrentWorkingDirectory => {
				if let Ok(path) = getcwd_tilde() {
					let items: Vec<&str> = path.split('/').collect();
					for (index, item) in items.iter().enumerate() {
						let item = if item.is_empty() {
							item.to_string()
						}
						else if index != items.len() - 1 {
							item[0..=0].to_string() + "/"
						}
						else {
							item.to_string()
						};
						write!(f, "{}", item)?;
					}
				};
				// TODO: Establish some error reporting
				Ok(())
			}
            WindowTitleElement::CustomText(text) => write!(f, "{}", text),
		}
    }
}
