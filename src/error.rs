pub fn fail(msg: String) -> ! {
    eprintln!("error: {}", msg);
    std::process::exit(1);
}