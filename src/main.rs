extern crate pulldown_cmark;

use pulldown_cmark::Parser;
use pulldown_cmark::html;

use std::io;
use std::io::{Read, Write};
use std::path::Path;
use std::fs::File;

fn render_html(text: &str) -> String {
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
    let md = read_file("test.md");
    let html = render_html(&md.unwrap());
    println!("{}", html);
}
