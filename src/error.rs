pub fn fail(msg: String) -> ! {
    eprintln!("error: {}", msg);
    #[cfg(feature = "testing")]
    panic!("{}", msg);
    #[cfg(not(feature = "testing"))]
    std::process::exit(1);
}