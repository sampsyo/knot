extern crate pulldown_cmark;
extern crate getopts;

use std::io;
use std::io::{Read, Write};
use std::path::Path;
use std::fs;
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
    let mut file = try!(fs::File::open(filename));
    let mut contents = String::new();
    try!(file.read_to_string(&mut contents));
    Ok(contents)
}

// A little convenient extension trait for getopts.
trait MatchesExt {
    fn opt_str_or(&self, opt: &str, default: &str) -> String;
}
impl MatchesExt for Matches {
    fn opt_str_or(&self, opt: &str, default: &str) -> String {
        self.opt_str(opt).unwrap_or(default.to_string())
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let outdir : String;
    let indir : String;
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

        outdir = matches.opt_str_or("out", "_public");
        indir = if matches.free.len() > 1 {
            matches.free[0].clone()
        } else {
            ".".to_string()
        }
    }

    println!("{:?} -> {:?}", indir, outdir);

    for entry in fs::read_dir(indir).unwrap() {
        let e = entry.unwrap();

        // This is a little sad. Waiting on:
        // https://github.com/rust-lang/rfcs/issues/900
        let os_name = e.file_name();
        let name = os_name.to_string_lossy();

        if !name.starts_with(".") && !name.starts_with("_") {
            println!("{:?}", e.path());
        }
    }

    let md = read_file("test.md");
    let html = markdown_to_html(&md.unwrap());
    println!("{}", html);
}
