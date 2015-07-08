extern crate pulldown_cmark;
extern crate getopts;

use std::io;
use std::io::{Read, Write};
use std::path::Path;
use std::fs::File;
use std::env;

use pulldown_cmark::Parser;
use pulldown_cmark::html;
use getopts::{Options, Matches};

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

fn opt_str_or(matches: &Matches, opt: &str, default: &str) -> String {
    matches.opt_str(opt).unwrap_or(default.to_string())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let outdir : String;
    {
        let mut opts = Options::new();
        opts.optopt("o", "out", "output directory", "PATH");
        let matches = match opts.parse(&args[1..]) {
            Ok(m) => { m }
            Err(f) => {
                writeln!(&mut std::io::stderr(), "{}", f).unwrap();
                return;
            }
        };

        outdir = opt_str_or(&matches, "out", "_public");
    }

    let outpath = Path::new(&outdir);
    println!("{:?}", outpath);

    let md = read_file("test.md");
    let html = markdown_to_html(&md.unwrap());
    println!("{}", html);
}
