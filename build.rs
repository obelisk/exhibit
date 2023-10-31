use std::fs::File;
use std::io::{Read, Write};

fn main() {
    // Open web/join.html
    let mut file = File::options().read(true).open("web/join.html").unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    let contents = contents.replace("<title>Exhibit</title>", format!("<title>Exhibit v{}</title>", env!("CARGO_PKG_VERSION")).as_str());
    let mut file = File::options().write(true).truncate(true).open("web/join.html").unwrap();
    file.write(contents.as_bytes()).unwrap();
}