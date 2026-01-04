/*
TODO:
[x] error handling on the latex compilation
[x] latex template
[x] block math
[x] inline code
[x] block code
[x] links
[x] images
[x] comments
[x] raw html
[x] block quotes
[x] basic (inline) footnotes
[x] fix titles
[ ] unordered lists
[ ] ordered lists
[ ] clean up
[ ] compile all
*/
use std::path::{Path, PathBuf};
use std::process::Command;
use regex::Regex;

struct CompilerConfig {
    images_dir: PathBuf,
    post_template: String,
    math_template: String,
}

#[derive(Debug, PartialEq, Clone)]
enum TextFormat {
    Raw,                // first parsing pass, math
    Plain,
    Bold,
    Italic,
    InlineMath,
    InlineCode,
    FootnoteRef,
    Link(String),       // URL
}

#[derive(Debug)]
struct Text {
    src: String,
    fmt: TextFormat,
}

#[derive(Debug)]
enum Block {
    Paragraph(Vec<Text>),
    Header(usize, String),  // level, source
    Code(String, String),        // standalone code block
    Math(String),
    Image(String, String, u32), // alt, url, width percentage 
    Html(String),
    Quote(String),
    Footnote(String, Vec<Text>)   // id, text
}

impl Text {
    fn render(&self, cfg: &CompilerConfig) -> String {
        match self.fmt {
            TextFormat::Plain => {
                String::clone(&self.src)
            }
            TextFormat::Bold => {
                let s = String::clone(&self.src);
                return format!("<span class=\"bold\"> {} </span>", s);
            }
            TextFormat::Italic => {
                let s = String::clone(&self.src);
                return format!("<span class=\"italic\"> {} </span>", s);
            }
            TextFormat::InlineMath => {
                let svg = render_math_to_svg(&self.src, cfg, false).unwrap_or_else(
                    |e| format!("<code class='latex-error'>{}</code>", e)
                );
                format!("<span class=\"inline-math\">{}</span>", svg)
            }
            TextFormat::InlineCode => {
                format!(" <span class=\"inline-code\">{}</span>", &self.src)
            }
            TextFormat::Link(ref url) => {
                format!("<a href=\"{}\">{}</a>", url, &self.src)
            }
            TextFormat::FootnoteRef => {
                format!(
                    "<sup id=\"ref{}\"><a href=\"#fn{}\">[{}]</a></sup>", 
                    &self.src, &self.src, &self.src
                )
            }
            _ => {
                return String::clone(&self.src);
            }
        }
    }
}

fn tagged(s: String, tag: &'static str) -> String {
    format!("<{0}>{1}</{0}>\n", tag, s)
}

impl Block {
    fn render(&self, cfg: &CompilerConfig) -> String {
        match self {
            Block::Paragraph(chunks) => {
                let c = chunks.iter().map(|text| text.render(cfg)).collect::<String>();
                tagged(c, "p")
            },
            Block::Header(level, src) => {
                let tag = if *level == 1 { "h1" } else {"h2"};
                let mut s = tagged(String::clone(src), tag);
                if tag == "h1" {
                    s.push_str("<hr><br>")
                }
                s
            }
            Block::Math(s) => {
                let svg = render_math_to_svg(s, cfg, false).unwrap_or_else(
                    |e| format!("<code class='latex-error'>{}</code>", e)
                );
                format!("<span class=\"display-math\">{}</span>", svg)
            }
            Block::Code(lang, src) => {
                format!("<pre><code class=\"code-{}\">{}</code></pre>", lang, src)
            }
            Block::Image(alt, url, width) => {
                let full_path = cfg.images_dir.join(url);
                let path_str = full_path.to_str().unwrap();
                if *width == 100 {
                    format!("<img src=\"{}\" alt=\"{}\" class=\"image\">", path_str, alt)
                } else {
                    format!("<img src=\"{}\" alt=\"{}\" class=\"image\" style=\"width: {}%;\">", path_str, alt, width)
                }
            }
            Block::Html(src) => {
                src.to_string()
            }
            Block::Quote(src) => {
                format!("<p class=quote>{}</p>\n", src)
            }
            Block::Footnote(id, chunks) => {
                let c = chunks.iter().map(|text| text.render(cfg)).collect::<String>();
                format!(
                    "<p id=\"fn{}\"><a href=\"#ref{}\">[{}]</a> {}</p>",
                    id, id, id, c
                )
            }
        }
    }
}

