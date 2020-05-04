extern crate comrak;
extern crate getopts;
extern crate toml;
extern crate crypto;
extern crate base32;
extern crate mustache;

use std::io;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::fs;
use std::env;
use std::str;

use comrak::{parse_document, format_html, Arena, ComrakOptions};
use comrak::nodes::NodeValue;
use crypto::digest::Digest;

const FILENAME_BYTES : usize = 8;
const MARKDOWN_NOTE_NAME : &'static str = "note.md";

fn hash_str(h: &mut dyn Digest, nbytes: usize) -> String {
    let hashbytes = h.output_bytes();
    let mut bytes : Vec<u8> = vec![0; hashbytes];
    h.result(&mut bytes);

    // zero or too many => get the whole hash.
    let trunc = if nbytes == 0 || nbytes > hashbytes { hashbytes }
        else { nbytes };
    let trunc_bytes = &bytes[0 .. trunc - 1];

    let slug = base32::encode(base32::Alphabet::Crockford, trunc_bytes);
    slug.to_lowercase()
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
        // Parse the Markdown.
        let arena = Arena::new();
        let root = parse_document(&arena, &text, &ComrakOptions::default());

        // Look for the first heading in the AST.
        for child in root.children() {
            match child.data.borrow().value {
                NodeValue::Heading(_) => {
                    for child in child.children() {
                        match &child.data.borrow().value {
                            NodeValue::Text(text) => {
                                the_header = str::from_utf8(text).unwrap().
                                    to_string();
                            },
                            _ => ()
                        }
                    }
                    break;
                },
                _ => (),
            }
        }

        // Render HTML.
        let mut html = vec![];
        format_html(root, &ComrakOptions::default(), &mut html).unwrap();
        String::from_utf8(html).unwrap()
    };

    (body, the_header)
}

