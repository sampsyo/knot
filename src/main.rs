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

fn read_file(filename: &Path) -> Result<String, io::Error> {
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

fn is_note(e: &fs::DirEntry) -> bool {
    // This is a little sad. Waiting on:
    // https://github.com/rust-lang/rfcs/issues/900
    let os_name = e.file_name();
    let name = os_name.to_string_lossy();

    // Don't hardcode these! Also, eventually work with directories.
    !name.starts_with(".") && !name.starts_with("_") &&
        (name.ends_with(".markdown") || name.ends_with(".md")
         || name.ends_with(".mkdn"))
}

fn render_note(note: &Path, destdir: &Path) -> io::Result<()> {
    if let Some(name) = note.file_name() {
        let dest = destdir.join(name);
        println!("{:?} -> {:?}", note, dest);

        let md = read_file(note);
        match read_file(note) {
            Err(err) => println!("could not read note {:?}: {}", name, err),
            Ok(md) => {
                let html = markdown_to_html(&md);
                println!("{}", html);
            }
        }
    } else {
        println!("no filename"); // how?
    }
    Ok(())
}

fn render_notes(indir: &str, outdir: &str) -> io::Result<()> {
    let outpath = Path::new(&outdir);

    let rd = try!(fs::read_dir(indir));
    for entry in rd {
        let e = try!(entry);
        if is_note(&e) {
            try!(render_note(&e.path(), &outpath));
        }
    }

    Ok(())
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
        indir = if matches.free.len() >= 1 {
            matches.free[0].clone()
        } else {
            ".".to_string()
        }
    }

    println!("{:?} -> {:?}", indir, outdir);
    if let Err(err) = std::fs::create_dir_all(&outdir) {
        println!("could not create output directory {}: {}", outdir, err);
    }
    if let Err(err) = render_notes(&indir, &outdir) {
        println!("rendering failed: {}", err);
    }
}
