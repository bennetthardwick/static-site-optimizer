use clap::{load_yaml, App};

use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

type AtomicPath = Vec<Rc<String>>;

enum Message {
    OnPath(AtomicPath),
}

mod amp;
mod original;

fn minify_html(file: String) -> Result<String, Box<dyn std::error::Error>> {
    let mut minifier = html_minifier::HTMLMinifier::new();
    minifier.digest(&file)?;
    Ok(minifier.get_html())
}

fn pass_html(file: String) -> Result<String, Box<dyn std::error::Error>> {
    Ok(file)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    let output = matches.value_of("output").unwrap();
    let output = Rc::new(OsString::from(output));

    let input = matches.value_of("input").unwrap();
    let input = Rc::new(OsString::from(input));

    let mut base: String = matches.value_of("base").unwrap().into();
    if !base.ends_with("/") {
        base.push('/');
    }

    if let Ok(meta) = fs::metadata(output.as_os_str()) {
        if meta.is_dir() {
            fs::remove_dir_all(PathBuf::from(output.as_os_str()))?;
        } else {
            panic!("Output is not directory!");
        }
    }

    let is_amp = matches.is_present("amp");
    let should_minify = matches.is_present("minify_html");

    let gtag_id = matches.value_of("gtag_id").map(|id| format!(r#"
        <script async custom-element="amp-analytics" src="https://cdn.ampproject.org/v0/amp-analytics-0.1.js"></script>
        <amp-analytics type="gtag" data-credentials="include">
            <script type="application/json">
            {{
              "vars" : {{
                "gtag_id": "{gtag}",
                "config" : {{
                  "{gtag}": {{ "groups": "default" }}
                }}
              }}
            }}
            </script>
        </amp-analytics>
    "#, gtag = id));

    let minify_fn: &dyn Fn(String) -> Result<String, Box<dyn std::error::Error>> = if should_minify
    {
        &minify_html
    } else {
        &pass_html
    };

    let options = original::Options {
        inline_styles: matches.is_present("inline_styles"),
        amp_link: is_amp,
    };

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

                let meta = fs::metadata(&path_buf)?;

                if meta.is_dir() {
                    existing_count += 1;
                    fs::create_dir(&output_path_buf)?;

                    let dir = fs::read_dir(&path_buf)?;

                    for entry in dir {
                        let mut new_path = path.clone();
                        new_path.push(Rc::new(String::from(
                            entry?
                                .file_name()
                                .to_str()
                                .expect("file contained non-unicode characters"),
                        )));

                        messages.push(Message::OnPath(new_path));
                    }
                }

                if meta.is_file() {
                    if path_buf.extension().map(|x| x == "html").unwrap_or(false) {
                        if is_amp && path_buf.file_stem().map(|x| x != "404").unwrap_or(false) {
                            let mut url: PathBuf =
                                path_buf.strip_prefix(&input.as_os_str())?.into();

                            // All the path stuff below here is disgusting but I'm tired and it works
                            if url.ends_with("index.html") {
                                url.pop();
                            } else {
                                let file: OsString = url.file_stem().unwrap().into();
                                url.pop();
                                url.push(file);
                            }

                            let (amp, original) = {
                                let file = fs::read_to_string(&path_buf)?;
                                let mut canonical = format!("{}{}", &base, &url.to_str().unwrap());

                                if !canonical.ends_with("/") {
                                    canonical.push('/');
                                }

                                (
                                    amp::fixup_amp_html(
                                        &file,
                                        &canonical,
                                        &base,
                                        &input.to_str().unwrap(),
                                        &gtag_id,
                                    )?,
                                    original::fixup_original_html(
                                        &file,
                                        &canonical,
                                        &base,
                                        &input.to_str().unwrap(),
                                        &options,
                                    )?,
                                )
                            };

                            let amp_buf = if output_path_buf
                                .file_name()
                                .map(|x| x == "index.html")
                                .unwrap_or(false)
                            {
                                fs::write(&output_path_buf, minify_fn(original)?)?;

                                let mut output_path_buf = output_path_buf.clone();
                                output_path_buf.pop();
                                output_path_buf.push("amp");

                                fs::create_dir(&output_path_buf)?;

                                output_path_buf.push("index.html");
                                output_path_buf
                            } else {
                                let stem = output_path_buf.file_stem().expect("File had no name");
                                let mut output_path_buf = output_path_buf.clone();
                                output_path_buf.pop();
                                output_path_buf.push(stem);

                                fs::create_dir(&output_path_buf)?;

                                output_path_buf.push("index.html");

                                fs::write(&output_path_buf, minify_fn(original)?)?;

                                output_path_buf.pop();
                                output_path_buf.push("amp");

                                fs::create_dir(&output_path_buf)?;

                                output_path_buf.push("index.html");
                                output_path_buf
                            };

                            page_count += 1;
                            fs::write(amp_buf, minify_fn(amp)?)?;
                        } else {
                            let url: PathBuf = path_buf.strip_prefix(&input.as_os_str())?.into();
                            let mut canonical = format!("{}{}", &base, &url.to_str().unwrap());

                            if !canonical.ends_with("/") {
                                canonical.push('/');
                            }

                            let file = fs::read_to_string(&path_buf)?;
                            let file = original::fixup_original_html(
                                &file,
                                &canonical,
                                &base,
                                &input.to_str().unwrap(),
                                &options,
                            )?;
                            fs::write(&output_path_buf, minify_fn(file)?)?;
                        }
                    } else {
                        existing_count += 1;
                        fs::copy(path_buf, output_path_buf)?;
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