fn read_file(filename: &Path) -> Result<String, io::Error> {
    let mut file = fs::File::open(filename)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

// A little convenient extension trait for getopts.
trait MatchesExt {
    fn opt_str_or(&self, opt: &str, default: &str) -> String;
}
impl MatchesExt for getopts::Matches {
    fn opt_str_or(&self, opt: &str, default: &str) -> String {
        self.opt_str(opt).unwrap_or(default.to_string())
    }
}

fn render_note(note: &Path, config: &Config) -> io::Result<()> {
    if let Some(name) = note.file_name() {
        // Get the destination and create its enclosing directory.
        let basename = note_dirname(&note, &config.secret);
        let notedir = config.outdir.join(&basename);
        std::fs::create_dir_all(&notedir)?;
        let dest = notedir.join("index.html");
        if !config.quiet {
            println!("{} -> {}", name.to_string_lossy(), basename);
        }

        // Render the HTML from the Markdown.
        let md = read_file(note)?;
        let (content, title) = render_markdown(&md);

        // Render the template to the destination file.
        let data = mustache::MapBuilder::new()
            .insert_str("content", content)
            .insert_str("title", title)
            .insert_str("sourcefile", MARKDOWN_NOTE_NAME)
            .insert_str("key", basename)
            .build();
        let mut f = fs::File::create(dest)?;
        config.template.render_data(&mut f, &data).unwrap();

        // Also copy the raw Markdown to the directory.
        fs::copy(note, notedir.join(MARKDOWN_NOTE_NAME))?;
    } else {
        println!("no filename"); // how?
    }
    Ok(())
}

// Vec::contains, but for String/&str matching.
fn str_vec_contains(v: &Vec<String>, s: &str) -> bool {
    v.iter().any(|t| t == s)
}

// Get the last chunk after a dot in a string, if the string contains a dot. If
// the string ends in a dot, the extension is the empty string.
fn extension(s: &str) -> Option<&str> {
    let mut split = s.rsplitn(2, ".");
    if let Some(ext) = split.next() {
        if let Some(_) = split.next() {
            Some(ext)
        } else {
            None
        }
    } else {
        None
    }
}

// Try to render one of the things in the source directory. This only does
// anything if the entry looks note-like, based on its filename.
// TODO It should probably return a boolean indicating whether it did anything.
fn render_entry(entry: &fs::DirEntry, config: &Config) -> io::Result<()> {
    // This is a little sad. Waiting on:
    // https://github.com/rust-lang/rfcs/issues/900
    let os_name = entry.file_name();
    let name = os_name.to_string_lossy();

    // Filter out invisible names and our own bookkeeping.
    // TODO Don't hardcode these!
    if name.starts_with(".") || name.starts_with("_") {
        return Ok(());
    }

    let ft = entry.file_type()?;
    if ft.is_dir() {
        // All directories are notes.
        // TODO
        Ok(())
    } else {
        // Test file extension.
        if let Some(ext) = extension(&name) {
            if str_vec_contains(&config.extensions, &ext) {
                render_note(&entry.path(), &config)
            } else {
                // Not a note extension.
                Ok(())
            }
        } else {
            // No extension.
            Ok(())
        }
    }
}

fn render_notes(config: &Config) -> io::Result<()> {
    // Render the notes themselves.
    for entry in fs::read_dir(&config.indir)? {
        let e = entry?;
        render_entry(&e, &config)?;
    }

    // Copy the static files.
    let staticdir = config.confdir.join("static");
    if let Ok(rd) = fs::read_dir(&staticdir) {
        for entry in rd {
            let e = entry?;
            // TODO copy directories too
            let frompath = e.path();
            let topath = config.outdir.join(e.file_name());
            if !config.quiet {
                println!("{} -> {}", frompath.to_string_lossy(),
                         topath.to_string_lossy());
            }
            fs::copy(frompath, topath)?;
        }
    }

    Ok(())
}

fn usage(program: &str, opts: &getopts::Options, error: bool) {
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
    indir: PathBuf,
    outdir: PathBuf,
    confdir: PathBuf,
    quiet: bool,
    extensions: Vec<String>,
}

fn load_config(opts: Options) -> Result<Config, &'static str> {
    let confdirpath = PathBuf::from(opts.confdir);
    let conffilepath = confdirpath.join("knot.toml");

    // Load the configuration.
    let conftoml = match read_file(&conffilepath) {
      Err(_) => return Err("no config"),
      Ok(t) => t
    };
    // TODO Handle parse errors.
    let configdata = conftoml.parse::<toml::Value>().unwrap();

    // Extract secret from the configuration.
    // TODO check for missing key
    let secret = match configdata["secret"].as_str() {
        Some(v) => v,
        None => {
            return Err("secret must be a string");
        }
    };

    // Extract extensions from the configuration.
    let extensions = if let Some(extsvalue) = configdata.get("extensions") {
        match extsvalue.as_array() {
            Some(vs) => {
                let mut ss: Vec<String> = Vec::new();
                for v in vs {
                    match v.as_str() {
                        Some(s) => ss.push(s.to_string()),
                        None => return Err("extensions must be strings")
                    };
                };
                ss
            },
            None => return Err("extensions must be a list")
        }
    } else {
        // TODO any less-terrible way to do this?
        vec!["md".to_string(), "mkdn".to_string(), "markdown".to_string(),
             "txt".to_string()]
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
        indir: PathBuf::from(opts.indir),
        outdir: PathBuf::from(opts.outdir),
        confdir: confdirpath,
        quiet: opts.quiet,
        extensions: extensions,
    })
}

struct Options {
    indir: String,
    outdir: String,
    confdir: String,
    quiet: bool,
}

fn get_options() -> Options {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = getopts::Options::new();
    opts.optopt("o", "out", "output directory", "PATH");
    opts.optflag("h", "help", "show this help message");
    opts.optopt("c", "config", "configuration directory", "PATH");
    opts.optflag("q", "quiet", "do not show progress");
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
        std::process::exit(0);  // Maybe return instead?
    }

    // Directories for rendering.
    let outdir = matches.opt_str_or("out", "_public");
    let indir = if matches.free.len() >= 1 {
        matches.free[0].clone()
    } else {
        ".".to_string()
    };
    let confdir = matches.opt_str_or("config", "_knot");

    // Quiet flag.
    let quiet = matches.opt_present("quiet");

    Options {
        indir: indir,
        outdir: outdir,
        confdir: confdir,
        quiet: quiet,
    }
}

fn main() {
    // Parse command-line options.
    let opts = get_options();

    // Configuration.
    // TODO do something sensible when config is missing.
    let config = load_config(opts).unwrap();

    if let Err(err) = render_notes(&config) {
        println!("rendering failed: {}", err);
    }
}
