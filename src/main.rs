extern crate pulldown_cmark;
extern crate getopts;

use std::io;
use std::io::{Read, Write};
use std::path::Path;
use std::fs::File;
use std::env;

use pulldown_cmark::Parser;
use pulldown_cmark::html;
use getopts::Options;

// Stolen from the pulldown_cmark example.
fn markdown_to_html(text: &str) -> String {
    let mut s = String::with_capacity(text.len() * 3 / 2);
    let p = Parser::new(&text);
    html::push_html(&mut s, p);
    s
}

fn read_file(filename: &str) -> Result<String, io::Error> {
    let mut file = try!(File::open(filename));
    let mut contents = String::new();
    try!(file.read_to_string(&mut contents));
    Ok(contents)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optopt("o", "out", "output directory", "PATH");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => { m }
        Err(f) => {
            writeln!(&mut std::io::stderr(), "{}", f).unwrap();
            return;
        }
    };

    let outdir = matches.opt_str("out").unwrap_or("_public".to_string());
    let outpath = Path::new(&outdir);
    println!("{:?}", outpath);

    let md = read_file("test.md");
    let html = markdown_to_html(&md.unwrap());
    println!("{}", html);
}
