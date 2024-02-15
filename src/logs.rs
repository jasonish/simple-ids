// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::{
    io::{BufRead, BufReader, Read},
    process::Stdio,
    thread,
};

use clap::Parser;
use regex::Regex;

use crate::{context::Context, EVEBOX_CONTAINER_NAME, SURICATA_CONTAINER_NAME};

#[derive(Parser, Debug)]
pub(crate) struct LogArgs {
    #[arg(short, long, help = "Follow log output")]
    follow: bool,
    #[arg(help = "Service to display logs for, default = all")]
    services: Vec<String>,
}

pub(crate) fn logs(ctx: &Context, args: LogArgs) {
    let containers = [SURICATA_CONTAINER_NAME, EVEBOX_CONTAINER_NAME];
    let max_container_name_len = containers.iter().map(|s| s.len()).max().unwrap_or(0);
    let mut handles = vec![];

    for container in containers {
        if !args.services.is_empty() {
            match container {
                SURICATA_CONTAINER_NAME => {
                    if !args.services.contains(&"suricata".to_string()) {
                        continue;
                    }
                }
                EVEBOX_CONTAINER_NAME => {
                    if !args.services.contains(&"evebox".to_string()) {
                        continue;
                    }
                }
                _ => unimplemented!(),
            }
        }

        let mut command = ctx.manager.command();
        command.arg("logs");
        command.arg("--timestamps");
        if args.follow {
            command.arg("--follow");
        }
        command.arg(container);
        let handle = thread::spawn(move || {
            match command
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(mut output) => {
                    let mut handles = vec![];

                    let stdout = output.stdout.take().unwrap();

                    let handle = thread::spawn(move || {
                        log_line_printer(
                            format!(
                                "{:width$} | stdout",
                                container,
                                width = max_container_name_len
                            ),
                            stdout,
                        );
                    });
                    handles.push(handle);

                    let stderr = output.stderr.take().unwrap();
                    let handle = thread::spawn(move || {
                        log_line_printer(
                            format!(
                                "{:width$} | stderr",
                                container,
                                width = max_container_name_len
                            ),
                            stderr,
                        );
                    });
                    handles.push(handle);

                    for handle in handles {
                        let _ = handle.join();
                    }
                }
                Err(err) => {
                    panic!("{}", err);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.join();
    }
}

fn log_line_printer<R: Read + Sync + Send + 'static>(prefix: String, output: R) {
    let evebox_ts_pattern = r".....\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}.....";
    let re = Regex::new(evebox_ts_pattern).unwrap();

    let reader = BufReader::new(output).lines();
    for line in reader {
        if let Ok(line) = line {
            let line = re.replace_all(&line, "");
            println!("{} | {}", prefix, line);
        } else {
            return;
        }
    }
}
