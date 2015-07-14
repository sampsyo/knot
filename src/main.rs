extern crate pulldown_cmark;
extern crate getopts;
extern crate toml;
extern crate crypto;
extern crate rustc_serialize;
extern crate mustache;

use std::io;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::fs;
use std::env;

use pulldown_cmark::{Parser, Event, Tag};
use pulldown_cmark::html;
use getopts::{Options, Matches};
use crypto::digest::Digest;
use rustc_serialize::base64;
use rustc_serialize::base64::ToBase64;

const FILENAME_BYTES : usize = 10;
const MARKDOWN_NOTE_NAME : &'static str = "note.md";

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

fn note_dirname(note_path: &Path, secret: &str) -> String {
    let name = note_path.file_stem().unwrap();  // Better have a name!

    let mut h = crypto::sha2::Sha256::new();
    // Eventually, this should use the raw data: bytes on Unix, something
    // (Unicode with surrogates?) on Windows. But Path::as_bytes() is
    // currently unstable.
    h.input_str(&name.to_string_lossy());
    h.input_str(secret);
    hash_str(&mut h, FILENAME_BYTES)
}

// Produce the HTML body for the Markdown document along with the text of the
// first header.
fn render_markdown(text: &str) -> (String, String) {
    // We will collect the first header in the document here during parsing.
    let mut the_header = String::new();

    let body = {
        // Magic ratio stolen from the pulldown_cmark example.
        let mut out = String::with_capacity(text.len() * 3 / 2);
        let parser = Parser::new(&text);

        // Hook into the parser to pull out the first heading.
        let mut first_header = true;
        let mut in_header = false;
        let extracting_parser = parser.inspect(|event| {
            match *event {
                Event::Start(ref t) => {
                    match *t {
                        Tag::Header(_) => if first_header {
                            in_header = true;
                            first_header = false;
                        },
                        _ => (),
                    }
                },
                Event::End(ref t) => {
                    match *t {
                        Tag::Header(_) => if in_header {
                            in_header = false;
                        },
                        _ => (),
                    }
                },
                Event::Text(ref s) => if in_header {
                    the_header.push_str(&s);
                },
                _ => (),
            };
        });

        // Run the parser and render HTML.
        html::push_html(&mut out, extracting_parser);
        out
    };

    (body, the_header)
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

    // TODO Don't hardcode these! Also, eventually work with directories.
    !name.starts_with(".") && !name.starts_with("_") &&
        (name.ends_with(".markdown") || name.ends_with(".md")
         || name.ends_with(".mkdn") || name.ends_with(".txt"))
}

fn render_note(note: &Path, destdir: &Path, config: &Config) -> io::Result<()> {
    if let Some(name) = note.file_name() {
        // Get the destination and create its enclosing directory.
        let basename = note_dirname(&note, &config.secret);
        let notedir = destdir.join(&basename);
        try!(std::fs::create_dir_all(&notedir));
        let dest = notedir.join("index.html");
        println!("{} -> {}", name.to_string_lossy(), basename);

        // Render the HTML from the Markdown.
        let md = try!(read_file(note));
        let (content, title) = render_markdown(&md);

        // Render the template to the destination file.
        let data = mustache::MapBuilder::new()
            .insert_str("content", content)
            .insert_str("title", title)
            .build();
        let mut f = try!(fs::File::create(dest));
        config.template.render_data(&mut f, &data);

        // Also copy the raw Markdown to the directory.
        try!(fs::copy(note, notedir.join(MARKDOWN_NOTE_NAME)));
    } else {
        println!("no filename"); // how?
    }
    Ok(())
}

fn render_notes(indir: &str, outdir: &str, config: &Config) -> io::Result<()> {
    let outpath = Path::new(&outdir);

    // Render the notes themselves.
    for entry in try!(fs::read_dir(indir)) {
        let e = try!(entry);
        if is_note(&e) {
            try!(render_note(&e.path(), &outpath, &config));
        }
    }

    // Copy the static files.
    let staticdir = config.confdir.join("static");
    if let Ok(rd) = fs::read_dir(&staticdir) {
        for entry in rd {
            let e = try!(entry);
            // TODO copy directories too
            let frompath = e.path();
            let topath = outpath.join(e.file_name());
            println!("{} -> {}", frompath.to_string_lossy(),
                     topath.to_string_lossy());
            try!(fs::copy(frompath, topath));
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
    confdir: PathBuf,
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
        confdir: PathBuf::from(confdir),
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

    if let Err(err) = render_notes(&indir, &outdir, &config) {
        println!("rendering failed: {}", err);
    }
}
