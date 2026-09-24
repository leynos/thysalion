//! Readers for the suite-once contract: shell commands, workflow jobs and
//! steps, and manifest features, read textually or as TOML.
//!
//! Kept apart from `workflow_suite_contract.rs` so each file stays under the
//! repository's 400-line limit. Only that contract uses these readers. The
//! text sits behind small wrapper types (`Command`, `Workflow`, `Job`,
//! `Step`, `Manifest`), so each question is a method on what it reads.

use cap_std::{ambient_authority, fs_utf8::Dir};

/// Cargo options that take their value as the next word.
const CARGO_VALUE_OPTIONS: [&str; 6] = [
    "--config",
    "-Z",
    "-C",
    "--manifest-path",
    "--color",
    "--target-dir",
];

/// Make options that take their value as the next word.
const MAKE_VALUE_OPTIONS: [&str; 8] = [
    "-C",
    "-f",
    "-I",
    "-o",
    "-W",
    "--directory",
    "--file",
    "--makefile",
];

/// Cargo subcommands that run the suite.
const SUITE_SUBCOMMANDS: [&str; 3] = ["test", "nextest", "llvm-cov"];

/// Make targets that run the suite: `test`, `all` (which runs it),
/// `coverage` (under `cargo llvm-cov`) and the fast local variants. A bare
/// `make` runs the default goal, which is `all`, so it counts as well.
const SUITE_TARGETS: [&str; 5] = ["test", "all", "coverage", "dev-test", "test-fast"];

/// Opens the crate manifest directory as a capability-scoped handle.
pub(crate) fn manifest_dir() -> std::io::Result<Dir> {
    Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
}

/// Returns every workflow file's name and text.
pub(crate) fn workflows() -> std::io::Result<Vec<(String, String)>> {
    let dir = manifest_dir()?.open_dir(".github/workflows")?;
    let mut found = Vec::new();
    for entry in dir.entries()? {
        let name = entry?.file_name()?;
        let is_workflow = name.rsplit_once('.').is_some_and(|(_, extension)| {
            extension.eq_ignore_ascii_case("yml") || extension.eq_ignore_ascii_case("yaml")
        });
        if is_workflow {
            let text = dir.read_to_string(&name)?;
            found.push((name, text));
        }
    }
    Ok(found)
}

/// One shell command line from a workflow.
#[derive(Clone, Copy)]
pub(crate) struct Command<'a>(&'a str);

/// The words of one shell segment: a command between separators.
struct Segment<'a>(Vec<&'a str>);

impl<'a> Command<'a> {
    /// Reads a workflow line as a command: without a `run:` prefix, whether
    /// or not it opens a step with `- `, and empty for a comment. Every line is read, so a suite
    /// run inside a multi-line `run: |` block is seen as well as a single-line one.
    pub(crate) fn from_line(line: &'a str) -> Self {
        let trimmed = line.trim();
        let item = trimmed.strip_prefix("- ").map_or(trimmed, str::trim_start);
        let command = if trimmed.starts_with('#') {
            ""
        } else {
            item.strip_prefix("run:").map_or(trimmed, str::trim)
        };
        Self(command)
    }

    /// Returns the command's text.
    pub(crate) const fn text(self) -> &'a str { self.0 }

    /// Returns `true` if any segment of the command runs the suite.
    pub(crate) fn runs_suite(self) -> bool { self.segments().iter().any(Segment::runs_suite) }

    /// Splits the command at `;`, `|`, `&` and newlines, spaced or not, so
    /// a suite run after `&&`, `||`, `;` or `|` is read as its own command.
    fn segments(self) -> Vec<Segment<'a>> {
        self.0
            .split([';', '|', '&', '\n'])
            .map(|part| {
                Segment(
                    part.split_whitespace()
                        .map(|word| word.trim_matches(|c| c == '"' || c == '\''))
                        .filter(|word| !word.is_empty())
                        .collect(),
                )
            })
            .collect()
    }
}