fn render_document(blocks: Vec<Block>, cfg: &CompilerConfig) -> String {
    blocks.iter().map(|block| block.render(cfg)).collect()
}

fn parse_blocks(input: String) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut lines = input.lines().peekable();
    let mut text_buf = String::new();
    let mut is_new_block = true;

    while let Some(line) = lines.next() {
        if line.trim().is_empty() {
            if !text_buf.is_empty() {
                blocks.push(
                    Block::Paragraph(vec![Text {src: text_buf.clone(), fmt: TextFormat::Raw}])
                );
                text_buf = String::new();
            }
            is_new_block = true;
            continue;
        }

        if !is_new_block {
            text_buf.push_str(line);
            text_buf.push(' ');     // spaces for newlines
            continue;
        }

        // headers
        if line.starts_with("#") {  
            let level = line.chars().take_while(|&c| c == '#').count();
            let text = line[level..].trim().to_string();
            blocks.push(Block::Header(level, text));
        } 

        // code block
        else if line.starts_with("```") {
            let language = line[3..].trim().to_string();
            while let Some(line) = lines.next() {
                if line.starts_with("```") { break; }
                text_buf.push_str(line);
                text_buf.push('\n');
            }
            blocks.push(Block::Code(language, text_buf.clone()));
            text_buf = String::new();
        } 

        // math block
        else if line.starts_with("\\[") {
            while let Some(line) = lines.next() {
                if line.starts_with("\\]") { break; }
                text_buf.push_str(line);
                text_buf.push('\n');
            }
            blocks.push(Block::Math(text_buf.clone()));
            text_buf = String::new();
        } 

        // standalone images
        else if line.starts_with("![") {
            let image_regex = Regex::new(r"!\[([^\]]*)\]\(([^)]+)\)(?:\{(\d+)\})?").unwrap();
            if let Some(caps) = image_regex.captures(line) {
                let alt = caps[1].to_string();
                let url = caps[2].to_string();
                let width = caps.get(3)
                    .map(|m| m.as_str().parse::<u32>().unwrap())
                    .unwrap_or(100);
                blocks.push(Block::Image(alt, url, width));
            }
        }

        // comments
        else if line.starts_with("<!--") {
            while let Some(line) = lines.next() {
                if line.starts_with("-->") {
                    break;
                }
            }
        }

        // raw html
        else if line.starts_with("<html>") {
            let mut buf = String::new();
            while let Some(line) = lines.next() {
                if line.starts_with("</html>") {
                    break;
                }
                buf.push_str(line);
            }
            blocks.push(Block::Html(buf));
        }

        // block quotes (single line for now, '>>' syntax)
        else if line.starts_with(">> ") {
            let quote = line[3..].trim().to_string();
            blocks.push(Block::Quote(quote));
        }

        // footnote defns (single line for now)
        else if line.starts_with("[^") {
            let footnote_regex = Regex::new(r"^\[\^(\d+)\]:\s*(.*)").unwrap();
            if let Some(caps) = footnote_regex.captures(line) {
                let footnote_id = caps[1].to_string();
                let contents = caps[2].to_string();
                blocks.push(
                    Block::Footnote(
                        footnote_id,
                        vec![Text {src: contents, fmt: TextFormat::Raw}])
                );
            }
        }

        // paragraph
        else {
            text_buf.push_str(line);
            text_buf.push(' ');
        }
        is_new_block = false;
    }

    if !text_buf.is_empty() {
        blocks.push(
            Block::Paragraph(vec![Text {src: text_buf.clone(), fmt: TextFormat::Raw}]
        ));
    }
    blocks
}

