//! Commands the user runs around an encode.
//!
//! Two things happen either side of every encode: something before it — mount the
//! source drive, copy a licence, check that the source really is there — and
//! something after it — copy the result somewhere, notify a media server, eject
//! the drive. Both are the user's own programs, run on the user's machine, which
//! is exactly why the rule here is that they are never handed to a shell.
//!
//! # No shell, ever
//!
//! The command is split into a program and an argument *vector* before anything
//! runs, and the program is started directly. Nothing is concatenated into a
//! command line, so a title called `Episode 1 & del important.mkv` is a file name
//! and not a second command, and neither can a path from the appcast nor a folder
//! name from a source drive become an instruction. Quoting is honoured only to
//! say where one argument ends and the next begins — that is tokenising, not
//! interpreting: `;`, `&`, `|`, `>`, and `%VAR%` are all literal characters here,
//! because the only thing being substituted is Vise's own `%output%` family.
//!
//! A command that times out is killed rather than left running, and a command that
//! fails never stops the encode or corrupts the queue: the failure is reported in
//! the activity log with the exit code, and the encode goes on. A pre-action that
//! fails *is* the one case where continuing is a decision the user has not made,
//! so it is offered as a choice rather than assumed either way.

use std::fmt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use vise_core::JobSpec;

/// The variables a command may use.
///
/// Named after what the user is looking at rather than after the job model, and
/// spelled in the `%name%` form that every platform's own configuration uses, so
/// that a command copied out of somebody else's post is a command this accepts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Variables {
    /// The file being written.
    pub output: String,
    /// The file being read.
    pub input: String,
    /// The title's name.
    pub title: String,
    /// The job's number in the queue.
    pub job_id: u64,
    /// The output container, lower case.
    pub container: String,
    /// The folder the output goes in.
    pub folder: String,
}

impl Variables {
    /// The variables for one job.
    pub fn of(spec: &JobSpec, title: &str, job_id: u64) -> Variables {
        let output = Path::new(&spec.output_path);
        Variables {
            output: spec.output_path.clone(),
            input: spec.input_path.clone(),
            title: title.to_string(),
            job_id,
            container: format!("{:?}", spec.container).to_ascii_lowercase(),
            folder: output
                .parent()
                .map(|parent| parent.to_string_lossy().to_string())
                .unwrap_or_default(),
        }
    }

    /// The value of one variable, if it is one of ours.
    ///
    /// The whole table is here rather than being searched for, because a command
    /// that mentions `%PATH%` must be left alone: Vise substitutes the variables
    /// it has and passes every other `%...%` through untouched.
    pub fn value_of(&self, name: &str) -> Option<String> {
        match name.trim().trim_matches('%').to_ascii_lowercase().as_str() {
            "output" => Some(self.output.clone()),
            "input" => Some(self.input.clone()),
            "title" => Some(self.title.clone()),
            "job_id" | "jobid" => Some(self.job_id.to_string()),
            "container" => Some(self.container.clone()),
            "folder" => Some(self.folder.clone()),
            _ => None,
        }
    }

