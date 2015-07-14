extern crate pulldown_cmark;
extern crate getopts;
extern crate toml;
extern crate crypto;
extern crate rustc_serialize;
extern crate mustache;

use std::io;
use std::io::{Read, Write};
use std::path::Path;
use std::fs;
use std::env;

use pulldown_cmark::Parser;
use pulldown_cmark::html;
use getopts::{Options, Matches};
use crypto::digest::Digest;
use rustc_serialize::base64;
use rustc_serialize::base64::ToBase64;

const FILENAME_BYTES : usize = 10;

fn hash_str(h: &mut Digest, nbytes: usize) -> String {
    let hashbytes = h.output_bytes();
    let mut bytes : Vec<u8> = vec![0; hashbytes];
    h.result(&mut bytes);

    let config = base64::Config {
        char_set: base64::UrlSafe,
        newline: base64::Newline::LF,
        pad: false,
        line_length: None,
    };

    // zero or too many => get the whole hash.
    let trunc = if nbytes == 0 || nbytes > hashbytes { hashbytes }
        else { nbytes };
    bytes[0 .. trunc - 1].to_base64(config)
}

fn note_filename(name: &str, secret: &str) -> String {
    let mut h = crypto::sha2::Sha256::new();
    h.input_str(name);
    h.input_str(secret);
    hash_str(&mut h, FILENAME_BYTES) + ".html"
}

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

fn dump_file(filename: &Path, contents: &str) -> io::Result<()> {
    let mut f = try!(fs::File::create(filename));
    f.write_all(contents.as_bytes())
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
        let key = note_filename(&name.to_string_lossy(), ""); // TODO use secret

        let dest = destdir.join(key);
        println!("{:?} -> {:?}", note, dest);

        let md = try!(read_file(note));
        let html = markdown_to_html(&md);
        try!(dump_file(&dest, &html));
    } else {
        println!("no filename"); // how?
    }
    Ok(())
}

fn render_notes(indir: &str, outdir: &str) -> io::Result<()> {
    let outpath = Path::new(&outdir);

    try!(std::fs::create_dir_all(&outpath));

    let rd = try!(fs::read_dir(indir));
    for entry in rd {
        let e = try!(entry);
        if is_note(&e) {
            try!(render_note(&e.path(), &outpath));
        }
    }

    Ok(())
}

fn usage(program: &str, opts: &Options, error: bool) {
    let brief = format!("usage: {} [OPTIONS] NOTEDIR", program);
    let message = opts.usage(&brief);
    let message_bytes = message.as_bytes();

    // Not sure why I can't `let writer = if error io::stderr else io::stdout`.
    if error {
        io::stderr().write_all(&message_bytes).unwrap();
    } else {
        io::stdout().write_all(&message_bytes).unwrap();
    }
}

struct Config {
    secret: String,
    template: mustache::Template,
}

fn load_config(confdir: &str) -> Result<Config, &'static str> {
    let confdirpath = Path::new(&confdir);
    let conffilepath = confdirpath.join("knot.toml");

    // Load the configuration.
    let conftoml = match read_file(&conffilepath) {
      Err(_) => return Err("no config"),
      Ok(t) => t
    };
    let mut parser = toml::Parser::new(&conftoml);
    let configdata = match parser.parse() {
        Some(v) => v,
        None => {
            println!("TOML parse error: {:?}", parser.errors);
            return Err("could not parse config");
        }
    };

    // Extract useful information from the configuration.
    let secret = match configdata["secret"].as_str() {
        Some(v) => v,
        None => {
            return Err("secret must be a string");
        }
    };

    // Load and compile the template.
    let templpath = confdirpath.join("template.html");
    let templ = match mustache::compile_path(templpath) {
        Err(_) => return Err("no template found"),
        Ok(t) => t
    };

    Ok(Config {
        secret: secret.to_string(),
        template: templ,
    })
}

fn main() {
    // Parse command-line options.
    let outdir : String;
    let indir : String;
    let confdir : String;
    {
        let args: Vec<String> = env::args().collect();
        let program = args[0].clone();

        let mut opts = Options::new();
        opts.optopt("o", "out", "output directory", "PATH");
        opts.optflag("h", "help", "show this help message");
        opts.optopt("c", "config", "configuration directory", "PATH");
        let matches = match opts.parse(&args[1..]) {
            Ok(m) => { m }
            Err(f) => {
                writeln!(&mut std::io::stderr(), "{}", f).unwrap();
                usage(&program, &opts, true);
                std::process::exit(1);

                // Because this is unstable:
                // std::env::set_exit_status(1);
                // return;
            }
        };

        // Help flag.
        if matches.opt_present("help") {
            usage(&program, &opts, false);
            return;
        }

        // Directories for rendering.
        outdir = matches.opt_str_or("out", "_public");
        indir = if matches.free.len() >= 1 {
            matches.free[0].clone()
        } else {
            ".".to_string()
        };
        confdir = matches.opt_str_or("config", "_knot");
    }

    // Configuration.
    let config = load_config(&confdir).unwrap();

    println!("{:?} -> {:?}", indir, outdir);
    if let Err(err) = render_notes(&indir, &outdir) {
        println!("rendering failed: {}", err);
    }
}