// also responsible for postprocessing links/footnotes
fn push_fmted_text( s_buf: &mut String, texts: &mut Vec<Text>,
                    fmt_c: &mut TextFormat, fmt_new: TextFormat){
    // println!("{}", s_buf);
    if s_buf.is_empty() { return };
    let link_regex = Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap();
    let footnote_regex = Regex::new(r"\[\^(\d+)\]").unwrap();

    // link check
    if let Some(mat) = link_regex.find(s_buf) && fmt_new == TextFormat::Plain{
        // push stuff before the link if it exists
        if mat.start() > 0 {
            texts.push(Text{
                src: s_buf[..mat.start()].to_string(), 
                fmt: fmt_c.clone()
            });
        }
        // push the link
        let caps = link_regex.captures(&s_buf).unwrap();
        let link_text = caps[1].to_string();
        let url = caps[2].to_string();
        texts.push(Text{
            src: link_text,
            fmt: TextFormat::Link(url)
        });
        // handle remaining links
        if mat.end() < s_buf.len() {
            let mut remaining = s_buf[mat.end()..].to_string();
            push_fmted_text(&mut remaining, texts, fmt_c, fmt_new);
            return;
        }

    // footnote check
    } else if let Some(mat) = footnote_regex.find(s_buf) && fmt_new == TextFormat::Plain {
        // push stuff before footnote
        if mat.start() > 0 {
            texts.push(Text{
                src: s_buf[..mat.start()].to_string(),
                fmt: fmt_c.clone()
              });
          }
          // push the footnote ref
          let caps = footnote_regex.captures(&s_buf).unwrap();
          let footnote_id = caps[1].to_string();
          texts.push(Text{
              src: footnote_id,
              fmt: TextFormat::FootnoteRef
          });
          // handle remaining text
          if mat.end() < s_buf.len() {
              let mut remaining = s_buf[mat.end()..].to_string();
              push_fmted_text(&mut remaining, texts, fmt_c, fmt_new);
              return;
          }
    } else {
        texts.push(Text{src: s_buf.clone(), fmt: fmt_c.clone()});
    }
    *fmt_c = if *fmt_c == fmt_new { TextFormat::Plain } else {fmt_new};
    *s_buf = String::new();
}

fn parse_text(src: String) -> Vec<Text> {
    let chars = src.chars().peekable();
    let mut s_buf = String::new();
    let mut texts = Vec::new();
    let mut escaped = false;
    let mut fmt = TextFormat::Plain;
    let mut in_literal_mode = false;

    for c in chars {
        if escaped {
            s_buf.push(c);
            escaped = false;
            continue;
        } if in_literal_mode { // todo deconflate
            if c == '$' && fmt == TextFormat::InlineMath {
                push_fmted_text(&mut s_buf, &mut texts, &mut fmt, TextFormat::Plain);
                in_literal_mode = false;
            } else if c == '`' && fmt == TextFormat::InlineCode {
                push_fmted_text(&mut s_buf, &mut texts, &mut fmt, TextFormat::Plain);
                in_literal_mode = false;
            } else {
                s_buf.push(c);
            }
            continue;
        }
        match c { 
            '\\' => { escaped = true; }
            '*' => { push_fmted_text(&mut s_buf, &mut texts, &mut fmt, TextFormat::Bold); }
            '_' => { push_fmted_text(&mut s_buf, &mut texts, &mut fmt, TextFormat::Italic); }
            '$' => {
                push_fmted_text(&mut s_buf, &mut texts, &mut fmt, TextFormat::InlineMath);
                in_literal_mode = !in_literal_mode;
            }
            '`' => {
                push_fmted_text(&mut s_buf, &mut texts, &mut fmt, TextFormat::InlineCode);
                in_literal_mode = !in_literal_mode;
            }
            _ => { s_buf.push(c); }
        }
    }
    let f2 = fmt.clone();
    push_fmted_text(&mut s_buf, &mut texts, &mut fmt, f2);
    // texts.push(Text{src: s_buf, fmt: fmt.clone()});
    texts
}