    /// Every name this build substitutes.
    #[allow(dead_code, reason = "the list exists so the tests can walk every name")]
    pub const NAMES: [&'static str; 6] =
        ["output", "input", "title", "job_id", "container", "folder"];
}

/// Substitute Vise's own variables into one argument.
///
/// An unknown `%name%` is left exactly as written rather than replaced with
/// nothing: a command that uses `%PATH%` should still see `%PATH%`, and one that
/// misspells `%outpt%` should get a path that does not exist rather than a string
/// the shell would have expanded.
pub fn substitute(argument: &str, variables: &Variables) -> String {
    let mut out = String::with_capacity(argument.len());
    let mut rest = argument;
    while let Some(start) = rest.find('%') {
        let (before, tail) = rest.split_at(start);
        out.push_str(before);
        let Some(end) = tail[1..].find('%') else {
            // An unmatched `%` is a literal character, as it would be in a path.
            out.push('%');
            out.push_str(&tail[1..]);
            return out;
        };
        let name = &tail[1..1 + end];
        match variables.value_of(name) {
            Some(value) => out.push_str(&value),
            None => {
                out.push('%');
                out.push_str(name);
                out.push('%');
            }
        }
        rest = &tail[end + 2..];
    }
    out.push_str(rest);
    out
}

/// Split a command into a program and its arguments.
///
/// This is tokenising, not interpreting. Double quotes group, a backslash before a
/// quote escapes it, and every other character — including `;`, `&`, `|`, `>`, `<`,
/// `^`, and `%` — is an ordinary character in an ordinary word. An empty result is
/// a refusal rather than an empty command, because a command that is nothing but
/// a comment is a command the user will not notice was ignored.
pub fn tokenize(command: &str) -> Result<Vec<String>, String> {
    let mut arguments: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quoted = false;
    let mut characters = command.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '"' => {
                // An empty pair of quotes is an empty argument, which a caller may
                // have meant; the `started` flag is what remembers that.
                quoted = !quoted;
                started = true;
            }
            '\\' if characters.peek() == Some(&'"') => {
                // The one escape that exists: a literal quote inside an argument.
                characters.next();
                current.push('"');
                started = true;
            }
            character if character.is_whitespace() && !quoted => {
                if started {
                    arguments.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            character => {
                current.push(character);
                started = true;
            }
        }
    }
    if quoted {
        return Err("the command has an unclosed quote".to_string());
    }
    if started {
        arguments.push(current);
    }
    if arguments.is_empty() {
        return Err("the command is empty".to_string());
    }
    Ok(arguments)
}

/// A command, resolved and ready to be run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Invocation {
    /// The program to run.
    pub program: String,
    /// Its arguments, in order, with the variables already put in.
    pub args: Vec<String>,
}

impl Invocation {
    /// The command as a person would read it, for the log.
    ///
    /// Re-quoted so the log says what ran rather than what was typed: an argument
    /// containing a space is shown in quotes, and that is also why this string is
    /// never run.
    pub fn display(&self) -> String {
        // The program is quoted as well as its arguments: the log has to read as
        // the command that ran, and a program path nearly always holds a space.
        let mut text = quote(&self.program);
        for argument in &self.args {
            text.push(' ');
            text.push_str(&quote(argument));
        }
        text
    }
}

/// One argument, shown the way a person would write it in the log.
///
/// Never run: this only ever describes what was started, and the string it
/// produces is never handed to anything that could interpret it.
fn quote(argument: &str) -> String {
    if argument.is_empty() || argument.contains(char::is_whitespace) {
        format!("\"{}\"", argument.replace('"', "\\\""))
    } else {
        argument.to_string()
    }
}

/// Resolve `command` against `variables`.
///
/// The program is substituted too, not only the arguments: a command whose program
/// is `%folder%\\notify.exe` is a legitimate thing to want, and leaving the program
/// unsubstituted would make that one silently fail while every other line worked.
pub fn invocation(command: &str, variables: &Variables) -> Result<Invocation, String> {
    let tokens = tokenize(command)?;
    let mut substituted = tokens
        .into_iter()
        .map(|token| substitute(&token, variables));
    let program = substituted.next().unwrap_or_default();
    if program.trim().is_empty() {
        return Err("the command has no program to run".to_string());
    }
    Ok(Invocation {
        program,
        args: substituted.collect(),
    })
}

/// How long a command may run before it is killed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeout(pub u32);

impl Timeout {
    /// A timeout, refusing one that is not a number of seconds at all.
    pub fn seconds(value: u32) -> Timeout {
        Timeout(value.clamp(1, 3_600))
    }

    /// The timeout as a duration.
    pub fn duration(self) -> Duration {
        Duration::from_secs(u64::from(self.0))
    }
}

impl Default for Timeout {
    fn default() -> Self {
        // Five minutes: long enough for a copy of a disc image, short enough that a
        // program which has asked for a console nobody will give it cannot hold the
        // queue for the rest of the session.
        Timeout(300)
    }
}

/// What running a command produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The program ran and finished.
    Finished {
        /// Its exit code, which is zero for a program that means it worked.
        code: i32,
    },
    /// The program was killed for taking longer than the timeout.
    TimedOut {
        /// The timeout it was given.
        seconds: u32,
    },
    /// The program could not be started at all.
    CouldNotStart {
        /// What the platform said.
        reason: String,
    },
}

impl Outcome {
    /// Whether the command can be counted as having worked.
    pub fn succeeded(&self) -> bool {
        matches!(self, Outcome::Finished { code: 0 })
    }

