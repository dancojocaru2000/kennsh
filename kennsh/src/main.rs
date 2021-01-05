mod str_extension;
mod command;
mod error;
mod window_title;
mod syntax_parser;
// mod highlight;
mod env_util;

use c_wrapper::cwd::getcwd_tilde;
use error::Error;
pub(crate) use error::Result;

use kennsh_syscall_macro::syscall;
use rustyline::{Editor, error::ReadlineError};
use str_extension::StringExtensions;
use window_title::{WindowTitleElement, set_window_title};
use crate::syntax_parser::ParseError;

static SHELL_NAME: &str = "kennsh";

#[derive(Clone, Copy)]
enum ANSIColor {
    FourBitNormal(u8),
    FourBitBright(u8),
    EightBit(u8),
    Reset,
}

impl ANSIColor {
    pub fn fg(&self) -> String {
        match self {
            ANSIColor::FourBitNormal(code) => format!("\x1b[{}m", 30 + code),
            ANSIColor::FourBitBright(code) => format!("\x1b[{}m", 90 + code),
            ANSIColor::EightBit(code) => format!("\x1b[38;5;{}m", code),
            ANSIColor::Reset => format!("\x1b[{}m", 39),
        }
    }

    pub fn bg(&self) -> String {
        match self {
            ANSIColor::FourBitNormal(code) => format!("\x1b[{}m", 40 + code),
            ANSIColor::FourBitBright(code) => format!("\x1b[{}m", 100 + code),
            ANSIColor::EightBit(code) => format!("\x1b[48;5;{}m", code),
            ANSIColor::Reset => format!("\x1b[{}m", 49),
        }
    }
}

fn powerline_arrow(previous: &ANSIColor, next: &ANSIColor) -> String {
    static POWERLINE_ARROW: &str = "";
    format!(
        "{1}{2}{0}{3}",
        POWERLINE_ARROW,
        previous.fg(),
        next.bg(),
        ANSIColor::Reset.fg(),
    )
}

fn powerline_blocks(blocks: Vec<(String, ANSIColor)>) -> String {
    if blocks.len() < 1 {
        "".to_owned()
    }
    else {
        let it1 = blocks.iter();
        let it2 = blocks
            .iter()
            .skip(1)
            .map(|e| Some(e))
            .chain(core::iter::repeat(None));
        let mut result = String::new();
        for ((current_string, current_color), next) in it1.zip(it2) {
            result += &current_color.bg();
            result += &current_string;
            let next_color = if let Some((_, next_color)) = next {
                next_color
            }
            else {
                &ANSIColor::Reset
            };
            result += &powerline_arrow(current_color, next_color);
            result += &ANSIColor::Reset.fg();
        }        
        result
    }
}

// TODO: Make the prompt customizable instead of a fixed function
fn prompt(error: bool) -> String {
    let use_powerline = env_util::env_is_true("use_powerline");
    if use_powerline {
        use ANSIColor::*;
        let mut blocks = vec![];
        blocks.push((format!("{} kennsh ", ANSIColor::FourBitNormal(0).fg()), if error { FourBitNormal(1) } else { FourBitBright(4) }));
        if let Ok(path) = syscall!(getcwd_tilde) {
            blocks.push((format!(" {} ", path), FourBitNormal(5)));
        }
        powerline_blocks(blocks) + " "
    }
    else {
        "\x1b[".to_owned() +
        if error { "41" } else { "7" } +
        "m" +
        "[kennsh]" +
        &if let Ok(path) = syscall!(getcwd_tilde) {
            path
                .rsplit('/')
                .nth(0)
                .map(|s| " ".to_owned() + s)
                .map(|s| s)
                .unwrap_or("".to_owned())
        } else { "".to_owned() } + 
        " >" +
        "\x1b[m" +
        " "
    }
}

