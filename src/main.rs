use clap::{load_yaml, App};
use std::str::FromStr;

use std::cell::RefCell;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use lol_html::html_content::ContentType;
use lol_html::{element, text, HtmlRewriter, Settings};

const BOILERPLATE: &'static str = r#"<style amp-boilerplate>body{-webkit-animation:-amp-start 8s steps(1,end) 0s 1 normal both;-moz-animation:-amp-start 8s steps(1,end) 0s 1 normal both;-ms-animation:-amp-start 8s steps(1,end) 0s 1 normal both;animation:-amp-start 8s steps(1,end) 0s 1 normal both}@-webkit-keyframes -amp-start{from{visibility:hidden}to{visibility:visible}}@-moz-keyframes -amp-start{from{visibility:hidden}to{visibility:visible}}@-ms-keyframes -amp-start{from{visibility:hidden}to{visibility:visible}}@-o-keyframes -amp-start{from{visibility:hidden}to{visibility:visible}}@keyframes -amp-start{from{visibility:hidden}to{visibility:visible}}</style><noscript><style amp-boilerplate>body{-webkit-animation:none;-moz-animation:none;-ms-animation:none;animation:none}</style></noscript><script async src="https://cdn.ampproject.org/v0.js"></script>"#;

type AtomicPath = Vec<Rc<String>>;

enum Message {
    OnPath(AtomicPath),
}

fn is_valid_font_url(url: &str) -> bool {
    url.starts_with("https://fast.fonts.net")
        || url.starts_with("https://fonts.googleapis.com")
        || url.starts_with("https://maxcdn.bootstrapcdn.com")
        || url.starts_with("https://use.typekit.net/")
}