    /// One line saying what happened, for the activity log.
    pub fn describe(&self, invocation: &Invocation) -> String {
        match self {
            Outcome::Finished { code: 0 } => {
                format!("action ran: {}", invocation.display())
            }
            Outcome::Finished { code } => {
                format!(
                    "action finished with exit code {code}: {}",
                    invocation.display()
                )
            }
            Outcome::TimedOut { seconds } => format!(
                "action was stopped after {seconds} second(s) and did not finish: {}",
                invocation.display()
            ),
            Outcome::CouldNotStart { reason } => format!(
                "action could not be started ({reason}): {}",
                invocation.display()
            ),
        }
    }
}

/// Run `invocation` directly, with a timeout.
///
/// The program is started with no shell between it and the operating system, and
/// with its standard streams detached: a command that waited for input would
/// otherwise wait forever on a window nobody is watching, and one that wrote to
/// the window's output would write into a console that does not exist.
pub fn run(invocation: &Invocation, timeout: Timeout) -> Outcome {
    let mut child = match Command::new(&invocation.program)
        .args(&invocation.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            return Outcome::CouldNotStart {
                reason: e.to_string(),
            }
        }
    };
    let deadline = Instant::now() + timeout.duration();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return Outcome::Finished {
                    code: status.code().unwrap_or(-1),
                }
            }
            Ok(None) => {}
            Err(e) => {
                return Outcome::CouldNotStart {
                    reason: e.to_string(),
                };
            }
        }
        if Instant::now() >= deadline {
            // A command that ignores a polite request to stop is killed: the queue
            // is not waiting on a program the user has forgotten about.
            let _ = child.kill();
            let _ = child.wait();
            return Outcome::TimedOut { seconds: timeout.0 };
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// What a failed action does to the encode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnFailure {
    /// Carry on and say what happened.
    Continue,
    /// Refuse to start the encode.
    StopEncode,
}

impl OnFailure {
    /// Stable identifier the preferences record.
    pub fn id(self) -> &'static str {
        match self {
            OnFailure::Continue => "continue",
            OnFailure::StopEncode => "stop_encode",
        }
    }

    /// Read one the preferences record, or the safer answer.
    pub fn read(level: i32) -> OnFailure {
        match level {
            i32::MIN..=-1 => OnFailure::StopEncode,
            _ => OnFailure::Continue,
        }
    }
}

/// What an action's outcome means for the encode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The encode may go ahead.
    Proceed {
        /// Lines the activity log should record.
        log: Vec<String>,
    },
    /// The encode must not start.
    Refuse {
        /// The line the queue records as its pause reason.
        reason: String,
    },
}

impl Verdict {}

/// Run a pre-encode action and say whether the encode may go ahead.
///
/// A command that works is one line in the log. A command that fails is one line
/// as well, plus the pause reason when the preferences say a failed pre-action
/// stops the encode. A command that cannot even be parsed is always refused: a
/// command the user wrote with a typo in it should say so rather than quietly run
/// something else, and running nothing is not what anybody meant by it.
pub fn run_pre(
    command: &str,
    variables: &Variables,
    timeout: Timeout,
    on_failure: OnFailure,
) -> Result<Verdict, String> {
    if command.trim().is_empty() {
        return Ok(Verdict::Proceed { log: Vec::new() });
    }
    let invocation = invocation(command, variables)?;
    let outcome = run(&invocation, timeout);
    let line = outcome.describe(&invocation);
    if outcome.succeeded() {
        return Ok(Verdict::Proceed { log: vec![line] });
    }
    match on_failure {
        OnFailure::Continue => Ok(Verdict::Proceed { log: vec![line] }),
        OnFailure::StopEncode => Ok(Verdict::Refuse {
            reason: format!("the pre-encode action did not work: {line}"),
        }),
    }
}

/// Run a post-encode action and say what happened.
///
/// A post-action runs after the file exists, so there is nothing left for it to
/// spoil: every outcome is a line in the log and the queue carries on. That is the
/// whole of its failure handling, and it is deliberate — a copy program that fails
/// must not be able to fail an encode that has already been written.
pub fn run_post(
    command: &str,
    variables: &Variables,
    timeout: Timeout,
) -> Result<Vec<String>, String> {
    if command.trim().is_empty() {
        return Ok(Vec::new());
    }
    let invocation = invocation(command, variables)?;
    Ok(vec![run(&invocation, timeout).describe(&invocation)])
}