fn main() {
    // Configure readline
    let mut rl = Editor::<()>::new();

    let mut last_exit_code: u8 = 0;

    loop {
        // Set window title
        set_window_title(vec![
            WindowTitleElement::ShellName,
            WindowTitleElement::Separator,
            WindowTitleElement::LastExitCode(last_exit_code),
            WindowTitleElement::Separator,
            WindowTitleElement::ShortCurrentWorkingDirectory,
        ]);
       
        // If not at the beginning of the line, add artificial newline
        if c_wrapper::file::FileDescriptor::wrap_stdout(|o| o.is_a_tty()) {
            if let Some((col, _)) = crossterm::cursor::position().ok() {
                if col != 0 {
                    println!("\x1b[7m⏎\x1b[27m")
                    // println!("⏎")
                }
            }
        }
        // If some application would forget to reset stdin to blocking, reset it
        let _ = c_wrapper::file::FileDescriptor::wrap_stdin(|stdin| {
            stdin.set_nonblocking(false)
        });

        let readline = rl.readline(&prompt(last_exit_code != 0));
        match readline {
            Ok(line) => {
                let line = line.trim().to_string();
                if !line.is_blank() {
                    rl.add_history_entry(line.clone());
                    if env_util::env_is_true("stderr_red") {
                        eprint!("\x1b[91m");
                    }
                    match command::handle(line.clone()) {
                        Ok(errorcode) => {
                            last_exit_code = errorcode;
                            // if errorcode != 0 {
                            //     eprintln!("\x1b[3mkennsh: The program exited with the following code:\x1b[m {}", errorcode);
                            // }
                        },
                        Err(e) => match e {
                            Error::SyscallError { call_name: f, error: e } => {
                                // TODO: Once settings are up, print only in verbose flag
                                eprintln!("\x1b[3mkennsh: While attempting to use the \x1b[m\x1b[4m{}\x1b[m\x1b[3m system call, the following error occured:\x1b[m {}", f, e);
                                eprintln!("\x1b[3m        This is generally a sign of an internal error; please file a bug report\x1b[0m");
                            }
                            Error::RequestExit(errorcode) => std::process::exit(errorcode.unwrap_or_else(|| last_exit_code).into()),
                            Error::ParseError(pe) => {
                                let ParseError {
                                    start_index,
                                    end_index,
                                    reason,
                                } = pe;
                                eprintln!("\x1b[3mkennsh: Syntax error:\x1b[0m {}", reason);
                                eprintln!("{}", line);
                                eprint!("{}", " ".repeat((start_index - 1).max(0)));
                                eprint!("\x1b[31m");
                                eprint!("{}", "^".repeat((end_index - start_index).max(1)));
                                eprintln!("\x1b[0m");
                            }
                            Error::CommandNotFound(cmd) => {
                                last_exit_code = 127;
                                eprintln!("\x1b[3mkennsh: The command was not found:\x1b[0m {}", cmd)
                            }
                            Error::FileNotFound(path) => {
                                last_exit_code = 125;
                                eprint!("\x1b[3mkennsh: The file or directory was not found");
                                if let Some(path) = path {
                                    eprint!(":\x1b[0m {}", path);
                                }
                                eprintln!("\x1b[0m")
                            }
                            Error::ExitCodeParseError(exit_code) => {
                                last_exit_code = 1;
                                eprintln!("\x1b[3mkennsh: An invalid exit code was given to the exit command:\x1b[0m {}", exit_code)
                            }
                            Error::CommandPermissionDenied(cmd) => {
                                last_exit_code = 126;
                                eprintln!("\x1b[3mkennsh: Permission was denied to run the following command (is it executable?):\x1b[0m {}", cmd)
                            }
                            Error::FilePermissionDenied(file) => {
                                last_exit_code = 126;
                                if let Some(file) = file {
                                    eprintln!("\x1b[3mkennsh: Permission was denied to access the following file:\x1b[0m {}", file)
                                }
                                else {
                                    eprintln!("\x1b[3mkennsh: Permission was denied to access a file\x1b[0m")
                                }
                            }
                            Error::OtherError(message) => {
                                last_exit_code = 1;
                                eprintln!("\x1b[3mkennsh: Error:\x1b[0m {}", message)
                            }
                            Error::ChdirRequested(path) => {
                                panic!(format!("chdir to path {} was not handled", path))
                            }
                            Error::DynamicLibraryError(error) => {
                                last_exit_code = 124;
                                eprintln!("\x1b[3mkennsh: Dynamic Library error:\x1b[0m {}", error)
                            }
                            Error::NoStatusChange => {}
                            Error::EnvRemoveRequested(key) => {
                                panic!(format!("env key {} removal was not handled", key))
                            }
                            Error::EnvChangeRequested(key, value) => {
                                panic!(format!("env key {} set to {} was not handled", key, value))
                            }
                        }
                    };
                    // TODO: Store error code somewhere
                    //       As a temporary measure, it is stored in an envvar
                    std::env::set_var("status", last_exit_code.to_string());
                    // ANSI Reset
                    print!("\x1b[m");
                }
            },
            Err(ReadlineError::Eof) => {
                break
            },
            _ => {}
        }
    }
}