fn fixup_html(
    input: &str,
    canonical: &str,
    url_base: &str,
    path_base: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut output = vec![];

    let styles: RefCell<String> = Default::default();

    {
        let mut rewriter = HtmlRewriter::try_new(
            Settings {
                element_content_handlers: vec![
                    element!("html", |el| {
                        el.set_attribute("amp", "")?;
                        Ok(())
                    }),
                    element!("script", |el| {
                        el.remove();
                        Ok(())
                    }),
                    element!(r#"link[rel=canonical]"#, |el| {
                        el.remove();
                        Ok(())
                    }),
                    element!("img", |el| {
                        el.set_tag_name("amp-img")?;
                        el.set_attribute("layout", "fill")?;
                        Ok(())
                    }),
                    element!(r#"meta[name="viewport"]"#, |el| {
                        el.remove();
                        Ok(())
                    }),
                    element!(r#"meta[name="viewport"]"#, |el| {
                        el.remove();
                        Ok(())
                    }),
                    text!("style", |t| {
                        styles.borrow_mut().push_str(t.as_str());
                        t.remove();
                        Ok(())
                    }),
                    element!(r#"link[rel="stylesheet"]"#, |el| {
                        let target = el
                            .get_attribute("href")
                            .expect("link did not have href attribute");

                        // Gross
                        if target.starts_with(url_base) {
                            let path = PathBuf::from_str(&target)?;
                            let path = path.strip_prefix(url_base)?;

                            let mut file_path = PathBuf::from_str(path_base)?;
                            file_path.push(path);

                            let style_contents = fs::read_to_string(file_path)?;
                            styles.borrow_mut().push_str(&style_contents);

                            el.remove();
                        } else if is_valid_font_url(&target) {
                            // pass
                        } else {
                            el.remove();
                        }

                        Ok(())
                    }),
                    element!(r#"meta[charset]"#, |el| {
                        el.remove();
                        Ok(())
                    }),
                    element!("head", |el| {
                        el.append(BOILERPLATE, ContentType::Html);
                        el.append(r#"<meta charset="utf-8">"#, ContentType::Html);
                        el.append(
                            r#"<meta name="viewport" content="width=device-width">"#,
                            ContentType::Html,
                        );
                        el.append(
                            &format!(
                                r#"<link rel="canonical" href="{}" charset="utf-8">"#,
                                canonical
                            ),
                            ContentType::Html,
                        );
                        el.append("<style amp-custom></style>", ContentType::Html);
                        Ok(())
                    }),
                ],
                ..Settings::default()
            },
            |c: &[u8]| output.extend_from_slice(c),
        )?;

        rewriter.write(input.as_bytes())?;
        rewriter.end()?;
    }

    let mut new_output = vec![];

    let mut rewriter = HtmlRewriter::try_new(
        Settings {
            element_content_handlers: vec![element!("style[amp-custom]", |el| {
                el.append(&styles.borrow(), ContentType::Html);
                Ok(())
            })],
            ..Settings::default()
        },
        |c: &[u8]| new_output.extend_from_slice(c),
    )?;

    rewriter.write(&output)?;
    rewriter.end()?;

    Ok(String::from_utf8(new_output)?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    let output = matches.value_of("output").unwrap();
    let output = Rc::new(OsString::from(output));

    let input = matches.value_of("input").unwrap();
    let input = Rc::new(OsString::from(input));

    let base = matches.value_of("base").unwrap();

    if let Ok(meta) = fs::metadata(output.as_os_str()) {
        if meta.is_dir() {
            fs::remove_dir_all(PathBuf::from(output.as_os_str()))?;
        } else {
            panic!("Output is not directory!");
        }
    }

    let mut messages = vec![Message::OnPath(vec![])];

    let mut page_count: usize = 0;
    let mut existing_count: usize = 0;

    while let Some(message) = messages.pop() {
        match message {
            Message::OnPath(path) => {
                let input = Rc::clone(&input);
                let output = Rc::clone(&output);
                let path_buf =
                    path.iter()
                        .fold(PathBuf::from(input.as_os_str()), |mut buf, segment| {
                            buf.push(&segment.as_str());
                            buf
                        });

                let output_path_buf =
                    path.iter()
                        .fold(PathBuf::from(output.as_os_str()), |mut buf, segment| {
                            buf.push(&segment.as_str());
                            buf
                        });

                let meta = fs::metadata(&path_buf).unwrap();

                if meta.is_dir() {
                    existing_count += 1;
                    fs::create_dir(&output_path_buf).unwrap();

                    let dir = fs::read_dir(&path_buf).unwrap();

                    for entry in dir {
                        let mut new_path = path.clone();
                        new_path.push(Rc::new(String::from(
                            entry
                                .unwrap()
                                .file_name()
                                .to_str()
                                .expect("file contained non-unicode characters"),
                        )));

                        messages.push(Message::OnPath(new_path));
                    }
                }

                if meta.is_file() {
                    if path_buf.extension().map(|x| x == "html").unwrap_or(false) {
                        let mut url: PathBuf = path_buf.strip_prefix(&input.as_os_str())?.into();

                        // All the path stuff below here is disgusting but I'm tired and it works
                        if url.ends_with("index.html") {
                            url.pop();
                        } else {
                            let file: OsString = url.file_stem().unwrap().into();
                            url.pop();
                            url.push(file);
                        }

                        let contents = {
                            let file = fs::read_to_string(&path_buf).unwrap();
                            fixup_html(
                                &file,
                                &format!("{}/{}/", &base, &url.to_str().unwrap()),
                                base,
                                &input.to_str().unwrap(),
                            )
                            .unwrap()
                        };

                        let amp_buf = if output_path_buf
                            .file_name()
                            .map(|x| x == "index.html")
                            .unwrap_or(false)
                        {
                            fs::copy(path_buf, &output_path_buf).unwrap();

                            let mut output_path_buf = output_path_buf.clone();
                            output_path_buf.pop();
                            output_path_buf.push("amp.html");
                            output_path_buf
                        } else {
                            let stem = output_path_buf.file_stem().expect("File had no name");
                            let mut output_path_buf = output_path_buf.clone();
                            output_path_buf.pop();
                            output_path_buf.push(stem);

                            fs::create_dir(&output_path_buf).unwrap();

                            output_path_buf.push("index.html");

                            fs::copy(&path_buf, &output_path_buf).unwrap();

                            output_path_buf.pop();
                            output_path_buf.push("amp.html");
                            output_path_buf
                        };

                        page_count += 1;
                        fs::write(amp_buf, contents).unwrap();
                    } else {
                        existing_count += 1;
                        fs::copy(path_buf, output_path_buf).unwrap();
                    }
                }
            }
        }
    }

    println!("Convertion complete!");
    println!(
        "Created {} amp pages in and copied {} to the {} directory",
        page_count,
        existing_count,
        output.to_str().unwrap()
    );

    Ok(())
}