/// Why an action could not be set.
#[allow(
    dead_code,
    reason = "named by the settings surface in its own error text"
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionError {
    /// The command does not tokenise.
    Malformed(String),
    /// The timeout is not a number of seconds a program could be given.
    BadTimeout(u32),
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActionError::Malformed(reason) => write!(f, "{reason}"),
            ActionError::BadTimeout(seconds) => {
                write!(
                    f,
                    "{seconds} second(s) is not a timeout this build will give a command"
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vise_core::ContainerFormat;

    fn variables() -> Variables {
        Variables::of(
            &JobSpec {
                input_path: "C:\\media\\Season 1\\E01.mkv".to_string(),
                output_path: "D:\\out\\E01.mp4".to_string(),
                container: ContainerFormat::Mp4,
                ..JobSpec::default()
            },
            "Pilot",
            7,
        )
    }

    #[test]
    fn the_variables_are_the_ones_a_user_would_expect() {
        let variables = variables();
        assert_eq!(
            variables.value_of("output").as_deref(),
            Some("D:\\out\\E01.mp4")
        );
        assert_eq!(
            variables.value_of("input").as_deref(),
            Some("C:\\media\\Season 1\\E01.mkv")
        );
        assert_eq!(variables.value_of("title").as_deref(), Some("Pilot"));
        assert_eq!(variables.value_of("job_id").as_deref(), Some("7"));
        assert_eq!(variables.value_of("container").as_deref(), Some("mp4"));
        assert_eq!(variables.value_of("folder").as_deref(), Some("D:\\out"));
        assert_eq!(
            variables.value_of("PATH"),
            None,
            "not ours, so not substituted"
        );
        // Every name this build claims to substitute is one it can substitute.
        for name in Variables::NAMES {
            assert!(
                variables.value_of(name).is_some(),
                "{name} is offered but cannot be substituted"
            );
        }
    }

    #[test]
    fn substitution_replaces_only_the_names_this_build_has() {
        let variables = variables();
        assert_eq!(substitute("%output%", &variables), "D:\\out\\E01.mp4");
        assert_eq!(
            substitute("--in=%input% --out=%output%", &variables),
            "--in=C:\\media\\Season 1\\E01.mkv --out=D:\\out\\E01.mp4"
        );
        // Somebody else's variables pass straight through: this is not a shell,
        // and pretending to expand %PATH% would expand it to nothing.
        assert_eq!(substitute("%PATH%", &variables), "%PATH%");
        assert_eq!(substitute("%outpt%", &variables), "%outpt%");
        assert_eq!(substitute("100%", &variables), "100%");
        assert_eq!(substitute("a%%b", &variables), "a%%b");
        assert_eq!(substitute("%job_id% of %title%", &variables), "7 of Pilot");
    }

    #[test]
    fn a_title_with_shell_characters_in_it_is_only_a_file_name() {
        // This is the whole point of not using a shell: the characters that would
        // chain one command onto another are part of the path.
        let variables = Variables::of(
            &JobSpec {
                input_path: "C:\\media\\x.mkv".to_string(),
                output_path: "D:\\out\\Episode 1 & del important.mkv.mp4".to_string(),
                container: ContainerFormat::Mp4,
                ..JobSpec::default()
            },
            "Pilot",
            1,
        );
        let built = invocation("copy.exe %output% %input%", &variables).expect("built");
        assert_eq!(
            built.args,
            vec![
                "D:\\out\\Episode 1 & del important.mkv.mp4".to_string(),
                "C:\\media\\x.mkv".to_string()
            ]
        );
        // The same string run through a shell would have run `del` on something.
        let joined = built.display();
        assert!(joined.contains("&"), "{joined}");
        assert_eq!(built.program, "copy.exe");
    }

    #[test]
    fn quoting_says_where_one_argument_ends_and_not_what_it_means() {
        let arguments =
            tokenize("notify.exe \"C:\\Program Files\\Vise\\send.exe\" --file %output%")
                .expect("tokenised");
        assert_eq!(
            arguments,
            vec![
                "notify.exe",
                "C:\\Program Files\\Vise\\send.exe",
                "--file",
                "%output%"
            ]
        );
        // An escaped quote is a quote in the argument, not a delimiter.
        let escaped = tokenize("say.exe \"a \\\"b\\\" c\"").expect("tokenised");
        assert_eq!(escaped, vec!["say.exe", "a \"b\" c"]);
        // Every metacharacter a shell would act on is an ordinary character here.
        let literal = tokenize("tool.exe a;rm -rf / & b | c > d").expect("tokenised");
        assert_eq!(
            literal,
            vec!["tool.exe", "a;rm", "-rf", "/", "&", "b", "|", "c", ">", "d"]
        );
        assert_eq!(
            tokenize("copy.exe \"\"").expect("tokenised"),
            vec!["copy.exe".to_string(), String::new()]
        );
    }

    #[test]
    fn a_command_that_does_not_tokenise_is_refused_by_name() {
        assert!(tokenize("").expect_err("empty").contains("empty"));
        assert!(tokenize("   ").expect_err("blank").contains("empty"));
        assert!(tokenize("say.exe \"unclosed")
            .expect_err("quote")
            .contains("unclosed"));
        assert!(invocation("", &variables()).is_err());
        assert!(invocation("\"\" --flag", &variables()).is_err());
    }

    #[test]
    fn the_program_is_substituted_too_and_the_log_shows_what_will_run() {
        let variables = variables();
        let built = invocation("\"%folder%\\notify.exe\" %output%", &variables).expect("built");
        assert_eq!(built.program, "D:\\out\\notify.exe");
        assert_eq!(built.args, vec!["D:\\out\\E01.mp4".to_string()]);
        // Neither of these two strings holds a space, so neither is quoted: the
        // log reads as the command rather than as a list of quoted words.
        assert_eq!(built.display(), "D:\\out\\notify.exe D:\\out\\E01.mp4");
        let spaced = Invocation {
            program: "C:\\Program Files\\Vise\\notify.exe".to_string(),
            args: vec!["C:\\Program Files\\Vise\\E01.mp4".to_string()],
        };
        assert_eq!(
            spaced.display(),
            "\"C:\\Program Files\\Vise\\notify.exe\" \"C:\\Program Files\\Vise\\E01.mp4\"",
            "an argument with a space is shown quoted"
        );
        assert_eq!(
            Invocation {
                program: "a.exe".into(),
                args: vec!["plain".into()]
            }
            .display(),
            "a.exe plain"
        );
    }

    #[test]
    fn a_timeout_is_clamped_to_something_a_program_can_be_given() {
        assert_eq!(Timeout::seconds(0).0, 1);
        assert_eq!(Timeout::seconds(100_000).0, 3_600);
        assert_eq!(Timeout::default().0, 300);
        assert_eq!(Timeout::seconds(30).duration(), Duration::from_secs(30));
    }

    #[test]
    fn a_command_that_cannot_be_started_is_reported_rather_than_panicking() {
        let built = Invocation {
            program: "C:\\definitely\\not\\here.exe".to_string(),
            args: Vec::new(),
        };
        let outcome = run(&built, Timeout::seconds(5));
        assert!(!outcome.succeeded());
        assert!(
            matches!(&outcome, Outcome::CouldNotStart { reason } if !reason.is_empty()),
            "expected a program that cannot start, got {outcome:?}"
        );
        assert!(outcome.describe(&built).contains("could not be started"));
    }

    #[cfg(windows)]
    #[test]
    fn a_real_command_runs_and_its_exit_code_is_the_answer() {
        // Started as a program with an argument vector: `cmd.exe` is given its
        // arguments the same way any other program is, and the string Vise builds
        // is never handed to it as a command line.
        let ok = Invocation {
            program: "cmd.exe".to_string(),
            args: vec!["/c".to_string(), "exit".to_string(), "0".to_string()],
        };
        assert_eq!(
            run(&ok, Timeout::seconds(30)),
            Outcome::Finished { code: 0 }
        );
        let bad = Invocation {
            program: "cmd.exe".to_string(),
            args: vec!["/c".to_string(), "exit".to_string(), "3".to_string()],
        };
        let outcome = run(&bad, Timeout::seconds(30));
        assert_eq!(outcome, Outcome::Finished { code: 3 });
        assert!(!outcome.succeeded());
        assert!(outcome.describe(&bad).contains("exit code 3"));
    }

    #[cfg(windows)]
    #[test]
    fn a_command_that_will_not_stop_is_killed_at_the_timeout() {
        let slow = Invocation {
            program: "cmd.exe".to_string(),
            args: vec![
                "/c".to_string(),
                "ping".to_string(),
                "-n".to_string(),
                "30".to_string(),
                "127.0.0.1".to_string(),
            ],
        };
        let started = Instant::now();
        let outcome = run(&slow, Timeout::seconds(1));
        assert_eq!(outcome, Outcome::TimedOut { seconds: 1 });
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the wait is bounded by the timeout, not by the program"
        );
        assert!(outcome.describe(&slow).contains("did not finish"));
    }

    #[test]
    fn a_pre_action_that_works_lets_the_encode_go() {
        let verdict = run_pre(
            "cmd.exe /c exit 0",
            &variables(),
            Timeout::seconds(30),
            OnFailure::StopEncode,
        )
        .expect("parsed");
        assert!(matches!(verdict, Verdict::Proceed { .. }));
    }

    #[test]
    fn a_pre_action_that_fails_can_stop_the_encode_or_only_say_so() {
        let variables = variables();
        // A command that cannot start is a command that did not work, whichever
        // way the preferences are set.
        let refuse = run_pre(
            "C:\\definitely\\not\\here.exe",
            &variables,
            Timeout::seconds(5),
            OnFailure::StopEncode,
        )
        .expect("parsed");
        let Verdict::Refuse { reason } = refuse else {
            panic!("a failed pre-action that stops the encode must refuse it")
        };
        assert!(reason.contains("pre-encode action"), "{reason}");

        let survive = run_pre(
            "C:\\definitely\\not\\here.exe",
            &variables,
            Timeout::seconds(5),
            OnFailure::Continue,
        )
        .expect("parsed");
        let Verdict::Proceed { log } = survive else {
            panic!("a failed pre-action the user asked to survive must let it go")
        };
        assert_eq!(log.len(), 1);
        assert!(log[0].contains("could not be started"), "{}", log[0]);
    }

    #[test]
    fn a_malformed_command_stops_the_encode_whatever_the_preference_is() {
        // A command with a typo in it should say so; running nothing is not what
        // anybody meant, and neither is carrying on silently.
        for policy in [OnFailure::Continue, OnFailure::StopEncode] {
            let problem = run_pre(
                "say.exe \"unclosed",
                &variables(),
                Timeout::seconds(5),
                policy,
            )
            .expect_err("a command with an unclosed quote cannot be read");
            assert!(problem.contains("unclosed quote"), "{problem}");
        }
    }

    #[test]
    fn an_empty_action_is_not_an_action() {
        assert!(matches!(
            run_pre("   ", &variables(), Timeout::seconds(5), OnFailure::StopEncode)
                .expect("nothing to run"),
            Verdict::Proceed { log } if log.is_empty()
        ));
        assert!(run_post("", &variables(), Timeout::seconds(5))
            .expect("nothing to run")
            .is_empty());
    }

    #[test]
    fn a_post_action_cannot_fail_an_encode_that_has_been_written() {
        let outcome = run_post(
            "C:\\definitely\\not\\here.exe %output%",
            &variables(),
            Timeout::seconds(5),
        )
        .expect("parsed");
        assert_eq!(outcome.len(), 1);
        assert!(outcome[0].contains("D:\\out\\E01.mp4"), "{}", outcome[0]);
        assert!(
            !outcome[0].contains("pause"),
            "a post-action is a log line, never a reason to stop"
        );
    }

    #[test]
    fn the_failure_policy_reads_back_from_the_record() {
        assert_eq!(OnFailure::read(0), OnFailure::Continue);
        assert_eq!(OnFailure::read(-1), OnFailure::StopEncode);
        for policy in [OnFailure::Continue, OnFailure::StopEncode] {
            assert!(!policy.id().is_empty());
        }
    }

    #[test]
    fn an_action_error_says_what_was_wrong_in_one_line() {
        assert!(ActionError::Malformed("the command is empty".into())
            .to_string()
            .contains("empty"));
        assert!(ActionError::BadTimeout(0).to_string().contains("0 second"));
    }
}
