use lol_html::html_content::ContentType;
use lol_html::{element, HtmlRewriter, Settings};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Default)]
pub(crate) struct Options {
    pub(crate) inline_styles: bool,
    pub(crate) amp_link: bool,
}

pub(crate) fn fixup_original_html(
    input: &str,
    canonical: &str,
    url_base: &str,
    path_base: &str,
    options: &Options,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut element_content_handlers = vec![];

    let mut output = vec![];

    {
        let add_amp_link = element!("head", |el| {
            el.append(
                &format!(r#"<link rel="amphtml" href="{}/amp/">"#, canonical),
                ContentType::Html,
            );

            Ok(())
        });
        let inline_styles = element!(r#"link[rel="stylesheet"]"#, |el| {
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
                el.replace(
                    &format!("<style>{}</style>", &style_contents),
                    ContentType::Html,
                );
            }

            Ok(())
        });

        if options.amp_link {
            element_content_handlers.push(add_amp_link);
        }

        if options.inline_styles {
            element_content_handlers.push(inline_styles);
        }

        let mut rewriter = HtmlRewriter::try_new(
            Settings {
                element_content_handlers,
                ..Settings::default()
            },
            |c: &[u8]| output.extend_from_slice(c),
        )?;

        rewriter.write(&input.as_bytes())?;
        rewriter.end()?;
    }

    Ok(String::from_utf8(output)?)
}