fn parse_inner(block: Block) -> Block {
    match block {
        Block::Paragraph(ts) => {
            // assume its raw in this pass
            if let Some(raw_text) = ts.first() {
                Block::Paragraph(parse_text(raw_text.src.clone()))
            } else {
                Block::Paragraph(ts)
            }
        },
        Block::Footnote(id, ts) => {
            if let Some(raw_text) = ts.first() {
                Block::Footnote(id, parse_text(raw_text.src.clone()))
            } else {
                Block::Footnote(id, ts)
            }
        }
        _ => block 
    }
}

fn parse(input: String) -> Vec<Block> {
    let blocks = parse_blocks(input);
    let content = blocks.into_iter().map(|b| parse_inner(b)).collect();
    content
}


fn compile_post(in_path: &Path,
                out_path: &Path,
                cfg: &CompilerConfig,
) {
    println!("compiling: {} => {}", in_path.display(), out_path.display());

    // read file
    let file = std::fs::read_to_string(in_path).unwrap();

    // parse
    let parsed = parse(file);
    // println!("{:?}", parsed);

    // render contents
    let content = render_document(parsed, cfg);
    let _ = parsed;

    // paste contents into template
    let title = in_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    let post_html = cfg.post_template.clone()
        .replace("{{content}}", &content)
        .replace("{{title}}", title);

    // write output to file
    let _ = std::fs::write(out_path, post_html);
}


fn render_math_to_svg(math: &str, 
    cfg: &CompilerConfig, is_display: bool) -> Result<String, String> {
    let temp_dir = tempfile::tempdir().unwrap();
    let tex_path = temp_dir.path().join("math.tex");
   
    let inner_contents = 
        if is_display { format!("\\[{}\\]", math) } 
        else { format!("${}$", math) };

    let latex_content = cfg.math_template.clone().replace("{{content}}", &inner_contents);
    // println!("{:?}", latex_content);
    std::fs::write(&tex_path, latex_content).unwrap();
    
    let latex_output = Command::new("latex")
        .args(&[
            "-interaction=nonstopmode",
            "-halt-on-error",
            "-output-directory", temp_dir.path().to_str().unwrap(),
            tex_path.to_str().unwrap()
        ])
        .output()
        .unwrap();

    if !latex_output.status.success() {
        let err = String::from_utf8_lossy(&latex_output.stdout);
        println!("\tcompiling TeX expr: {}... ERR:\n{}", math, err);
        return Err(format!("LaTeX failed: {}", err));
    }
    
    println!("\tcompiling TeX expr: {}... OK", math.replace("\n", " "));
    let dvi_path = temp_dir.path().join("math.dvi");
    
    // Debug: check if DVI file exists
    if !dvi_path.exists() {
        return Err(format!("DVI file not found at {:?}", dvi_path));
    }
    
    let svg_output = Command::new("dvisvgm")
        .args(&["--no-fonts", "--exact", "--stdout", dvi_path.to_str().unwrap()])
        .output()
        .unwrap();
    
    // println!("dvisvgm stderr: {}", String::from_utf8_lossy(&svg_output.stderr));
    // println!("dvisvgm status: {}", svg_output.status);
    
    Ok(String::from_utf8_lossy(&svg_output.stdout).to_string())
}


fn main() {

    let images_dir = Path::new("../static/images").to_path_buf();  // TODO: change when nginx
    let post_template_path = Path::new("templates/template.html");
    let math_template_path = Path::new("templates/math.tex");
    let post_template = std::fs::read_to_string(post_template_path).unwrap();
    let math_template = std::fs::read_to_string(math_template_path).unwrap();

    let cfg = CompilerConfig {
        images_dir,
        post_template,
        math_template
    };

    compile_post(
        Path::new("posts/test_post.md"),
        Path::new("www/test_post.html"),
        &cfg
    );

    // let s = String::from("text [t1](u1) [t2](u2)");
    // let link_regex = Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap();
    // let m = link_regex.find(&s);
    // println!("{:?}", m);
    
    // println!("{:?}", render_math_to_svg("\\mathbb{R}^n", false));
}






