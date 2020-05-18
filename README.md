# static-site-optimizer

An optimizer for your static site. Inline css, automatically generate amp pages.

## Usage

After building your site (using [zola](https://www.getzola.org/) for example), point the cli to your build folder and tell it what your site's base url is:

```
cargo run --release -- \
  <static site folder> \
  --base-url https://<site base url> \
  --outdir <output directory> \
  --amp \
  --inline-styles
```

After the command runs upload your output directory to your server.

## Features

Note: this is still very WIP. I'm fixing AMP issues as I see Google complain about them.

- [x] amp boilerplate
- [x] collapse styles
- [x] set canonical
- [x] remove scripts
- [x] amp-img (everything becomes layout="fill", so parents must be relative)
- [ ] iframes
- [ ] audio
- [ ] video