impl Segment<'_> {
    /// Returns the program the segment runs and its arguments, skipping
    /// leading variable assignments.
    fn invocation(&self) -> Option<(&str, &[&str])> {
        let start = self.0.iter().position(|word| !word.contains('='))?;
        let (program, arguments) = self.0.get(start..)?.split_first()?;
        let name = program.rsplit('/').next().unwrap_or(program);
        Some((name, arguments))
    }

    /// Returns `true` if the segment runs the suite.
    fn runs_suite(&self) -> bool {
        match self.invocation() {
            Some(("cargo", arguments)) => Self::operands(arguments, &CARGO_VALUE_OPTIONS)
                .first()
                .is_some_and(|subcommand| SUITE_SUBCOMMANDS.contains(subcommand)),
            Some(("make", arguments)) => {
                let targets = Self::operands(arguments, &MAKE_VALUE_OPTIONS);
                targets.is_empty() || targets.iter().any(|target| SUITE_TARGETS.contains(target))
            }
            _ => false,
        }
    }

    /// Returns a command line's operands: its words less options, their
    /// values, toolchain selectors and variable assignments.
    fn operands<'w>(arguments: &[&'w str], value_options: &[&str]) -> Vec<&'w str> {
        let mut found = Vec::new();
        let mut words = arguments.iter();
        while let Some(word) = words.next() {
            let is_flag = word.starts_with('-') || word.starts_with('+');
            if value_options.contains(word) {
                words.next();
            } else if !is_flag && !word.contains('=') {
                found.push(*word);
            }
        }
        found
    }
}

/// Returns a line's indentation width.
fn indent(line: &str) -> usize { line.len() - line.trim_start().len() }

/// Returns `true` for a line that opens a YAML list item.
fn is_item(line: &str) -> bool { line.trim_start().starts_with("- ") }

/// Returns `true` for a line that carries no YAML content.
fn is_blank_or_comment(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#')
}

/// A workflow document's text.
#[derive(Clone, Copy)]
pub(crate) struct Workflow<'a>(pub(crate) &'a str);

/// One job of a workflow: its name and the lines under it.
pub(crate) struct Job<'a> {
    /// The job's key under `jobs:`.
    pub(crate) name: &'a str,
    /// The lines between this job's key and the next one.
    lines: Vec<&'a str>,
}

/// One step of a job, as its lines.
pub(crate) struct Step<'a>(Vec<&'a str>);

impl<'a> Workflow<'a> {
    /// Returns each job, in order. A job key is any line indented one level
    /// under `jobs:`, whatever that level's width.
    pub(crate) fn jobs(self) -> Vec<Job<'a>> {
        let section: Vec<&'a str> = self
            .0
            .lines()
            .skip_while(|line| line.trim_end() != "jobs:")
            .skip(1)
            .filter(|line| !is_blank_or_comment(line))
            .take_while(|line| indent(line) > 0)
            .collect();
        let level = section.first().map_or(0, |line| indent(line));
        let mut found: Vec<Job<'a>> = Vec::new();
        for line in section {
            match line.trim().strip_suffix(':') {
                Some(name) if indent(line) == level => found.push(Job {
                    name,
                    lines: Vec::new(),
                }),
                _ => found
                    .last_mut()
                    .into_iter()
                    .for_each(|job| job.lines.push(line)),
            }
        }
        found
    }
}

impl<'a> Job<'a> {
    /// Returns `true` if the job carries its own `if:` condition, read at
    /// the job's own key indentation, whatever its width.
    pub(crate) fn is_conditional(&self) -> bool {
        let level = self.lines.iter().map(|line| indent(line)).min();
        self.lines
            .iter()
            .any(|line| Some(indent(line)) == level && line.trim_start().starts_with("if:"))
    }

    /// Returns the job's lines as commands.
    pub(crate) fn commands(&self) -> impl Iterator<Item = Command<'a>> + '_ {
        self.lines.iter().map(|line| Command::from_line(line))
    }

