// This file is part of rust-u4pak.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::{fmt::Display, fs::File, io::Read, path::PathBuf};

use crate::{Error, Result};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
enum ParseState {
    Space,
    Comment,
    PlainString,
    QuotedString,
    Quote,
}

fn parser_error(source: &[u8], index: usize, message: impl Display) -> Error {
    let prefix = &source[0..index];
    let lineno = prefix.iter().copied().filter(|&byte| byte == b'\n').count() + 1;
    let line_start =
        if let Some(line_start) = prefix.iter().copied().rposition(|byte| byte == b'\n') {
            line_start + 1
        } else {
            0
        };
    let column = String::from_utf8_lossy(&prefix[line_start..]).len() + 1;

    let line_end = index
        + if let Some(line_end) = (source[index..])
            .iter()
            .copied()
            .position(|byte| byte == b'\n')
        {
            line_end
        } else {
            source.len()
        };

    let line = String::from_utf8_lossy(&source[line_start..line_end]);
    let lineno_str = format!("{}: ", lineno);
    let mut message = format!(
        "{}:{}: {}\n{}{}\n",
        lineno, column, message, lineno_str, line
    );

    for _ in 0..lineno_str.len() {
        message.push(' ');
    }

    if column > 1 {
        for _ in 0..(column - 1) {
            message.push('-');
        }
    }
    message.push('^');

    Error::new(message)
}

pub fn parse_arg_file(bin_name: String, source: &[u8]) -> Result<Vec<String>> {
    let mut args = vec![bin_name];

    let mut state = ParseState::Space;
    let mut start_index = 0usize;
    let mut buffer = String::new();

    for (index, &byte) in source.iter().enumerate() {
        match state {
            ParseState::Space => match byte {
                b'"' => {
                    start_index = index + 1;
                    state = ParseState::QuotedString;
                }
                b'#' => {
                    state = ParseState::Comment;
                }
                _ if byte.is_ascii_whitespace() => {}
                _ => {
                    start_index = index;
                    state = ParseState::PlainString;
                }
            },
            ParseState::Comment => {
                if byte == b'\n' {
                    state = ParseState::Space;
                }
            }
            ParseState::PlainString => {
                if byte.is_ascii_whitespace() {
                    let value = match std::str::from_utf8(&source[start_index..index]) {
                        Ok(value) => value,
                        Err(error) => {
                            return Err(parser_error(
                                source,
                                start_index + error.valid_up_to(),
                                error,
                            ));
                        }
                    };
                    buffer.push_str(value);
                    args.push(buffer.to_owned());
                    buffer.clear();
                    state = ParseState::Space;
                } else if byte == b'"' {
                    let value = match std::str::from_utf8(&source[start_index..index]) {
                        Ok(value) => value,
                        Err(error) => {
                            return Err(parser_error(
                                source,
                                start_index + error.valid_up_to(),
                                error,
                            ));
                        }
                    };
                    buffer.push_str(value);
                    start_index = index + 1;
                    state = ParseState::QuotedString;
                }
            }
            ParseState::QuotedString => {
                if byte == b'"' {
                    state = ParseState::Quote;
                }
            }
            ParseState::Quote => {
                if byte == b'"' {
                    buffer.push('"');
                    start_index = index + 1;
                    state = ParseState::QuotedString;
                } else if byte.is_ascii_whitespace() {
                    let value = match std::str::from_utf8(&source[start_index..index - 1]) {
                        Ok(value) => value,
                        Err(error) => {
                            return Err(parser_error(
                                source,
                                start_index + error.valid_up_to(),
                                error,
                            ));
                        }
                    };
                    buffer.push_str(value);
                    args.push(buffer.to_owned());
                    buffer.clear();
                    state = ParseState::Space;
                } else {
                    let value = match std::str::from_utf8(&source[start_index..index - 1]) {
                        Ok(value) => value,
                        Err(error) => {
                            return Err(parser_error(
                                source,
                                start_index + error.valid_up_to(),
                                error,
                            ));
                        }
                    };
                    buffer.push_str(value);
                    start_index = index;
                    state = ParseState::PlainString;
                }
            }
        }
    }

    match state {
        ParseState::Comment | ParseState::Space => {}
        ParseState::PlainString => {
            let value = match std::str::from_utf8(&source[start_index..]) {
                Ok(value) => value,
                Err(error) => {
                    return Err(parser_error(
                        source,
                        start_index + error.valid_up_to(),
                        error,
                    ));
                }
            };
            buffer.push_str(value);
            args.push(buffer);
        }
        ParseState::QuotedString => {
            let index = if let Some(&b'\n') = source.last() {
                source.len() - 1
            } else {
                source.len()
            };
            return Err(parser_error(source, index, "unexpected end of file"));
        }
        ParseState::Quote => {
            let value = match std::str::from_utf8(&source[start_index..source.len() - 1]) {
                Ok(value) => value,
                Err(error) => {
                    return Err(parser_error(
                        source,
                        start_index + error.valid_up_to(),
                        error,
                    ));
                }
            };
            buffer.push_str(value);
            args.push(buffer);
        }
    }

    Ok(args)
}

pub fn get_args_from_file() -> Result<Option<Vec<String>>> {
    let mut args = std::env::args();
    if args.len() != 2 {
        return Ok(None);
    }

    let bin_name = if let Some(arg) = args.next() {
        arg
    } else {
        return Ok(None);
    };

    let path = if let Some(arg) = args.next() {
        if let Some(index) = arg.rfind('.') {
            let ext = &arg[index + 1..];
            if ext.eq_ignore_ascii_case("u4pak") {
                arg
            } else if ext.eq_ignore_ascii_case("pak") {
                return Ok(Some(vec![
                    bin_name,
                    "info".to_string(),
                    "-h".to_string(),
                    arg,
                ]));
            } else {
                return Ok(None);
            }
        } else {
            return Ok(None);
        }
    } else {
        return Ok(None);
    };

    let path = PathBuf::from(path);
    let mut file = match File::open(&path) {
        Ok(file) => file,
        Err(error) => return Err(Error::io_with_path(error, path)),
    };
    let mut source = Vec::new();
    match file.read_to_end(&mut source) {
        Ok(_) => {}
        Err(error) => return Err(Error::io_with_path(error, path)),
    }

    match parse_arg_file(bin_name, &source) {
        Ok(args) => {
            if let Some(path) = path.parent() {
                // no components means the file path was relative to the current directory
                // and passing an empty path to set_current_dir() gives an error
                if path.components().count() > 0 {
                    // so that relative paths inside of the .u4pak file work
                    std::env::set_current_dir(path)?;
                }
            }
            Ok(Some(args))
        }
        Err(error) => Err(error.with_path(path)),
    }
}
