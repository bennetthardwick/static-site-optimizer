use lol_html::html_content::ContentType;
use lol_html::{element, text, HtmlRewriter, Settings};
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

const BOILERPLATE: &'static str = r#"<style amp-boilerplate>body{-webkit-animation:-amp-start 8s steps(1,end) 0s 1 normal both;-moz-animation:-amp-start 8s steps(1,end) 0s 1 normal both;-ms-animation:-amp-start 8s steps(1,end) 0s 1 normal both;animation:-amp-start 8s steps(1,end) 0s 1 normal both}@-webkit-keyframes -amp-start{from{visibility:hidden}to{visibility:visible}}@-moz-keyframes -amp-start{from{visibility:hidden}to{visibility:visible}}@-ms-keyframes -amp-start{from{visibility:hidden}to{visibility:visible}}@-o-keyframes -amp-start{from{visibility:hidden}to{visibility:visible}}@keyframes -amp-start{from{visibility:hidden}to{visibility:visible}}</style><noscript><style amp-boilerplate>body{-webkit-animation:none;-moz-animation:none;-ms-animation:none;animation:none}</style></noscript><script async src="https://cdn.ampproject.org/v0.js"></script>"#;

fn is_valid_font_url(url: &str) -> bool {
    url.starts_with("https://fast.fonts.net")
        || url.starts_with("https://fonts.googleapis.com")
        || url.starts_with("https://maxcdn.bootstrapcdn.com")
        || url.starts_with("https://use.typekit.net/")
}

pub(crate) fn fixup_amp_html(
    input: &str,
    canonical: &str,
    url_base: &str,
    path_base: &str,
    gtag_snippet: &Option<String>,
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
            element_content_handlers: vec![
                element!("style", |el| {
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
                    el.append(
                        &format!("<style amp-custom>{}</style>", &styles.borrow()),
                        ContentType::Html,
                    );

                    if gtag_snippet.is_some() {
                        el.append(r#"<script async custom-element="amp-analytics" src="https://cdn.ampproject.org/v0/amp-analytics-0.1.js"></script>"#, ContentType::Html);
                    }

                    Ok(())
                }),
                element!("body", |el| {
                    if let Some(snippet) = gtag_snippet {
                        el.append(&snippet, ContentType::Html);
                    }

                    Ok(())
                }),
            ],
            ..Settings::default()
        },
        |c: &[u8]| new_output.extend_from_slice(c),
    )?;

    rewriter.write(&output)?;
    rewriter.end()?;

    Ok(String::from_utf8(new_output)?)
}
