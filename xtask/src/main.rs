// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use clap::{Parser, ValueEnum};
use newline_converter::dos2unix;
use progenitor::{GenerationSettings, Generator, TagStyle};
use regex::Regex;
use rustfmt_wrapper::config::Edition;
use semver::{Prerelease, Version};
use similar::{Algorithm, ChangeTag, TextDiff};
use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

#[derive(Parser)]
#[command(name = "xtask")]
#[command(about = "build tasks")]
enum Xtask {
    #[command(about = "bump the global version number")]
    Bump {
        #[clap(long)]
        place: VersionPlace,
    },
    #[command(about = "generate Four Star sdk and cli")]
    Generate {
        #[clap(long)]
        check: bool,
        #[clap(short = 'v', long)]
        verbose: bool,
    },
}

#[derive(Clone, ValueEnum)]
enum VersionPlace {
    Minor,
    Major,
    Patch,
    Pre,
}

fn main() -> Result<(), String> {
    let xtask = Xtask::parse();

    match xtask {
        Xtask::Bump { place } => bump_package_versions(&place),
        Xtask::Generate { check, verbose } => generate(check, verbose),
    }
}

fn bump_package_versions(place: &VersionPlace) -> Result<(), String> {
    let crate_version_pattern = Regex::new(r#"(?m)^version = "(.*)"$"#).unwrap();

    let contents = fs::read_to_string("Cargo.toml").unwrap();
    let version_line = crate_version_pattern.captures(&contents).unwrap();
    let mut version: Version = version_line.get(1).unwrap().as_str().parse().unwrap();
    version = version.up(place);

    let old_version_line = version_line.get(0).unwrap().as_str();
    let new_version_line = format!(r#"version = "{}""#, version);
    let new_contents = contents.replace(old_version_line, &new_version_line);

    fs::write("Cargo.toml", new_contents).unwrap();

    println!("Updated to {}", version);

    Ok(())
}

trait Bump {
    fn up(self, place: &VersionPlace) -> Self;
}

impl Bump for Version {
    fn up(mut self, place: &VersionPlace) -> Self {
        match place {
            VersionPlace::Major => {
                if self.pre == Prerelease::EMPTY {
                    self.major += 1;
                    self.minor = 0;
                    self.patch = 0;
                }
                self.pre = Prerelease::EMPTY;
            }
            VersionPlace::Minor => {
                if self.pre == Prerelease::EMPTY {
                    self.minor += 1;
                    self.patch = 0;
                }
                self.pre = Prerelease::EMPTY;
            }
            VersionPlace::Patch => {
                if self.pre == Prerelease::EMPTY {
                    self.patch += 1;
                }
                self.pre = Prerelease::EMPTY;
            }
            VersionPlace::Pre => match self.pre.as_str().split_once('.') {
                Some((label, number)) => {
                    let num = number.parse::<u64>().unwrap();
                    self.pre = Prerelease::new(&format!("{}.{}", label, num + 1)).unwrap();
                }
                None => panic!("Found unexpected prelease format: {}", self.pre),
            },
        }

        self
    }
}

// TODO flag to --check the generated file that we can use in CI to keep people
// from modifying the generated file by hand or forgetting to update it.
fn generate(check: bool, verbose: bool) -> Result<(), String> {
    let xtask_path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let root_path = xtask_path.parent().unwrap().to_path_buf();
    let mut spec_path = root_path.clone();
    spec_path.push("sprue-api-spec.json");

    let file = File::open(spec_path).unwrap();
    let spec = serde_json::from_reader(file).unwrap();
    let mut generator = Generator::new(
        GenerationSettings::default()
            .with_interface(progenitor::InterfaceStyle::Builder)
            .with_tag(TagStyle::Separate)
            .with_derive("schemars::JsonSchema"),
    );

    let mut result = Ok(());

    // TODO I'd like to generate a hash as well to have a way to check if the
    // spec has changed since the last generation.

    // SDK
    print!("generating sdk ... ");
    std::io::stdout().flush().unwrap();

    let code = generator.generate_tokens(&spec).unwrap().to_string();
    let contents = format_code(code);

    let mut out_path = root_path.clone();
    out_path.push("sprue-sdk");
    out_path.push("src");
    out_path.push("generated");
    out_path.push("sdk.rs");

    if check {
        let checked_in = std::fs::read_to_string(out_path.clone()).unwrap();
        let checked_in = dos2unix(&checked_in);
        if checked_in != contents {
            println!("❌");
            if verbose {
                show_diff(&checked_in, &contents);
            }
            result = Err(format!(
                "{} is out of date relative to spec file",
                out_path.to_string_lossy(),
            ));
        } else {
            println!("👍");
        }
    } else {
        std::fs::write(out_path, contents).unwrap();
        println!("done.");
    }

    // CLI
    print!("generating cli ... ");
    std::io::stdout().flush().unwrap();
    let code = generator.cli(&spec, "sprue_sdk").unwrap().to_string();
    let contents = format_code(format!("{}\n\n{}", "use sprue_sdk::*;", code));

    let mut out_path = root_path;
    out_path.push("sprue-cli");
    out_path.push("src");
    out_path.push("generated");
    out_path.push("cli.rs");

    if check {
        let checked_in = std::fs::read_to_string(out_path.clone()).unwrap();
        let checked_in = dos2unix(&checked_in);
        if checked_in != contents {
            println!("❌");
            if verbose {
                show_diff(&checked_in, &contents);
            }
            result = Err(format!(
                "{} is out of date relative to sprue-api-spec.json",
                out_path.to_string_lossy(),
            ));
        } else {
            println!("👍");
        }
    } else {
        std::fs::write(out_path, contents).unwrap();
        println!("done.");
    }

    result
}

fn show_diff(expected: &str, actual: &str) {
    for hunk in TextDiff::configure()
        .algorithm(Algorithm::Myers)
        .diff_lines(expected, actual)
        .unified_diff()
        .context_radius(5)
        .iter_hunks()
    {
        println!("{}", hunk.header());
        for change in hunk.iter_changes() {
            let marker = match change.tag() {
                ChangeTag::Delete => '-',
                ChangeTag::Insert => '+',
                ChangeTag::Equal => ' ',
            };
            print!("{}{}", marker, change);
            if change.missing_newline() {
                println!();
            }
        }
    }
}

fn format_code(code: String) -> String {
    let contents = format!(
        "// The contents of this file are generated; do not modify them.\n\n{}",
        code,
    );
    let contents = rustfmt_wrapper::rustfmt_config(
        rustfmt_wrapper::config::Config {
            edition: Some(Edition::Edition2024),
            max_width: Some(100),
            format_strings: Some(true),
            normalize_doc_attributes: Some(true),
            wrap_comments: Some(true),
            ..Default::default()
        },
        contents,
    )
    .unwrap();
    let contents = dos2unix(&contents);

    // Add newlines after end-braces at <= two levels of indentation. Rustfmt's
    // `blank_lines_lower_bound` is broken.
    let regex = regex::Regex::new(r#"(})(\n\s{0,8}[^} ])"#).unwrap();
    let contents = regex.replace_all(&contents, "$1\n$2");

    let regex = regex::Regex::new(r#"(\n\s*///)(\S)"#).unwrap();
    regex.replace_all(&contents, "$1 $2").to_string()
}
