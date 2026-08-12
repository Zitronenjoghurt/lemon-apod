use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::io::{self, Write};
use std::sync::LazyLock;
use std::time::Duration;
use tracing_subscriber::fmt::MakeWriter;

static BARS: LazyLock<MultiProgress> = LazyLock::new(MultiProgress::new);

const COUNTED: &str =
    "{prefix:>9.cyan.bold} [{bar:28}] {human_pos}/{human_len} {msg} [{elapsed_precise}, eta {eta}]";
const UNCOUNTED: &str = "{prefix:>9.cyan.bold} {spinner} {msg} [{elapsed_precise}]";
const TICK: Duration = Duration::from_millis(120);

pub fn bar(prefix: &'static str, len: usize) -> ProgressBar {
    let bar = BARS.add(
        ProgressBar::new(len as u64).with_prefix(prefix).with_style(
            ProgressStyle::with_template(COUNTED)
                .expect("static template is valid")
                .progress_chars("=> "),
        ),
    );
    bar.enable_steady_tick(TICK);
    bar
}

pub fn spinner(
    prefix: &'static str,
    message: impl Into<std::borrow::Cow<'static, str>>,
) -> ProgressBar {
    let spinner = BARS.add(
        ProgressBar::new_spinner()
            .with_prefix(prefix)
            .with_message(message)
            .with_style(ProgressStyle::with_template(UNCOUNTED).expect("static template is valid")),
    );
    spinner.enable_steady_tick(TICK);
    spinner
}

pub fn done(bar: &ProgressBar, summary: impl AsRef<str>) {
    bar.finish_and_clear();
    BARS.remove(bar);
    println(summary.as_ref());
}

pub fn println(line: &str) {
    if BARS.is_hidden() {
        println!("{line}");
        return;
    }
    let _ = BARS.println(line);
}

#[derive(Clone, Copy)]
pub struct LogWriter;

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        BARS.suspend(|| io::stderr().write(buf))
    }

    fn flush(&mut self) -> io::Result<()> {
        BARS.suspend(|| io::stderr().flush())
    }
}

impl<'a> MakeWriter<'a> for LogWriter {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        *self
    }
}