    /// Splits the job into steps, each a run of lines starting at a `- `
    /// item at the step list's own indentation, so a step can be found by
    /// what it does, not its name, and a nested list inside a step does not
    /// split it.
    pub(crate) fn steps(&self) -> Vec<Step<'a>> {
        let items = self
            .lines
            .iter()
            .skip_while(|line| line.trim() != "steps:")
            .skip(1);
        let level = items
            .clone()
            .find(|line| is_item(line))
            .map(|line| indent(line));
        let mut found: Vec<Step<'a>> = Vec::new();
        for line in items {
            if is_item(line) && Some(indent(line)) == level {
                found.push(Step(Vec::new()));
            }
            found
                .last_mut()
                .into_iter()
                .for_each(|step| step.0.push(line));
        }
        found
    }
}

impl Step<'_> {
    /// Returns `true` if the step carries an `if:` condition.
    pub(crate) fn is_conditional(&self) -> bool {
        self.0.iter().any(|line| {
            line.trim_start()
                .trim_start_matches("- ")
                .starts_with("if:")
        })
    }

    /// Returns `true` if one of the step's lines runs exactly `command`.
    pub(crate) fn runs(&self, command: Command<'_>) -> bool {
        self.0.iter().any(|line| {
            let trimmed = line.trim_start().trim_start_matches("- ");
            trimmed.starts_with("run:") && Command::from_line(trimmed).text() == command.text()
        })
    }

    /// Returns `true` if one of the step's lines, trimmed, is `expected`.
    pub(crate) fn has_line(&self, expected: &str) -> bool {
        self.0.iter().any(|line| line.trim() == expected)
    }

    /// Returns `true` if the step uses an action whose reference contains
    /// `action`.
    pub(crate) fn uses(&self, action: &str) -> bool {
        self.0
            .iter()
            .any(|line| line.contains("uses:") && line.contains(action))
    }
}

/// A parsed `Cargo.toml`.
pub(crate) struct Manifest(toml::Value);

impl Manifest {
    /// Parses a manifest.
    ///
    /// # Errors
    ///
    /// Returns the parser's error when the text is not TOML.
    pub(crate) fn parse(text: &str) -> Result<Self, toml::de::Error> {
        Ok(Self(toml::from_str(text)?))
    }

    /// Returns the package name, or an empty string for a virtual manifest.
    pub(crate) fn package(&self) -> String {
        self.0
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    }

    /// Returns the manifest's feature names: the `[features]` keys, plus
    /// each optional dependency Cargo turns into an implicit feature. An
    /// optional dependency named anywhere as `dep:<name>` exposes no
    /// implicit feature, as Cargo documents, so it is left out.
    pub(crate) fn features(&self) -> Vec<String> {
        let table = self.0.get("features").and_then(toml::Value::as_table);
        let mut found: Vec<String> = table
            .map(|t| t.keys().cloned().collect())
            .unwrap_or_default();
        let suppressed: Vec<String> = found
            .iter()
            .flat_map(|name| self.feature_values(name))
            .filter_map(|value| value.strip_prefix("dep:").map(str::to_owned))
            .collect();
        found.extend(
            self.optional_dependencies()
                .into_iter()
                .filter(|name| !suppressed.contains(name)),
        );
        found
    }

    /// Returns the values one feature enables.
    fn feature_values(&self, feature: &str) -> Vec<String> {
        self.0
            .get("features")
            .and_then(|features| features.get(feature))
            .and_then(toml::Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(toml::Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns every optional dependency's name, target tables included.
    fn optional_dependencies(&self) -> Vec<String> {
        let tables = ["dependencies", "dev-dependencies", "build-dependencies"];
        let mut dependency_tables: Vec<&toml::Value> =
            tables.iter().filter_map(|name| self.0.get(*name)).collect();
        if let Some(targets) = self.0.get("target").and_then(toml::Value::as_table) {
            dependency_tables.extend(
                targets
                    .values()
                    .flat_map(|platform| tables.iter().filter_map(move |name| platform.get(*name))),
            );
        }
        dependency_tables
            .iter()
            .filter_map(|table| table.as_table())
            .flat_map(|table| table.iter())
            .filter(|(_, spec)| spec.get("optional").and_then(toml::Value::as_bool) == Some(true))
            .map(|(name, _)| name.clone())
            .collect()
    }
}
