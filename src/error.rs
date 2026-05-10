#[cfg(not(test))]
pub fn fail(msg: String) -> ! {
    eprintln!("error: {}", msg);
    std::process::exit(1);
}

#[cfg(test)]
pub fn fail(msg: String) -> ! {
    eprintln!("error: {}", msg);
    panic!("{}", msg);
}