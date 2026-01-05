use std::path::{Path, PathBuf};
use std::process::Command;
use regex::Regex;

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
struct ListItem {
    // marker: String,
    level: usize,
    content: Vec<Text>,
}

#[derive(Debug)]
enum Block {
    Paragraph(Vec<Text>),
    Header(usize, String),       // level, source
    Code(String, String),        // standalone code block
    Math(String),
    Image(String, String, u32),  // alt, url, width percentage 
    Html(String),
    Quote(String),
    Footnote(String, Vec<Text>), // id, text
    List(bool, Vec<ListItem>),   // true = ordered, false = unordered
}

struct CompilerConfig {
    posts_dir: PathBuf,
    images_dir: PathBuf,
    output_dir: PathBuf,
    post_template: String,
    math_template: String,
}


fn main() {
    let args: Vec<String> = std::env::args().collect();
   
    let posts_dir = Path::new("posts/").to_path_buf();  // markdown src
    let images_dir = Path::new("/static/images").to_path_buf();
    let output_dir = Path::new("www/posts").to_path_buf();
    let post_template_path = Path::new("templates/template.html");
    let math_template_path = Path::new("templates/math.tex");
    let post_template = std::fs::read_to_string(post_template_path).unwrap();
    let math_template = std::fs::read_to_string(math_template_path).unwrap();

    let cfg = CompilerConfig {
        posts_dir,
        images_dir,
        output_dir,
        post_template,
        math_template
    };

    if args.len() > 1 {
        // Compile specific file
        let input_path = Path::new(&args[1]);
        let output_path = cfg.output_dir
            .join(input_path.file_stem().unwrap())
            .with_extension("html");
        compile_post(input_path, &output_path, &cfg);
    } else {
        // Compile all
        println!("compiling all posts...");
        compile_all(&cfg);
    }
}
/* ========================================
                  compiling 
   ======================================== */
fn compile_all(cfg: &CompilerConfig) {
    let entries = std::fs::read_dir(&cfg.posts_dir).unwrap();
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) == Some("md") {
            let output_path = cfg.output_dir.join(path.file_stem().unwrap()).with_extension("html");
            compile_post(&path, &output_path, &cfg);
        }

    }
}


fn compile_post(in_path: &Path,
                out_path: &Path,
                cfg: &CompilerConfig,
) {
    println!("compiling: {} => {}", in_path.display(), out_path.display());

    // read file
    if let Ok(file) = std::fs::read_to_string(in_path){
        // parse
        let parsed = parse(file);

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
    } else {
        println!("error; invalid file path: {}", in_path.display());
    }
}


/* ========================================
                   parsing 
   ======================================== */
fn parse(input: String) -> Vec<Block> {
    // parse blocks
    let blocks = parse_blocks(input);

    // postprocess text elements where needed
    let content = blocks.into_iter().map(|b| parse_inner(b)).collect();
    content
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

        // ordered lists 
        else if let Some(li0) = captures_ol_li(line) {
            let mut items = vec![li0];
            while let Some(line) = lines.next() {
                if let Some(item) = captures_ol_li(line) {
                    items.push(item);
                } else if let Some(item) = captures_ul_li(line) {
                    items.push(item);
                } else {
                    break;
                }
            }
            blocks.push(
                Block::List(true, items)
            );
        }

        // unordered lists
        else if let Some(li0) = captures_ul_li(line) {
            let mut items = vec![li0];
            while let Some(line) = lines.next() {
                if let Some(item) = captures_ul_li(line) {
                    items.push(item);
                } else if let Some(item) = captures_ol_li(line) {
                    items.push(item);
                } else {
                    break;
                }
            }
            blocks.push(
                Block::List(false, items)
            );
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

fn captures_ol_li(line: &str) -> Option<ListItem> {
    let r = Regex::new(r"^( *)([^\s.]+)\.\s+(.*)").unwrap();
    if let Some(caps) = r.captures(line) {
        let level = caps[1].len() / 4;  // spaces divided by 4
        let content = parse_text(caps[3].to_string());
        Some(ListItem{level, content})
    } else {
        None
    }
}

fn captures_ul_li(line: &str) -> Option<ListItem> {
    let r = Regex::new(r"^( *)[-*]\s+(.*)").unwrap();
    if let Some(caps) = r.captures(line) {
        let level = caps[1].len() / 4;  // spaces divided by 4
        let content = parse_text(caps[2].to_string());
        Some(ListItem{level, content})
    } else {
        None
    }
}

// some blocks need postprocessing
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

// also responsible for postprocessing links/footnotes
fn push_fmted_text( s_buf: &mut String, texts: &mut Vec<Text>,
                    fmt_c: &mut TextFormat, fmt_new: TextFormat){
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


/* ========================================
                    rendering
   ======================================== */
fn render_document(blocks: Vec<Block>, cfg: &CompilerConfig) -> String {
    blocks.iter().map(|block| block.render(cfg)).collect()
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

impl Block {
    fn render(&self, cfg: &CompilerConfig) -> String {
        match self {
            Block::Paragraph(chunks) => {
                let c = chunks.iter().map(|text| text.render(cfg)).collect::<String>();
                format!("<p>{}</p>\n", c)
            },
            Block::Header(level, src) => {
                let tag = if *level == 1 { "h1" } else {"h2"};
                let mut s = format!("<{}>{}</{}>\n", tag, src, tag);
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
            Block::List(is_ordered, list) => {
                let mut s = String::new();
                let mut current_level = 0;
                let tag = if *is_ordered { "ol" } else { "ul" };
                
                // Start first list
                s.push_str(&format!("<{}>", tag));
                
                for (i, item) in list.iter().enumerate() {
                    let inner_text = item.content.iter().map(|t| t.render(cfg)).collect::<String>();
                    
                    // Handle level changes
                    if item.level > current_level {
                        // Open nested lists (don't close the previous <li> yet)
                        for _ in current_level..item.level {
                            s.push_str(&format!("<{}>", tag));
                        }
                    } else if item.level < current_level {
                        // Close nested lists and previous <li>
                        s.push_str("</li>");
                        for _ in item.level..current_level {
                            s.push_str(&format!("</{}>", tag));
                            s.push_str("</li>");
                        }
                    } else if i > 0 {
                        // Same level, close previous <li>
                        s.push_str("</li>");
                    }
                    
                    // Add current item
                    s.push_str(&format!("<li>{}", inner_text));
                    current_level = item.level;
                }
                
                // Close remaining tags
                s.push_str("</li>");
                for _ in 0..current_level {
                    s.push_str(&format!("</{}>", tag));
                    s.push_str("</li>");
                }
                s.push_str(&format!("</{}>", tag));
                
                s
            }
        }
    }
}

fn render_math_to_svg(math: &str, 
    cfg: &CompilerConfig, is_display: bool) -> Result<String, String> {
    let temp_dir = tempfile::tempdir().unwrap();
    let tex_path = temp_dir.path().join("math.tex");
   
    let inner_contents = 
        if is_display { format!("\\[{}\\]", math) } 
        else { format!("${}$", math) };

    let latex_content = cfg.math_template.clone().replace("{{content}}", &inner_contents);
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
    
    if !dvi_path.exists() {
        return Err(format!("DVI file not found at {:?}", dvi_path));
    }
    
    let svg_output = Command::new("dvisvgm")
        .args(&["--no-fonts", "--exact", "--stdout", dvi_path.to_str().unwrap()])
        .output()
        .unwrap();
    
    Ok(String::from_utf8_lossy(&svg_output.stdout).to_string())
}


