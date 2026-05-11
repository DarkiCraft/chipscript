// Phase labels used in every error message.
pub const LEX:    &str = "lexer";
pub const PARSE:  &str = "parser";
pub const ANALYZE: &str = "analyzer";
pub const CODEGEN: &str = "codegen";

/// Abort with a phase-tagged, line-located error.
///   error[lexer] line 12: unexpected character '@'
pub fn fail_at(phase: &str, line: u32, msg: impl AsRef<str>) -> ! {
    eprintln!("error[{}] line {}: {}", phase, line, msg.as_ref());
    #[cfg(feature = "testing")]
    panic!("error[{}] line {}: {}", phase, line, msg.as_ref());
    #[cfg(not(feature = "testing"))]
    std::process::exit(1);
}

/// Abort with a phase-tagged error (no line info — used by analyzer/codegen).
///   error[analyzer]: undeclared variable 'x'
pub fn fail(phase: &str, msg: impl AsRef<str>) -> ! {
    eprintln!("error[{}]: {}", phase, msg.as_ref());
    #[cfg(feature = "testing")]
    panic!("error[{}]: {}", phase, msg.as_ref());
    #[cfg(not(feature = "testing"))]
    std::process::exit(1);
}
