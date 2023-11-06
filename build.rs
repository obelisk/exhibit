use regex::Regex;

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;

const ELM_FILES: [&str; 1] = ["Join"];

fn main() {
    println!("cargo:rerun-if-changed=web/");

    let title_detector = Regex::new(r"<title>Exhibit v\d+\.\d+\.\d+</title>").unwrap();
    // Open web/join.html
    let mut file = File::options().read(true).open("web/join.html").unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    let result = title_detector.replace(
        &contents,
        format!("<title>Exhibit v{}</title>", env!("CARGO_PKG_VERSION")),
    );

    let mut file = File::options()
        .write(true)
        .truncate(true)
        .open("web/join.html")
        .unwrap();
    file.write(result.as_bytes()).unwrap();

    for file in ELM_FILES {
        if let Err(e) = Command::new("elm")
            .current_dir(Path::new("web/elm"))
            .arg("make")
            .arg(format!("src/{file}.elm"))
            .arg("--output")
            .arg(format!("../static/{file}.js"))
            //.arg("--optimize")
            .output()
        {
            println!("cargo:warning={:?}", e);
        }
    }
}
