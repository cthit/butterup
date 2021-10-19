use crate::local;
use crate::planner::{self, TransferKind};
use crate::remote;
use crate::util::{format_duration, path_as_utf8};
use crate::Opt;
use ssh2::Session;
use std::io::{self, Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

const TMP_FOLDER: &str = ".tmp";

pub fn run(opt: &Opt, sync_all: bool) -> anyhow::Result<()> {
    // TODO: currently we only sync the latest local files
    // --all will force a sync of ALL files on local which does not exist on remote
    if sync_all {
        error!("backup --all is not yet implemented");
        unimplemented!();
    }

    info!("generating backup plan");
    let local_list = local::file_list(opt)?;
    let session = remote::connect(opt)?;
    let remote_list = remote::file_list(opt, &session)?;

    let plan = planner::plan(&local_list, &remote_list);

    if plan.transfers.is_empty() {
        info!("nothing to do");
        return Ok(());
    }

    info!("performing backup plan");
    for (item, transfer) in plan.transfers {
        let parent = match transfer {
            TransferKind::DeltaFrom(parent) => Some(
                local_list
                    .get(&parent)
                    .unwrap_or_else(|| &remote_list[&parent])
                    .as_str(),
            ),
            TransferKind::Full => None,
        };

        send_snapshot(opt, &session, &local_list[&item], parent)?;
    }

    Ok(())
}

struct CmdOutput {
    exit_status: i32,
    stdout: String,
    stderr: String,
}

fn do_cmd(session: &Session, cmd: &str) -> anyhow::Result<CmdOutput> {
    let mut ch = session.channel_session()?;

    ch.exec(cmd)?;
    ch.send_eof()?;

    let mut stdout = String::new();
    let mut stderr = String::new();

    ch.stderr().read_to_string(&mut stderr)?;
    ch.read_to_string(&mut stdout)?;

    ch.close()?;
    ch.wait_close()?;
    let exit_status = ch.exit_status()?;

    Ok(CmdOutput {
        stdout,
        stderr,
        exit_status,
    })
}

fn clear_tmp_dir(opt: &Opt, session: &Session) -> anyhow::Result<bool> {
    let tmp_path = opt.remote.path.join(TMP_FOLDER);
    let tmp_path = path_as_utf8(&tmp_path)?;
    let cmd = format!(r#"rm -r "{}""#, tmp_path);
    let result = do_cmd(session, &cmd)?;

    let success = result.exit_status == 0;

    Ok(success)
}

fn create_tmp_dir(opt: &Opt, session: &Session) -> anyhow::Result<()> {
    let tmp_path = opt.remote.path.join(TMP_FOLDER);
    let tmp_path = path_as_utf8(&tmp_path)?;
    let cmd = format!(r#"mkdir "{}""#, tmp_path);
    let result = do_cmd(session, &cmd)?;

    if result.exit_status != 0 {
        anyhow::bail!("failed to create {} dir on remote", TMP_FOLDER);
    }

    Ok(())
}

fn send_snapshot(
    opt: &Opt,
    session: &Session,
    snapshot: &str,
    parent: Option<&str>,
) -> anyhow::Result<()> {
    if parent.is_none() {
        info!("[{}] sending full snapshot data", snapshot);
    } else {
        info!("[{}] sending snapshot delta", snapshot);
    }

    if clear_tmp_dir(opt, session)? {
        warn!(
            "[{}] {} dir did already exist, it is likely that a previous upload failed.",
            snapshot, TMP_FOLDER
        );
    }

    create_tmp_dir(opt, session)?;

    let start_time = Instant::now();

    // spawn btrfs send
    let mut send = Command::new("btrfs")
        .arg("send")
        .args(parent.iter().flat_map(|parent| ["-p", parent]))
        .arg(snapshot)
        .current_dir(&opt.path)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    // capture btrfs send output
    let mut send_stdout = send
        .stdout
        .take()
        .ok_or_else(|| anyhow::format_err!("failed to take stdout"))?;

    // #### UPLOAD SNAPSHOT FILE ####
    const CHUNK_SIZE: usize = 1024 * 1024 * 100; // 100MB

    let (data_tx, data_rx) = mpsc::sync_channel(10);
    let tmp_path = opt.remote.path.join(TMP_FOLDER);

    // spawn a thread to stream data from btrfs send in chunks
    thread::spawn(move || -> io::Result<()> {
        'outer: for _ in 0.. {
            let mut buf: Vec<u8> = vec![0u8; CHUNK_SIZE];
            let mut len = 0;
            loop {
                let free = &mut buf[len..];
                let n = send_stdout.read(free)?;
                len += n;

                if n == 0 || n == free.len() {
                    buf.truncate(len);
                    if data_tx.send(buf).is_err() {
                        break 'outer;
                    }

                    // check if we reached EOF
                    if n == 0 {
                        break 'outer;
                    } else {
                        continue 'outer;
                    }
                }
            }
        }

        Ok(())
    });

    let mut byte_count = 0;
    let mut i = 0;
    while let Ok(data) = data_rx.recv() {
        byte_count += data.len();
        info!("[{}] uploading {} bytes...", snapshot, byte_count);
        let snapshot_file = tmp_path.join(format!("{:016}", i));
        let mut ch = session.scp_send(&snapshot_file, 0o600, data.len() as u64, None)?;
        ch.write_all(&data)?;
        i += 1;
    }

    info!(
        "[{}] re-creating snapshot (this can take a while)",
        snapshot
    );
    let remote_path = path_as_utf8(&opt.remote.path)?;
    let tmp_path = path_as_utf8(&tmp_path)?;
    let cmd = format!(
        r#"cat "{}"/* | btrfs receive -e "{}""#,
        tmp_path, remote_path
    );
    let out = do_cmd(session, &cmd)?;

    let time_elapsed = start_time.elapsed();

    if out.exit_status != 0 {
        anyhow::bail!(
            "btrfs receive failed\nstdout:\n{}\nstderr:\n{}",
            out.stdout,
            out.stderr
        );
    }

    info!(
        "[{}] snapshot was {} bytes, time taken was {}",
        snapshot,
        byte_count,
        format_duration(time_elapsed)
    );

    if !clear_tmp_dir(opt, session)? {
        anyhow::bail!("failed to remove {} dir", TMP_FOLDER);
    }

    Ok(())
}
